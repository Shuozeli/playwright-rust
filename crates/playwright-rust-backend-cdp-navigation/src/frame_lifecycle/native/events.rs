use super::*;
use serde_json::Value;

fn field<'a>(value: &'a Value, name: &'static str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or(NativeLifecycleError::InvalidEvent(name))
}

impl CdpFrameLifecycle {
    pub(super) fn commit_native<'a>(
        &mut self,
        now: Duration,
        id: ObjectId,
        loader: &'a str,
        url: Option<&'a str>,
    ) -> Result<NativeLifecycleNotice<'a>> {
        let index = self
            .frame_index(id)
            .ok_or(NativeLifecycleError::UnknownFrame)?;
        self.validate_id(loader)?;
        if self.frames[index].as_ref().is_some_and(|previous| {
            previous.document.is_some() && self.frame_loader(index, previous) == loader
        }) {
            return Ok(NativeLifecycleNotice::Ignored);
        }
        let request = self.requests.iter().enumerate().find_map(|(i, row)| {
            row.as_ref()
                .filter(|r| r.frame == id && r.document && self.request_native_id(i, r) == loader)
                .map(|r| r.id)
        });
        let owned_loader = self
            .allocator_backed
            .then(|| Self::owned_id(loader))
            .transpose()?;
        self.next_document = self
            .next_document
            .checked_add(1)
            .ok_or(LifecycleError::SizeOverflow)?;
        let document = DocumentGeneration(self.next_document);
        self.lifecycle.apply(
            now,
            LifecycleInput::Commit {
                frame: id,
                document,
                request,
            },
        )?;
        self.prune(Some((id, request)))?;
        if !self.allocator_backed {
            self.write_id(index, loader);
        }
        self.frames[index] = Some(NativeFrame {
            id,
            loader_len: loader.len(),
            loader: owned_loader,
            document: Some(document),
        });
        Ok(NativeLifecycleNotice::Committed {
            frame: id,
            document,
            loader,
            url,
        })
    }

    pub(super) fn event<'e>(
        &mut self,
        now: Duration,
        session: &str,
        event: &'e CdpEvent,
        routes: &impl CdpLifecycleRoutes,
    ) -> Result<NativeLifecycleNotice<'e>> {
        let p = &event.params;
        match event.method.as_str() {
            "Page.frameNavigated" => {
                let frame = p
                    .get("frame")
                    .ok_or(NativeLifecycleError::InvalidEvent("frame"))?;
                let Some(id) = routes.event_frame(session, field(frame, "id")?) else {
                    return Ok(NativeLifecycleNotice::Ignored);
                };
                self.commit_native(
                    now,
                    id,
                    field(frame, "loaderId")?,
                    frame.get("url").and_then(Value::as_str),
                )
            }
            "Page.lifecycleEvent" => {
                let event = match field(p, "name")? {
                    "DOMContentLoaded" => DocumentLifecycle::DomContentLoaded,
                    "load" => DocumentLifecycle::Load,
                    _ => return Ok(NativeLifecycleNotice::Ignored),
                };
                let Some(frame) = routes.event_frame(session, field(p, "frameId")?) else {
                    return Ok(NativeLifecycleNotice::Ignored);
                };
                let index = self
                    .frame_index(frame)
                    .ok_or(NativeLifecycleError::UnknownFrame)?;
                let row = self.frames[index].as_ref().unwrap();
                let loader = field(p, "loaderId")?;
                let Some(document) = row
                    .document
                    .filter(|_| self.frame_loader(index, row) == loader)
                else {
                    return Ok(NativeLifecycleNotice::Ignored);
                };
                self.lifecycle.apply(
                    now,
                    LifecycleInput::DocumentEvent {
                        frame,
                        document,
                        event,
                    },
                )?;
                Ok(NativeLifecycleNotice::Lifecycle {
                    frame,
                    document,
                    event,
                })
            }
            "Page.navigatedWithinDocument" => {
                let Some(frame) = routes.event_frame(session, field(p, "frameId")?) else {
                    return Ok(NativeLifecycleNotice::Ignored);
                };
                if self.frame_index(frame).is_none() {
                    return Err(NativeLifecycleError::UnknownFrame);
                }
                Ok(NativeLifecycleNotice::SameDocument {
                    frame,
                    url: p.get("url").and_then(Value::as_str),
                })
            }
            "Network.requestWillBeSent" => self.start(now, session, p, routes),
            "Network.responseReceived" | "Network.loadingFinished" | "Network.loadingFailed" => {
                let native = field(p, "requestId")?;
                let Some(index) = self.request_index(session, native, routes)? else {
                    return Ok(NativeLifecycleNotice::Ignored);
                };
                let (request_id, request_frame) = {
                    let row = self.requests[index].as_ref().unwrap();
                    (row.id, row.frame)
                };
                if event.method == "Network.responseReceived" {
                    let response = p
                        .get("response")
                        .filter(|v| v.is_object())
                        .ok_or(NativeLifecycleError::InvalidEvent("response"))?;
                    return Ok(NativeLifecycleNotice::Response {
                        frame: request_frame,
                        request: request_id,
                        response,
                    });
                }
                let failure = if event.method == "Network.loadingFailed" {
                    Some((
                        field(p, "errorText")?,
                        match p.get("canceled") {
                            None => false,
                            Some(Value::Bool(value)) => *value,
                            Some(_) => return Err(NativeLifecycleError::InvalidEvent("canceled")),
                        },
                    ))
                } else {
                    None
                };
                self.lifecycle.apply(
                    now,
                    LifecycleInput::RequestFinished {
                        request: request_id,
                    },
                )?;
                self.requests[index] = None;
                match failure {
                    Some((error_text, canceled)) => Ok(NativeLifecycleNotice::RequestFailed {
                        frame: request_frame,
                        request: request_id,
                        error_text,
                        canceled,
                    }),
                    None => Ok(NativeLifecycleNotice::RequestFinished {
                        frame: request_frame,
                        request: request_id,
                    }),
                }
            }
            _ => Ok(NativeLifecycleNotice::Ignored),
        }
    }

    fn start<'e>(
        &mut self,
        now: Duration,
        session: &str,
        p: &'e Value,
        routes: &impl CdpLifecycleRoutes,
    ) -> Result<NativeLifecycleNotice<'e>> {
        let request = p
            .get("request")
            .ok_or(NativeLifecycleError::InvalidEvent("request"))?;
        let url = field(request, "url")?;
        if url.starts_with("data:") {
            return Ok(NativeLifecycleNotice::Ignored);
        }
        let native_frame = match p.get("frameId") {
            None => None,
            Some(Value::String(frame)) => Some(frame.as_str()),
            Some(_) => return Err(NativeLifecycleError::InvalidEvent("frameId")),
        };
        let Some(frame) = routes.request_frame(session, native_frame) else {
            return Ok(NativeLifecycleNotice::Ignored);
        };
        if self.frame_index(frame).is_none() {
            return Err(NativeLifecycleError::UnknownFrame);
        }
        let native_id = field(p, "requestId")?;
        let loader = field(p, "loaderId")?;
        let document =
            p.get("type").and_then(Value::as_str) == Some("Document") && native_id == loader;
        self.validate_id(native_id)?;
        self.validate_id(session)?;
        let previous = self.request_index(session, native_id, routes)?;
        let old = previous.and_then(|index| {
            self.requests[index]
                .as_ref()
                .map(|request| (request.id, request.frame, request.document, request.favicon))
        });
        if old
            .as_ref()
            .is_some_and(|old| old.1 != frame || old.2 != document)
        {
            return Err(LifecycleError::ConflictingIdentity.into());
        }
        let redirect = match p.get("redirectResponse") {
            None => false,
            Some(Value::Object(_)) => true,
            Some(_) => return Err(NativeLifecycleError::InvalidEvent("redirectResponse")),
        };
        if previous.is_some() && !redirect {
            return Ok(NativeLifecycleNotice::Ignored);
        }
        let favicon =
            url.ends_with("/favicon.ico") || old.as_ref().is_some_and(|request| request.3);
        let index = match previous.or_else(|| self.requests.iter().position(Option::is_none)) {
            Some(index) => index,
            None if self.allocator_backed => {
                self.requests
                    .try_reserve(1)
                    .map_err(|_| LifecycleError::Allocation)?;
                self.requests.push(None);
                self.requests.len() - 1
            }
            None => return Err(LifecycleError::RequestCapacity.into()),
        };
        let owned_native_id = self
            .allocator_backed
            .then(|| Self::owned_id(native_id))
            .transpose()?;
        let owned_session = self
            .allocator_backed
            .then(|| Self::owned_id(session))
            .transpose()?;
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(LifecycleError::SizeOverflow)?;
        let request = ObjectId::new(self.next_request);
        if let Some(old) = old.as_ref() {
            self.lifecycle
                .apply(now, LifecycleInput::RequestFinished { request: old.0 })?;
        }
        self.lifecycle.apply(
            now,
            LifecycleInput::RequestStarted {
                frame,
                request,
                favicon,
            },
        )?;
        if !self.allocator_backed {
            self.write_id(self.request_cell(index), native_id);
            self.write_id(self.request_cell(index) + 1, session);
        }
        self.requests[index] = Some(NativeRequest {
            id: request,
            frame,
            request_len: native_id.len(),
            session_len: session.len(),
            native_id: owned_native_id,
            session: owned_session,
            document,
            favicon,
        });
        Ok(NativeLifecycleNotice::RequestStarted {
            frame,
            request,
            document_loader: document.then_some(loader),
            redirected_from: old.map(|request| request.0),
        })
    }
}
