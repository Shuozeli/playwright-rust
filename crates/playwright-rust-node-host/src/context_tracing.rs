//! Optional BrowserContext trace export with a real JPEG screenshot resource.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(feature = "context-tracing-network")]
use std::sync::Arc;
use std::sync::Mutex;

#[cfg(feature = "context-tracing-snapshots")]
use playwright_rust_cap_evaluation::{EvaluateParams, EvaluationValue};
use playwright_rust_core::ObjectId;
#[cfg(feature = "context-tracing-network")]
use playwright_rust_har_recording::{HarAttachment, HarExport};
use playwright_rust_trace_playwright_v8::{
    AfterAction, ApiTraceEvent, BeforeAction, ContextOptions, FrameSnapshot, FrameSnapshotViewport,
    PlaywrightV8TraceWriter, ScreencastFrame, SdkLanguage, TraceOrigin, TraceWriterLimits,
};
#[cfg(feature = "context-tracing-console")]
use playwright_rust_trace_playwright_v8::{ConsoleArgument, ConsoleLocation, ConsoleMessageEvent};
use serde::Deserialize;
use serde_json::json;
use sha1::{Digest, Sha1};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[cfg(feature = "context-tracing-console")]
use crate::ConsoleMessage;
use crate::screenshot::{ScreenshotFormat, ScreenshotOptions, ScreenshotScale};
use crate::{HostError, HostSession};

#[derive(Default)]
pub(crate) struct Installation {
    active: Mutex<BTreeMap<ObjectId, ActiveTrace>>,
}

#[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextApiRequestTrace {
    pub method: String,
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<Vec<u8>>,
    pub response_status: u16,
    pub response_status_text: String,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Vec<u8>,
}

struct ActiveTrace {
    screenshots: bool,
    // Retain the complete Playwright start configuration in Rust even while
    // the corresponding artifact producers are still separate opt-in leaves.
    // This makes lifecycle decisions independent of the JavaScript adapter.
    snapshots: bool,
    sources: bool,
    name: Option<String>,
    live_directory: Option<PathBuf>,
    #[cfg(feature = "context-tracing-network")]
    network_registration: Option<u64>,
    chunk: Option<TraceChunk>,
    next_group_id: u64,
    next_timestamp: u64,
}

#[derive(Default)]
struct TraceStartOptions {
    screenshots: bool,
    snapshots: bool,
    sources: bool,
    name: Option<String>,
    title: Option<String>,
    live_directory: Option<PathBuf>,
}

#[derive(Clone, Default)]
struct TraceChunk {
    title: Option<String>,
    events: Vec<OwnedTraceEvent>,
    group_stack: Vec<String>,
    pending_actions: Vec<String>,
    source_files: Vec<String>,
    stacks: Vec<OwnedTraceStack>,
    latest_screencast: Option<TraceFrame>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TraceStackFrame {
    file: String,
    line: u32,
    column: u32,
    function: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedTraceStack {
    call_id: String,
    frame: TraceStackFrame,
}

#[derive(Clone)]
enum OwnedTraceEvent {
    GroupBefore {
        call_id: String,
        title: String,
        parent_id: Option<String>,
        timestamp: f64,
    },
    GroupAfter {
        call_id: String,
        timestamp: f64,
    },
    #[cfg(feature = "context-tracing-actions")]
    ActionBefore {
        call_id: String,
        title: Option<String>,
        class_name: &'static str,
        method: &'static str,
        page_id: Option<String>,
        parent_id: Option<String>,
        timestamp: f64,
        before_snapshot: Option<String>,
    },
    #[cfg(feature = "context-tracing-actions")]
    ActionAfter {
        call_id: String,
        timestamp: f64,
        result: Option<serde_json::Value>,
        after_snapshot: Option<String>,
    },
    #[cfg(feature = "context-tracing-snapshots")]
    FrameSnapshot(OwnedFrameSnapshot),
    #[cfg(feature = "context-tracing-console")]
    Console {
        page_id: String,
        message_type: String,
        text: String,
        args: Vec<OwnedConsoleArgument>,
        url: String,
        line_number: u32,
        column_number: u32,
        timestamp: f64,
    },
}

#[cfg(feature = "context-tracing-snapshots")]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapturedFrameSnapshot {
    frame_url: String,
    doctype: Option<String>,
    html: serde_json::Value,
    viewport: OwnedFrameSnapshotViewport,
    wall_time: f64,
}

#[cfg(feature = "context-tracing-snapshots")]
#[derive(Clone, Copy, Debug, Deserialize)]
struct OwnedFrameSnapshotViewport {
    width: u32,
    height: u32,
}

#[cfg(feature = "context-tracing-snapshots")]
#[derive(Clone, Debug)]
struct OwnedFrameSnapshot {
    snapshot_name: String,
    call_id: String,
    page_id: String,
    frame_id: String,
    frame_url: String,
    timestamp: f64,
    wall_time: f64,
    doctype: Option<String>,
    html: serde_json::Value,
    viewport: OwnedFrameSnapshotViewport,
}

#[cfg(feature = "context-tracing-console")]
#[derive(Clone)]
struct OwnedConsoleArgument {
    preview: String,
    value: serde_json::Value,
}

#[derive(Clone)]
struct TraceCapture {
    screenshots: bool,
    name: Option<String>,
    live_directory: Option<PathBuf>,
    title: Option<String>,
    events: Vec<OwnedTraceEvent>,
    source_files: Vec<String>,
    stacks: Vec<OwnedTraceStack>,
    frame: Option<TraceFrame>,
    #[cfg(feature = "context-tracing-network")]
    network_registration: Option<u64>,
}

impl Installation {
    fn start(&self, context: ObjectId, options: TraceStartOptions) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        if active.contains_key(&context) {
            return Err(HostError::InvalidArgument(
                "Tracing has been already started".into(),
            ));
        }
        active.insert(
            context,
            ActiveTrace {
                screenshots: options.screenshots,
                snapshots: options.snapshots,
                sources: options.sources,
                name: options.name,
                live_directory: options.live_directory,
                #[cfg(feature = "context-tracing-network")]
                network_registration: None,
                chunk: Some(TraceChunk {
                    title: options.title,
                    ..TraceChunk::default()
                }),
                next_group_id: 0,
                next_timestamp: 0,
            },
        );
        Ok(())
    }

    #[cfg(feature = "context-tracing-network")]
    fn set_network_registration(
        &self,
        context: ObjectId,
        registration: u64,
    ) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let trace = active.get_mut(&context).ok_or_else(|| {
            HostError::InvalidArgument("Must start tracing before recording network".into())
        })?;
        trace.network_registration = Some(registration);
        Ok(())
    }

    #[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
    fn network_registration(&self, context: ObjectId) -> Result<Option<u64>, HostError> {
        Ok(self
            .active
            .lock()
            .map_err(HostError::backend)?
            .get(&context)
            .and_then(|trace| trace.network_registration))
    }

    fn stop(&self, context: ObjectId, export: bool) -> Result<Option<TraceCapture>, HostError> {
        let active = self
            .active
            .lock()
            .map_err(HostError::backend)?
            .remove(&context);
        match active {
            Some(mut active) => take_chunk(&mut active, export),
            None if !export => Ok(None),
            None => Err(HostError::InvalidArgument(
                "Must start tracing before stopping".into(),
            )),
        }
    }

    #[cfg(feature = "context-tracing-chunks")]
    fn start_chunk(
        &self,
        context: ObjectId,
        name: Option<String>,
        title: Option<String>,
    ) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let trace = active.get_mut(&context).ok_or_else(|| {
            HostError::InvalidArgument("Must start tracing before starting a new chunk".into())
        })?;
        if name.is_some() {
            trace.name = name;
        }
        trace.chunk = Some(TraceChunk {
            title,
            ..TraceChunk::default()
        });
        Ok(())
    }

    #[cfg(feature = "context-tracing-chunks")]
    fn stop_chunk(
        &self,
        context: ObjectId,
        export: bool,
    ) -> Result<Option<TraceCapture>, HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return if export {
                Err(HostError::InvalidArgument(
                    "Must start tracing before stopping".into(),
                ))
            } else {
                Ok(None)
            };
        };
        take_chunk(trace, export)
    }

    #[cfg(feature = "context-tracing-chunks")]
    fn group(&self, context: ObjectId, title: String) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return Ok(());
        };
        let Some(chunk) = trace.chunk.as_mut() else {
            return Ok(());
        };
        trace.next_group_id += 1;
        trace.next_timestamp += 1;
        let call_id = format!("group@{}", trace.next_group_id);
        let parent_id = chunk.group_stack.last().cloned();
        chunk.events.push(OwnedTraceEvent::GroupBefore {
            call_id: call_id.clone(),
            title,
            parent_id,
            timestamp: trace.next_timestamp as f64,
        });
        chunk.group_stack.push(call_id);
        Ok(())
    }

    #[cfg(feature = "context-tracing-chunks")]
    fn group_end(&self, context: ObjectId) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return Ok(());
        };
        let Some(chunk) = trace.chunk.as_mut() else {
            return Ok(());
        };
        let Some(call_id) = chunk.group_stack.pop() else {
            return Ok(());
        };
        trace.next_timestamp += 1;
        chunk.events.push(OwnedTraceEvent::GroupAfter {
            call_id,
            timestamp: trace.next_timestamp as f64,
        });
        Ok(())
    }

    #[cfg(feature = "context-tracing-actions")]
    fn action_begin(
        &self,
        context: ObjectId,
        page_id: Option<String>,
        action: &str,
        detail: Option<&str>,
        source_file: Option<&str>,
        stack_json: Option<&str>,
    ) -> Result<Option<String>, HostError> {
        let (class_name, method, title) = trace_action(action, detail)?;
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return Ok(None);
        };
        let Some(chunk) = trace.chunk.as_mut() else {
            return Ok(None);
        };
        if trace.sources
            && let Some(source_file) = source_file
            && !chunk.source_files.iter().any(|file| file == source_file)
        {
            chunk.source_files.push(source_file.to_owned());
        }
        trace.next_group_id += 1;
        trace.next_timestamp += 1;
        let call_id = format!("call@{}", trace.next_group_id);
        if let Some(stack_json) = stack_json {
            let frame = serde_json::from_str::<TraceStackFrame>(stack_json).map_err(|error| {
                HostError::InvalidArgument(format!("invalid trace stack frame: {error}"))
            })?;
            chunk.stacks.push(OwnedTraceStack {
                call_id: call_id.clone(),
                frame,
            });
        }
        let parent_id = chunk.group_stack.last().cloned();
        #[cfg(feature = "context-tracing-snapshots")]
        let before_snapshot = (trace.snapshots
            && page_id.is_some()
            && !action.starts_with("dialog-")
            && action != "frame-expect-title"
            && !(action == "page-wait-for-event" && detail == Some("dialog")))
        .then(|| format!("before@{call_id}"));
        #[cfg(not(feature = "context-tracing-snapshots"))]
        let before_snapshot = None;
        chunk.events.push(OwnedTraceEvent::ActionBefore {
            call_id: call_id.clone(),
            title,
            class_name,
            method,
            page_id,
            parent_id,
            timestamp: trace.next_timestamp as f64,
            before_snapshot,
        });
        chunk.pending_actions.push(call_id.clone());
        Ok(Some(call_id))
    }

    #[cfg(feature = "context-tracing-actions")]
    fn action_end(
        &self,
        context: ObjectId,
        call_id: &str,
        result: Option<serde_json::Value>,
    ) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return Ok(());
        };
        let Some(chunk) = trace.chunk.as_mut() else {
            return Ok(());
        };
        let Some(index) = chunk
            .pending_actions
            .iter()
            .position(|pending| pending == call_id)
        else {
            return Ok(());
        };
        chunk.pending_actions.swap_remove(index);
        #[cfg(feature = "context-tracing-snapshots")]
        let after_snapshot = (trace.snapshots
            && chunk.events.iter().any(|event| {
                matches!(
                    event,
                    OwnedTraceEvent::ActionBefore {
                        call_id: before_call_id,
                        page_id: Some(_),
                        before_snapshot: Some(_),
                        ..
                    } if before_call_id == call_id
                )
            }))
        .then(|| format!("after@{call_id}"));
        #[cfg(not(feature = "context-tracing-snapshots"))]
        let after_snapshot = None;
        trace.next_timestamp += 1;
        chunk.events.push(OwnedTraceEvent::ActionAfter {
            call_id: call_id.to_owned(),
            timestamp: trace.next_timestamp as f64,
            result,
            after_snapshot,
        });
        Ok(())
    }

    #[cfg(feature = "context-tracing-snapshots")]
    fn requested_snapshot_name(
        &self,
        context: ObjectId,
        call_id: &str,
        phase: &str,
    ) -> Result<Option<String>, HostError> {
        let active = self.active.lock().map_err(HostError::backend)?;
        let Some(chunk) = active.get(&context).and_then(|trace| trace.chunk.as_ref()) else {
            return Ok(None);
        };
        Ok(chunk.events.iter().find_map(|event| match (phase, event) {
            (
                "before",
                OwnedTraceEvent::ActionBefore {
                    call_id: event_call_id,
                    before_snapshot,
                    ..
                },
            ) if event_call_id == call_id => before_snapshot.clone(),
            (
                "after",
                OwnedTraceEvent::ActionAfter {
                    call_id: event_call_id,
                    after_snapshot,
                    ..
                },
            ) if event_call_id == call_id => after_snapshot.clone(),
            _ => None,
        }))
    }

    #[cfg(feature = "context-tracing-snapshots")]
    fn record_frame_snapshot(
        &self,
        context: ObjectId,
        mut snapshot: OwnedFrameSnapshot,
    ) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return Ok(());
        };
        let Some(chunk) = trace.chunk.as_mut() else {
            return Ok(());
        };
        let still_requested = chunk.events.iter().any(|event| match event {
            OwnedTraceEvent::ActionBefore {
                call_id,
                before_snapshot,
                ..
            } => {
                call_id == &snapshot.call_id
                    && before_snapshot.as_deref() == Some(snapshot.snapshot_name.as_str())
            }
            OwnedTraceEvent::ActionAfter {
                call_id,
                after_snapshot,
                ..
            } => {
                call_id == &snapshot.call_id
                    && after_snapshot.as_deref() == Some(snapshot.snapshot_name.as_str())
            }
            _ => false,
        });
        if !still_requested {
            return Ok(());
        }
        trace.next_timestamp += 1;
        snapshot.timestamp = trace.next_timestamp as f64;
        chunk.events.push(OwnedTraceEvent::FrameSnapshot(snapshot));
        Ok(())
    }

    #[cfg(feature = "context-tracing-snapshots")]
    fn close_fallback_screenshot_requested(
        &self,
        context: ObjectId,
        call_id: &str,
        phase: &str,
    ) -> Result<bool, HostError> {
        if phase != "before" {
            return Ok(false);
        }
        let active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get(&context) else {
            return Ok(false);
        };
        Ok(trace.screenshots
            && trace.chunk.as_ref().is_some_and(|chunk| {
                chunk.events.iter().any(|event| {
                    matches!(
                        event,
                        OwnedTraceEvent::ActionBefore {
                            call_id: event_call_id,
                            class_name: "Page",
                            method: "close",
                            ..
                        } if event_call_id == call_id
                    )
                })
            }))
    }

    #[cfg(feature = "context-tracing-snapshots")]
    fn record_screencast_frame(
        &self,
        context: ObjectId,
        frame: TraceFrame,
    ) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(chunk) = active
            .get_mut(&context)
            .and_then(|trace| trace.chunk.as_mut())
        else {
            return Ok(());
        };
        chunk.latest_screencast = Some(frame);
        Ok(())
    }

    #[cfg(feature = "context-tracing-actions")]
    fn live_capture(&self, context: ObjectId) -> Result<Option<TraceCapture>, HostError> {
        let active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get(&context) else {
            return Ok(None);
        };
        let Some(chunk) = trace.chunk.as_ref() else {
            return Ok(None);
        };
        if trace.live_directory.is_none() || trace.name.is_none() {
            return Ok(None);
        }
        Ok(Some(TraceCapture {
            screenshots: false,
            name: trace.name.clone(),
            live_directory: trace.live_directory.clone(),
            title: chunk.title.clone(),
            events: chunk.events.clone(),
            source_files: chunk.source_files.clone(),
            stacks: chunk.stacks.clone(),
            frame: chunk.latest_screencast.clone(),
            #[cfg(feature = "context-tracing-network")]
            network_registration: trace.network_registration,
        }))
    }

    #[cfg(feature = "context-tracing-console")]
    pub(crate) fn record_console(
        &self,
        context: ObjectId,
        page: ObjectId,
        message: &ConsoleMessage,
    ) -> Result<(), HostError> {
        let mut active = self.active.lock().map_err(HostError::backend)?;
        let Some(trace) = active.get_mut(&context) else {
            return Ok(());
        };
        let Some(chunk) = trace.chunk.as_mut() else {
            return Ok(());
        };
        let values = serde_json::from_str::<serde_json::Value>(message.arguments_json())
            .map_err(HostError::backend)?;
        let values = values.as_array().ok_or_else(|| {
            HostError::Backend("console argument snapshot is not a JSON array".into())
        })?;
        let mut args = Vec::new();
        args.try_reserve_exact(values.len())
            .map_err(HostError::backend)?;
        for value in values {
            args.push(OwnedConsoleArgument {
                preview: console_preview(value),
                value: value.clone(),
            });
        }
        chunk.events.push(OwnedTraceEvent::Console {
            page_id: page.get().to_string(),
            message_type: message.message_type().to_owned(),
            text: message.text().to_owned(),
            args,
            url: message.location_url().to_owned(),
            line_number: message.line_number(),
            column_number: message.column_number(),
            timestamp: message.timestamp_micros() as f64 / 1000.0,
        });
        Ok(())
    }

    #[cfg(not(feature = "context-tracing-network"))]
    pub(crate) fn cancel(&self, context: ObjectId) {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&context);
    }

    #[cfg(feature = "context-tracing-network")]
    pub(crate) fn cancel(&self, context: ObjectId) -> Option<u64> {
        self.active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&context)
            .and_then(|trace| trace.network_registration)
    }
}

#[cfg(feature = "context-tracing-snapshots")]
const FRAME_SNAPSHOT_EXPRESSION: &str = r#"JSON.stringify((() => {
  const inHead = node => {
    let root = node.getRootNode();
    while (root && root.host) {
      if (document.head && document.head.contains(root.host)) return true;
      root = root.host.getRootNode();
    }
    return !!document.head && document.head.contains(node);
  };
  const serialize = node => {
    if (node.nodeType === Node.TEXT_NODE) return node.nodeValue || '';
    if (node.nodeType !== Node.ELEMENT_NODE) return null;
    if (node.tagName === 'IFRAME' && inHead(node)) return null;
    const attributes = {};
    for (const attribute of node.attributes) attributes[attribute.name] = attribute.value;
    const result = [node.tagName, attributes];
    for (const child of node.childNodes) {
      const value = serialize(child);
      if (value !== null) result.push(value);
    }
    if (node.shadowRoot) {
      const shadow = ['#shadow-root', {}];
      for (const child of node.shadowRoot.childNodes) {
        const value = serialize(child);
        if (value !== null) shadow.push(value);
      }
      result.push(shadow);
    }
    return result;
  };
  return {
    frameUrl: location.href,
    doctype: document.doctype ? document.doctype.name : undefined,
    html: serialize(document.documentElement) || ['HTML'],
    viewport: { width: Math.max(1, innerWidth), height: Math.max(1, innerHeight) },
    wallTime: Date.now(),
  };
})())"#;

#[cfg(feature = "context-tracing-console")]
fn console_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn take_chunk(trace: &mut ActiveTrace, export: bool) -> Result<Option<TraceCapture>, HostError> {
    let Some(mut chunk) = trace.chunk.take() else {
        return if export {
            Err(HostError::InvalidArgument(
                "Must start tracing before stopping".into(),
            ))
        } else {
            Ok(None)
        };
    };
    while let Some(call_id) = chunk.group_stack.pop() {
        trace.next_timestamp += 1;
        chunk.events.push(OwnedTraceEvent::GroupAfter {
            call_id,
            timestamp: trace.next_timestamp as f64,
        });
    }
    #[cfg(feature = "context-tracing-actions")]
    for event in &mut chunk.events {
        if let OwnedTraceEvent::ActionBefore {
            call_id,
            title,
            method: "evaluateExpression",
            ..
        } = event
            && chunk
                .pending_actions
                .iter()
                .any(|pending| pending == call_id)
        {
            *title = None;
        }
    }
    #[cfg(feature = "context-tracing-actions")]
    chunk.pending_actions.clear();
    Ok(Some(TraceCapture {
        screenshots: trace.screenshots,
        name: trace.name.clone(),
        live_directory: trace.live_directory.clone(),
        title: chunk.title,
        events: chunk.events,
        source_files: chunk.source_files,
        stacks: chunk.stacks,
        frame: chunk.latest_screencast,
        #[cfg(feature = "context-tracing-network")]
        network_registration: trace.network_registration,
    }))
}

#[cfg(feature = "context-tracing-actions")]
fn trace_action(
    action: &str,
    detail: Option<&str>,
) -> Result<(&'static str, &'static str, Option<String>), HostError> {
    let fixed = |class_name, method, title: &str| Ok((class_name, method, Some(title.to_owned())));
    match action {
        "page-route" => fixed("Page", "route", "Route requests"),
        "route-continue" => fixed("Route", "continue", "Continue request"),
        "page-evaluate" => fixed("Page", "evaluateExpression", "Evaluate"),
        "page-evaluate-handle" => Ok(("Page", "evaluateExpressionHandle", None)),
        "page-wait-for-event" => Ok(("Page", "waitForEventInfo", None)),
        "page-set-content" => fixed("Page", "setContent", "Set content"),
        "page-click" => fixed("Page", "click", "Click"),
        "page-dblclick" => fixed("Page", "dblclick", "Double click"),
        "page-hover" => fixed("Page", "hover", "Hover"),
        "mouse-move" => fixed("Mouse", "move", "Mouse move"),
        "mouse-dblclick" => fixed("Mouse", "dblclick", "Double click"),
        "keyboard-insert-text" => fixed(
            "Keyboard",
            "insertText",
            &format!("Insert \"{}\"", detail.unwrap_or_default()),
        ),
        "locator-set-input-files" => fixed("Locator", "setInputFiles", "Set input files"),
        "page-wait-for-timeout" => fixed("Page", "waitForTimeout", "Wait for timeout"),
        "page-screenshot" => fixed("Page", "screenshot", "Screenshot"),
        "locator-expect" => {
            let title = match detail.unwrap_or_default() {
                "to.be.visible" => "Expect visible",
                "to.be.hidden" => "Expect hidden",
                "to.be.attached" => "Expect attached",
                "to.be.detached" => "Expect detached",
                _ => "Expect locator",
            };
            fixed("Locator", "expect", title)
        }
        "frame-expect-title" => fixed("Frame", "expect", "Expect title"),
        "page-reload" => fixed("Page", "reload", "Reload"),
        "page-close" => fixed("Page", "close", "Close page"),
        "dialog-accept" => fixed("Dialog", "accept", "Accept dialog"),
        "dialog-dismiss" => fixed("Dialog", "dismiss", "Dismiss dialog"),
        "api-request" => {
            let detail = detail.ok_or_else(|| {
                HostError::InvalidArgument(
                    "api-request trace action requires method and URL".into(),
                )
            })?;
            let (method, url) = detail.split_once('\n').ok_or_else(|| {
                HostError::InvalidArgument("api-request trace action has invalid detail".into())
            })?;
            let display = url::Url::parse(url)
                .ok()
                .map(|url| {
                    let mut value = url.path().to_owned();
                    if let Some(query) = url.query() {
                        value.push('?');
                        value.push_str(query);
                    }
                    value
                })
                .unwrap_or_else(|| url.to_owned());
            Ok((
                "APIRequestContext",
                "fetch",
                Some(format!("{method} \"{display}\"")),
            ))
        }
        "browser-context-storage-state" => fixed("BrowserContext", "storageState", "Storage state"),
        "page-goto" => {
            let url = detail.ok_or_else(|| {
                HostError::InvalidArgument("page-goto trace action requires a URL".into())
            })?;
            let display = url::Url::parse(url)
                .ok()
                .map(|url| {
                    let mut value = url.path().to_owned();
                    if let Some(query) = url.query() {
                        value.push('?');
                        value.push_str(query);
                    }
                    value
                })
                .unwrap_or_else(|| url.to_owned());
            Ok(("Page", "goto", Some(format!("Navigate to \"{display}\""))))
        }
        _ => Err(HostError::InvalidArgument(format!(
            "unsupported tracing action {action}"
        ))),
    }
}

impl HostSession {
    #[cfg(feature = "context-tracing-network")]
    fn context_trace_har_installation(
        &self,
    ) -> Result<Arc<crate::har_recording::Installation>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        Ok(state
            .as_ref()
            .ok_or(HostError::TargetClosed)?
            .har_recording
            .clone())
    }

    #[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
    pub fn context_tracing_record_api_request(
        &self,
        context_id: &str,
        trace: ContextApiRequestTrace,
    ) -> Result<(), HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let Some(registration) = self.context_tracing.network_registration(context)? else {
            return Ok(());
        };
        self.context_trace_har_installation()?
            .record_api_request(registration, trace)
    }

    #[cfg(feature = "context-tracing-network")]
    fn finish_context_trace_network(
        &self,
        capture: &TraceCapture,
        export: bool,
    ) -> Result<Option<TraceNetwork>, HostError> {
        let Some(registration) = capture.network_registration else {
            return Ok(None);
        };
        let installation = self.context_trace_har_installation()?;
        if !export {
            installation.discard_context_trace(registration);
            return Ok(None);
        }
        trace_network_from_har(installation.finish_context_trace(registration)?).map(Some)
    }

    #[cfg(feature = "context-tracing-network")]
    fn snapshot_context_trace_network(
        &self,
        capture: &TraceCapture,
        export: bool,
    ) -> Result<Option<TraceNetwork>, HostError> {
        let Some(registration) = capture.network_registration else {
            return Ok(None);
        };
        if !export {
            return Ok(None);
        }
        trace_network_from_har(
            self.context_trace_har_installation()?
                .snapshot_context_trace(registration)?,
        )
        .map(Some)
    }

    pub(crate) fn cancel_context_tracing(&self, context: ObjectId) {
        #[cfg(feature = "context-tracing-network")]
        if let Some(registration) = self.context_tracing.cancel(context)
            && let Ok(installation) = self.context_trace_har_installation()
        {
            installation.discard_context_trace(registration);
        }
        #[cfg(not(feature = "context-tracing-network"))]
        self.context_tracing.cancel(context);
    }

    pub fn context_tracing_start(
        &self,
        context_id: &str,
        screenshots: bool,
        snapshots: bool,
        sources: bool,
        name: Option<String>,
        title: Option<String>,
    ) -> Result<(), HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let live_directory = self.trace_directory().map(Path::to_path_buf);
        self.context_tracing.start(
            context,
            TraceStartOptions {
                screenshots,
                snapshots,
                sources,
                name,
                title,
                live_directory,
            },
        )?;
        #[cfg(feature = "context-tracing-network")]
        if snapshots {
            let installation = self.context_trace_har_installation()?;
            let registration = match installation.start_context_trace(context, self.version()) {
                Ok(registration) => registration,
                Err(error) => {
                    self.context_tracing.cancel(context);
                    return Err(error);
                }
            };
            if let Err(error) = self
                .context_tracing
                .set_network_registration(context, registration)
            {
                installation.discard_context_trace(registration);
                self.context_tracing.cancel(context);
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn context_tracing_stop(
        &self,
        context_id: &str,
        path: Option<PathBuf>,
    ) -> Result<(), HostError> {
        if path.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(HostError::InvalidArgument(
                "trace path must resolve to an absolute path".into(),
            ));
        }
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let export = path.is_some();
        let Some(capture) = self.context_tracing.stop(context, export)? else {
            return Ok(());
        };
        #[cfg(feature = "context-tracing-network")]
        let network = self.finish_context_trace_network(&capture, export)?;
        let Some(path) = path else {
            return Ok(());
        };
        self.persist_context_trace(
            context_id,
            path,
            capture,
            #[cfg(feature = "context-tracing-network")]
            network,
        )
        .await
    }

    #[cfg(feature = "context-tracing-chunks")]
    pub fn context_tracing_start_chunk(
        &self,
        context_id: &str,
        name: Option<String>,
        title: Option<String>,
    ) -> Result<(), HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        self.context_tracing.start_chunk(context, name, title)
    }

    #[cfg(feature = "context-tracing-chunks")]
    pub async fn context_tracing_stop_chunk(
        &self,
        context_id: &str,
        path: Option<PathBuf>,
    ) -> Result<(), HostError> {
        if path.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(HostError::InvalidArgument(
                "trace path must resolve to an absolute path".into(),
            ));
        }
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let export = path.is_some();
        let Some(capture) = self.context_tracing.stop_chunk(context, export)? else {
            return Ok(());
        };
        #[cfg(feature = "context-tracing-network")]
        let network = self.snapshot_context_trace_network(&capture, export)?;
        let Some(path) = path else {
            return Ok(());
        };
        self.persist_context_trace(
            context_id,
            path,
            capture,
            #[cfg(feature = "context-tracing-network")]
            network,
        )
        .await
    }

    #[cfg(feature = "context-tracing-chunks")]
    pub fn context_tracing_group(&self, context_id: &str, title: String) -> Result<(), HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        self.context_tracing.group(context, title)
    }

    #[cfg(feature = "context-tracing-chunks")]
    pub fn context_tracing_group_end(&self, context_id: &str) -> Result<(), HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        self.context_tracing.group_end(context)
    }

    #[cfg(feature = "context-tracing-actions")]
    pub fn context_tracing_action_begin(
        &self,
        context_id: &str,
        page_id: Option<&str>,
        action: &str,
        detail: Option<&str>,
        source_file: Option<&str>,
        stack_json: Option<&str>,
    ) -> Result<Option<String>, HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let page_id = page_id
            .map(|page_id| {
                let page = page_id
                    .parse::<u64>()
                    .ok()
                    .filter(|value| *value != 0)
                    .map(ObjectId::new)
                    .ok_or_else(|| HostError::InvalidArgument("invalid portable Page ID".into()))?;
                self.require_visible_page_id(page)?;
                Ok::<_, HostError>(page_id.to_owned())
            })
            .transpose()?;
        self.context_tracing
            .action_begin(context, page_id, action, detail, source_file, stack_json)
    }

    #[cfg(feature = "context-tracing-actions")]
    pub fn context_tracing_action_end(
        &self,
        context_id: &str,
        call_id: &str,
        result_json: Option<&str>,
    ) -> Result<(), HostError> {
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let result = result_json
            .map(serde_json::from_str)
            .transpose()
            .map_err(|error| {
                HostError::InvalidArgument(format!("invalid trace result: {error}"))
            })?;
        self.context_tracing.action_end(context, call_id, result)?;
        if let Some(capture) = self.context_tracing.live_capture(context)? {
            self.persist_live_context_trace(context_id, &capture, None)?;
        }
        Ok(())
    }

    #[cfg(feature = "context-tracing-snapshots")]
    pub async fn context_tracing_capture_snapshot(
        &self,
        context_id: &str,
        page_id: &str,
        call_id: &str,
        phase: &str,
    ) -> Result<(), HostError> {
        if !matches!(phase, "before" | "after") {
            return Err(HostError::InvalidArgument(
                "trace snapshot phase must be before or after".into(),
            ));
        }
        let context = parse_context(context_id)?;
        self.require_visible_context_id(context)?;
        let Some(snapshot_name) = self
            .context_tracing
            .requested_snapshot_name(context, call_id, phase)?
        else {
            return Ok(());
        };
        let page = self
            .published_pages(context_id)?
            .into_iter()
            .find(|page| page.id == page_id)
            .ok_or(HostError::TargetClosed)?;
        let value = self
            .frame_evaluate(
                &page.main_frame_id,
                EvaluateParams::expression(FRAME_SNAPSHOT_EXPRESSION),
            )
            .await?;
        let EvaluationValue::String(json) = value else {
            return Err(HostError::Backend(
                "trace frame snapshot evaluation returned a non-string value".into(),
            ));
        };
        let captured: CapturedFrameSnapshot =
            serde_json::from_str(&json).map_err(HostError::backend)?;
        let published_page_id = page.id.clone();
        let capture_close_fallback = self
            .context_tracing
            .close_fallback_screenshot_requested(context, call_id, phase)?;
        self.context_tracing.record_frame_snapshot(
            context,
            OwnedFrameSnapshot {
                snapshot_name,
                call_id: call_id.to_owned(),
                page_id: page.id,
                frame_id: page.main_frame_id,
                frame_url: captured.frame_url,
                timestamp: 0.0,
                wall_time: captured.wall_time,
                doctype: captured.doctype,
                html: captured.html,
                viewport: captured.viewport,
            },
        )?;
        if capture_close_fallback
            && let Ok(image) = self
                .page_screenshot(
                    &published_page_id,
                    ScreenshotOptions {
                        format: Some(ScreenshotFormat::Jpeg),
                        quality: Some(80),
                        full_page: Some(false),
                        scale: Some(ScreenshotScale::Css),
                        ..ScreenshotOptions::default()
                    },
                    30_000,
                )
                .await
        {
            let bytes = image.as_bytes().to_vec();
            let (width, height) = jpeg_dimensions(&bytes)?;
            self.context_tracing.record_screencast_frame(
                context,
                TraceFrame {
                    page_id: published_page_id,
                    resource: format!("{:x}.jpeg", Sha1::digest(&bytes)),
                    width,
                    height,
                    bytes,
                },
            )?;
        }
        if phase == "after"
            && let Some(capture) = self.context_tracing.live_capture(context)?
        {
            self.persist_live_context_trace(context_id, &capture, None)?;
        }
        Ok(())
    }

    fn persist_live_context_trace(
        &self,
        context_id: &str,
        capture: &TraceCapture,
        #[cfg(feature = "context-tracing-network")] network: Option<&TraceNetwork>,
        #[cfg(not(feature = "context-tracing-network"))] _network: Option<&()>,
    ) -> Result<(), HostError> {
        let (Some(directory), Some(name)) = (&capture.live_directory, &capture.name) else {
            return Ok(());
        };
        let trace = trace_jsonl(
            context_id,
            self.version(),
            capture.title.as_deref(),
            &capture.events,
            None,
        )?;
        persist_live_trace(
            directory,
            name,
            &trace,
            #[cfg(feature = "context-tracing-network")]
            network,
        )
    }

    async fn persist_context_trace(
        &self,
        context_id: &str,
        path: PathBuf,
        capture: TraceCapture,
        #[cfg(feature = "context-tracing-network")] network: Option<TraceNetwork>,
    ) -> Result<(), HostError> {
        let mut frame = capture.frame.clone();
        if capture.screenshots
            && let Some(page) = self.published_pages(context_id)?.into_iter().last()
        {
            let image = match self
                .page_screenshot(
                    &page.id,
                    ScreenshotOptions {
                        format: Some(ScreenshotFormat::Jpeg),
                        quality: Some(80),
                        full_page: Some(false),
                        scale: Some(ScreenshotScale::Css),
                        ..ScreenshotOptions::default()
                    },
                    30_000,
                )
                .await
            {
                Ok(image) => Some(image),
                Err(error) if recoverable_trace_screenshot_error(&error) => None,
                Err(error) => return Err(error),
            };
            if let Some(image) = image {
                let bytes = image.as_bytes().to_vec();
                let (width, height) = jpeg_dimensions(&bytes)?;
                let resource = format!("{:x}.jpeg", Sha1::digest(&bytes));
                frame = Some(TraceFrame {
                    page_id: page.id,
                    resource,
                    width,
                    height,
                    bytes,
                });
            }
        }
        self.persist_context_trace_capture(
            context_id,
            path,
            capture,
            #[cfg(feature = "context-tracing-network")]
            network,
            frame,
        )
        .await
    }

    async fn persist_context_trace_capture(
        &self,
        context_id: &str,
        path: PathBuf,
        capture: TraceCapture,
        #[cfg(feature = "context-tracing-network")] network: Option<TraceNetwork>,
        frame: Option<TraceFrame>,
    ) -> Result<(), HostError> {
        let trace = trace_jsonl(
            context_id,
            self.version(),
            capture.title.as_deref(),
            &capture.events,
            frame.as_ref(),
        )?;
        self.persist_live_context_trace(
            context_id,
            &capture,
            #[cfg(feature = "context-tracing-network")]
            network.as_ref(),
            #[cfg(not(feature = "context-tracing-network"))]
            None,
        )?;
        let source_files = capture.source_files;
        let stacks = capture.stacks;
        tokio::task::spawn_blocking(move || {
            persist_trace(
                &path,
                &trace,
                frame.as_ref(),
                #[cfg(feature = "context-tracing-network")]
                network.as_ref(),
                &source_files,
                &stacks,
            )
        })
        .await
        .map_err(HostError::backend)?
    }
}

fn recoverable_trace_screenshot_error(error: &HostError) -> bool {
    matches!(error, HostError::TargetClosed | HostError::FrameDetached)
        || matches!(error, HostError::Backend(message) if message.contains("CDP layout metrics contain an invalid device scale factor") || message.contains("Inspected target navigated or closed"))
}

#[derive(Clone)]
struct TraceFrame {
    page_id: String,
    resource: String,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

#[cfg(feature = "context-tracing-network")]
struct TraceNetwork {
    jsonl: Vec<u8>,
    attachments: Vec<HarAttachment>,
}

#[cfg(feature = "context-tracing-network")]
fn trace_network_from_har(export: HarExport) -> Result<TraceNetwork, HostError> {
    let mut document =
        serde_json::from_slice::<serde_json::Value>(&export.json).map_err(HostError::backend)?;
    let entries = document
        .pointer_mut("/log/entries")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| HostError::Backend("trace HAR export has no entries".into()))?;
    let entries = std::mem::take(entries);
    let mut omitted = std::collections::HashSet::new();
    let mut jsonl = Vec::new();
    for mut entry in entries {
        let content = entry
            .pointer_mut("/response/content")
            .and_then(serde_json::Value::as_object_mut);
        if let Some(content) = content {
            let mime = content
                .get("mimeType")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            let attachment = content
                .remove("_file")
                .or_else(|| content.remove("_sha1"))
                .and_then(|value| value.as_str().map(str::to_owned));
            if let Some(name) = attachment {
                if matches!(mime.as_str(), "application/javascript" | "text/javascript") {
                    omitted.insert(name);
                } else {
                    content.insert("_sha1".to_owned(), serde_json::Value::String(name));
                }
            }
        }
        serde_json::to_writer(
            &mut jsonl,
            &json!({ "type": "resource-snapshot", "snapshot": entry }),
        )
        .map_err(HostError::backend)?;
        jsonl.push(b'\n');
    }
    Ok(TraceNetwork {
        jsonl,
        attachments: export
            .attachments
            .into_iter()
            .filter(|attachment| !omitted.contains(&attachment.name))
            .collect(),
    })
}

fn trace_jsonl(
    context_id: &str,
    browser_version: &str,
    title: Option<&str>,
    events: &[OwnedTraceEvent],
    frame: Option<&TraceFrame>,
) -> Result<Vec<u8>, HostError> {
    let options = json!({});
    let mut writer = PlaywrightV8TraceWriter::new(
        Vec::new(),
        // Playwright does not impose an API-visible byte ceiling on a trace
        // chunk. Keep only the exact event-count bound for this in-memory
        // capture; fallible allocation and checked byte accounting in the
        // writer remain the resource boundary.
        TraceWriterLimits::new(usize::MAX, u64::MAX, events.len() + 2),
    )
    .map_err(HostError::backend)?;
    writer
        .write_context(&ContextOptions {
            origin: TraceOrigin::Library,
            browser_name: "chromium",
            channel: Some("chromium"),
            platform: std::env::consts::OS,
            wall_time_ms: 0.0,
            monotonic_time_ms: 0.0,
            title,
            options: &options,
            sdk_language: Some(SdkLanguage::JavaScript),
            test_id_attribute_name: None,
            context_id: Some(context_id),
            playwright_version: Some(browser_version),
        })
        .map_err(HostError::backend)?;
    let empty_params = json!({});
    for event in events {
        match event {
            OwnedTraceEvent::GroupBefore {
                call_id,
                title,
                parent_id,
                timestamp,
            } => writer.write_api_event(ApiTraceEvent::Before(BeforeAction {
                call_id,
                start_time_ms: *timestamp,
                title: Some(title),
                class_name: "Tracing",
                method: "tracingGroup",
                params: &empty_params,
                step_id: None,
                before_snapshot: None,
                stack: None,
                page_id: None,
                parent_id: parent_id.as_deref(),
            })),
            OwnedTraceEvent::GroupAfter { call_id, timestamp } => {
                writer.write_api_event(ApiTraceEvent::After(AfterAction {
                    call_id,
                    end_time_ms: *timestamp,
                    after_snapshot: None,
                    error: None,
                    result: None,
                    point: None,
                }))
            }
            #[cfg(feature = "context-tracing-actions")]
            OwnedTraceEvent::ActionBefore {
                call_id,
                title,
                class_name,
                method,
                page_id,
                parent_id,
                timestamp,
                before_snapshot,
            } => writer.write_api_event(ApiTraceEvent::Before(BeforeAction {
                call_id,
                start_time_ms: *timestamp,
                title: title.as_deref(),
                class_name,
                method,
                params: &empty_params,
                step_id: None,
                before_snapshot: before_snapshot.as_deref(),
                stack: None,
                page_id: page_id.as_deref(),
                parent_id: parent_id.as_deref(),
            })),
            #[cfg(feature = "context-tracing-actions")]
            OwnedTraceEvent::ActionAfter {
                call_id,
                timestamp,
                result,
                after_snapshot,
            } => writer.write_api_event(ApiTraceEvent::After(AfterAction {
                call_id,
                end_time_ms: *timestamp,
                after_snapshot: after_snapshot.as_deref(),
                error: None,
                result: result.as_ref(),
                point: None,
            })),
            #[cfg(feature = "context-tracing-snapshots")]
            OwnedTraceEvent::FrameSnapshot(snapshot) => {
                writer.write_frame_snapshot(FrameSnapshot {
                    snapshot_name: Some(&snapshot.snapshot_name),
                    call_id: &snapshot.call_id,
                    page_id: &snapshot.page_id,
                    frame_id: &snapshot.frame_id,
                    frame_url: &snapshot.frame_url,
                    timestamp: snapshot.timestamp,
                    wall_time: Some(snapshot.wall_time),
                    collection_time: 0.0,
                    doctype: snapshot.doctype.as_deref(),
                    html: &snapshot.html,
                    resource_overrides: &[],
                    viewport: FrameSnapshotViewport {
                        width: snapshot.viewport.width,
                        height: snapshot.viewport.height,
                    },
                    is_main_frame: true,
                })
            }
            #[cfg(feature = "context-tracing-console")]
            OwnedTraceEvent::Console {
                page_id,
                message_type,
                text,
                args,
                url,
                line_number,
                column_number,
                timestamp,
            } => {
                let args = args
                    .iter()
                    .map(|argument| ConsoleArgument {
                        preview: &argument.preview,
                        value: &argument.value,
                    })
                    .collect::<Vec<_>>();
                writer.write_console_event(ConsoleMessageEvent::new(
                    *timestamp,
                    Some(page_id),
                    message_type,
                    text,
                    Some(&args),
                    ConsoleLocation {
                        url,
                        line_number: *line_number,
                        column_number: *column_number,
                    },
                ))
            }
        }
        .map_err(HostError::backend)?;
    }
    if let Some(frame) = frame {
        writer
            .write_screencast_frame(ScreencastFrame::new(
                &frame.page_id,
                &frame.resource,
                frame.width,
                frame.height,
                0.0,
            ))
            .map_err(HostError::backend)?;
    }
    writer.finish().map_err(HostError::backend)
}

fn persist_trace(
    path: &Path,
    trace: &[u8],
    frame: Option<&TraceFrame>,
    #[cfg(feature = "context-tracing-network")] network: Option<&TraceNetwork>,
    source_files: &[String],
    stacks: &[OwnedTraceStack],
) -> Result<(), HostError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(HostError::backend)?;
    let file = File::create(path).map_err(HostError::backend)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    archive
        .start_file("trace.trace", options)
        .map_err(HostError::backend)?;
    archive.write_all(trace).map_err(HostError::backend)?;
    archive
        .start_file("trace.stacks", options)
        .map_err(HostError::backend)?;
    archive
        .write_all(&trace_stacks_json(stacks)?)
        .map_err(HostError::backend)?;
    #[cfg(feature = "context-tracing-network")]
    if let Some(network) = network {
        archive
            .start_file("trace.network", options)
            .map_err(HostError::backend)?;
        archive
            .write_all(&network.jsonl)
            .map_err(HostError::backend)?;
        for attachment in &network.attachments {
            archive
                .start_file(format!("resources/{}", attachment.name), options)
                .map_err(HostError::backend)?;
            archive
                .write_all(&attachment.bytes)
                .map_err(HostError::backend)?;
        }
    }
    if let Some(frame) = frame {
        archive
            .start_file(format!("resources/{}", frame.resource), options)
            .map_err(HostError::backend)?;
        archive
            .write_all(&frame.bytes)
            .map_err(HostError::backend)?;
    }
    for source_file in source_files {
        let Ok(bytes) = std::fs::read(source_file) else {
            continue;
        };
        let name = format!(
            "resources/src@{:x}.txt",
            Sha1::digest(source_file.as_bytes())
        );
        archive
            .start_file(name, options)
            .map_err(HostError::backend)?;
        archive.write_all(&bytes).map_err(HostError::backend)?;
    }
    archive.finish().map_err(HostError::backend)?;
    Ok(())
}

fn trace_stacks_json(stacks: &[OwnedTraceStack]) -> Result<Vec<u8>, HostError> {
    let mut files = Vec::<String>::new();
    let mut serialized = Vec::new();
    for stack in stacks {
        let file = files
            .iter()
            .position(|file| file == &stack.frame.file)
            .unwrap_or_else(|| {
                files.push(stack.frame.file.clone());
                files.len() - 1
            });
        let call_id = stack
            .call_id
            .strip_prefix("call@")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| HostError::Backend("trace stack has invalid call ID".into()))?;
        serialized.push(json!([
            call_id,
            [[
                file,
                stack.frame.line,
                stack.frame.column,
                stack.frame.function,
            ]]
        ]));
    }
    serde_json::to_vec(&json!({ "files": files, "stacks": serialized })).map_err(HostError::backend)
}

fn persist_live_trace(
    directory: &Path,
    name: &str,
    trace: &[u8],
    #[cfg(feature = "context-tracing-network")] network: Option<&TraceNetwork>,
) -> Result<(), HostError> {
    let name = if name.is_empty() {
        "trace".to_owned()
    } else {
        name.chars()
            .map(|character| {
                if matches!(character, '/' | '\\') || character.is_control() {
                    '_'
                } else {
                    character
                }
            })
            .collect()
    };
    std::fs::create_dir_all(directory).map_err(HostError::backend)?;
    std::fs::write(directory.join(format!("{name}.trace")), trace).map_err(HostError::backend)?;
    #[cfg(feature = "context-tracing-network")]
    if let Some(network) = network {
        std::fs::write(directory.join(format!("{name}.network")), &network.jsonl)
            .map_err(HostError::backend)?;
    }
    Ok(())
}

fn parse_context(value: &str) -> Result<ObjectId, HostError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value != 0)
        .map(ObjectId::new)
        .ok_or_else(|| HostError::InvalidArgument("invalid portable Context ID".into()))
}

fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32), HostError> {
    if bytes.get(..2) != Some(&[0xff, 0xd8]) {
        return Err(HostError::Backend(
            "trace screenshot is not a JPEG image".into(),
        ));
    }
    let mut offset = 2usize;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            offset += 1;
            continue;
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let Some(&marker) = bytes.get(offset) else {
            break;
        };
        offset += 1;
        if marker == 0xd8 || marker == 0xd9 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let Some(length_bytes) = bytes.get(offset..offset + 2) else {
            break;
        };
        let length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
        if length < 2 || offset + length > bytes.len() {
            break;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && length >= 7
        {
            let height = u32::from(u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]));
            let width = u32::from(u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]));
            if width > 0 && height > 0 {
                return Ok((width, height));
            }
        }
        offset += length;
    }
    Err(HostError::Backend(
        "trace screenshot JPEG dimensions are unavailable".into(),
    ))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "context-tracing-network")]
    use super::trace_network_from_har;
    use super::{
        Installation, OwnedTraceEvent, OwnedTraceStack, TraceFrame, TraceStackFrame,
        TraceStartOptions, jpeg_dimensions, persist_live_trace, persist_trace,
        recoverable_trace_screenshot_error, trace_action, trace_jsonl, trace_stacks_json,
    };
    #[cfg(feature = "context-tracing-snapshots")]
    use super::{OwnedFrameSnapshot, OwnedFrameSnapshotViewport};
    use crate::HostError;
    use playwright_rust_core::ObjectId;
    #[cfg(feature = "context-tracing-network")]
    use playwright_rust_har_recording::{HarAttachment, HarExport};
    #[cfg(feature = "context-tracing-snapshots")]
    use serde_json::json;
    use sha1::{Digest, Sha1};
    use std::io::Read;

    #[test]
    fn jpeg_dimensions_reads_the_start_of_frame_segment() {
        // Arrange
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01,
            0x90, 0x01, 0xf4, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x00, 0x03, 0x11, 0x00, 0xff,
            0xd9,
        ];

        // Act
        let dimensions = jpeg_dimensions(&jpeg).unwrap();

        // Assert
        assert_eq!(dimensions, (500, 400));
    }

    #[cfg(feature = "context-tracing-actions")]
    #[test]
    fn api_request_action_uses_the_http_method_and_url_path() {
        // Arrange
        let detail = "POST\nhttps://example.test/simple.json?one=two";

        // Act
        let action = trace_action("api-request", Some(detail)).unwrap();

        // Assert
        assert_eq!(action.0, "APIRequestContext");
        assert_eq!(action.1, "fetch");
        assert_eq!(action.2.as_deref(), Some("POST \"/simple.json?one=two\""));
    }

    #[test]
    fn trace_stacks_serialize_call_ids_and_deduplicate_files() {
        // Arrange
        let stacks = ["call@2", "call@4"].map(|call_id| OwnedTraceStack {
            call_id: call_id.to_owned(),
            frame: TraceStackFrame {
                file: "/tests/tracing.spec.ts".to_owned(),
                line: 12,
                column: 34,
                function: "test callback".to_owned(),
            },
        });

        // Act
        let value: serde_json::Value =
            serde_json::from_slice(&trace_stacks_json(&stacks).unwrap()).unwrap();

        // Assert
        assert_eq!(
            value["files"],
            serde_json::json!(["/tests/tracing.spec.ts"])
        );
        assert_eq!(
            value["stacks"][0],
            serde_json::json!([2, [[0, 12, 34, "test callback"]]])
        );
        assert_eq!(value["stacks"][1][0], 4);
    }

    #[test]
    fn trace_export_recovers_only_from_transient_screenshot_failures() {
        // Arrange
        let navigation = HostError::Backend(
            "ProtocolViolation: CDP layout metrics contain an invalid device scale factor".into(),
        );
        let closed = HostError::TargetClosed;
        let allocation = HostError::ResourceExhausted("screenshot bytes".into());

        // Act
        let navigation_is_recoverable = recoverable_trace_screenshot_error(&navigation);
        let close_is_recoverable = recoverable_trace_screenshot_error(&closed);
        let allocation_is_recoverable = recoverable_trace_screenshot_error(&allocation);

        // Assert
        assert!(navigation_is_recoverable);
        assert!(close_is_recoverable);
        assert!(!allocation_is_recoverable);
    }

    #[test]
    fn stopping_without_export_discards_or_ignores_missing_tracing() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(3);
        tracing
            .start(
                context,
                TraceStartOptions {
                    screenshots: true,
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();

        // Act
        let active = tracing.stop(context, false).unwrap();
        let absent = tracing.stop(context, false).unwrap();
        let export_error = match tracing.stop(context, true) {
            Ok(_) => panic!("export without an active trace must fail"),
            Err(error) => error,
        };

        // Assert
        let active = active.unwrap();
        assert!(active.screenshots);
        assert!(active.events.is_empty());
        assert!(absent.is_none());
        assert!(
            export_error
                .to_string()
                .contains("Must start tracing before stopping")
        );
    }

    #[test]
    fn tracing_start_retains_playwright_capture_options_in_rust() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(13);

        // Act
        tracing
            .start(
                context,
                TraceStartOptions {
                    screenshots: true,
                    snapshots: true,
                    sources: true,
                    name: Some("named-trace".into()),
                    title: Some("trace title".into()),
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();

        // Assert
        let active = tracing.active.lock().unwrap();
        let trace = active.get(&context).unwrap();
        assert!(trace.screenshots);
        assert!(trace.snapshots);
        assert!(trace.sources);
        assert_eq!(trace.name.as_deref(), Some("named-trace"));
        assert_eq!(
            trace.chunk.as_ref().unwrap().title.as_deref(),
            Some("trace title")
        );
    }

    #[test]
    fn trace_archive_contains_the_jsonl_reference_and_matching_resource() {
        // Arrange
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("trace.zip");
        let frame = TraceFrame {
            page_id: "7".into(),
            resource: "frame.jpeg".into(),
            width: 500,
            height: 400,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
        };
        let trace = trace_jsonl("3", "1.58.0", None, &[], Some(&frame)).unwrap();

        // Act
        persist_trace(
            &path,
            &trace,
            Some(&frame),
            #[cfg(feature = "context-tracing-network")]
            None,
            &[],
            &[],
        )
        .unwrap();

        // Assert
        let file = std::fs::File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut trace_entry = String::new();
        archive
            .by_name("trace.trace")
            .unwrap()
            .read_to_string(&mut trace_entry)
            .unwrap();
        assert!(trace_entry.contains("\"type\":\"screencast-frame\""));
        let mut resource = Vec::new();
        archive
            .by_name("resources/frame.jpeg")
            .unwrap()
            .read_to_end(&mut resource)
            .unwrap();
        assert_eq!(resource, frame.bytes);
        let mut stacks = String::new();
        archive
            .by_name("trace.stacks")
            .unwrap()
            .read_to_string(&mut stacks)
            .unwrap();
        assert_eq!(stacks, r#"{"files":[],"stacks":[]}"#);
    }

    #[cfg(feature = "context-tracing-actions")]
    #[test]
    fn trace_sources_are_rust_owned_and_scoped_to_the_active_chunk() {
        // Arrange
        let directory = tempfile::tempdir().unwrap();
        let source_file = directory.path().join("source-fixture.ts");
        let source_bytes = b"export const answer = 42;\n";
        std::fs::write(&source_file, source_bytes).unwrap();
        let source_file = source_file.to_str().unwrap().to_owned();
        let tracing = Installation::default();
        let context = ObjectId::new(14);
        tracing
            .start(
                context,
                TraceStartOptions {
                    sources: true,
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();
        let call_id = tracing
            .action_begin(
                context,
                Some("15".into()),
                "page-click",
                None,
                Some(&source_file),
                None,
            )
            .unwrap()
            .unwrap();
        tracing.action_end(context, &call_id, None).unwrap();
        let capture = tracing.stop(context, true).unwrap().unwrap();
        let trace = trace_jsonl("14", "1.58.0", None, &capture.events, None).unwrap();
        let archive_path = directory.path().join("sources.zip");

        // Act
        persist_trace(
            &archive_path,
            &trace,
            None,
            #[cfg(feature = "context-tracing-network")]
            None,
            &capture.source_files,
            &capture.stacks,
        )
        .unwrap();

        // Assert
        assert_eq!(capture.source_files, std::slice::from_ref(&source_file));
        let expected_name = format!(
            "resources/src@{:x}.txt",
            Sha1::digest(source_file.as_bytes())
        );
        let file = std::fs::File::open(archive_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut source = Vec::new();
        archive
            .by_name(&expected_name)
            .unwrap()
            .read_to_end(&mut source)
            .unwrap();
        assert_eq!(source, source_bytes);
    }

    #[cfg(feature = "context-tracing-network")]
    #[test]
    fn trace_network_wraps_har_entries_and_omits_javascript_bodies() {
        // Arrange
        let export = HarExport {
            json: serde_json::to_vec(&serde_json::json!({
                "log": { "entries": [
                    {
                        "request": { "method": "GET", "url": "https://example.test/style.css" },
                        "response": { "content": { "mimeType": "text/css", "_file": "style.css" } }
                    },
                    {
                        "request": { "method": "GET", "url": "https://example.test/script.js" },
                        "response": { "content": { "mimeType": "application/javascript", "_file": "script.js" } }
                    }
                ] }
            }))
            .unwrap(),
            attachments: vec![
                HarAttachment {
                    name: "style.css".into(),
                    bytes: b"body{}".to_vec(),
                },
                HarAttachment {
                    name: "script.js".into(),
                    bytes: b"void 0".to_vec(),
                },
            ],
        };

        // Act
        let network = trace_network_from_har(export).unwrap();

        // Assert
        let jsonl = String::from_utf8(network.jsonl).unwrap();
        assert_eq!(jsonl.matches("\"type\":\"resource-snapshot\"").count(), 2);
        assert!(jsonl.contains("\"_sha1\":\"style.css\""));
        assert!(!jsonl.contains("\"_sha1\":\"script.js\""));
        assert_eq!(network.attachments.len(), 1);
        assert_eq!(network.attachments[0].name, "style.css");
    }

    #[test]
    fn trace_jsonl_has_no_private_record_or_total_byte_ceiling() {
        // Arrange
        let title = "x".repeat(1024 * 1024 + 1);
        let events = [OwnedTraceEvent::GroupBefore {
            call_id: "group@1".into(),
            title,
            parent_id: None,
            timestamp: 1.0,
        }];

        // Act
        let trace = trace_jsonl("3", "1.58.0", None, &events, None).unwrap();

        // Assert
        assert!(trace.len() > 1024 * 1024);
        assert!(
            trace
                .windows(1024)
                .any(|window| window.iter().all(|byte| *byte == b'x'))
        );
    }

    #[test]
    fn tracing_start_title_is_written_to_the_initial_chunk_header() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(5);
        tracing
            .start(
                context,
                TraceStartOptions {
                    title: Some("initial title".into()),
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();

        // Act
        let capture = tracing.stop(context, true).unwrap().unwrap();
        let trace = String::from_utf8(
            trace_jsonl(
                "5",
                "1.58.0",
                capture.title.as_deref(),
                &capture.events,
                None,
            )
            .unwrap(),
        )
        .unwrap();

        // Assert
        assert!(trace.contains("\"title\":\"initial title\""));
    }

    #[cfg(feature = "context-tracing-actions")]
    #[test]
    fn trace_actions_use_rust_owned_titles_without_fabricating_pending_after_events() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(6);
        tracing
            .start(context, TraceStartOptions::default())
            .unwrap();
        let goto = tracing
            .action_begin(
                context,
                Some("7".into()),
                "page-goto",
                Some("https://example.test/empty.html?x=1"),
                None,
                None,
            )
            .unwrap()
            .unwrap();
        tracing.action_end(context, &goto, None).unwrap();
        let storage = tracing
            .action_begin(
                context,
                None,
                "browser-context-storage-state",
                None,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        tracing.action_end(context, &storage, None).unwrap();
        for action in [
            "page-reload",
            "page-close",
            "page-set-content",
            "page-click",
            "page-dblclick",
            "page-hover",
            "mouse-move",
            "mouse-dblclick",
            "locator-set-input-files",
            "locator-expect",
            "page-wait-for-timeout",
            "dialog-dismiss",
        ] {
            let call_id = tracing
                .action_begin(context, Some("7".into()), action, None, None, None)
                .unwrap()
                .unwrap();
            tracing.action_end(context, &call_id, None).unwrap();
        }
        let insert = tracing
            .action_begin(
                context,
                Some("7".into()),
                "keyboard-insert-text",
                Some("abc"),
                None,
                None,
            )
            .unwrap()
            .unwrap();
        tracing.action_end(context, &insert, None).unwrap();
        tracing
            .action_begin(context, Some("7".into()), "page-evaluate", None, None, None)
            .unwrap();
        tracing
            .action_begin(context, Some("7".into()), "page-click", None, None, None)
            .unwrap();

        // Act
        let capture = tracing.stop(context, true).unwrap().unwrap();
        let trace =
            String::from_utf8(trace_jsonl("6", "1.58.0", None, &capture.events, None).unwrap())
                .unwrap();

        // Assert
        assert!(trace.contains("\"title\":\"Navigate to \\\"/empty.html?x=1\\\"\""));
        assert!(trace.contains("\"title\":\"Storage state\""));
        assert!(trace.contains("\"title\":\"Reload\""));
        assert!(trace.contains("\"title\":\"Close page\""));
        assert!(trace.contains("\"title\":\"Set content\""));
        assert!(trace.contains("\"title\":\"Click\""));
        assert!(trace.contains("\"title\":\"Double click\""));
        assert!(trace.contains("\"title\":\"Mouse move\""));
        assert!(trace.contains("\"title\":\"Insert \\\"abc\\\"\""));
        assert!(trace.contains("\"title\":\"Set input files\""));
        assert!(trace.contains("\"title\":\"Expect locator\""));
        assert!(trace.contains("\"title\":\"Wait for timeout\""));
        assert!(trace.contains("\"title\":\"Dismiss dialog\""));
        assert_eq!(trace.matches("\"type\":\"before\"").count(), 17);
        assert_eq!(trace.matches("\"type\":\"after\"").count(), 15);
        assert!(trace.contains("\"method\":\"evaluateExpression\""));
        assert_eq!(trace.matches("\"title\":\"Click\"").count(), 2);
    }

    #[cfg(feature = "context-tracing-snapshots")]
    #[test]
    fn action_snapshots_reference_real_frame_events_and_buffer_placeholder() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(16);
        tracing
            .start(
                context,
                TraceStartOptions {
                    snapshots: true,
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();
        let call_id = tracing
            .action_begin(
                context,
                Some("17".into()),
                "page-screenshot",
                None,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        let snapshot = |name: &str| OwnedFrameSnapshot {
            snapshot_name: name.into(),
            call_id: call_id.clone(),
            page_id: "17".into(),
            frame_id: "18".into(),
            frame_url: "https://example.test/".into(),
            timestamp: 0.0,
            wall_time: 1.0,
            doctype: Some("html".into()),
            html: json!(["HTML", {}, ["HEAD"], ["BODY", {}, "hello"]]),
            viewport: OwnedFrameSnapshotViewport {
                width: 800,
                height: 600,
            },
        };
        tracing
            .record_frame_snapshot(context, snapshot(&format!("before@{call_id}")))
            .unwrap();
        tracing
            .action_end(context, &call_id, Some(json!({ "binary": "<Buffer>" })))
            .unwrap();
        tracing
            .record_frame_snapshot(context, snapshot(&format!("after@{call_id}")))
            .unwrap();

        // Act
        let capture = tracing.stop(context, true).unwrap().unwrap();
        let trace =
            String::from_utf8(trace_jsonl("16", "1.58.0", None, &capture.events, None).unwrap())
                .unwrap();

        // Assert
        assert!(trace.contains(&format!("\"beforeSnapshot\":\"before@{call_id}\"")));
        assert!(trace.contains(&format!("\"afterSnapshot\":\"after@{call_id}\"")));
        assert_eq!(trace.matches("\"type\":\"frame-snapshot\"").count(), 2);
        assert!(trace.contains("\"result\":{\"binary\":\"<Buffer>\"}"));
    }

    #[cfg(feature = "context-tracing-snapshots")]
    #[test]
    fn screencast_fallback_is_requested_only_before_page_close() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(19);
        tracing
            .start(
                context,
                TraceStartOptions {
                    screenshots: true,
                    snapshots: true,
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();
        let set_content = tracing
            .action_begin(
                context,
                Some("20".into()),
                "page-set-content",
                None,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        tracing.action_end(context, &set_content, None).unwrap();
        let close = tracing
            .action_begin(context, Some("20".into()), "page-close", None, None, None)
            .unwrap()
            .unwrap();

        // Act
        let set_before = tracing
            .close_fallback_screenshot_requested(context, &set_content, "before")
            .unwrap();
        let set_after = tracing
            .close_fallback_screenshot_requested(context, &set_content, "after")
            .unwrap();
        let close_before = tracing
            .close_fallback_screenshot_requested(context, &close, "before")
            .unwrap();
        let close_after = tracing
            .close_fallback_screenshot_requested(context, &close, "after")
            .unwrap();

        // Assert
        assert!(!set_before);
        assert!(!set_after);
        assert!(close_before);
        assert!(!close_after);
    }

    #[cfg(feature = "context-tracing-snapshots")]
    #[test]
    fn dialog_and_title_actions_do_not_reference_absent_snapshots() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(21);
        tracing
            .start(
                context,
                TraceStartOptions {
                    snapshots: true,
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();

        // Act
        let mut observations = Vec::new();
        for (action, detail) in [
            ("dialog-dismiss", None),
            ("frame-expect-title", None),
            ("page-wait-for-event", Some("dialog")),
        ] {
            let call_id = tracing
                .action_begin(context, Some("22".into()), action, detail, None, None)
                .unwrap()
                .unwrap();
            let before = tracing
                .requested_snapshot_name(context, &call_id, "before")
                .unwrap();
            tracing.action_end(context, &call_id, None).unwrap();
            let after = tracing
                .requested_snapshot_name(context, &call_id, "after")
                .unwrap();
            observations.push((before, after));
        }

        // Assert
        assert_eq!(observations, vec![(None, None), (None, None), (None, None)]);
    }

    #[cfg(feature = "context-tracing-actions")]
    #[test]
    fn named_live_trace_flushes_the_completed_primitive_result() {
        // Arrange
        let directory = tempfile::tempdir().unwrap();
        let tracing = Installation::default();
        let context = ObjectId::new(10);
        tracing
            .start(
                context,
                TraceStartOptions {
                    name: Some("named".into()),
                    live_directory: Some(directory.path().to_path_buf()),
                    ..TraceStartOptions::default()
                },
            )
            .unwrap();
        let call_id = tracing
            .action_begin(
                context,
                Some("11".into()),
                "page-evaluate",
                None,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        tracing
            .action_end(
                context,
                &call_id,
                Some(serde_json::json!({ "value": 31415926 })),
            )
            .unwrap();
        let capture = tracing.live_capture(context).unwrap().unwrap();
        let trace = trace_jsonl("10", "1.58.0", None, &capture.events, None).unwrap();

        // Act
        persist_live_trace(
            directory.path(),
            capture.name.as_deref().unwrap(),
            &trace,
            #[cfg(feature = "context-tracing-network")]
            None,
        )
        .unwrap();

        // Assert
        let content = std::fs::read_to_string(directory.path().join("named.trace")).unwrap();
        assert!(content.contains("31415926"));
        assert!(content.contains("\"method\":\"evaluateExpression\""));
    }

    #[cfg(feature = "context-tracing-console")]
    #[test]
    fn trace_console_events_preserve_page_message_arguments_and_location() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(8);
        tracing
            .start(context, TraceStartOptions::default())
            .unwrap();
        let message = crate::ConsoleMessage::new(
            "log".into(),
            "hello 42".into(),
            "[\"hello\",42]".into(),
            "https://example.test/console.html".into(),
            7,
            11,
            250_000,
        )
        .unwrap();

        // Act
        tracing
            .record_console(context, ObjectId::new(9), &message)
            .unwrap();
        let capture = tracing.stop(context, true).unwrap().unwrap();
        let trace =
            String::from_utf8(trace_jsonl("8", "1.58.0", None, &capture.events, None).unwrap())
                .unwrap();

        // Assert
        assert!(trace.contains("\"type\":\"console\""));
        assert!(trace.contains("\"pageId\":\"9\""));
        assert!(trace.contains("\"messageType\":\"log\""));
        assert!(trace.contains("\"text\":\"hello 42\""));
        assert!(trace.contains("\"preview\":\"hello\""));
        assert!(trace.contains("\"lineNumber\":7"));
        assert!(trace.contains("\"columnNumber\":11"));
    }

    #[cfg(feature = "context-tracing-chunks")]
    #[test]
    fn trace_chunk_auto_closes_open_groups_in_order() {
        // Arrange
        let tracing = Installation::default();
        let context = ObjectId::new(4);
        tracing
            .start(context, TraceStartOptions::default())
            .unwrap();
        tracing.group(context, "outer".into()).unwrap();
        tracing.group(context, "inner".into()).unwrap();

        // Act
        let capture = tracing.stop_chunk(context, true).unwrap().unwrap();
        let trace =
            String::from_utf8(trace_jsonl("4", "1.58.0", None, &capture.events, None).unwrap())
                .unwrap();

        // Assert
        assert_eq!(trace.matches("\"method\":\"tracingGroup\"").count(), 2);
        assert_eq!(trace.matches("\"type\":\"after\"").count(), 2);
        assert!(trace.contains("\"title\":\"outer\""));
        assert!(trace.contains("\"title\":\"inner\""));
        assert!(trace.contains("\"parentId\":\"group@1\""));
    }
}
