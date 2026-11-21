//! Opt-in Playwright v8 network tracing for browser-independent API requests.
//!
//! The recorder spools JSONL and content blobs to a temporary directory while
//! tracing is active, then packages them into the Playwright-compatible ZIP at
//! stop time. Language bindings only select this capability and delegate.

use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Instant, SystemTime},
};

use serde::Serialize;
use sha1::{Digest, Sha1};
use tempfile::{NamedTempFile, TempDir};
use time::{OffsetDateTime, format_description::well_known::Iso8601};
use url::Url;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TraceHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceRequest {
    pub method: String,
    pub headers: Vec<TraceHeader>,
    pub body: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceResponse {
    pub url: String,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<TraceHeader>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraceTimestamp {
    pub started_date_time: String,
    pub monotonic_time_ms: f64,
}

pub trait TraceClock: Send + Sync + 'static {
    fn now(&self) -> Result<TraceTimestamp, ApiRequestTraceError>;
}

#[derive(Debug)]
pub struct SystemTraceClock {
    monotonic_origin: Instant,
}

impl SystemTraceClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            monotonic_origin: Instant::now(),
        }
    }
}

impl Default for SystemTraceClock {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceClock for SystemTraceClock {
    fn now(&self) -> Result<TraceTimestamp, ApiRequestTraceError> {
        let wall_time = OffsetDateTime::from(SystemTime::now())
            .format(&Iso8601::DEFAULT)
            .map_err(ApiRequestTraceError::Timestamp)?;
        Ok(TraceTimestamp {
            started_date_time: wall_time,
            monotonic_time_ms: self.monotonic_origin.elapsed().as_secs_f64() * 1_000.0,
        })
    }
}

#[derive(Clone)]
pub struct ApiRequestTraceRecorder {
    inner: Arc<RecorderInner>,
}

struct RecorderInner {
    clock: Arc<dyn TraceClock>,
    state: Mutex<RecorderState>,
}

enum RecorderState {
    Idle,
    Recording(Recording),
}

struct Recording {
    _spool: TempDir,
    network_path: PathBuf,
    resources_path: PathBuf,
    resource_names: HashSet<String>,
    capture_network: bool,
    failure: Option<String>,
}

impl ApiRequestTraceRecorder {
    #[must_use]
    pub fn system() -> Self {
        Self::with_clock(Arc::new(SystemTraceClock::new()))
    }

    #[must_use]
    pub fn with_clock(clock: Arc<dyn TraceClock>) -> Self {
        Self {
            inner: Arc::new(RecorderInner {
                clock,
                state: Mutex::new(RecorderState::Idle),
            }),
        }
    }

    pub fn start(&self, capture_network: bool) -> Result<(), ApiRequestTraceError> {
        let mut state = lock(&self.inner.state);
        if matches!(*state, RecorderState::Recording(_)) {
            return Err(ApiRequestTraceError::AlreadyRecording);
        }
        let spool = tempfile::tempdir().map_err(ApiRequestTraceError::Io)?;
        let network_path = spool.path().join("trace.network");
        File::create(&network_path).map_err(ApiRequestTraceError::Io)?;
        let resources_path = spool.path().join("resources");
        fs::create_dir(&resources_path).map_err(ApiRequestTraceError::Io)?;
        *state = RecorderState::Recording(Recording {
            _spool: spool,
            network_path,
            resources_path,
            resource_names: HashSet::new(),
            capture_network,
            failure: None,
        });
        Ok(())
    }

    #[must_use]
    pub fn is_recording(&self) -> bool {
        matches!(*lock(&self.inner.state), RecorderState::Recording(_))
    }

    pub fn record(
        &self,
        request: TraceRequest,
        response: TraceResponse,
    ) -> Result<(), ApiRequestTraceError> {
        let timestamp = self.inner.clock.now()?;
        let mut state = lock(&self.inner.state);
        let RecorderState::Recording(recording) = &mut *state else {
            return Ok(());
        };
        if !recording.capture_network {
            return Ok(());
        }
        if let Some(message) = &recording.failure {
            return Err(ApiRequestTraceError::RecorderFailed(message.clone()));
        }
        if let Err(error) = recording.append(timestamp, request, response) {
            recording.failure = Some(error.to_string());
            return Err(error);
        }
        Ok(())
    }

    pub fn stop(&self, output: impl AsRef<Path>) -> Result<(), ApiRequestTraceError> {
        let recording = {
            let mut state = lock(&self.inner.state);
            let RecorderState::Recording(_) = &*state else {
                return Err(ApiRequestTraceError::NotRecording);
            };
            match std::mem::replace(&mut *state, RecorderState::Idle) {
                RecorderState::Recording(recording) => recording,
                RecorderState::Idle => unreachable!("recording state checked above"),
            }
        };
        if let Some(message) = recording.failure {
            return Err(ApiRequestTraceError::RecorderFailed(message));
        }
        recording.package(output.as_ref())
    }

    pub fn discard(&self) {
        *lock(&self.inner.state) = RecorderState::Idle;
    }
}

impl Recording {
    fn append(
        &mut self,
        timestamp: TraceTimestamp,
        request: TraceRequest,
        response: TraceResponse,
    ) -> Result<(), ApiRequestTraceError> {
        let parsed_url = Url::parse(&response.url).map_err(ApiRequestTraceError::Url)?;
        let request_mime =
            header_value(&request.headers, "content-type").unwrap_or("application/octet-stream");
        let response_mime = header_value(&response.headers, "content-type").unwrap_or("x-unknown");
        let request_post_data = request
            .body
            .as_deref()
            .map(|body| self.attach_post_data(body, request_mime))
            .transpose()?;
        let response_content = self.attach_response_content(&response.body, response_mime)?;
        let event = ResourceSnapshotEvent {
            event_type: "resource-snapshot",
            snapshot: ResourceSnapshot {
                started_date_time: timestamp.started_date_time,
                time: -1,
                request: HarRequest {
                    method: request.method,
                    url: response.url,
                    http_version: "HTTP/1.1",
                    cookies: Vec::new(),
                    headers: request.headers,
                    query_string: parsed_url
                        .query_pairs()
                        .map(|(name, value)| NameValue {
                            name: name.into_owned(),
                            value: value.into_owned(),
                        })
                        .collect(),
                    post_data: request_post_data,
                    headers_size: -1,
                    body_size: request.body.as_ref().map_or(0, Vec::len),
                },
                response: HarResponse {
                    status: response.status,
                    status_text: response.status_text,
                    http_version: "HTTP/1.1",
                    cookies: Vec::new(),
                    headers: response.headers,
                    content: response_content,
                    headers_size: -1,
                    body_size: response.body.len(),
                    redirect_url: String::new(),
                    transfer_size: -1,
                },
                cache: EmptyObject {},
                timings: Timings {
                    send: -1,
                    wait: -1,
                    receive: -1,
                },
                api_request: true,
                monotonic_time: timestamp.monotonic_time_ms,
            },
        };
        let file = OpenOptions::new()
            .append(true)
            .open(&self.network_path)
            .map_err(ApiRequestTraceError::Io)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &event).map_err(ApiRequestTraceError::Json)?;
        writer.write_all(b"\n").map_err(ApiRequestTraceError::Io)?;
        writer.flush().map_err(ApiRequestTraceError::Io)
    }

    fn attach_post_data(
        &mut self,
        body: &[u8],
        mime_type: &str,
    ) -> Result<HarPostData, ApiRequestTraceError> {
        let resource = self.attach_resource(body, mime_type)?;
        Ok(HarPostData {
            mime_type: mime_type.to_owned(),
            text: String::new(),
            params: Vec::new(),
            sha1: resource,
        })
    }

    fn attach_response_content(
        &mut self,
        body: &[u8],
        mime_type: &str,
    ) -> Result<HarContent, ApiRequestTraceError> {
        let sha1 = (!body.is_empty())
            .then(|| self.attach_resource(body, mime_type))
            .transpose()?;
        Ok(HarContent {
            size: body.len(),
            mime_type: mime_type.to_owned(),
            sha1,
        })
    }

    fn attach_resource(
        &mut self,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, ApiRequestTraceError> {
        let digest = hex_digest(bytes);
        let extension = mime_extension(mime_type);
        let name = format!("{digest}.{extension}");
        if self.resource_names.insert(name.clone()) {
            fs::write(self.resources_path.join(&name), bytes).map_err(ApiRequestTraceError::Io)?;
        }
        Ok(name)
    }

    fn package(self, output: &Path) -> Result<(), ApiRequestTraceError> {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(ApiRequestTraceError::Io)?;
        let temporary = NamedTempFile::new_in(parent).map_err(ApiRequestTraceError::Io)?;
        let mut archive = ZipWriter::new(temporary.reopen().map_err(ApiRequestTraceError::Io)?);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        archive
            .start_file("trace.network", options)
            .map_err(ApiRequestTraceError::Zip)?;
        io::copy(
            &mut File::open(&self.network_path).map_err(ApiRequestTraceError::Io)?,
            &mut archive,
        )
        .map_err(ApiRequestTraceError::Io)?;
        let mut names = self.resource_names.into_iter().collect::<Vec<_>>();
        names.sort_unstable();
        for name in names {
            archive
                .start_file(format!("resources/{name}"), options)
                .map_err(ApiRequestTraceError::Zip)?;
            io::copy(
                &mut File::open(self.resources_path.join(&name))
                    .map_err(ApiRequestTraceError::Io)?,
                &mut archive,
            )
            .map_err(ApiRequestTraceError::Io)?;
        }
        archive.finish().map_err(ApiRequestTraceError::Zip)?;
        temporary
            .persist(output)
            .map_err(|error| ApiRequestTraceError::Io(error.error))?;
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn header_value<'a>(headers: &'a [TraceHeader], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

fn mime_extension(mime_type: &str) -> &'static str {
    match mime_type.split(';').next().unwrap_or_default().trim() {
        "application/json" | "text/json" => "json",
        "text/html" => "html",
        "text/css" => "css",
        "text/javascript" | "application/javascript" => "js",
        "text/plain" => "txt",
        _ => "dat",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSnapshotEvent {
    #[serde(rename = "type")]
    event_type: &'static str,
    snapshot: ResourceSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSnapshot {
    started_date_time: String,
    time: i64,
    request: HarRequest,
    response: HarResponse,
    cache: EmptyObject,
    timings: Timings,
    #[serde(rename = "_apiRequest")]
    api_request: bool,
    #[serde(rename = "_monotonicTime")]
    monotonic_time: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarRequest {
    method: String,
    url: String,
    http_version: &'static str,
    cookies: Vec<EmptyObject>,
    headers: Vec<TraceHeader>,
    query_string: Vec<NameValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post_data: Option<HarPostData>,
    headers_size: i64,
    body_size: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarResponse {
    status: u16,
    status_text: String,
    http_version: &'static str,
    cookies: Vec<EmptyObject>,
    headers: Vec<TraceHeader>,
    content: HarContent,
    headers_size: i64,
    body_size: usize,
    redirect_url: String,
    #[serde(rename = "_transferSize")]
    transfer_size: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarPostData {
    mime_type: String,
    text: String,
    params: Vec<NameValue>,
    #[serde(rename = "_sha1")]
    sha1: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HarContent {
    size: usize,
    mime_type: String,
    #[serde(rename = "_sha1", skip_serializing_if = "Option::is_none")]
    sha1: Option<String>,
}

#[derive(Serialize)]
struct NameValue {
    name: String,
    value: String,
}

#[derive(Serialize)]
struct Timings {
    send: i64,
    wait: i64,
    receive: i64,
}

#[derive(Serialize)]
struct EmptyObject {}

#[derive(Debug, thiserror::Error)]
pub enum ApiRequestTraceError {
    #[error("API request tracing is already started")]
    AlreadyRecording,
    #[error("API request tracing is not started")]
    NotRecording,
    #[error("API request tracing failed: {0}")]
    RecorderFailed(String),
    #[error("invalid traced request URL: {0}")]
    Url(url::ParseError),
    #[error("failed to format trace timestamp: {0}")]
    Timestamp(time::error::Format),
    #[error("API request trace JSON error: {0}")]
    Json(serde_json::Error),
    #[error("API request trace I/O error: {0}")]
    Io(io::Error),
    #[error("API request trace ZIP error: {0}")]
    Zip(zip::result::ZipError),
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read, sync::Arc};

    use tempfile::tempdir;
    use zip::ZipArchive;

    use super::{
        ApiRequestTraceRecorder, TraceClock, TraceHeader, TraceRequest, TraceResponse,
        TraceTimestamp,
    };

    #[derive(Debug)]
    struct FixedClock;

    impl TraceClock for FixedClock {
        fn now(&self) -> Result<TraceTimestamp, super::ApiRequestTraceError> {
            Ok(TraceTimestamp {
                started_date_time: "2026-11-19T15:00:00.000Z".to_owned(),
                monotonic_time_ms: 42.0,
            })
        }
    }

    #[test]
    fn packages_isolated_playwright_network_events_and_post_data() {
        // Arrange
        let recorder = ApiRequestTraceRecorder::with_clock(Arc::new(FixedClock));
        let output_dir = tempdir().expect("output directory");
        let output = output_dir.path().join("trace.zip");
        recorder.start(true).expect("start tracing");
        let request = TraceRequest {
            method: "POST".to_owned(),
            headers: vec![TraceHeader {
                name: "content-type".to_owned(),
                value: "text/plain".to_owned(),
            }],
            body: Some(b"test".to_vec()),
        };
        let response = TraceResponse {
            url: "https://example.test/simple.json?one=two".to_owned(),
            status: 200,
            status_text: "OK".to_owned(),
            headers: vec![TraceHeader {
                name: "Content-Length".to_owned(),
                value: "15".to_owned(),
            }],
            body: br#"{"ok":true}"#.to_vec(),
        };

        // Act
        recorder.record(request, response).expect("record request");
        recorder.stop(&output).expect("stop tracing");

        // Assert
        let mut archive = ZipArchive::new(File::open(output).expect("trace archive"))
            .expect("read trace archive");
        let mut network = String::new();
        archive
            .by_name("trace.network")
            .expect("network entry")
            .read_to_string(&mut network)
            .expect("network JSONL");
        let event: serde_json::Value = serde_json::from_str(network.trim()).expect("event JSON");
        assert_eq!(event["type"], "resource-snapshot");
        assert_eq!(event["snapshot"]["request"]["method"], "POST");
        assert_eq!(
            event["snapshot"]["request"]["url"],
            "https://example.test/simple.json?one=two"
        );
        assert!(event["snapshot"]["request"]["postData"]["_sha1"].is_string());
        assert_eq!(event["snapshot"]["response"]["status"], 200);
        assert!(
            archive.len() >= 3,
            "request and response resources are attached"
        );
    }

    #[test]
    fn keeps_each_recorder_state_independent() {
        // Arrange
        let first = ApiRequestTraceRecorder::with_clock(Arc::new(FixedClock));
        let second = ApiRequestTraceRecorder::with_clock(Arc::new(FixedClock));

        // Act
        first.start(true).expect("start first recorder");

        // Assert
        assert!(first.is_recording());
        assert!(!second.is_recording());
    }

    #[test]
    fn omits_network_events_when_snapshots_are_disabled() {
        // Arrange
        let recorder = ApiRequestTraceRecorder::with_clock(Arc::new(FixedClock));
        let output_dir = tempdir().expect("output directory");
        let output = output_dir.path().join("trace.zip");
        recorder.start(false).expect("start tracing");
        let request = TraceRequest {
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: None,
        };
        let response = TraceResponse {
            url: "https://example.test/".to_owned(),
            status: 200,
            status_text: "OK".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
        };

        // Act
        recorder.record(request, response).expect("ignore request");
        recorder.stop(&output).expect("stop tracing");

        // Assert
        let mut archive = ZipArchive::new(File::open(output).expect("trace archive"))
            .expect("read trace archive");
        let mut network = String::new();
        archive
            .by_name("trace.network")
            .expect("network entry")
            .read_to_string(&mut network)
            .expect("network JSONL");
        assert!(network.is_empty());
        assert_eq!(archive.len(), 1);
    }
}
