//! Opt-in, browser-independent Playwright v8 `.trace` JSONL writer.
//!
//! The event shapes follow the pinned upstream Playwright `traceV8.ts` and
//! `tracing.ts` at commit `961381ec73a9fa2661d92504eab1a6bd6eaffc75`.
//! Each JSON line is encoded into one bounded scratch buffer before it reaches
//! the injected writer. This crate does not install a recorder, browser,
//! filesystem, archive, network stream, or background task. It supports the
//! context header and API before/log/after events; snapshots, resources,
//! attachments, and the `.network`/ZIP parts remain separate capabilities.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

mod trace_directory;

pub use trace_directory::{
    PreparedTraceDirectory, TraceDirectoryError, TraceDirectoryLimits, prepare_trace_directory,
};

pub const TRACE_VERSION: u8 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceWriterLimits {
    pub max_record_bytes: usize,
    pub max_total_bytes: u64,
    pub max_events: usize,
}

impl TraceWriterLimits {
    pub const fn new(max_record_bytes: usize, max_total_bytes: u64, max_events: usize) -> Self {
        Self {
            max_record_bytes,
            max_total_bytes,
            max_events,
        }
    }

    fn validate(self) -> Result<(), TraceWriterError> {
        if self.max_record_bytes == 0 {
            return Err(TraceWriterError::ZeroLimit("record bytes"));
        }
        if self.max_total_bytes == 0 {
            return Err(TraceWriterError::ZeroLimit("total bytes"));
        }
        if self.max_events == 0 {
            return Err(TraceWriterError::ZeroLimit("events"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TraceWriterMetrics {
    pub events: usize,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum TraceOrigin {
    #[serde(rename = "library")]
    Library,
    #[serde(rename = "testRunner")]
    TestRunner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum SdkLanguage {
    #[serde(rename = "javascript")]
    JavaScript,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "java")]
    Java,
    #[serde(rename = "csharp")]
    CSharp,
    #[serde(rename = "jsonl")]
    Jsonl,
}

#[derive(Clone, Copy, Debug)]
pub struct ContextOptions<'a> {
    pub origin: TraceOrigin,
    pub browser_name: &'a str,
    pub channel: Option<&'a str>,
    pub platform: &'a str,
    pub wall_time_ms: f64,
    pub monotonic_time_ms: f64,
    pub title: Option<&'a str>,
    pub options: &'a Value,
    pub sdk_language: Option<SdkLanguage>,
    pub test_id_attribute_name: Option<&'a str>,
    pub context_id: Option<&'a str>,
    pub playwright_version: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StackFrame<'a> {
    pub file: &'a str,
    pub line: u32,
    pub column: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BeforeAction<'a> {
    #[serde(rename = "callId")]
    pub call_id: &'a str,
    #[serde(rename = "startTime")]
    pub start_time_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'a str>,
    #[serde(rename = "class")]
    pub class_name: &'a str,
    pub method: &'a str,
    pub params: &'a Value,
    #[serde(rename = "stepId", skip_serializing_if = "Option::is_none")]
    pub step_id: Option<&'a str>,
    #[serde(rename = "beforeSnapshot", skip_serializing_if = "Option::is_none")]
    pub before_snapshot: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<&'a [StackFrame<'a>]>,
    #[serde(rename = "pageId", skip_serializing_if = "Option::is_none")]
    pub page_id: Option<&'a str>,
    #[serde(rename = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ErrorDetails<'a> {
    pub message: &'a str,
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct AfterAction<'a> {
    #[serde(rename = "callId")]
    pub call_id: &'a str,
    #[serde(rename = "endTime")]
    pub end_time_ms: f64,
    #[serde(rename = "afterSnapshot", skip_serializing_if = "Option::is_none")]
    pub after_snapshot: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<&'a ErrorDetails<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point: Option<Point>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct LogEvent<'a> {
    #[serde(rename = "callId")]
    pub call_id: &'a str,
    #[serde(rename = "time")]
    pub time_ms: f64,
    pub message: &'a str,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ScreencastFrame<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    #[serde(rename = "pageId")]
    pub page_id: &'a str,
    pub sha1: &'a str,
    pub width: u32,
    pub height: u32,
    pub timestamp: f64,
    #[serde(rename = "frameSwapWallTime", skip_serializing_if = "Option::is_none")]
    pub frame_swap_wall_time: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FrameSnapshot<'a> {
    #[serde(rename = "snapshotName", skip_serializing_if = "Option::is_none")]
    pub snapshot_name: Option<&'a str>,
    #[serde(rename = "callId")]
    pub call_id: &'a str,
    #[serde(rename = "pageId")]
    pub page_id: &'a str,
    #[serde(rename = "frameId")]
    pub frame_id: &'a str,
    #[serde(rename = "frameUrl")]
    pub frame_url: &'a str,
    pub timestamp: f64,
    #[serde(rename = "wallTime", skip_serializing_if = "Option::is_none")]
    pub wall_time: Option<f64>,
    #[serde(rename = "collectionTime")]
    pub collection_time: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctype: Option<&'a str>,
    pub html: &'a Value,
    #[serde(rename = "resourceOverrides")]
    pub resource_overrides: &'a [Value],
    pub viewport: FrameSnapshotViewport,
    #[serde(rename = "isMainFrame")]
    pub is_main_frame: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct FrameSnapshotViewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct FrameSnapshotEvent<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    snapshot: FrameSnapshot<'a>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ConsoleArgument<'a> {
    pub preview: &'a str,
    pub value: &'a Value,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ConsoleLocation<'a> {
    pub url: &'a str,
    #[serde(rename = "lineNumber")]
    pub line_number: u32,
    #[serde(rename = "columnNumber")]
    pub column_number: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ConsoleMessageEvent<'a> {
    #[serde(rename = "type")]
    event_type: &'static str,
    pub time: f64,
    #[serde(rename = "pageId", skip_serializing_if = "Option::is_none")]
    pub page_id: Option<&'a str>,
    #[serde(rename = "messageType")]
    pub message_type: &'a str,
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<&'a [ConsoleArgument<'a>]>,
    pub location: ConsoleLocation<'a>,
}

impl<'a> ConsoleMessageEvent<'a> {
    pub const fn new(
        time: f64,
        page_id: Option<&'a str>,
        message_type: &'a str,
        text: &'a str,
        args: Option<&'a [ConsoleArgument<'a>]>,
        location: ConsoleLocation<'a>,
    ) -> Self {
        Self {
            event_type: "console",
            time,
            page_id,
            message_type,
            text,
            args,
            location,
        }
    }

    fn validate(self) -> Result<(), TraceWriterError> {
        timestamp("console time", self.time)?;
        nonempty("console message type", self.message_type)?;
        Ok(())
    }
}

impl<'a> ScreencastFrame<'a> {
    pub const fn new(
        page_id: &'a str,
        sha1: &'a str,
        width: u32,
        height: u32,
        timestamp: f64,
    ) -> Self {
        Self {
            event_type: "screencast-frame",
            page_id,
            sha1,
            width,
            height,
            timestamp,
            frame_swap_wall_time: None,
        }
    }

    fn validate(self) -> Result<(), TraceWriterError> {
        nonempty("page ID", self.page_id)?;
        nonempty("screencast resource", self.sha1)?;
        if self.width == 0 || self.height == 0 {
            return Err(TraceWriterError::InvalidField(
                "screencast dimensions must be non-zero",
            ));
        }
        timestamp("screencast time", self.timestamp)?;
        if let Some(wall_time) = self.frame_swap_wall_time {
            timestamp("screencast frame-swap wall time", wall_time)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "type")]
pub enum ApiTraceEvent<'a> {
    #[serde(rename = "before")]
    Before(BeforeAction<'a>),
    #[serde(rename = "after")]
    After(AfterAction<'a>),
    #[serde(rename = "log")]
    Log(LogEvent<'a>),
}

impl ApiTraceEvent<'_> {
    fn validate(self) -> Result<(), TraceWriterError> {
        match self {
            Self::Before(event) => {
                nonempty("call ID", event.call_id)?;
                nonempty("class", event.class_name)?;
                nonempty("method", event.method)?;
                timestamp("start time", event.start_time_ms)?;
                if !event.params.is_object() {
                    return Err(TraceWriterError::InvalidField("params must be an object"));
                }
                if let Some(frames) = event.stack {
                    for frame in frames {
                        nonempty("stack file", frame.file)?;
                    }
                }
            }
            Self::After(event) => {
                nonempty("call ID", event.call_id)?;
                timestamp("end time", event.end_time_ms)?;
                if let Some(error) = event.error {
                    nonempty("error name", error.name)?;
                }
                if let Some(point) = event.point {
                    finite("point x", point.x)?;
                    finite("point y", point.y)?;
                }
            }
            Self::Log(event) => {
                nonempty("call ID", event.call_id)?;
                timestamp("log time", event.time_ms)?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct ContextOptionsRecord<'a> {
    version: u8,
    #[serde(rename = "type")]
    event_type: &'static str,
    origin: TraceOrigin,
    #[serde(rename = "browserName")]
    browser_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<&'a str>,
    platform: &'a str,
    #[serde(rename = "wallTime")]
    wall_time_ms: f64,
    #[serde(rename = "monotonicTime")]
    monotonic_time_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    options: &'a Value,
    #[serde(rename = "sdkLanguage", skip_serializing_if = "Option::is_none")]
    sdk_language: Option<SdkLanguage>,
    #[serde(
        rename = "testIdAttributeName",
        skip_serializing_if = "Option::is_none"
    )]
    test_id_attribute_name: Option<&'a str>,
    #[serde(rename = "contextId", skip_serializing_if = "Option::is_none")]
    context_id: Option<&'a str>,
    #[serde(rename = "playwrightVersion", skip_serializing_if = "Option::is_none")]
    playwright_version: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriterState {
    New,
    Ready,
    Poisoned,
}

pub struct PlaywrightV8TraceWriter<W> {
    writer: W,
    limits: TraceWriterLimits,
    metrics: TraceWriterMetrics,
    state: WriterState,
}

impl<W: Write> PlaywrightV8TraceWriter<W> {
    pub fn new(writer: W, limits: TraceWriterLimits) -> Result<Self, TraceWriterError> {
        limits.validate()?;
        Ok(Self {
            writer,
            limits,
            metrics: TraceWriterMetrics::default(),
            state: WriterState::New,
        })
    }

    pub const fn metrics(&self) -> TraceWriterMetrics {
        self.metrics
    }

    pub fn write_context(&mut self, context: &ContextOptions<'_>) -> Result<(), TraceWriterError> {
        match self.state {
            WriterState::New => {}
            WriterState::Ready => return Err(TraceWriterError::ContextAlreadyWritten),
            WriterState::Poisoned => return Err(TraceWriterError::WriterPoisoned),
        }
        nonempty("platform", context.platform)?;
        timestamp("wall time", context.wall_time_ms)?;
        timestamp("monotonic time", context.monotonic_time_ms)?;
        if !context.options.is_object() {
            return Err(TraceWriterError::InvalidField("options must be an object"));
        }
        let record = ContextOptionsRecord {
            version: TRACE_VERSION,
            event_type: "context-options",
            origin: context.origin,
            browser_name: context.browser_name,
            channel: context.channel,
            platform: context.platform,
            wall_time_ms: context.wall_time_ms,
            monotonic_time_ms: context.monotonic_time_ms,
            title: context.title,
            options: context.options,
            sdk_language: context.sdk_language,
            test_id_attribute_name: context.test_id_attribute_name,
            context_id: context.context_id,
            playwright_version: context.playwright_version,
        };
        self.write_record(&record)?;
        self.state = WriterState::Ready;
        Ok(())
    }

    pub fn write_api_event(&mut self, event: ApiTraceEvent<'_>) -> Result<(), TraceWriterError> {
        match self.state {
            WriterState::New => return Err(TraceWriterError::ContextMissing),
            WriterState::Ready => {}
            WriterState::Poisoned => return Err(TraceWriterError::WriterPoisoned),
        }
        event.validate()?;
        self.write_record(&event)
    }

    pub fn write_screencast_frame(
        &mut self,
        event: ScreencastFrame<'_>,
    ) -> Result<(), TraceWriterError> {
        match self.state {
            WriterState::New => return Err(TraceWriterError::ContextMissing),
            WriterState::Ready => {}
            WriterState::Poisoned => return Err(TraceWriterError::WriterPoisoned),
        }
        event.validate()?;
        self.write_record(&event)
    }

    pub fn write_frame_snapshot(
        &mut self,
        snapshot: FrameSnapshot<'_>,
    ) -> Result<(), TraceWriterError> {
        match self.state {
            WriterState::New => return Err(TraceWriterError::ContextMissing),
            WriterState::Ready => {}
            WriterState::Poisoned => return Err(TraceWriterError::WriterPoisoned),
        }
        nonempty("snapshot call ID", snapshot.call_id)?;
        nonempty("snapshot page ID", snapshot.page_id)?;
        nonempty("snapshot frame ID", snapshot.frame_id)?;
        timestamp("snapshot time", snapshot.timestamp)?;
        timestamp("snapshot collection time", snapshot.collection_time)?;
        if let Some(wall_time) = snapshot.wall_time {
            timestamp("snapshot wall time", wall_time)?;
        }
        if snapshot.viewport.width == 0 || snapshot.viewport.height == 0 {
            return Err(TraceWriterError::InvalidField(
                "snapshot viewport dimensions must be non-zero",
            ));
        }
        self.write_record(&FrameSnapshotEvent {
            event_type: "frame-snapshot",
            snapshot,
        })
    }

    pub fn write_console_event(
        &mut self,
        event: ConsoleMessageEvent<'_>,
    ) -> Result<(), TraceWriterError> {
        match self.state {
            WriterState::New => return Err(TraceWriterError::ContextMissing),
            WriterState::Ready => {}
            WriterState::Poisoned => return Err(TraceWriterError::WriterPoisoned),
        }
        event.validate()?;
        self.write_record(&event)
    }

    /// Flushes and returns the injected writer only after a complete header.
    pub fn finish(mut self) -> Result<W, TraceWriterError> {
        match self.state {
            WriterState::New => return Err(TraceWriterError::ContextMissing),
            WriterState::Poisoned => return Err(TraceWriterError::WriterPoisoned),
            WriterState::Ready => {}
        }
        self.writer.flush().map_err(TraceWriterError::Write)?;
        Ok(self.writer)
    }

    fn write_record(&mut self, event: &impl Serialize) -> Result<(), TraceWriterError> {
        if self.metrics.events == self.limits.max_events {
            return Err(TraceWriterError::EventCapacityExceeded {
                limit: self.limits.max_events,
            });
        }
        let mut line = BoundedLine::new(self.limits.max_record_bytes);
        if let Err(error) = serde_json::to_writer(&mut line, event) {
            if let Some(attempted) = line.overflow {
                return Err(TraceWriterError::RecordBytesExceeded {
                    attempted,
                    limit: self.limits.max_record_bytes,
                });
            }
            if line.allocation_failed {
                return Err(TraceWriterError::AllocationFailed);
            }
            return Err(TraceWriterError::Encoding(error));
        }
        line.write_all(b"\n").map_err(|error| {
            if let Some(attempted) = line.overflow {
                TraceWriterError::RecordBytesExceeded {
                    attempted,
                    limit: self.limits.max_record_bytes,
                }
            } else if line.allocation_failed {
                TraceWriterError::AllocationFailed
            } else {
                TraceWriterError::Write(error)
            }
        })?;
        let line_bytes =
            u64::try_from(line.bytes.len()).map_err(|_| TraceWriterError::TotalBytesOverflow)?;
        let next_total = self
            .metrics
            .bytes
            .checked_add(line_bytes)
            .ok_or(TraceWriterError::TotalBytesOverflow)?;
        if next_total > self.limits.max_total_bytes {
            return Err(TraceWriterError::TotalBytesExceeded {
                attempted: next_total,
                limit: self.limits.max_total_bytes,
            });
        }
        if let Err(error) = self.writer.write_all(&line.bytes) {
            self.state = WriterState::Poisoned;
            return Err(TraceWriterError::Write(error));
        }
        self.metrics.events += 1;
        self.metrics.bytes = next_total;
        Ok(())
    }
}

fn nonempty(field: &'static str, value: &str) -> Result<(), TraceWriterError> {
    if value.is_empty() {
        return Err(TraceWriterError::InvalidField(field));
    }
    Ok(())
}

fn timestamp(field: &'static str, value: f64) -> Result<(), TraceWriterError> {
    finite(field, value)?;
    if value < 0.0 {
        return Err(TraceWriterError::InvalidTimestamp(field));
    }
    Ok(())
}

fn finite(field: &'static str, value: f64) -> Result<(), TraceWriterError> {
    if !value.is_finite() {
        return Err(TraceWriterError::InvalidTimestamp(field));
    }
    Ok(())
}

struct BoundedLine {
    bytes: Vec<u8>,
    limit: usize,
    overflow: Option<usize>,
    allocation_failed: bool,
}

impl BoundedLine {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            overflow: None,
            allocation_failed: false,
        }
    }
}

impl Write for BoundedLine {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let attempted = self.bytes.len().saturating_add(bytes.len());
        if attempted > self.limit {
            self.overflow = Some(attempted);
            return Err(io::Error::other("trace record byte limit exceeded"));
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.allocation_failed = true;
            return Err(io::Error::other("trace record allocation failed"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum TraceWriterError {
    ZeroLimit(&'static str),
    InvalidField(&'static str),
    InvalidTimestamp(&'static str),
    ContextMissing,
    ContextAlreadyWritten,
    WriterPoisoned,
    EventCapacityExceeded { limit: usize },
    RecordBytesExceeded { attempted: usize, limit: usize },
    TotalBytesExceeded { attempted: u64, limit: u64 },
    TotalBytesOverflow,
    AllocationFailed,
    Encoding(serde_json::Error),
    Write(io::Error),
}

impl Display for TraceWriterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLimit(field) => write!(formatter, "trace {field} limit must be non-zero"),
            Self::InvalidField(field) => write!(formatter, "invalid trace {field}"),
            Self::InvalidTimestamp(field) => write!(formatter, "invalid trace {field} timestamp"),
            Self::ContextMissing => formatter.write_str("trace context-options event is missing"),
            Self::ContextAlreadyWritten => {
                formatter.write_str("trace context-options event was already written")
            }
            Self::WriterPoisoned => formatter.write_str("trace writer failed after partial I/O"),
            Self::EventCapacityExceeded { limit } => {
                write!(formatter, "trace reached its {limit}-event limit")
            }
            Self::RecordBytesExceeded { attempted, limit } => write!(
                formatter,
                "trace record needs at least {attempted} bytes, exceeding limit {limit}"
            ),
            Self::TotalBytesExceeded { attempted, limit } => write!(
                formatter,
                "trace would use {attempted} bytes, exceeding total limit {limit}"
            ),
            Self::TotalBytesOverflow => formatter.write_str("trace total byte count overflowed"),
            Self::AllocationFailed => formatter.write_str("trace record allocation failed"),
            Self::Encoding(error) => Display::fmt(error, formatter),
            Self::Write(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for TraceWriterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encoding(error) => Some(error),
            Self::Write(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use serde_json::{Value, json};

    use super::{
        AfterAction, ApiTraceEvent, BeforeAction, ConsoleArgument, ConsoleLocation,
        ConsoleMessageEvent, ContextOptions, LogEvent, PlaywrightV8TraceWriter, Point,
        ScreencastFrame, SdkLanguage, TRACE_VERSION, TraceOrigin, TraceWriterError,
        TraceWriterLimits,
    };

    fn context(options: &Value) -> ContextOptions<'_> {
        ContextOptions {
            origin: TraceOrigin::Library,
            browser_name: "chromium",
            channel: Some("chrome"),
            platform: "darwin",
            wall_time_ms: 1000.5,
            monotonic_time_ms: 200.25,
            title: Some("case"),
            options,
            sdk_language: Some(SdkLanguage::Python),
            test_id_attribute_name: Some("data-testid"),
            context_id: Some("context@1"),
            playwright_version: Some("1.58.0"),
        }
    }

    fn before<'a>(params: &'a Value) -> ApiTraceEvent<'a> {
        ApiTraceEvent::Before(BeforeAction {
            call_id: "call@1",
            start_time_ms: 201.25,
            title: None,
            class_name: "Page",
            method: "goto",
            params,
            step_id: None,
            before_snapshot: None,
            stack: None,
            page_id: Some("page@1"),
            parent_id: None,
        })
    }

    #[test]
    fn writes_pinned_playwright_v8_header_and_correlated_api_jsonl() {
        // Arrange
        let options = json!({"viewport": {"width": 800, "height": 600}});
        let params = json!({"url": "https://example.test/\"quote\""});
        let result = json!({"response": null});
        let limits = TraceWriterLimits::new(4096, 8192, 4);
        let mut writer = PlaywrightV8TraceWriter::new(Vec::new(), limits).unwrap();

        // Act
        writer.write_context(&context(&options)).unwrap();
        writer.write_api_event(before(&params)).unwrap();
        writer
            .write_api_event(ApiTraceEvent::Log(LogEvent {
                call_id: "call@1",
                time_ms: 202.5,
                message: "navigating\nnow",
            }))
            .unwrap();
        writer
            .write_api_event(ApiTraceEvent::After(AfterAction {
                call_id: "call@1",
                end_time_ms: 203.75,
                after_snapshot: None,
                error: None,
                result: Some(&result),
                point: Some(Point { x: -5.0, y: 3.0 }),
            }))
            .unwrap();
        let metrics = writer.metrics();
        let bytes = writer.finish().unwrap();

        // Assert
        assert_eq!(metrics.events, 4);
        assert_eq!(metrics.bytes, u64::try_from(bytes.len()).unwrap());
        assert!(bytes.ends_with(b"\n"));
        let lines: Vec<Value> = bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["version"], TRACE_VERSION);
        assert_eq!(lines[0]["type"], "context-options");
        assert_eq!(lines[0]["origin"], "library");
        assert_eq!(lines[0]["browserName"], "chromium");
        assert_eq!(lines[0]["sdkLanguage"], "python");
        assert_eq!(lines[0]["options"], options);
        assert_eq!(lines[1]["type"], "before");
        assert_eq!(lines[1]["class"], "Page");
        assert_eq!(lines[1]["method"], "goto");
        assert_eq!(lines[1]["params"], params);
        assert_eq!(lines[2]["type"], "log");
        assert_eq!(lines[2]["message"], "navigating\nnow");
        assert_eq!(lines[3]["type"], "after");
        assert_eq!(lines[3]["callId"], lines[1]["callId"]);
        assert_eq!(lines[3]["result"], result);
        assert_eq!(lines[3]["point"], json!({"x": -5.0, "y": 3.0}));
    }

    #[test]
    fn writes_a_pinned_screencast_frame_reference_after_the_context_header() {
        // Arrange
        let options = json!({});
        let mut writer =
            PlaywrightV8TraceWriter::new(Vec::new(), TraceWriterLimits::new(1024, 2048, 2))
                .unwrap();
        writer.write_context(&context(&options)).unwrap();

        // Act
        writer
            .write_screencast_frame(ScreencastFrame::new(
                "page@1",
                "frame.jpeg",
                500,
                400,
                201.5,
            ))
            .unwrap();
        let bytes = writer.finish().unwrap();

        // Assert
        let event: Value = serde_json::from_slice(
            bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .nth(1)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(event["type"], "screencast-frame");
        assert_eq!(event["pageId"], "page@1");
        assert_eq!(event["sha1"], "frame.jpeg");
        assert_eq!(event["width"], 500);
        assert_eq!(event["height"], 400);
    }

    #[test]
    fn writes_a_pinned_console_event_after_the_context_header() {
        // Arrange
        let options = json!({});
        let value = json!("hello");
        let args = [ConsoleArgument {
            preview: "hello",
            value: &value,
        }];
        let mut writer =
            PlaywrightV8TraceWriter::new(Vec::new(), TraceWriterLimits::new(2048, 4096, 2))
                .unwrap();
        writer.write_context(&context(&options)).unwrap();

        // Act
        writer
            .write_console_event(ConsoleMessageEvent::new(
                201.5,
                Some("page@1"),
                "log",
                "hello",
                Some(&args),
                ConsoleLocation {
                    url: "https://example.test/console.html",
                    line_number: 7,
                    column_number: 11,
                },
            ))
            .unwrap();
        let bytes = writer.finish().unwrap();

        // Assert
        let event: Value = serde_json::from_slice(
            bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .nth(1)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(event["type"], "console");
        assert_eq!(event["pageId"], "page@1");
        assert_eq!(event["messageType"], "log");
        assert_eq!(
            event["args"][0],
            json!({"preview": "hello", "value": "hello"})
        );
        assert_eq!(event["location"]["lineNumber"], 7);
        assert_eq!(event["location"]["columnNumber"], 11);
    }

    #[test]
    fn context_header_is_required_and_written_once() {
        // Arrange
        let options = json!({});
        let params = json!({});
        let mut writer =
            PlaywrightV8TraceWriter::new(Vec::new(), TraceWriterLimits::new(512, 1024, 2)).unwrap();

        // Act
        let before_error = writer.write_api_event(before(&params)).unwrap_err();
        writer.write_context(&context(&options)).unwrap();
        let duplicate_error = writer.write_context(&context(&options)).unwrap_err();
        let metrics = writer.metrics();
        let bytes = writer.finish().unwrap();

        // Assert
        assert!(matches!(before_error, TraceWriterError::ContextMissing));
        assert!(matches!(
            duplicate_error,
            TraceWriterError::ContextAlreadyWritten
        ));
        assert_eq!(metrics.events, 1);
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    }

    #[test]
    fn oversized_record_is_rejected_before_any_sink_write() {
        // Arrange
        let options = json!({"long": "x".repeat(2048)});
        let mut sink = Vec::new();
        let mut writer =
            PlaywrightV8TraceWriter::new(&mut sink, TraceWriterLimits::new(128, 8192, 10)).unwrap();

        // Act
        let error = writer.write_context(&context(&options)).unwrap_err();
        let metrics = writer.metrics();

        // Assert
        assert!(matches!(
            error,
            TraceWriterError::RecordBytesExceeded { .. }
        ));
        assert_eq!(metrics.events, 0);
        assert_eq!(metrics.bytes, 0);
        assert!(sink.is_empty());
    }

    #[test]
    fn total_byte_limit_and_event_limit_reject_without_partial_line() {
        // Arrange
        let options = json!({});
        let params = json!({});
        let header_bytes = {
            let mut header_writer =
                PlaywrightV8TraceWriter::new(Vec::new(), TraceWriterLimits::new(1024, 8192, 2))
                    .unwrap();
            header_writer.write_context(&context(&options)).unwrap();
            header_writer.finish().unwrap().len()
        };
        let mut total_writer = PlaywrightV8TraceWriter::new(
            Vec::new(),
            TraceWriterLimits::new(1024, u64::try_from(header_bytes).unwrap(), 2),
        )
        .unwrap();
        let mut count_writer =
            PlaywrightV8TraceWriter::new(Vec::new(), TraceWriterLimits::new(1024, 8192, 1))
                .unwrap();
        total_writer.write_context(&context(&options)).unwrap();
        count_writer.write_context(&context(&options)).unwrap();

        // Act
        let total_error = total_writer.write_api_event(before(&params)).unwrap_err();
        let count_error = count_writer.write_api_event(before(&params)).unwrap_err();
        let total_bytes = total_writer.finish().unwrap();
        let count_bytes = count_writer.finish().unwrap();

        // Assert
        assert!(matches!(
            total_error,
            TraceWriterError::TotalBytesExceeded { .. }
        ));
        assert!(matches!(
            count_error,
            TraceWriterError::EventCapacityExceeded { .. }
        ));
        assert_eq!(total_bytes.len(), header_bytes);
        assert_eq!(count_bytes.len(), header_bytes);
    }

    #[test]
    fn invalid_timestamps_and_non_object_params_fail_before_writing() {
        // Arrange
        let options = json!({});
        let params = json!(["not", "an", "object"]);
        let mut writer =
            PlaywrightV8TraceWriter::new(Vec::new(), TraceWriterLimits::new(1024, 8192, 3))
                .unwrap();
        let mut invalid_context = context(&options);
        invalid_context.wall_time_ms = f64::NAN;

        // Act
        let context_error = writer.write_context(&invalid_context).unwrap_err();
        writer.write_context(&context(&options)).unwrap();
        let params_error = writer.write_api_event(before(&params)).unwrap_err();
        let time_error = writer
            .write_api_event(ApiTraceEvent::Log(LogEvent {
                call_id: "call@1",
                time_ms: f64::INFINITY,
                message: "bad time",
            }))
            .unwrap_err();
        let metrics = writer.metrics();
        let bytes = writer.finish().unwrap();

        // Assert
        assert!(matches!(
            context_error,
            TraceWriterError::InvalidTimestamp("wall time")
        ));
        assert!(matches!(
            params_error,
            TraceWriterError::InvalidField("params must be an object")
        ));
        assert!(matches!(
            time_error,
            TraceWriterError::InvalidTimestamp("log time")
        ));
        assert_eq!(metrics.events, 1);
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    }

    struct PartialFailureWriter {
        bytes: Vec<u8>,
    }

    impl Write for PartialFailureWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.bytes.is_empty() {
                let count = bytes.len().min(3);
                self.bytes.extend_from_slice(&bytes[..count]);
                return Ok(count);
            }
            Err(io::Error::other("injected disk failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn partial_sink_failure_poisoned_writer_cannot_append() {
        // Arrange
        let options = json!({});
        let mut sink = PartialFailureWriter { bytes: Vec::new() };
        let mut writer =
            PlaywrightV8TraceWriter::new(&mut sink, TraceWriterLimits::new(1024, 8192, 3)).unwrap();

        // Act
        let write_error = writer.write_context(&context(&options)).unwrap_err();
        let retry_error = writer.write_context(&context(&options)).unwrap_err();
        let metrics = writer.metrics();

        // Assert
        assert!(matches!(write_error, TraceWriterError::Write(_)));
        assert!(matches!(retry_error, TraceWriterError::WriterPoisoned));
        assert_eq!(metrics.events, 0);
        assert_eq!(metrics.bytes, 0);
        assert_eq!(sink.bytes, b"{\"v");
    }

    #[test]
    fn all_limits_must_be_explicit_and_nonzero() {
        // Arrange
        let bad_limits = [
            TraceWriterLimits::new(0, 1, 1),
            TraceWriterLimits::new(1, 0, 1),
            TraceWriterLimits::new(1, 1, 0),
        ];

        // Act
        let errors: Vec<_> = bad_limits
            .into_iter()
            .map(
                |limits| match PlaywrightV8TraceWriter::new(Vec::new(), limits) {
                    Ok(_) => panic!("zero limit must fail"),
                    Err(error) => error,
                },
            )
            .collect();

        // Assert
        assert!(
            errors
                .iter()
                .all(|error| matches!(error, TraceWriterError::ZeroLimit(_)))
        );
    }
}
