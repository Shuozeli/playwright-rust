//! Host adapter for backend-neutral HAR recording.
//!
//! This layer selects Page/Context scope and translates normalized network
//! records. Browser commands, response-body retrieval and filesystem writes
//! remain owned by the host session.

use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use playwright_rust_backend_cdp_topology::CdpTopology;
use playwright_rust_cdp::{CdpEvent, domains::network::ResponseBody, generated::page};
use playwright_rust_core::ObjectId;
#[cfg(feature = "har-recording-regexp")]
use playwright_rust_ecmascript_regexp::EcmaScriptRegex;
use playwright_rust_har_recording::{
    HarContentPolicy, HarExport, HarHeader, HarMode, HarPageSnapshot, HarRecorder,
    HarRecorderOptions, HarRequestSnapshot, HarResponseSnapshot, HarSecurityDetails, HarSizes,
    HarTiming, playwright_page_id,
};
use playwright_rust_plugin_chromium_frame_observer::{
    ChromiumFrameEventObserver, FrameObservationResult,
};
use tokio::sync::Notify;

#[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
use crate::ContextApiRequestTrace;
use crate::{
    HarUpdateContent, HarUpdateMode, HostError, HostSession,
    network_events::{
        NetworkEventRecord, NetworkRequestRecord, NetworkResponseRecord, RequestSizes,
        completed_request_sizes,
    },
    page_routes::{RouteContinue, route_matches},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegistrationOptions {
    pub path: PathBuf,
    pub filter: HarUrlFilter,
    pub content: HarContentPolicy,
    pub mode: HarMode,
    pub browser_name: String,
    pub browser_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HarUrlFilter {
    Glob(String),
    RegExp { source: String, flags: String },
}

enum CompiledHarUrlFilter {
    Glob(String),
    #[cfg(feature = "har-recording-regexp")]
    RegExp(Arc<EcmaScriptRegex>),
}

impl CompiledHarUrlFilter {
    fn compile(filter: HarUrlFilter) -> Result<Self, HostError> {
        match filter {
            HarUrlFilter::Glob(pattern) => {
                route_matches(&pattern, "https://playwright.invalid/")?;
                Ok(Self::Glob(pattern))
            }
            HarUrlFilter::RegExp { source, flags } => {
                #[cfg(feature = "har-recording-regexp")]
                {
                    EcmaScriptRegex::compile(source, flags)
                        .map(Arc::new)
                        .map(Self::RegExp)
                        .map_err(|error| {
                            HostError::InvalidArgument(format!("invalid HAR RegExp: {error}"))
                        })
                }
                #[cfg(not(feature = "har-recording-regexp"))]
                {
                    let _ = (source, flags);
                    Err(HostError::UnsupportedOperation(
                        "HAR RegExp filtering requires the har-recording-regexp profile".into(),
                    ))
                }
            }
        }
    }

    fn matches(&self, url: &str) -> Result<bool, HostError> {
        match self {
            Self::Glob(pattern) => route_matches(pattern, url),
            #[cfg(feature = "har-recording-regexp")]
            Self::RegExp(pattern) => Ok(pattern.test(url)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    Page { page: ObjectId, context: ObjectId },
    Context(ObjectId),
}

impl Scope {
    fn context(self) -> ObjectId {
        match self {
            Self::Page { context, .. } | Self::Context(context) => context,
        }
    }

    fn matches(self, page: Option<ObjectId>, context: Option<ObjectId>) -> bool {
        match self {
            Self::Page { page: expected, .. } => page == Some(expected),
            Self::Context(expected) => context == Some(expected),
        }
    }
}

struct Registration {
    scope: Scope,
    path: PathBuf,
    filter: CompiledHarUrlFilter,
    recorder: HarRecorder,
    export_on_context_finish: bool,
}

#[derive(Debug)]
pub(crate) struct PendingExport {
    pub path: PathBuf,
    pub export: HarExport,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Publication {
    pub response_body_request: Option<String>,
}

#[derive(Default)]
struct State {
    registrations: BTreeMap<u64, Registration>,
    capture_jobs: VecDeque<PageCaptureJob>,
}

pub(crate) struct Installation {
    next_registration: AtomicU64,
    #[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
    next_api_request: AtomicU64,
    state: Mutex<State>,
    clock: Arc<dyn Clock>,
    capture_wakeup: Arc<Notify>,
}

pub(crate) trait Clock: Send + Sync {
    fn now_micros(&self) -> Result<i64, HostError>;
}

pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now_micros(&self) -> Result<i64, HostError> {
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(HostError::backend)?
            .as_micros();
        i64::try_from(micros).map_err(|error| HostError::ResourceExhausted(error.to_string()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PageCapturePhase {
    DomContentLoaded,
    Load,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PageCaptureJob {
    pub page: ObjectId,
    pub phase: PageCapturePhase,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PageCapture {
    pub title: String,
    pub event_time_micros: i64,
}

pub(crate) fn page_capture_expression(phase: PageCapturePhase) -> &'static str {
    match phase {
        PageCapturePhase::DomContentLoaded => {
            "({title:document.title,eventTime:performance.timing.domContentLoadedEventStart})"
        }
        PageCapturePhase::Load => {
            "({title:document.title,eventTime:performance.timing.loadEventStart})"
        }
    }
}

pub(crate) fn decode_page_capture(value: &serde_json::Value) -> Result<PageCapture, HostError> {
    let value = value
        .get("result")
        .and_then(|value| value.get("value"))
        .ok_or_else(|| HostError::Backend("HAR page capture has no result value".into()))?;
    let title = value
        .get("title")
        .and_then(|value| value.as_str())
        .ok_or_else(|| HostError::Backend("HAR page capture has no title".into()))?
        .to_owned();
    let event_time_millis = value
        .get("eventTime")
        .and_then(|value| value.as_i64())
        .ok_or_else(|| HostError::Backend("HAR page capture has no event time".into()))?;
    let event_time_micros = event_time_millis
        .checked_mul(1_000)
        .ok_or_else(|| HostError::ResourceExhausted("HAR page timing overflow".into()))?;
    Ok(PageCapture {
        title,
        event_time_micros,
    })
}

struct LifecycleDocument {
    loader_id: String,
    dom_content_loaded: bool,
    load: bool,
}

pub(crate) struct Observer {
    installation: Arc<Installation>,
    downstream: Arc<dyn ChromiumFrameEventObserver>,
    documents: Mutex<BTreeMap<ObjectId, LifecycleDocument>>,
}

impl Observer {
    pub fn new(
        installation: Arc<Installation>,
        downstream: Arc<dyn ChromiumFrameEventObserver>,
    ) -> Self {
        Self {
            installation,
            downstream,
            documents: Mutex::new(BTreeMap::new()),
        }
    }
}

impl ChromiumFrameEventObserver for Observer {
    fn observes_page(&self, page: ObjectId) -> bool {
        self.downstream.observes_page(page)
    }

    fn prepare_page(&self, topology: &CdpTopology, page: ObjectId) -> FrameObservationResult {
        self.downstream.prepare_page(topology, page)
    }

    fn discard_prepared_page(
        &self,
        topology: &CdpTopology,
        page: ObjectId,
    ) -> FrameObservationResult {
        self.downstream.discard_prepared_page(topology, page)
    }

    fn initialize_frame(
        &self,
        topology: &CdpTopology,
        session: &str,
        frame: &page::Frame,
    ) -> FrameObservationResult {
        self.downstream.initialize_frame(topology, session, frame)?;
        let Some(route) = topology
            .page_session_routes()
            .find(|route| route.session_id() == session)
        else {
            return Ok(());
        };
        if topology.main_frame_native_id(route.object_id()) != Some(frame.id.as_str()) {
            return Ok(());
        }
        self.installation
            .observe_page_created(route.object_id(), route.browser_context_object_id())
            .map_err(observer_error)?;
        self.documents
            .lock()
            .map_err(|error| observer_error(HostError::backend(error)))?
            .insert(
                route.object_id(),
                LifecycleDocument {
                    loader_id: frame.loader_id.clone(),
                    dom_content_loaded: false,
                    load: false,
                },
            );
        Ok(())
    }

    fn reconcile(&self, topology: &CdpTopology) -> FrameObservationResult {
        self.downstream.reconcile(topology)?;
        self.documents
            .lock()
            .map_err(|error| observer_error(HostError::backend(error)))?
            .retain(|page, _| topology.main_frame_native_id(*page).is_some());
        Ok(())
    }

    fn event(&self, topology: &CdpTopology, event: &CdpEvent) -> FrameObservationResult {
        self.downstream.event(topology, event)?;
        if event.method != "Page.lifecycleEvent" {
            return Ok(());
        }
        let phase = match event.params.get("name").and_then(|value| value.as_str()) {
            Some("DOMContentLoaded") => PageCapturePhase::DomContentLoaded,
            Some("load") => PageCapturePhase::Load,
            _ => return Ok(()),
        };
        let Some(session_id) = event.session_id.as_deref() else {
            return Ok(());
        };
        let Some(frame_id) = event.params.get("frameId").and_then(|value| value.as_str()) else {
            return Ok(());
        };
        let Some(loader_id) = event
            .params
            .get("loaderId")
            .and_then(|value| value.as_str())
        else {
            return Ok(());
        };
        let Some(route) = topology
            .page_session_routes()
            .find(|route| route.session_id() == session_id)
        else {
            return Ok(());
        };
        if topology.main_frame_native_id(route.object_id()) != Some(frame_id) {
            return Ok(());
        }
        let selected = {
            let mut documents = self
                .documents
                .lock()
                .map_err(|error| observer_error(HostError::backend(error)))?;
            let document =
                documents
                    .entry(route.object_id())
                    .or_insert_with(|| LifecycleDocument {
                        loader_id: loader_id.to_owned(),
                        dom_content_loaded: false,
                        load: false,
                    });
            if document.loader_id != loader_id {
                *document = LifecycleDocument {
                    loader_id: loader_id.to_owned(),
                    dom_content_loaded: false,
                    load: false,
                };
            }
            match phase {
                PageCapturePhase::DomContentLoaded => {
                    !std::mem::replace(&mut document.dom_content_loaded, true)
                }
                PageCapturePhase::Load => !std::mem::replace(&mut document.load, true),
            }
        };
        if selected {
            self.installation
                .observe_page_lifecycle(route.object_id(), route.browser_context_object_id(), phase)
                .map_err(observer_error)?;
        }
        Ok(())
    }

    fn seal(&self) {
        if let Ok(mut documents) = self.documents.lock() {
            documents.clear();
        }
        self.downstream.seal();
    }
}

fn observer_error(error: HostError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(error)
}

pub(crate) fn decode_response_body(body: ResponseBody) -> Result<Vec<u8>, HostError> {
    if !body.base64_encoded {
        return Ok(body.body.into_bytes());
    }
    let capacity = body
        .body
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_add(3))
        .ok_or_else(|| HostError::ResourceExhausted("response body size overflow".into()))?;
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(capacity)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
    STANDARD
        .decode_vec(body.body.as_bytes(), &mut decoded)
        .map_err(|error| HostError::Backend(format!("invalid base64 response body: {error}")))?;
    Ok(decoded)
}

impl Installation {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            next_registration: AtomicU64::new(1),
            #[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
            next_api_request: AtomicU64::new(1),
            state: Mutex::new(State::default()),
            clock,
            capture_wakeup: Arc::new(Notify::new()),
        }
    }

    pub fn start_page(
        &self,
        page: ObjectId,
        context: ObjectId,
        options: RegistrationOptions,
    ) -> Result<u64, HostError> {
        self.start(Scope::Page { page, context }, &[page], options, true)
    }

    pub fn start_context(
        &self,
        context: ObjectId,
        pages: &[ObjectId],
        options: RegistrationOptions,
    ) -> Result<u64, HostError> {
        self.start(Scope::Context(context), pages, options, true)
    }

    #[cfg(feature = "context-tracing-network")]
    pub(crate) fn start_context_trace(
        &self,
        context: ObjectId,
        browser_version: &str,
    ) -> Result<u64, HostError> {
        self.start(
            Scope::Context(context),
            &[],
            RegistrationOptions {
                path: PathBuf::new(),
                filter: HarUrlFilter::Glob("**/*".to_owned()),
                content: HarContentPolicy::Attach,
                mode: HarMode::Minimal,
                browser_name: "chromium".to_owned(),
                browser_version: browser_version.to_owned(),
            },
            false,
        )
    }

    #[cfg(feature = "context-tracing-network")]
    pub(crate) fn snapshot_context_trace(
        &self,
        registration_id: u64,
    ) -> Result<HarExport, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        let registration = state.registrations.get(&registration_id).ok_or_else(|| {
            HostError::InvalidArgument("unknown context trace network registration".into())
        })?;
        registration.recorder.export().map_err(HostError::backend)
    }

    #[cfg(feature = "context-tracing-network")]
    pub(crate) fn finish_context_trace(
        &self,
        registration_id: u64,
    ) -> Result<HarExport, HostError> {
        let registration = self
            .state
            .lock()
            .map_err(HostError::backend)?
            .registrations
            .remove(&registration_id)
            .ok_or_else(|| {
                HostError::InvalidArgument("unknown context trace network registration".into())
            })?;
        registration.recorder.export().map_err(HostError::backend)
    }

    #[cfg(feature = "context-tracing-network")]
    pub(crate) fn discard_context_trace(&self, registration_id: u64) {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .registrations
            .remove(&registration_id);
    }

    #[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
    pub(crate) fn record_api_request(
        &self,
        registration_id: u64,
        trace: ContextApiRequestTrace,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let registration = state
            .registrations
            .get_mut(&registration_id)
            .ok_or_else(|| {
                HostError::InvalidArgument("unknown context trace network registration".into())
            })?;
        if !registration.filter.matches(&trace.url)? {
            return Ok(());
        }
        let request_id = format!(
            "api-request@{}",
            self.next_api_request.fetch_add(1, Ordering::Relaxed)
        );
        let now = self.clock.now_micros()?;
        let mut timing = HarTiming::unavailable();
        timing.start_time_micros = now;
        timing.request_start_micros = now;
        timing.response_start_micros = now;
        timing.response_end_micros = now;
        registration
            .recorder
            .request_started(HarRequestSnapshot {
                id: request_id.clone(),
                page_id: None,
                service_worker_id: None,
                redirected_from: None,
                url: trace.url,
                method: trace.method,
                resource_type: "Other".to_owned(),
                headers: trace
                    .request_headers
                    .into_iter()
                    .map(|(name, value)| HarHeader::new(name, value))
                    .collect(),
                post_data: trace.request_body,
                timing,
            })
            .map_err(HostError::backend)?;
        registration
            .recorder
            .response_received(
                &request_id,
                HarResponseSnapshot {
                    status: i64::from(trace.response_status),
                    status_text: trace.response_status_text,
                    headers: trace
                        .response_headers
                        .into_iter()
                        .map(|(name, value)| HarHeader::new(name, value))
                        .collect(),
                    body: Some(trace.response_body),
                    timing,
                    http_version: "HTTP/1.1".to_owned(),
                    server_ip_address: None,
                    server_port: None,
                    security_details: None,
                    sizes: None,
                },
            )
            .map_err(HostError::backend)?;
        registration
            .recorder
            .request_finished(&request_id, None, None)
            .map_err(HostError::backend)
    }

    fn start(
        &self,
        scope: Scope,
        pages: &[ObjectId],
        options: RegistrationOptions,
        export_on_context_finish: bool,
    ) -> Result<u64, HostError> {
        // Compile the matcher before retaining any registration state.
        let filter = CompiledHarUrlFilter::compile(options.filter)?;
        let id = self.next_registration.fetch_add(1, Ordering::Relaxed);
        if id == u64::MAX {
            return Err(HostError::ResourceExhausted(
                "HAR registration identity space exhausted".into(),
            ));
        }
        let mut registration = Registration {
            scope,
            path: options.path,
            filter,
            export_on_context_finish,
            recorder: HarRecorder::new(HarRecorderOptions {
                content: options.content,
                mode: options.mode,
                browser_name: options.browser_name,
                browser_version: options.browser_version,
            }),
        };
        if options.mode == HarMode::Full {
            for page in pages {
                registration.recorder.record_page(HarPageSnapshot {
                    id: playwright_page_id(&page.get().to_string()),
                    started_time_micros: self.clock.now_micros()?,
                    title: String::new(),
                    on_content_load_micros: None,
                    on_load_micros: None,
                });
            }
        }
        self.state
            .lock()
            .map_err(HostError::backend)?
            .registrations
            .insert(id, registration);
        Ok(id)
    }

    pub fn capture_wakeup(&self) -> Arc<Notify> {
        self.capture_wakeup.clone()
    }

    pub fn observe_page_created(&self, page: ObjectId, context: ObjectId) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let now = self.clock.now_micros()?;
        for registration in state.registrations.values_mut() {
            if registration.recorder.mode() != HarMode::Full
                || !registration.scope.matches(Some(page), Some(context))
            {
                continue;
            }
            registration.recorder.ensure_page(HarPageSnapshot {
                id: playwright_page_id(&page.get().to_string()),
                started_time_micros: now,
                title: String::new(),
                on_content_load_micros: None,
                on_load_micros: None,
            });
        }
        Ok(())
    }

    pub fn observe_page_lifecycle(
        &self,
        page: ObjectId,
        context: ObjectId,
        phase: PageCapturePhase,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let observed = state.registrations.values().any(|registration| {
            registration.recorder.mode() == HarMode::Full
                && registration.scope.matches(Some(page), Some(context))
        });
        if !observed
            || state
                .capture_jobs
                .iter()
                .any(|job| job.page == page && job.phase == phase)
        {
            return Ok(());
        }
        state
            .capture_jobs
            .try_reserve(1)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        state.capture_jobs.push_back(PageCaptureJob { page, phase });
        drop(state);
        self.capture_wakeup.notify_one();
        Ok(())
    }

    pub fn take_page_capture_jobs(&self) -> Result<Vec<PageCaptureJob>, HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        Ok(state.capture_jobs.drain(..).collect())
    }

    pub fn complete_page_capture(
        &self,
        job: PageCaptureJob,
        capture: PageCapture,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let page_id = playwright_page_id(&job.page.get().to_string());
        for registration in state.registrations.values_mut() {
            let (on_content_load_micros, on_load_micros) = match job.phase {
                PageCapturePhase::DomContentLoaded => (Some(capture.event_time_micros), None),
                PageCapturePhase::Load => (None, Some(capture.event_time_micros)),
            };
            registration.recorder.update_page_lifecycle(
                &page_id,
                capture.title.clone(),
                on_content_load_micros,
                on_load_micros,
            );
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn observe(
        &self,
        page: Option<ObjectId>,
        context: Option<ObjectId>,
        event: &NetworkEventRecord,
    ) -> Result<Publication, HostError> {
        self.observe_with_request_override(page, context, event, None)
    }

    pub(crate) fn observe_with_request_override(
        &self,
        page: Option<ObjectId>,
        context: Option<ObjectId>,
        event: &NetworkEventRecord,
        request_override: Option<&RouteContinue>,
    ) -> Result<Publication, HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let mut response_body_request = None;
        for registration in state.registrations.values_mut() {
            if !registration.scope.matches(page, context) {
                continue;
            }
            match event {
                NetworkEventRecord::Request(request) => {
                    if !registration.filter.matches(
                        request_override
                            .map(|value| value.effective_url(&request.url))
                            .unwrap_or(&request.url),
                    )? {
                        continue;
                    }
                    if registration.recorder.mode() == HarMode::Full
                        && let Some(page) = page
                    {
                        registration.recorder.ensure_page(HarPageSnapshot {
                            id: playwright_page_id(&page.get().to_string()),
                            started_time_micros: self.clock.now_micros()?,
                            title: String::new(),
                            on_content_load_micros: None,
                            on_load_micros: None,
                        });
                    }
                    registration
                        .recorder
                        .request_started(request_snapshot(request, request_override))
                        .map_err(HostError::backend)?;
                }
                NetworkEventRecord::Response(response) => {
                    if !registration.recorder.contains_request(&response.request_id) {
                        continue;
                    }
                    registration
                        .recorder
                        .response_received(&response.request_id, response_snapshot(response))
                        .map_err(HostError::backend)?;
                }
                NetworkEventRecord::RequestFinished {
                    request,
                    response,
                    encoded_body_bytes,
                } => {
                    if !registration.recorder.contains_request(&request.id) {
                        continue;
                    }
                    registration
                        .recorder
                        .update_request(&request.id, request_snapshot(request, request_override))
                        .map_err(HostError::backend)?;
                    let response = response.as_ref().map(|response| {
                        response_snapshot_with_sizes(
                            response,
                            completed_request_sizes(request, response, *encoded_body_bytes).ok(),
                        )
                    });
                    registration
                        .recorder
                        .request_finished(&request.id, response, None)
                        .map_err(HostError::backend)?;
                    if registration.recorder.needs_response_body(&request.id) {
                        response_body_request.get_or_insert_with(|| request.id.clone());
                    }
                }
                NetworkEventRecord::RequestFailed {
                    request,
                    response,
                    failure_text,
                    ..
                } => {
                    if !registration.recorder.contains_request(&request.id) {
                        continue;
                    }
                    registration
                        .recorder
                        .update_request(&request.id, request_snapshot(request, request_override))
                        .map_err(HostError::backend)?;
                    registration
                        .recorder
                        .request_failed(
                            &request.id,
                            response.as_ref().map(response_snapshot),
                            failure_text,
                        )
                        .map_err(HostError::backend)?;
                }
            }
        }
        Ok(Publication {
            response_body_request,
        })
    }

    pub fn set_response_body(&self, request_id: &str, body: &[u8]) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        for registration in state.registrations.values_mut() {
            if registration.recorder.needs_response_body(request_id) {
                registration
                    .recorder
                    .set_response_body(request_id, body)
                    .map_err(HostError::backend)?;
            }
        }
        Ok(())
    }

    pub fn retire_service_worker(
        &self,
        context: ObjectId,
        worker_id: &str,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        for registration in state.registrations.values_mut() {
            if registration.scope.context() == context {
                registration.recorder.retire_service_worker(worker_id);
            }
        }
        Ok(())
    }

    pub fn finish_context(&self, context: ObjectId) -> Result<Vec<PendingExport>, HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        let ids = state
            .registrations
            .iter()
            .filter_map(|(id, registration)| {
                (registration.scope.context() == context && registration.export_on_context_finish)
                    .then_some(*id)
            })
            .collect::<Vec<_>>();
        let mut exports = Vec::new();
        exports
            .try_reserve_exact(ids.len())
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        for id in ids {
            let registration = state
                .registrations
                .remove(&id)
                .expect("snapshotted HAR registration remains present while locked");
            exports.push(PendingExport {
                path: registration.path,
                export: registration.recorder.export().map_err(HostError::backend)?,
            });
        }
        Ok(exports)
    }

    pub fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.registrations.clear();
            state.capture_jobs.clear();
        }
    }
}

impl HostSession {
    pub async fn page_record_har(
        &self,
        page_id: &str,
        path: &str,
        pattern: Option<String>,
        content: HarUpdateContent,
        mode: HarUpdateMode,
    ) -> Result<u64, HostError> {
        let filter = resolve_page_filter(self, page_id, pattern)?;
        self.page_record_har_with_filter(page_id, path, filter, content, mode)
    }

    pub async fn page_record_har_regexp(
        &self,
        page_id: &str,
        path: &str,
        source: String,
        flags: String,
        content: HarUpdateContent,
        mode: HarUpdateMode,
    ) -> Result<u64, HostError> {
        self.page_record_har_with_filter(
            page_id,
            path,
            HarUrlFilter::RegExp { source, flags },
            content,
            mode,
        )
    }

    fn page_record_har_with_filter(
        &self,
        page_id: &str,
        path: &str,
        filter: HarUrlFilter,
        content: HarUpdateContent,
        mode: HarUpdateMode,
    ) -> Result<u64, HostError> {
        let page = parse_id(page_id, "Page")?;
        self.require_visible_page_id(page)?;
        let (context, installation) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            let topology = live.composition.topology().map_err(HostError::backend)?;
            let context = topology
                .topology()
                .page_session_routes()
                .find(|route| route.object_id() == page)
                .map(|route| route.browser_context_object_id())
                .ok_or(HostError::TargetClosed)?;
            (context, live.har_recording.clone())
        };
        installation.start_page(
            page,
            context,
            options(path, filter, content, mode, &self.version),
        )
    }

    pub async fn context_record_har(
        &self,
        context_id: &str,
        path: &str,
        pattern: Option<String>,
        content: HarUpdateContent,
        mode: HarUpdateMode,
    ) -> Result<u64, HostError> {
        let filter = resolve_context_filter(self, context_id, pattern)?;
        self.context_record_har_with_filter(context_id, path, filter, content, mode)
    }

    pub async fn context_record_har_regexp(
        &self,
        context_id: &str,
        path: &str,
        source: String,
        flags: String,
        content: HarUpdateContent,
        mode: HarUpdateMode,
    ) -> Result<u64, HostError> {
        self.context_record_har_with_filter(
            context_id,
            path,
            HarUrlFilter::RegExp { source, flags },
            content,
            mode,
        )
    }

    fn context_record_har_with_filter(
        &self,
        context_id: &str,
        path: &str,
        filter: HarUrlFilter,
        content: HarUpdateContent,
        mode: HarUpdateMode,
    ) -> Result<u64, HostError> {
        let context = parse_id(context_id, "Context")?;
        self.require_visible_context_id(context)?;
        let pages = if mode == HarUpdateMode::Full {
            self.pages(context_id)?
                .into_iter()
                .map(|page| parse_id(&page.id, "Page"))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .har_recording
                .clone()
        };
        installation.start_context(
            context,
            &pages,
            options(path, filter, content, mode, &self.version),
        )
    }

    pub(crate) async fn finish_context_har(&self, context_id: &str) -> Result<(), HostError> {
        let context = parse_id(context_id, "Context")?;
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .har_recording
                .clone()
        };
        for pending in installation.finish_context(context)? {
            crate::har_archive::persist(pending.path, pending.export).await?;
        }
        Ok(())
    }
}

fn parse_id(value: &str, kind: &str) -> Result<ObjectId, HostError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|id| *id != 0)
        .map(ObjectId::new)
        .ok_or_else(|| HostError::InvalidArgument(format!("invalid portable {kind} ID")))
}

fn resolve_page_filter(
    host: &HostSession,
    page_id: &str,
    pattern: Option<String>,
) -> Result<HarUrlFilter, HostError> {
    let mut pattern = pattern.unwrap_or_else(|| "**/*".to_owned());
    if !has_wildcard(&pattern) {
        pattern = host.page_context_snapshot(page_id)?.resolve_url(&pattern)?;
    }
    Ok(HarUrlFilter::Glob(pattern))
}

fn resolve_context_filter(
    host: &HostSession,
    context_id: &str,
    pattern: Option<String>,
) -> Result<HarUrlFilter, HostError> {
    let mut pattern = pattern.unwrap_or_else(|| "**/*".to_owned());
    if !has_wildcard(&pattern) {
        pattern = host.context_snapshot(context_id)?.resolve_url(&pattern)?;
    }
    Ok(HarUrlFilter::Glob(pattern))
}

fn has_wildcard(pattern: &str) -> bool {
    pattern
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'['))
}

fn options(
    path: &str,
    filter: HarUrlFilter,
    content: HarUpdateContent,
    mode: HarUpdateMode,
    browser_version: &str,
) -> RegistrationOptions {
    RegistrationOptions {
        path: PathBuf::from(path),
        filter,
        content,
        mode,
        browser_name: "chromium".to_owned(),
        browser_version: browser_version.to_owned(),
    }
}

fn request_snapshot(
    request: &NetworkRequestRecord,
    request_override: Option<&RouteContinue>,
) -> HarRequestSnapshot {
    let url = request_override
        .map(|value| value.effective_url(&request.url))
        .unwrap_or(&request.url);
    let method = request_override
        .map(|value| value.effective_method(&request.method))
        .unwrap_or(&request.method);
    let request_headers = request_override
        .map(|value| value.effective_headers(&request.headers))
        .unwrap_or(&request.headers);
    let post_data = request_override
        .and_then(|value| value.effective_post_data(request.post_data.as_deref()))
        .or(request.post_data.as_deref());
    HarRequestSnapshot {
        id: request.id.clone(),
        page_id: request
            .service_worker_id
            .is_none()
            .then(|| playwright_page_id(&request.page_id)),
        service_worker_id: request.service_worker_id.clone(),
        redirected_from: request.redirected_from.clone(),
        url: url.to_owned(),
        method: method.to_owned(),
        resource_type: request.resource_type.clone(),
        headers: headers(request_headers),
        post_data: post_data.map(ToOwned::to_owned),
        timing: timing(request.timing),
    }
}

fn response_snapshot(response: &NetworkResponseRecord) -> HarResponseSnapshot {
    response_snapshot_with_sizes(response, None)
}

fn response_snapshot_with_sizes(
    response: &NetworkResponseRecord,
    sizes: Option<RequestSizes>,
) -> HarResponseSnapshot {
    HarResponseSnapshot {
        status: response.status,
        status_text: response.status_text.clone(),
        headers: headers(&response.headers),
        body: response.body.as_deref().map(ToOwned::to_owned),
        timing: timing(response.timing),
        http_version: response_http_version(response),
        server_ip_address: response_server_ip_address(response),
        server_port: response_server_port(response),
        security_details: security_details(response),
        sizes: sizes.map(|sizes| HarSizes {
            request_headers: sizes.request_headers_size,
            response_body: sizes.response_body_size,
            response_headers: sizes.response_headers_size,
            transfer: sizes.transfer_size,
        }),
    }
}

#[cfg(feature = "response-connection-details")]
fn response_http_version(response: &NetworkResponseRecord) -> String {
    response.connection_details.as_ref().map_or_else(
        || "HTTP/1.1".to_owned(),
        |details| details.http_version().to_owned(),
    )
}

#[cfg(not(feature = "response-connection-details"))]
fn response_http_version(_response: &NetworkResponseRecord) -> String {
    "HTTP/1.1".to_owned()
}

#[cfg(feature = "response-connection-details")]
fn response_server_ip_address(response: &NetworkResponseRecord) -> Option<String> {
    response
        .connection_details
        .as_ref()
        .and_then(|details| details.server_ip_address().map(str::to_owned))
}

#[cfg(not(feature = "response-connection-details"))]
fn response_server_ip_address(_response: &NetworkResponseRecord) -> Option<String> {
    None
}

#[cfg(feature = "response-connection-details")]
fn response_server_port(response: &NetworkResponseRecord) -> Option<u16> {
    response
        .connection_details
        .as_ref()
        .and_then(|details| details.server_port())
}

#[cfg(not(feature = "response-connection-details"))]
fn response_server_port(_response: &NetworkResponseRecord) -> Option<u16> {
    None
}

#[cfg(feature = "response-security-details")]
fn security_details(response: &NetworkResponseRecord) -> Option<HarSecurityDetails> {
    response
        .security_details
        .as_ref()
        .map(|details| HarSecurityDetails {
            protocol: Some(details.protocol().to_owned()),
            subject_name: Some(details.subject_name().to_owned()),
            issuer: Some(details.issuer().to_owned()),
            valid_from_micros: Some(details.valid_from_micros()),
            valid_to_micros: Some(details.valid_to_micros()),
        })
}

#[cfg(not(feature = "response-security-details"))]
fn security_details(_response: &NetworkResponseRecord) -> Option<HarSecurityDetails> {
    None
}

fn headers(values: &[(String, String)]) -> Vec<HarHeader> {
    values
        .iter()
        .map(|(name, value)| HarHeader::new(name, value))
        .collect()
}

fn timing(value: playwright_rust_network_model::RequestTiming) -> HarTiming {
    HarTiming {
        start_time_micros: value.start_time_micros,
        domain_lookup_start_micros: value.domain_lookup_start_micros,
        domain_lookup_end_micros: value.domain_lookup_end_micros,
        connect_start_micros: value.connect_start_micros,
        secure_connection_start_micros: value.secure_connection_start_micros,
        connect_end_micros: value.connect_end_micros,
        request_start_micros: value.request_start_micros,
        response_start_micros: value.response_start_micros,
        response_end_micros: value.response_end_micros,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use playwright_rust_network_model::RequestTiming;

    use super::*;

    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now_micros(&self) -> Result<i64, HostError> {
            Ok(self.0)
        }
    }

    fn installation() -> Installation {
        Installation::new(Arc::new(FixedClock(1_700_000_000_000_000)))
    }

    fn options(path: &str) -> RegistrationOptions {
        RegistrationOptions {
            path: path.into(),
            filter: HarUrlFilter::Glob("**/api/**".to_owned()),
            content: HarContentPolicy::Embed,
            mode: HarMode::Minimal,
            browser_name: "chromium".to_owned(),
            browser_version: "test".to_owned(),
        }
    }

    #[test]
    fn route_overrides_are_recorded_at_request_start_and_finish() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(18);
        let page = ObjectId::new(19);
        installation
            .start_context(context, &[], options("overrides.har"))
            .unwrap();
        let mut original = request("overridden", page, context);
        original.url = "https://example.test/api/echo_redir".to_owned();
        original.method = "GET".to_owned();
        original.headers = vec![("x-original".to_owned(), "yes".to_owned())];
        original.post_data = None;
        let request_override = RouteContinue::new(
            Some("https://example.test/api/echo".to_owned()),
            Some("POST".to_owned()),
            Some(vec![(
                "content-type".to_owned(),
                "application/json".to_owned(),
            )]),
            Some(b"11".to_vec()),
        )
        .unwrap();

        // Act
        installation
            .observe_with_request_override(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(original.clone()),
                Some(&request_override),
            )
            .unwrap();
        installation
            .observe_with_request_override(
                Some(page),
                Some(context),
                &NetworkEventRecord::RequestFinished {
                    request: original,
                    response: None,
                    encoded_body_bytes: None,
                },
                Some(&request_override),
            )
            .unwrap();
        let exports = installation.finish_context(context).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        let recorded = &document["log"]["entries"][0]["request"];

        // Assert
        assert_eq!(recorded["url"], "https://example.test/api/echo");
        assert_eq!(recorded["method"], "POST");
        assert_eq!(recorded["postData"]["text"], "11");
        assert_eq!(recorded["headers"][0]["name"], "content-type");
        assert_eq!(recorded["headers"][0]["value"], "application/json");
    }

    #[cfg(all(feature = "context-api-request", feature = "context-tracing-network"))]
    #[test]
    fn context_trace_records_api_request_metadata_and_body() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(7);
        let registration = installation.start_context_trace(context, "test").unwrap();
        let trace = ContextApiRequestTrace {
            method: "POST".to_owned(),
            url: "https://example.test/api/simple.json".to_owned(),
            request_headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            request_body: Some(br#"{"foo":"bar"}"#.to_vec()),
            response_status: 200,
            response_status_text: "OK".to_owned(),
            response_headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            response_body: br#"{"ok":true}"#.to_vec(),
        };

        // Act
        installation
            .record_api_request(registration, trace)
            .unwrap();

        // Assert
        let export = installation.finish_context_trace(registration).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&export.json).unwrap();
        let entry = &document["log"]["entries"][0];
        assert_eq!(entry["request"]["method"], "POST");
        assert_eq!(
            entry["request"]["url"],
            "https://example.test/api/simple.json"
        );
        assert_eq!(entry["response"]["status"], 200);
        assert_eq!(export.attachments.len(), 2);
    }

    #[cfg(feature = "har-recording-regexp")]
    #[test]
    fn regexp_filter_matches_full_urls_with_flags() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(8);
        let page = ObjectId::new(9);
        let mut registration = options("regexp.har");
        registration.filter = HarUrlFilter::RegExp {
            source: "HAR.X?HTML".to_owned(),
            flags: "i".to_owned(),
        };
        installation
            .start_context(context, &[], registration)
            .unwrap();
        let mut matching = request("matching", page, context);
        matching.url = "https://example.test/har.html".to_owned();
        let mut nonmatching = request("nonmatching", page, context);
        nonmatching.url = "https://example.test/one-style.css".to_owned();

        // Act
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(matching),
            )
            .unwrap();
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(nonmatching),
            )
            .unwrap();
        let exports = installation.finish_context(context).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        let entries = document["log"]["entries"].as_array().unwrap();

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]["request"]["url"],
            "https://example.test/har.html"
        );
    }

    #[cfg(feature = "har-recording-regexp")]
    #[test]
    fn regexp_filter_supports_ecmascript_lookaround_and_backreferences() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(12);
        let page = ObjectId::new(13);
        let mut registration = options("regexp-ecmascript.har");
        registration.filter = HarUrlFilter::RegExp {
            source: r"(?<=/)(api)/\1$".to_owned(),
            flags: String::new(),
        };
        installation
            .start_context(context, &[], registration)
            .unwrap();
        let mut matching = request("matching", page, context);
        matching.url = "https://example.test/api/api".to_owned();
        let mut nonmatching = request("nonmatching", page, context);
        nonmatching.url = "https://example.test/api/data".to_owned();

        // Act
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(matching),
            )
            .unwrap();
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(nonmatching),
            )
            .unwrap();
        let exports = installation.finish_context(context).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        let entries = document["log"]["entries"].as_array().unwrap();

        // Assert
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["request"]["url"], "https://example.test/api/api");
    }

    #[cfg(feature = "har-recording-regexp")]
    #[test]
    fn global_regexp_filter_preserves_javascript_last_index_state() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(14);
        let page = ObjectId::new(15);
        let mut registration = options("regexp-global.har");
        registration.filter = HarUrlFilter::RegExp {
            source: "api".to_owned(),
            flags: "g".to_owned(),
        };
        installation
            .start_context(context, &[], registration)
            .unwrap();
        let requests = ["first", "second", "third"].map(|id| request(id, page, context));

        // Act
        for request in requests {
            installation
                .observe(
                    Some(page),
                    Some(context),
                    &NetworkEventRecord::Request(request),
                )
                .unwrap();
        }
        let exports = installation.finish_context(context).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        let entries = document["log"]["entries"].as_array().unwrap();

        // Assert
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0]["request"]["url"],
            "https://example.test/api/data"
        );
        assert_eq!(
            entries[1]["request"]["url"],
            "https://example.test/api/data"
        );
    }

    fn request(id: &str, page: ObjectId, context: ObjectId) -> NetworkRequestRecord {
        NetworkRequestRecord {
            id: id.to_owned(),
            native_request_id: Some(format!("native-{id}")),
            page_id: page.get().to_string(),
            context_id: Some(context.get().to_string()),
            service_worker_id: None,
            url: "https://example.test/api/data".to_owned(),
            method: "GET".to_owned(),
            resource_type: "fetch".to_owned(),
            headers: vec![],
            post_data: None,
            is_navigation_request: false,
            frame_id: None,
            frame_available: true,
            redirected_from: None,
            timing: RequestTiming::pending(),
            size_owner: None,
        }
    }

    fn response(id: &str) -> NetworkResponseRecord {
        NetworkResponseRecord {
            id: format!("response-{id}"),
            request_id: id.to_owned(),
            context_id: None,
            service_worker_id: None,
            url: "https://example.test/api/data".to_owned(),
            status: 200,
            status_text: "OK".to_owned(),
            headers: vec![("content-type".to_owned(), "text/plain".to_owned())],
            from_service_worker: false,
            #[cfg(feature = "response-connection-details")]
            connection_details: None,
            #[cfg(feature = "response-security-details")]
            security_details: None,
            timing: RequestTiming::pending(),
            body: None,
            #[cfg(feature = "response-finished")]
            finished_owner: None,
        }
    }

    #[test]
    fn context_recorder_observes_page_lifecycle_and_requests_body_after_terminal() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(10);
        let page = ObjectId::new(11);
        installation
            .start_context(context, &[], options("context.har"))
            .unwrap();
        let request = request("request-1", page, context);
        let response = response("request-1");
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(request.clone()),
            )
            .unwrap();
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Response(response.clone()),
            )
            .unwrap();

        // Act
        let publication = installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::RequestFinished {
                    request,
                    response: Some(response),
                    encoded_body_bytes: Some(4),
                },
            )
            .unwrap();
        installation
            .set_response_body("request-1", b"body")
            .unwrap();
        let exports = installation.finish_context(context).unwrap();

        // Assert
        assert_eq!(
            publication.response_body_request.as_deref(),
            Some("request-1")
        );
        assert_eq!(exports.len(), 1);
        assert_eq!(exports[0].path, PathBuf::from("context.har"));
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        assert_eq!(
            document["log"]["entries"][0]["response"]["content"]["text"],
            "body"
        );
    }

    #[test]
    fn page_recorder_excludes_service_worker_requests() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(20);
        let page = ObjectId::new(21);
        installation
            .start_page(page, context, options("page.har"))
            .unwrap();
        let mut worker_request = request("worker-request", context, context);
        worker_request.page_id = context.get().to_string();
        worker_request.service_worker_id = Some("worker-1".to_owned());

        // Act
        installation
            .observe(
                None,
                Some(context),
                &NetworkEventRecord::Request(worker_request),
            )
            .unwrap();
        let exports = installation.finish_context(context).unwrap();

        // Assert
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        assert!(document["log"]["entries"].as_array().unwrap().is_empty());
    }

    #[test]
    fn one_body_copy_updates_each_matching_recorder() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(30);
        let page = ObjectId::new(31);
        installation
            .start_context(context, &[], options("a.har"))
            .unwrap();
        installation
            .start_page(page, context, options("b.har"))
            .unwrap();
        let request = request("request-1", page, context);
        let response = response("request-1");
        for event in [
            NetworkEventRecord::Request(request.clone()),
            NetworkEventRecord::Response(response.clone()),
            NetworkEventRecord::RequestFinished {
                request,
                response: Some(response),
                encoded_body_bytes: Some(6),
            },
        ] {
            installation
                .observe(Some(page), Some(context), &event)
                .unwrap();
        }

        // Act
        installation
            .set_response_body("request-1", b"shared")
            .unwrap();
        let exports = installation.finish_context(context).unwrap();

        // Assert
        assert_eq!(exports.len(), 2);
        for export in exports {
            assert!(
                String::from_utf8(export.export.json)
                    .unwrap()
                    .contains("shared")
            );
        }
    }

    #[test]
    fn full_recorder_uses_terminal_request_timing_and_sizes_without_public_size_owner() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(40);
        let page = ObjectId::new(41);
        let mut full = options("full.har");
        full.mode = HarMode::Full;
        installation.start_context(context, &[page], full).unwrap();
        let mut request = request("request-1", page, context);
        request.timing = RequestTiming {
            start_time_micros: 1_700_000_000_000_000,
            domain_lookup_start_micros: 1_000,
            domain_lookup_end_micros: 2_000,
            connect_start_micros: 2_000,
            secure_connection_start_micros: 3_000,
            connect_end_micros: 5_000,
            request_start_micros: 10_000,
            response_start_micros: 12_000,
            response_end_micros: 15_000,
        };
        let mut response = response("request-1");
        response.timing = request.timing;
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::Request(request.clone()),
            )
            .unwrap();

        // Act
        installation
            .observe(
                Some(page),
                Some(context),
                &NetworkEventRecord::RequestFinished {
                    request,
                    response: Some(response),
                    encoded_body_bytes: Some(64),
                },
            )
            .unwrap();
        installation
            .set_response_body("request-1", b"decoded body")
            .unwrap();
        let exports = installation.finish_context(context).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        let entry = &document["log"]["entries"][0];

        // Assert
        assert_eq!(entry["time"], 11.0);
        assert_eq!(entry["timings"]["dns"], 1.0);
        assert_eq!(entry["response"]["_transferSize"], 64);
        assert!(entry["request"]["headersSize"].as_i64().unwrap() > 0);
        assert!(entry["response"]["headersSize"].as_i64().unwrap() > 0);
        assert_eq!(
            entry["response"]["bodySize"].as_i64().unwrap()
                + entry["response"]["headersSize"].as_i64().unwrap(),
            64
        );
    }

    #[test]
    fn full_recorder_captures_each_page_lifecycle_phase_once() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(50);
        let page = ObjectId::new(51);
        let mut full = options("full-pages.har");
        full.mode = HarMode::Full;
        installation.start_context(context, &[page], full).unwrap();

        // Act
        installation
            .observe_page_lifecycle(page, context, PageCapturePhase::DomContentLoaded)
            .unwrap();
        installation
            .observe_page_lifecycle(page, context, PageCapturePhase::DomContentLoaded)
            .unwrap();
        installation
            .observe_page_lifecycle(page, context, PageCapturePhase::Load)
            .unwrap();
        let jobs = installation.take_page_capture_jobs().unwrap();
        for job in jobs.iter().copied() {
            let offset = match job.phase {
                PageCapturePhase::DomContentLoaded => 1_250_000,
                PageCapturePhase::Load => 2_500_000,
            };
            installation
                .complete_page_capture(
                    job,
                    PageCapture {
                        title: "Ready".to_owned(),
                        event_time_micros: 1_700_000_000_000_000 + offset,
                    },
                )
                .unwrap();
        }
        let exports = installation.finish_context(context).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&exports[0].export.json).unwrap();
        let page = &document["log"]["pages"][0];

        // Assert
        assert_eq!(jobs.len(), 2);
        assert_eq!(page["title"], "Ready");
        assert_eq!(page["pageTimings"]["onContentLoad"], 1250.0);
        assert_eq!(page["pageTimings"]["onLoad"], 2500.0);
    }
}
