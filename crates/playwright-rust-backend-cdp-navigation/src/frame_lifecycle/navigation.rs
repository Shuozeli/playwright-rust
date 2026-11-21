//! Per-invocation navigation matching and response retention.
//!
//! Install before native dispatch. The owner supplies the ordered Page notices,
//! the selected Frame's lifecycle snapshot and monotonic time. This component
//! has no CDP connection, task, timer, route retry or Page-wide stop operation.
//! Native reply retirement, redirectNavigation adoption, publication and FFI
//! output allocation remain responsibilities of the embedding executor.
use super::{LifecycleSnapshot, native::NativeLifecycleNotice, slots};
use playwright_rust_cap_navigation::WaitUntil;
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use std::{fmt, io::Cursor, mem::size_of, time::Duration};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationWaitError {
    InvalidProfile,
    SizeOverflow,
    Resources,
    Allocation,
    InvalidState,
    Capacity,
    InvalidIdentity,
    TimeWentBackwards,
    Interrupted,
    Network { canceled: bool },
    Timeout,
    Detached,
    PageClosed,
    Disconnected,
    EventLoss,
    Canceled,
    Download,
}
impl fmt::Display for NavigationWaitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Frame navigation wait: {self:?}")
    }
}
impl std::error::Error for NavigationWaitError {}
type Result<T> = std::result::Result<T, NavigationWaitError>;

/// Field limits apply to one invocation, including events received before the
/// native reply. Finite profiles also cap row counts; allocator-backed profiles
/// use `events` and `requests` only as initial capacities and grow observed rows.
#[derive(Clone, Copy, Debug)]
pub struct FrameNavigationWaitProfile {
    scope: ResourceScope,
    events: usize,
    requests: usize,
    id_bytes: usize,
    error_bytes: usize,
    url_bytes: usize,
    response_bytes: usize,
    arena_bytes: usize,
    bytes: u64,
    allocator_backed: bool,
}
impl FrameNavigationWaitProfile {
    pub fn new(
        scope: ResourceScope,
        events: usize,
        requests: usize,
        id_bytes: usize,
        error_bytes: usize,
        response_bytes: usize,
    ) -> Result<Self> {
        if [events, requests, id_bytes, error_bytes, response_bytes].contains(&0) {
            return Err(NavigationWaitError::InvalidProfile);
        }
        let arena_bytes = id_bytes
            .checked_add(error_bytes)
            .and_then(|n| n.checked_mul(events))
            .and_then(|n| {
                n.checked_add(
                    id_bytes
                        .checked_add(response_bytes)?
                        .checked_mul(requests)?,
                )
            })
            .and_then(|n| n.checked_add(id_bytes))
            .ok_or(NavigationWaitError::SizeOverflow)?;
        let bytes = events
            .checked_mul(size_of::<Option<Navigation>>())
            .and_then(|n| n.checked_add(requests.checked_mul(size_of::<Option<Request>>())?))
            .and_then(|n| n.checked_add(arena_bytes))
            .and_then(|n| n.checked_add(size_of::<FrameNavigationWait>()))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(NavigationWaitError::SizeOverflow)?;
        Ok(Self {
            scope,
            events,
            requests,
            id_bytes,
            error_bytes,
            url_bytes: 0,
            response_bytes,
            arena_bytes,
            bytes,
            allocator_backed: false,
        })
    }
    pub fn new_allocator_backed(
        scope: ResourceScope,
        events: usize,
        requests: usize,
        id_bytes: usize,
        error_bytes: usize,
        response_bytes: usize,
    ) -> Result<Self> {
        let mut profile = Self::new(
            scope,
            events,
            requests,
            id_bytes,
            error_bytes,
            response_bytes,
        )?;
        profile.allocator_backed = true;
        Ok(profile)
    }
    /// URL retention is optional; ordinary navigation receipts pay no URL cost.
    pub fn with_url_bytes(mut self, url_bytes: usize) -> Result<Self> {
        if url_bytes == 0 || self.url_bytes != 0 {
            return Err(NavigationWaitError::InvalidProfile);
        }
        let extra = self
            .events
            .checked_mul(url_bytes)
            .ok_or(NavigationWaitError::SizeOverflow)?;
        self.arena_bytes = self
            .arena_bytes
            .checked_add(extra)
            .ok_or(NavigationWaitError::SizeOverflow)?;
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(extra).map_err(|_| NavigationWaitError::SizeOverflow)?)
            .ok_or(NavigationWaitError::SizeOverflow)?;
        self.url_bytes = url_bytes;
        Ok(self)
    }
    pub const fn requirements(self) -> ResourceRequirements {
        if self.allocator_backed {
            return ResourceRequirements::none();
        }
        ResourceRequirements::none()
            .with_class(ResourceClass::BackendStateBytes, self.bytes, self.bytes)
            .with_metadata(1, 1)
    }
    pub const fn retained_bytes(self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Copy)]
struct Navigation {
    loader_len: Option<usize>,
    request: Option<usize>,
    failure: Option<bool>,
    error_len: usize,
    url_len: usize,
}
struct OwnedNavigation {
    loader: Option<Box<str>>,
    error: Option<Box<str>>,
    url: Option<Box<str>>,
}
/// One bounded navigation candidate copied out before invoking a predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationCandidate<'a> {
    Url(&'a str),
    Failed,
}
#[derive(Clone, Copy)]
enum Response {
    Pending,
    Null,
    Json(usize),
}
#[derive(Clone, Copy)]
struct Request {
    id: ObjectId,
    loader_len: usize,
    response: Response,
}
struct OwnedRequest {
    loader: Box<str>,
    response_json: Option<Box<[u8]>>,
}
#[derive(Clone, Copy)]
enum State {
    Receipt,
    Navigation,
    Lifecycle(Option<usize>),
    Response(Option<usize>),
    Complete(Option<usize>),
    Failed(NavigationWaitError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum NavigationPoll<'a> {
    Pending,
    /// JSON is borrowed from this invocation's owned storage, not from a CDP
    /// envelope. A caller retaining/decoding it must admit its own copy.
    Complete {
        frame: ObjectId,
        response_json: Option<&'a [u8]>,
    },
}

/// Non-cloneable owner: dropping it releases its response bytes and lease.
/// This is not yet the public Frame.goto command or an asynchronous executor.
pub struct FrameNavigationWait {
    events: Vec<Option<Navigation>>,
    owned_events: Vec<Option<OwnedNavigation>>,
    requests: Vec<Option<Request>>,
    owned_requests: Vec<Option<OwnedRequest>>,
    arena: Vec<u8>,
    profile: FrameNavigationWaitProfile,
    frame: ObjectId,
    wait: WaitUntil,
    deadline: Option<Duration>,
    now: Duration,
    receipt_len: Option<usize>,
    receipt: Option<Box<str>>,
    accept_any_navigation: bool,
    event_count: usize,
    pending_document: Option<usize>,
    failure_event: Option<usize>,
    state: State,
    _lease: Option<ResourceLease>,
}
impl FrameNavigationWait {
    pub fn new(
        profile: FrameNavigationWaitProfile,
        resources: &ProductionResourceLedger,
        frame: ObjectId,
        wait: WaitUntil,
        deadline: Option<Duration>,
    ) -> Result<Self> {
        if frame == ObjectId::new(0) {
            return Err(NavigationWaitError::InvalidIdentity);
        }
        let lease = if profile.allocator_backed {
            None
        } else {
            let ledger = resources
                .claim(profile.requirements())
                .map_err(|_| NavigationWaitError::Resources)?;
            Some(
                ledger
                    .reserve(
                        profile.scope,
                        ResourceClass::BackendStateBytes,
                        profile.bytes,
                    )
                    .map_err(|_| NavigationWaitError::Resources)?,
            )
        };
        let (events, requests, arena) = if profile.allocator_backed {
            let mut events = Vec::new();
            events
                .try_reserve(profile.events)
                .map_err(|_| NavigationWaitError::Allocation)?;
            let mut requests = Vec::new();
            requests
                .try_reserve(profile.requests)
                .map_err(|_| NavigationWaitError::Allocation)?;
            (events, requests, Vec::new())
        } else {
            let mut arena = Vec::new();
            arena
                .try_reserve_exact(profile.arena_bytes)
                .map_err(|_| NavigationWaitError::Allocation)?;
            if arena.capacity() != profile.arena_bytes {
                return Err(NavigationWaitError::Allocation);
            }
            arena.resize(profile.arena_bytes, 0);
            (
                slots(profile.events).map_err(|_| NavigationWaitError::Allocation)?,
                slots(profile.requests).map_err(|_| NavigationWaitError::Allocation)?,
                arena,
            )
        };
        Ok(Self {
            events,
            owned_events: Vec::new(),
            requests,
            owned_requests: Vec::new(),
            arena,
            profile,
            frame,
            wait,
            deadline,
            now: Duration::ZERO,
            receipt_len: None,
            receipt: None,
            accept_any_navigation: false,
            event_count: 0,
            pending_document: None,
            failure_event: None,
            state: State::Receipt,
            _lease: lease,
        })
    }

    /// `None` is the native same-document receipt. Native errors/downloads must
    /// be delivered as aborts instead, never interpreted as successful receipts.
    pub fn receipt(&mut self, now: Duration, loader: Option<&str>) -> Result<()> {
        self.tick(now)?;
        let result = (|| {
            if !matches!(self.state, State::Receipt) {
                return Err(NavigationWaitError::InvalidState);
            }
            let owned = loader
                .map(|loader| {
                    self.validate_id(loader)?;
                    self.profile
                        .allocator_backed
                        .then(|| owned_text(loader))
                        .transpose()
                })
                .transpose()?
                .flatten();
            if let Some(loader) = loader
                && !self.profile.allocator_backed
            {
                self.arena[..loader.len()].copy_from_slice(loader.as_bytes());
            }
            self.receipt_len = loader.map(str::len);
            self.receipt = owned;
            self.accept_any_navigation = false;
            self.state = State::Navigation;
            self.match_navigation()
        })();
        self.seal_error(result)
    }

    /// Wait for the next committed or same-document navigation without a
    /// command receipt. This is the Page.waitForNavigation mode: the first
    /// navigation observed after registration becomes the selected document.
    pub fn wait_for_next_navigation(&mut self, now: Duration) -> Result<()> {
        self.tick(now)?;
        let result = (|| {
            if !matches!(self.state, State::Receipt) {
                return Err(NavigationWaitError::InvalidState);
            }
            self.accept_any_navigation = true;
            self.state = State::Navigation;
            self.match_navigation()
        })();
        self.seal_error(result)
    }

    /// Inspect an event without selecting it. Callers must copy a URL out of
    /// their lifecycle lock before running any language-supplied predicate.
    pub fn candidate(&self, index: usize) -> Result<Option<NavigationCandidate<'_>>> {
        if self.profile.url_bytes == 0 || !matches!(self.state, State::Receipt) {
            return Err(NavigationWaitError::InvalidState);
        }
        let Some(event) = self.events.get(index).copied().flatten() else {
            return Ok(None);
        };
        if event.failure.is_some() {
            return Ok(Some(NavigationCandidate::Failed));
        }
        let url = self.event_url(index, event)?;
        Ok(Some(NavigationCandidate::Url(url)))
    }

    /// Select exactly an observed event while retaining one registration and
    /// one absolute deadline across all rejected candidates.
    pub fn select_candidate(&mut self, now: Duration, index: usize) -> Result<()> {
        self.tick(now)?;
        let result = (|| {
            if self.profile.url_bytes == 0 || !matches!(self.state, State::Receipt) {
                return Err(NavigationWaitError::InvalidState);
            }
            let event = self
                .events
                .get(index)
                .copied()
                .flatten()
                .ok_or(NavigationWaitError::InvalidState)?;
            if let Some(canceled) = event.failure {
                self.failure_event = Some(index);
                return Err(NavigationWaitError::Network { canceled });
            }
            self.state = State::Lifecycle(event.request);
            Ok(())
        })();
        self.seal_error(result)
    }

    pub fn observe(&mut self, now: Duration, notice: &NativeLifecycleNotice<'_>) -> Result<()> {
        self.tick(now)?;
        if matches!(self.state, State::Complete(_)) {
            return Ok(());
        }
        let result = self.observe_inner(notice);
        self.seal_error(result)
    }

    /// Invoke on every relevant Page transition and scheduled deadline, not
    /// only on selected-Frame notices: descendant network activity affects idle.
    /// After matching the navigation event, Playwright waits on the Frame's
    /// current lifecycle; it does not pin that wait to the matched generation.
    pub fn poll(
        &mut self,
        now: Duration,
        snapshot: Option<LifecycleSnapshot>,
    ) -> Result<NavigationPoll<'_>> {
        self.tick(now)?;
        if !matches!(self.state, State::Complete(_)) {
            let Some(snapshot) = snapshot else {
                return self
                    .abort(NavigationWaitError::Detached)
                    .map(|()| NavigationPoll::Pending);
            };
            if let State::Lifecycle(request) = self.state
                && snapshot.reached(self.wait)
            {
                self.state = State::Response(request);
            }
            if let State::Response(request) = self.state
                && request.is_none_or(|i| {
                    !matches!(self.requests[i].unwrap().response, Response::Pending)
                })
            {
                self.state = State::Complete(request);
            }
        }
        self.outcome().unwrap_or(Ok(NavigationPoll::Pending))
    }

    /// Borrow a settled result without advancing time or consulting lifecycle.
    pub fn outcome(&self) -> Option<Result<NavigationPoll<'_>>> {
        match self.state {
            State::Complete(request) => {
                let response_json = request.and_then(|i| self.request_response(i));
                Some(Ok(NavigationPoll::Complete {
                    frame: self.frame,
                    response_json,
                }))
            }
            State::Failed(error) => Some(Err(error)),
            _ => None,
        }
    }

    /// Borrow the completed document's native loader/request identity.
    /// Chromium uses this value as the `Network.getResponseBody` request ID
    /// for a document response. It remains owned by this wait.
    pub fn response_request_id(&self) -> Option<&str> {
        let State::Complete(Some(index)) = self.state else {
            return None;
        };
        let request = self.requests[index]?;
        self.request_loader(index, request).ok()
    }

    /// The embedding scheduler combines this with the Page's idle deadline.
    pub fn next_deadline(&self) -> Option<Duration> {
        self.outcome().is_none().then_some(self.deadline).flatten()
    }

    /// External owner reports timeout/close/detach/loss without issuing native
    /// stopLoading. A settled result/error is immutable.
    pub fn abort(&mut self, error: NavigationWaitError) -> Result<()> {
        match self.state {
            State::Complete(_) => Ok(()),
            State::Failed(previous) => Err(previous),
            _ => {
                self.state = State::Failed(error);
                Err(error)
            }
        }
    }
    pub fn failure_text(&self) -> Option<&str> {
        self.failure_event.and_then(|i| {
            let event = self.events[i]?;
            self.event_error(i, event).ok()
        })
    }
    fn tick(&mut self, now: Duration) -> Result<()> {
        match self.state {
            State::Complete(_) => return Ok(()),
            State::Failed(error) => return Err(error),
            _ => {}
        }
        if now < self.now {
            return self.abort(NavigationWaitError::TimeWentBackwards);
        }
        self.now = now;
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            return self.abort(NavigationWaitError::Timeout);
        }
        Ok(())
    }
    fn seal_error(&mut self, result: Result<()>) -> Result<()> {
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.abort(error),
        }
    }
    fn validate_id(&self, id: &str) -> Result<()> {
        if id.is_empty() || id.len() > self.profile.id_bytes {
            Err(NavigationWaitError::InvalidIdentity)
        } else {
            Ok(())
        }
    }
    fn event_offset(&self, i: usize) -> usize {
        self.profile.id_bytes
            + i * (self.profile.id_bytes + self.profile.error_bytes + self.profile.url_bytes)
    }
    fn request_offset(&self, i: usize) -> usize {
        self.event_offset(self.profile.events)
            + i * (self.profile.id_bytes + self.profile.response_bytes)
    }
    fn request_index(&self, request: ObjectId) -> Option<usize> {
        self.requests
            .iter()
            .position(|r| r.is_some_and(|r| r.id == request))
    }
    fn loader_matches(&self, i: usize, loader: &str) -> bool {
        let request = self.requests[i].unwrap();
        self.request_loader(i, request)
            .is_ok_and(|candidate| candidate == loader)
    }

    fn event_loader(&self, i: usize, event: Navigation) -> Result<Option<&str>> {
        if self.profile.allocator_backed {
            return self
                .owned_events
                .get(i)
                .and_then(Option::as_ref)
                .map(|event| event.loader.as_deref())
                .ok_or(NavigationWaitError::InvalidState);
        }
        event
            .loader_len
            .map(|len| {
                std::str::from_utf8(&self.arena[self.event_offset(i)..][..len])
                    .map_err(|_| NavigationWaitError::InvalidIdentity)
            })
            .transpose()
    }

    fn event_error(&self, i: usize, event: Navigation) -> Result<&str> {
        if self.profile.allocator_backed {
            return self
                .owned_events
                .get(i)
                .and_then(Option::as_ref)
                .and_then(|event| event.error.as_deref())
                .ok_or(NavigationWaitError::InvalidState);
        }
        let start = self.event_offset(i) + self.profile.id_bytes;
        std::str::from_utf8(&self.arena[start..start + event.error_len])
            .map_err(|_| NavigationWaitError::InvalidIdentity)
    }

    fn event_url(&self, i: usize, event: Navigation) -> Result<&str> {
        if self.profile.allocator_backed {
            return self
                .owned_events
                .get(i)
                .and_then(Option::as_ref)
                .and_then(|event| event.url.as_deref())
                .ok_or(NavigationWaitError::InvalidState);
        }
        let offset = self.event_offset(i) + self.profile.id_bytes + self.profile.error_bytes;
        std::str::from_utf8(&self.arena[offset..offset + event.url_len])
            .map_err(|_| NavigationWaitError::InvalidIdentity)
    }

    fn request_loader(&self, i: usize, request: Request) -> Result<&str> {
        if self.profile.allocator_backed {
            return self
                .owned_requests
                .get(i)
                .and_then(Option::as_ref)
                .map(|request| request.loader.as_ref())
                .ok_or(NavigationWaitError::InvalidState);
        }
        std::str::from_utf8(&self.arena[self.request_offset(i)..][..request.loader_len])
            .map_err(|_| NavigationWaitError::InvalidIdentity)
    }

    fn request_response(&self, i: usize) -> Option<&[u8]> {
        let request = self.requests[i]?;
        match request.response {
            Response::Json(len) if self.profile.allocator_backed => self
                .owned_requests
                .get(i)
                .and_then(Option::as_ref)
                .and_then(|request| request.response_json.as_deref())
                .filter(|bytes| bytes.len() == len),
            Response::Json(len) => {
                Some(&self.arena[self.request_offset(i) + self.profile.id_bytes..][..len])
            }
            Response::Pending | Response::Null => None,
        }
    }

    fn receipt_loader(&self) -> Result<Option<&str>> {
        if self.profile.allocator_backed {
            return Ok(self.receipt.as_deref());
        }
        self.receipt_len
            .map(|len| {
                std::str::from_utf8(&self.arena[..len])
                    .map_err(|_| NavigationWaitError::InvalidIdentity)
            })
            .transpose()
    }

    fn next_event_slot(&mut self) -> Result<usize> {
        let index = self.event_count;
        if index < self.events.len() {
            return Ok(index);
        }
        if !self.profile.allocator_backed {
            return Err(NavigationWaitError::Capacity);
        }
        self.events
            .try_reserve(1)
            .map_err(|_| NavigationWaitError::Allocation)?;
        self.owned_events
            .try_reserve(1)
            .map_err(|_| NavigationWaitError::Allocation)?;
        self.events.push(None);
        self.owned_events.push(None);
        Ok(index)
    }

    fn observe_inner(&mut self, notice: &NativeLifecycleNotice<'_>) -> Result<()> {
        match notice {
            NativeLifecycleNotice::RequestStarted {
                frame,
                request,
                document_loader: Some(loader),
                redirected_from,
            } if *frame == self.frame => {
                self.validate_id(loader)?;
                if *request == ObjectId::new(0) || self.request_index(*request).is_some() {
                    return Err(NavigationWaitError::InvalidIdentity);
                }
                let previous = redirected_from.and_then(|id| self.request_index(id));
                let index = if let Some(index) =
                    previous.or_else(|| self.requests.iter().position(Option::is_none))
                {
                    index
                } else if self.profile.allocator_backed {
                    self.requests
                        .try_reserve(1)
                        .map_err(|_| NavigationWaitError::Allocation)?;
                    self.owned_requests
                        .try_reserve(1)
                        .map_err(|_| NavigationWaitError::Allocation)?;
                    self.requests.push(None);
                    self.owned_requests.push(None);
                    self.requests.len() - 1
                } else {
                    return Err(NavigationWaitError::Capacity);
                };
                if previous.is_some() && !self.loader_matches(index, loader) {
                    return Err(NavigationWaitError::InvalidIdentity);
                }
                let owned = if self.profile.allocator_backed {
                    Some(OwnedRequest {
                        loader: owned_text(loader)?,
                        response_json: None,
                    })
                } else {
                    let offset = self.request_offset(index);
                    self.arena[offset..offset + loader.len()].copy_from_slice(loader.as_bytes());
                    None
                };
                self.requests[index] = Some(Request {
                    id: *request,
                    loader_len: loader.len(),
                    response: Response::Pending,
                });
                if self.profile.allocator_backed {
                    self.owned_requests[index] = owned;
                }
                self.pending_document = Some(index);
            }
            NativeLifecycleNotice::Committed {
                frame, loader, url, ..
            } if *frame == self.frame => {
                let request = self
                    .pending_document
                    .filter(|i| self.loader_matches(*i, loader));
                if request.is_some() {
                    self.pending_document = None;
                }
                self.navigation(Some(loader), request, *url)?;
            }
            NativeLifecycleNotice::SameDocument { frame, url } if *frame == self.frame => {
                self.navigation(None, None, *url)?
            }
            NativeLifecycleNotice::Response {
                frame,
                request,
                response,
            } if *frame == self.frame => {
                if let Some(i) = self.request_index(*request) {
                    if !matches!(self.requests[i].unwrap().response, Response::Pending) {
                        return Ok(());
                    }
                    if self.profile.allocator_backed {
                        let encoded = bounded_json(response, self.profile.response_bytes)?;
                        let len = encoded.len();
                        self.owned_requests[i]
                            .as_mut()
                            .ok_or(NavigationWaitError::InvalidState)?
                            .response_json = Some(encoded);
                        self.requests[i].as_mut().unwrap().response = Response::Json(len);
                    } else {
                        let offset = self.request_offset(i) + self.profile.id_bytes;
                        let mut writer = Cursor::new(
                            &mut self.arena[offset..offset + self.profile.response_bytes],
                        );
                        serde_json::to_writer(&mut writer, response)
                            .map_err(|_| NavigationWaitError::Capacity)?;
                        self.requests[i].as_mut().unwrap().response =
                            Response::Json(writer.position() as usize);
                    }
                }
            }
            NativeLifecycleNotice::RequestFailed {
                frame,
                request,
                error_text,
                canceled,
            } if *frame == self.frame => {
                if let Some(i) = self.request_index(*request) {
                    if matches!(self.requests[i].unwrap().response, Response::Pending) {
                        self.requests[i].as_mut().unwrap().response = Response::Null;
                    }
                    if self.pending_document == Some(i) {
                        // Copy by arena offsets below, never allocate a loader String.
                        self.pending_document = None;
                        self.failed_navigation(i, error_text, *canceled)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn navigation(
        &mut self,
        loader: Option<&str>,
        request: Option<usize>,
        url: Option<&str>,
    ) -> Result<()> {
        if !matches!(self.state, State::Receipt | State::Navigation) {
            return Ok(());
        }
        if let Some(loader) = loader {
            self.validate_id(loader)?;
        }
        let (url_len, owned_url) = if self.profile.url_bytes > 0 {
            let url = url.ok_or(NavigationWaitError::InvalidIdentity)?;
            if url.is_empty() || url.len() > self.profile.url_bytes {
                return Err(NavigationWaitError::Capacity);
            }
            (
                url.len(),
                self.profile
                    .allocator_backed
                    .then(|| owned_text(url))
                    .transpose()?,
            )
        } else {
            (0, None)
        };
        let owned_loader = if self.profile.allocator_backed {
            loader.map(owned_text).transpose()?
        } else {
            None
        };
        let i = self.next_event_slot()?;
        if !self.profile.allocator_backed {
            if let Some(loader) = loader {
                let offset = self.event_offset(i);
                self.arena[offset..offset + loader.len()].copy_from_slice(loader.as_bytes());
            }
            if let Some(url) = url
                && self.profile.url_bytes > 0
            {
                let offset =
                    self.event_offset(i) + self.profile.id_bytes + self.profile.error_bytes;
                self.arena[offset..offset + url.len()].copy_from_slice(url.as_bytes());
            }
        };
        self.events[i] = Some(Navigation {
            loader_len: loader.map(str::len),
            request,
            failure: None,
            error_len: 0,
            url_len,
        });
        if self.profile.allocator_backed {
            self.owned_events[i] = Some(OwnedNavigation {
                loader: owned_loader,
                error: None,
                url: owned_url,
            });
        }
        self.event_count += 1;
        self.match_navigation()
    }
    fn failed_navigation(&mut self, request: usize, text: &str, canceled: bool) -> Result<()> {
        if !matches!(self.state, State::Receipt | State::Navigation) {
            return Ok(());
        }
        if text.len() > self.profile.error_bytes {
            return Err(NavigationWaitError::Capacity);
        }
        let row = self.requests[request].unwrap();
        let owned = if self.profile.allocator_backed {
            Some(OwnedNavigation {
                loader: Some(owned_text(self.request_loader(request, row)?)?),
                error: Some(owned_text(text)?),
                url: None,
            })
        } else {
            None
        };
        let i = self.next_event_slot()?;
        if !self.profile.allocator_backed {
            let source = self.request_offset(request);
            let offset = self.event_offset(i);
            self.arena
                .copy_within(source..source + row.loader_len, offset);
            let error_offset = offset + self.profile.id_bytes;
            self.arena[error_offset..error_offset + text.len()].copy_from_slice(text.as_bytes());
        }
        self.events[i] = Some(Navigation {
            loader_len: Some(row.loader_len),
            request: Some(request),
            failure: Some(canceled),
            error_len: text.len(),
            url_len: 0,
        });
        if self.profile.allocator_backed {
            self.owned_events[i] = owned;
        }
        self.event_count += 1;
        self.match_navigation()
    }
    fn match_navigation(&mut self) -> Result<()> {
        if !matches!(self.state, State::Navigation) {
            return Ok(());
        }
        for i in 0..self.event_count {
            let event = self.events[i].unwrap();
            let same_loader = self
                .event_loader(i, event)?
                .zip(self.receipt_loader()?)
                .is_some_and(|(event, receipt)| event == receipt);
            let accepted = if self.accept_any_navigation {
                true
            } else {
                match self.receipt_len {
                    Some(_) => {
                        event.loader_len.is_some() && (same_loader || event.failure.is_none())
                    }
                    None => event.loader_len.is_none(),
                }
            };
            if !accepted {
                continue;
            }
            if !self.accept_any_navigation && self.receipt_len.is_some() && !same_loader {
                return Err(NavigationWaitError::Interrupted);
            }
            if let Some(canceled) = event.failure {
                self.failure_event = Some(i);
                return Err(NavigationWaitError::Network { canceled });
            }
            self.state = State::Lifecycle(event.request);
            break;
        }
        Ok(())
    }
}

fn owned_text(value: &str) -> Result<Box<str>> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| NavigationWaitError::Allocation)?;
    owned.push_str(value);
    Ok(owned.into_boxed_str())
}

fn bounded_json(value: &serde_json::Value, capacity: usize) -> Result<Box<[u8]>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| NavigationWaitError::Allocation)?;
    bytes.resize(capacity, 0);
    let mut writer = Cursor::new(bytes.as_mut_slice());
    serde_json::to_writer(&mut writer, value).map_err(|_| NavigationWaitError::Capacity)?;
    let len = usize::try_from(writer.position()).map_err(|_| NavigationWaitError::SizeOverflow)?;
    bytes.truncate(len);
    Ok(bytes.into_boxed_slice())
}
