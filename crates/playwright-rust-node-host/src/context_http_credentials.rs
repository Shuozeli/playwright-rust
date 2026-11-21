//! Optional Context-scoped HTTP authentication challenge handling.
//!
//! Credentials stay in Rust-owned storage with bounded fields. Chromium request interception is
//! enabled only for selected Contexts and every authentication challenge is
//! answered on the exact CDP session that emitted it. Page routing can share
//! the same `Fetch` installation without either capability overwriting the
//! other's protocol configuration.

use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::{CdpContextOwnership, CdpTopology};
use playwright_rust_cdp::{
    CdpConnection, CdpEvent, ServiceWorkerInitializationCommand,
    connection::CdpError,
    generated::{page, target},
};
#[cfg(not(feature = "page-routes"))]
use playwright_rust_cdp::{CdpEventReceiver, CdpEventRecvError};
#[cfg(not(feature = "page-routes"))]
use playwright_rust_composition_chromium::SharedChromiumTopology;
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_frame_observer::{
    ChromiumFrameEventObserver, FrameObservationResult,
};
#[cfg(not(feature = "page-routes"))]
use playwright_rust_plugin_chromium_session_initialization::{
    ChromiumSessionInitializationFuture, ChromiumSessionInitializationTarget,
    ChromiumSessionInitializer,
};
use serde_json::{Value, json};

use crate::{HostError, HostSession};

const MAX_NATIVE_ID_BYTES: usize = 4 * 1024;
pub const MAX_HTTP_CREDENTIAL_FIELD_BYTES: usize = 64 * 1024;
const SCOPE: ResourceScope = ResourceScope::extension(196, 0);

#[derive(Clone, Eq, PartialEq)]
pub struct ContextHttpCredentials {
    username: Box<str>,
    password: Box<str>,
    origin: Option<Box<str>>,
    send_always: bool,
}

impl std::fmt::Debug for ContextHttpCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContextHttpCredentials")
            .field("has_username", &!self.username.is_empty())
            .field("has_password", &!self.password.is_empty())
            .field("origin", &self.origin)
            .field("send_always", &self.send_always)
            .finish()
    }
}

impl ContextHttpCredentials {
    pub fn new(
        username: String,
        password: String,
        origin: Option<String>,
    ) -> Result<Self, HostError> {
        Self::new_with_send(username, password, origin, false)
    }

    pub fn new_with_send(
        username: String,
        password: String,
        origin: Option<String>,
        send_always: bool,
    ) -> Result<Self, HostError> {
        for (name, value) in [("username", &username), ("password", &password)] {
            if value.len() > MAX_HTTP_CREDENTIAL_FIELD_BYTES {
                return Err(HostError::ResourceExhausted(format!(
                    "HTTP credentials {name} exceeds {MAX_HTTP_CREDENTIAL_FIELD_BYTES} UTF-8 bytes"
                )));
            }
            if value.contains('\0') {
                return Err(HostError::InvalidArgument(format!(
                    "HTTP credentials {name} must not contain NUL"
                )));
            }
        }
        let origin = origin
            .map(|origin| {
                if origin.len() > MAX_HTTP_CREDENTIAL_FIELD_BYTES {
                    return Err(HostError::ResourceExhausted(format!(
                        "HTTP credentials origin exceeds {MAX_HTTP_CREDENTIAL_FIELD_BYTES} UTF-8 bytes"
                    )));
                }
                if origin.contains('\0') {
                    return Err(HostError::InvalidArgument(
                        "HTTP credentials origin must not contain NUL".into(),
                    ));
                }
                Ok(origin.to_ascii_lowercase().into_boxed_str())
            })
            .transpose()?;
        Ok(Self {
            username: username.into_boxed_str(),
            password: password.into_boxed_str(),
            origin,
            send_always,
        })
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn origin(&self) -> Option<&str> {
        self.origin.as_deref()
    }

    pub const fn send_always(&self) -> bool {
        self.send_always
    }

    fn matches_url(&self, url: &str) -> bool {
        let Some(expected) = self.origin.as_deref() else {
            return true;
        };
        url::Url::parse(url)
            .map(|url| url.origin().ascii_serialization().to_ascii_lowercase() == expected)
            .unwrap_or(false)
    }
}

struct ContextEntry {
    context: ObjectId,
    credentials: ContextHttpCredentials,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PendingActionKind {
    ContinueRequest,
    ContinueWithAuth {
        response: &'static str,
        username: Option<Box<str>>,
        password: Option<Box<str>>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingAction {
    session_id: Box<str>,
    request_id: Box<str>,
    kind: PendingActionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttemptedAuthentication {
    session_id: Box<str>,
    request_id: Box<str>,
    network_id: Option<Box<str>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RequestMapping {
    session_id: Box<str>,
    request_id: Box<str>,
    network_id: Box<str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SessionScope {
    pub(crate) page: Option<ObjectId>,
    pub(crate) context: ObjectId,
    pub(crate) ownership: CdpContextOwnership,
}

struct InitializingSession {
    session_id: Box<str>,
    scope: SessionScope,
}

struct ServiceWorkerSession {
    session_id: Box<str>,
    scope: SessionScope,
}

struct State {
    default_credentials: Option<ContextHttpCredentials>,
    entries: Vec<ContextEntry>,
    pending: VecDeque<PendingAction>,
    attempted: Vec<AttemptedAuthentication>,
    requests: Vec<RequestMapping>,
    initializing_sessions: Vec<InitializingSession>,
    service_worker_sessions: Vec<ServiceWorkerSession>,
    closed: bool,
}

pub(crate) const fn requirements() -> ResourceRequirements {
    let storage_bytes = (std::mem::size_of::<Installation>() + std::mem::size_of::<State>()) as u64;
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            storage_bytes,
            storage_bytes,
        )
        .with_metadata(1, 1)
}

pub(crate) struct Installation {
    connection: Arc<CdpConnection>,
    state: Mutex<State>,
    pub(crate) update_gate: tokio::sync::Mutex<()>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new_with_default(
        resources: &ProductionResourceLedger,
        connection: Arc<CdpConnection>,
        default_credentials: Option<ContextHttpCredentials>,
    ) -> Result<Arc<Self>, HostError> {
        let plan = requirements();
        let storage = resources
            .claim(plan)
            .map_err(HostError::backend)?
            .reserve(
                SCOPE,
                ResourceClass::BackendStateBytes,
                plan.class(ResourceClass::BackendStateBytes).global,
            )
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self {
            connection,
            state: Mutex::new(State {
                default_credentials,
                entries: Vec::new(),
                pending: VecDeque::new(),
                attempted: Vec::new(),
                requests: Vec::new(),
                initializing_sessions: Vec::new(),
                service_worker_sessions: Vec::new(),
                closed: false,
            }),
            update_gate: tokio::sync::Mutex::new(()),
            _storage: storage,
        }))
    }

    pub(crate) fn install(
        &self,
        context: ObjectId,
        credentials: ContextHttpCredentials,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if state.entries.iter().any(|entry| entry.context == context) {
            return Err(HostError::InvalidArgument(
                "HTTP credentials are already installed for this Context".into(),
            ));
        }
        state
            .entries
            .try_reserve(1)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        state.entries.push(ContextEntry {
            context,
            credentials,
        });
        Ok(())
    }

    pub(crate) fn remove(&self, context: ObjectId) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        state.entries.retain(|entry| entry.context != context);
        Ok(())
    }

    fn set(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
        credentials: Option<ContextHttpCredentials>,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        set_credentials(&mut state, context, ownership, credentials)
    }

    pub(crate) fn enabled(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<bool, HostError> {
        Ok(self.credentials(context, ownership)?.is_some())
    }

    pub(crate) fn credentials(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<Option<ContextHttpCredentials>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        Ok(match ownership {
            CdpContextOwnership::Default => state.default_credentials.clone(),
            CdpContextOwnership::Owned | CdpContextOwnership::External => state
                .entries
                .iter()
                .find(|entry| entry.context == context)
                .map(|entry| entry.credentials.clone()),
        })
    }

    fn enqueue(&self, action: PendingAction) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        enqueue_action_state(&mut state, action)
    }

    pub(crate) fn continue_request(
        &self,
        session_id: &str,
        request_id: &str,
        network_id: Option<&str>,
    ) -> Result<(), HostError> {
        validate_native_id("sessionId", session_id)?;
        validate_native_id("requestId", request_id)?;
        self.observe_request(session_id, request_id, network_id)?;
        self.enqueue(PendingAction {
            session_id: session_id.to_owned().into_boxed_str(),
            request_id: request_id.to_owned().into_boxed_str(),
            kind: PendingActionKind::ContinueRequest,
        })
    }

    fn observe_request(
        &self,
        session_id: &str,
        request_id: &str,
        network_id: Option<&str>,
    ) -> Result<(), HostError> {
        let Some(network_id) = network_id else {
            return Ok(());
        };
        validate_native_id("networkId", network_id)?;
        let mut state = self.state.lock().map_err(HostError::backend)?;
        observe_request_state(&mut state, session_id, request_id, network_id)
    }

    fn retire_request(&self, session_id: &str, network_id: &str) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        retire_request_state(&mut state, session_id, network_id);
        Ok(())
    }

    pub(crate) fn remember_initializing_session(
        &self,
        session_id: &str,
        scope: SessionScope,
    ) -> Result<(), HostError> {
        validate_native_id("sessionId", session_id)?;
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        remember_initializing_session_in_state(&mut state, session_id, scope)
    }

    pub(crate) fn initializing_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionScope>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        Ok(state
            .initializing_sessions
            .iter()
            .find(|entry| entry.session_id.as_ref() == session_id)
            .map(|entry| entry.scope))
    }

    pub(crate) fn retire_initializing_session(&self, session_id: &str) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        state
            .initializing_sessions
            .retain(|entry| entry.session_id.as_ref() != session_id);
        Ok(())
    }

    pub(crate) fn remember_service_worker_session(
        &self,
        session_id: &str,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        remember_service_worker_session_in_state(&mut state, session_id, context, ownership)
    }

    fn retire_service_worker_session(&self, session_id: &str) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        state
            .service_worker_sessions
            .retain(|entry| entry.session_id.as_ref() != session_id);
        Ok(())
    }

    fn service_worker_session(&self, session_id: &str) -> Result<Option<SessionScope>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        Ok(state
            .service_worker_sessions
            .iter()
            .find(|entry| entry.session_id.as_ref() == session_id)
            .map(|entry| entry.scope))
    }

    #[cfg(not(feature = "page-routes"))]
    fn service_worker_sessions(&self, context: ObjectId) -> Result<Vec<String>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let matching = state
            .service_worker_sessions
            .iter()
            .filter(|entry| entry.scope.context == context);
        let mut sessions = Vec::new();
        sessions
            .try_reserve_exact(matching.clone().count())
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        sessions.extend(matching.map(|entry| entry.session_id.to_string()));
        Ok(sessions)
    }

    #[cfg(not(feature = "page-routes"))]
    async fn update_session(&self, session_id: &str, enabled: bool) -> Result<(), HostError> {
        if !enabled {
            self.connection
                .send_command_with_session("Fetch.disable", json!({}), Some(session_id))
                .await
                .map_err(HostError::backend)?;
            return Ok(());
        }
        let disable_cache = self.connection.send_command_with_session(
            "Network.setCacheDisabled",
            json!({ "cacheDisabled": true }),
            Some(session_id),
        );
        let enable_fetch = self.connection.send_command_with_session(
            "Fetch.enable",
            json!({
                "handleAuthRequests": true,
                "patterns": [{ "urlPattern": "*", "requestStage": "Request" }]
            }),
            Some(session_id),
        );
        tokio::try_join!(disable_cache, enable_fetch).map_err(HostError::backend)?;
        Ok(())
    }

    fn reconcile_initializing_sessions(&self, topology: &CdpTopology) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        state.initializing_sessions.retain(|entry| {
            topology
                .target_session_route(entry.session_id.as_ref())
                .is_none()
        });
        Ok(())
    }

    fn authenticate(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
        session_id: &str,
        request_id: &str,
        url: &str,
    ) -> Result<(), HostError> {
        validate_native_id("sessionId", session_id)?;
        validate_native_id("requestId", request_id)?;
        let credentials = self.credentials(context, ownership)?;
        let matching = credentials.filter(|credentials| credentials.matches_url(url));
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let kind = authentication_kind(&mut state, matching, session_id, request_id)?;
        enqueue_action_state(
            &mut state,
            PendingAction {
                session_id: session_id.to_owned().into_boxed_str(),
                request_id: request_id.to_owned().into_boxed_str(),
                kind,
            },
        )
    }

    pub(crate) async fn finish_next(&self) -> Result<(), HostError> {
        let action = self
            .state
            .lock()
            .map_err(HostError::backend)?
            .pending
            .pop_front();
        let Some(action) = action else {
            return Ok(());
        };
        let result = match action.kind {
            PendingActionKind::ContinueRequest => {
                self.connection
                    .send_command_with_session(
                        "Fetch.continueRequest",
                        json!({ "requestId": action.request_id }),
                        Some(&action.session_id),
                    )
                    .await
            }
            PendingActionKind::ContinueWithAuth {
                response,
                username,
                password,
            } => {
                let auth_challenge_response =
                    auth_challenge_response(response, username.as_deref(), password.as_deref())?;
                self.connection
                    .send_command_with_session(
                        "Fetch.continueWithAuth",
                        json!({
                            "requestId": action.request_id,
                            "authChallengeResponse": auth_challenge_response
                        }),
                        Some(&action.session_id),
                    )
                    .await
            }
        };
        match result {
            Ok(_) => Ok(()),
            Err(error) if terminal_action_error(&error) => Ok(()),
            Err(error) => Err(HostError::backend(error)),
        }
    }

    pub(crate) fn close(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.closed = true;
        state.default_credentials = None;
        state.entries.clear();
        state.pending.clear();
        state.attempted.clear();
        state.requests.clear();
        state.initializing_sessions.clear();
        state.service_worker_sessions.clear();
    }

    pub(crate) fn observe_event(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
    ) -> FrameObservationResult {
        if event.method == "Target.detachedFromTarget" {
            if let Some(session_id) = event
                .params
                .as_object()
                .and_then(|params| params.get("sessionId"))
                .and_then(Value::as_str)
            {
                self.retire_initializing_session(session_id)?;
                self.retire_service_worker_session(session_id)?;
            }
            return Ok(());
        }
        if !matches!(
            event.method.as_str(),
            "Fetch.authRequired"
                | "Fetch.requestPaused"
                | "Network.loadingFinished"
                | "Network.loadingFailed"
        ) {
            return Ok(());
        }
        let session_id = event
            .session_id
            .as_deref()
            .ok_or_else(|| invalid("sessionId"))?;
        let params = event.params.as_object().ok_or_else(|| invalid("params"))?;
        let request_id = params
            .get("requestId")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("requestId"))?;
        if matches!(
            event.method.as_str(),
            "Network.loadingFinished" | "Network.loadingFailed"
        ) {
            self.retire_request(session_id, request_id)?;
            return Ok(());
        }
        if event.method == "Fetch.requestPaused" {
            let network_id = params.get("networkId").and_then(Value::as_str);
            self.observe_request(session_id, request_id, network_id)?;
            #[cfg(not(feature = "page-routes"))]
            self.continue_request(session_id, request_id, network_id)?;
            return Ok(());
        }
        let scope = if let Some(target) = topology.target_session_route(session_id) {
            let page = topology
                .page_object_id(target.page_target_id())
                .ok_or_else(|| invalid("Page route"))?;
            let context = topology
                .page_session_routes()
                .find(|route| route.object_id() == page)
                .map(|route| route.browser_context_object_id())
                .ok_or_else(|| invalid("BrowserContext route"))?;
            let ownership = topology
                .browser_context_routes()
                .find(|route| route.object_id() == context)
                .map(|route| route.ownership())
                .ok_or_else(|| invalid("BrowserContext ownership"))?;
            SessionScope {
                page: Some(page),
                context,
                ownership,
            }
        } else {
            self.initializing_session(session_id)?
                .or(self.service_worker_session(session_id)?)
                .ok_or_else(|| invalid("target route"))?
        };
        let url = params
            .get("request")
            .and_then(Value::as_object)
            .and_then(|request| request.get("url"))
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("request.url"))?;
        self.authenticate(scope.context, scope.ownership, session_id, request_id, url)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
    }
}

fn set_credentials(
    state: &mut State,
    context: ObjectId,
    ownership: CdpContextOwnership,
    credentials: Option<ContextHttpCredentials>,
) -> Result<(), HostError> {
    if ownership == CdpContextOwnership::Default {
        state.default_credentials = credentials;
        return Ok(());
    }
    let existing = state
        .entries
        .iter()
        .position(|entry| entry.context == context);
    match (existing, credentials) {
        (Some(index), Some(credentials)) => state.entries[index].credentials = credentials,
        (Some(index), None) => {
            state.entries.swap_remove(index);
        }
        (None, Some(credentials)) => {
            state
                .entries
                .try_reserve(1)
                .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
            state.entries.push(ContextEntry {
                context,
                credentials,
            });
        }
        (None, None) => {}
    }
    Ok(())
}

fn enqueue_action_state(state: &mut State, action: PendingAction) -> Result<(), HostError> {
    state
        .pending
        .try_reserve(1)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
    state.pending.push_back(action);
    Ok(())
}

fn observe_request_state(
    state: &mut State,
    session_id: &str,
    request_id: &str,
    network_id: &str,
) -> Result<(), HostError> {
    if let Some(mapping) = state.requests.iter_mut().find(|mapping| {
        mapping.session_id.as_ref() == session_id && mapping.request_id.as_ref() == request_id
    }) {
        mapping.network_id = network_id.to_owned().into_boxed_str();
        return Ok(());
    }
    state
        .requests
        .try_reserve(1)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
    state.requests.push(RequestMapping {
        session_id: session_id.to_owned().into_boxed_str(),
        request_id: request_id.to_owned().into_boxed_str(),
        network_id: network_id.to_owned().into_boxed_str(),
    });
    Ok(())
}

fn remember_initializing_session_in_state(
    state: &mut State,
    session_id: &str,
    scope: SessionScope,
) -> Result<(), HostError> {
    if let Some(entry) = state
        .initializing_sessions
        .iter_mut()
        .find(|entry| entry.session_id.as_ref() == session_id)
    {
        entry.scope = scope;
        return Ok(());
    }
    state
        .initializing_sessions
        .try_reserve(1)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
    state.initializing_sessions.push(InitializingSession {
        session_id: session_id.to_owned().into_boxed_str(),
        scope,
    });
    Ok(())
}

#[cfg(not(feature = "page-routes"))]
pub(crate) struct EventDriver {
    installation: Arc<Installation>,
    topology: SharedChromiumTopology,
    receiver: CdpEventReceiver,
}

#[cfg(not(feature = "page-routes"))]
impl EventDriver {
    pub(crate) fn new(
        installation: Arc<Installation>,
        topology: SharedChromiumTopology,
        connection: &CdpConnection,
    ) -> Result<Self, HostError> {
        Ok(Self {
            installation,
            topology,
            receiver: connection
                .subscribe_composable_events()
                .map_err(HostError::backend)?,
        })
    }

    pub(crate) async fn run(
        &mut self,
        stopped: &tokio_util::sync::CancellationToken,
    ) -> Result<(), HostError> {
        loop {
            let event = tokio::select! {
                biased;
                () = stopped.cancelled() => return Ok(()),
                event = self.receiver.recv() => event,
            };
            let event = match event {
                Ok(event) => event,
                Err(CdpEventRecvError::Closed) => return Err(HostError::TargetClosed),
                Err(CdpEventRecvError::Lagged(skipped)) => {
                    return Err(HostError::Backend(format!(
                        "HTTP authentication event lane lagged by {skipped} events"
                    )));
                }
            };
            {
                let topology = self.topology.lock().map_err(HostError::backend)?;
                self.installation
                    .observe_event(topology.topology(), &event)
                    .map_err(HostError::backend)?;
            }
            self.installation.finish_next().await?;
        }
    }
}

fn auth_challenge_response(
    response: &'static str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Value, HostError> {
    match (username, password) {
        (Some(username), Some(password)) => Ok(json!({
            "response": response,
            "username": username,
            "password": password
        })),
        (None, None) => Ok(json!({ "response": response })),
        _ => Err(HostError::Backend(
            "HTTP authentication action has incomplete credentials".into(),
        )),
    }
}

fn authentication_kind(
    state: &mut State,
    matching: Option<ContextHttpCredentials>,
    session_id: &str,
    request_id: &str,
) -> Result<PendingActionKind, HostError> {
    let attempted = state.attempted.iter().position(|attempt| {
        attempt.session_id.as_ref() == session_id && attempt.request_id.as_ref() == request_id
    });
    if let Some(index) = attempted {
        state.attempted.swap_remove(index);
        return Ok(PendingActionKind::ContinueWithAuth {
            response: "CancelAuth",
            username: None,
            password: None,
        });
    }
    let Some(credentials) = matching else {
        return Ok(PendingActionKind::ContinueWithAuth {
            response: "Default",
            username: None,
            password: None,
        });
    };
    state
        .attempted
        .try_reserve(1)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
    state.attempted.push(AttemptedAuthentication {
        session_id: session_id.to_owned().into_boxed_str(),
        request_id: request_id.to_owned().into_boxed_str(),
        network_id: state
            .requests
            .iter()
            .find(|mapping| {
                mapping.session_id.as_ref() == session_id
                    && mapping.request_id.as_ref() == request_id
            })
            .map(|mapping| mapping.network_id.clone()),
    });
    Ok(PendingActionKind::ContinueWithAuth {
        response: "ProvideCredentials",
        username: Some(credentials.username),
        password: Some(credentials.password),
    })
}

fn retire_request_state(state: &mut State, session_id: &str, network_id: &str) {
    state.attempted.retain(|attempt| {
        attempt.session_id.as_ref() != session_id
            || attempt.network_id.as_deref() != Some(network_id)
    });
    state.requests.retain(|mapping| {
        mapping.session_id.as_ref() != session_id || mapping.network_id.as_ref() != network_id
    });
}

fn validate_native_id(name: &str, value: &str) -> Result<(), HostError> {
    if value.is_empty() || value.len() > MAX_NATIVE_ID_BYTES {
        return Err(HostError::ResourceExhausted(format!(
            "HTTP authentication {name} must contain 1..={MAX_NATIVE_ID_BYTES} UTF-8 bytes"
        )));
    }
    Ok(())
}

fn terminal_action_error(error: &CdpError) -> bool {
    match error {
        CdpError::Closed | CdpError::SessionDetached | CdpError::PageClosed => true,
        CdpError::Protocol { message, .. } => {
            message.contains("Invalid InterceptionId")
                || message.contains("Target closed")
                || message.contains("Session closed")
                || message.contains("Session with given id not found")
        }
        _ => false,
    }
}

#[cfg(not(feature = "page-routes"))]
pub(crate) struct Initializer<Downstream> {
    downstream: Downstream,
    installation: Arc<Installation>,
}

#[cfg(not(feature = "page-routes"))]
impl<Downstream> Initializer<Downstream> {
    pub(crate) fn new(downstream: Downstream, installation: Arc<Installation>) -> Self {
        Self {
            downstream,
            installation,
        }
    }
}

#[cfg(not(feature = "page-routes"))]
impl<Downstream> ChromiumSessionInitializer for Initializer<Downstream>
where
    Downstream: ChromiumSessionInitializer,
{
    fn initialize<'a>(
        &'a self,
        session_id: &'a str,
        target_id: &'a str,
        ownership: CdpContextOwnership,
        is_page: bool,
    ) -> ChromiumSessionInitializationFuture<'a> {
        self.downstream
            .initialize(session_id, target_id, ownership, is_page)
    }

    fn initialize_scoped<'a>(
        &'a self,
        target: ChromiumSessionInitializationTarget<'a>,
    ) -> ChromiumSessionInitializationFuture<'a> {
        Box::pin(async move {
            let _gate = self.installation.update_gate.lock().await;
            if !self
                .installation
                .enabled(target.context, target.ownership)?
            {
                return self.downstream.initialize_scoped(target).await;
            }
            self.installation.remember_initializing_session(
                target.session_id,
                SessionScope {
                    page: Some(target.page),
                    context: target.context,
                    ownership: target.ownership,
                },
            )?;
            let disable_cache = self.installation.connection.send_command_with_session(
                "Network.setCacheDisabled",
                json!({ "cacheDisabled": true }),
                Some(target.session_id),
            );
            let enable_fetch = self.installation.connection.send_command_with_session(
                "Fetch.enable",
                json!({
                    "handleAuthRequests": true,
                    "patterns": [{ "urlPattern": "*", "requestStage": "Request" }]
                }),
                Some(target.session_id),
            );
            let downstream = self.downstream.initialize_scoped(target);
            let protocol = async {
                tokio::try_join!(disable_cache, enable_fetch)?;
                Ok::<_, Box<dyn Error + Send + Sync>>(())
            };
            tokio::try_join!(downstream, protocol)?;
            Ok(())
        })
    }
}

impl HostSession {
    /// Replace or clear a Context's HTTP credentials, then reconfigure every
    /// current Page/OOPIF and Service Worker interception session. The retained
    /// value is updated first, matching Playwright's target-initialization order.
    pub async fn set_context_http_credentials(
        &self,
        context_id: &str,
        credentials: Option<ContextHttpCredentials>,
    ) -> Result<(), HostError> {
        let (context, _call, _operation) = self.admit_operation(context_id)?;
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .context_http_credentials
                .clone()
        };
        let _gate = tokio::select! {
            biased;
            () = self.stop.cancelled() => return Err(HostError::TargetClosed),
            gate = installation.update_gate.lock() => gate,
        };
        let ownership = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            live.composition
                .topology()
                .map_err(HostError::backend)?
                .topology()
                .browser_context_route(context)
                .map(|route| route.ownership())
                .ok_or(HostError::TargetClosed)?
        };

        installation.set(context, ownership, credentials)?;

        #[cfg(feature = "page-routes")]
        self.refresh_context_fetch_interception(context).await?;

        #[cfg(not(feature = "page-routes"))]
        {
            let sessions = {
                let state = self.state.lock().map_err(HostError::backend)?;
                let live = state.as_ref().ok_or(HostError::TargetClosed)?;
                let topology = live.composition.topology().map_err(HostError::backend)?;
                let topology = topology.topology();
                let page_targets = topology
                    .page_session_routes()
                    .filter(|route| route.browser_context_object_id() == context)
                    .map(|route| route.target_id())
                    .collect::<Vec<_>>();
                let mut sessions = topology
                    .target_session_routes()
                    .filter(|route| page_targets.contains(&route.page_target_id()))
                    .map(|route| route.session_id().to_owned())
                    .collect::<Vec<_>>();
                sessions.extend(installation.service_worker_sessions(context)?);
                sessions
            };
            let enabled = installation.enabled(context, ownership)?;
            for session in sessions {
                installation.update_session(&session, enabled).await?;
            }
        }
        Ok(())
    }
}

pub(crate) struct Observer {
    installation: Arc<Installation>,
    downstream: Arc<dyn ChromiumFrameEventObserver>,
}

impl Observer {
    pub(crate) fn new(
        installation: Arc<Installation>,
        downstream: Arc<dyn ChromiumFrameEventObserver>,
    ) -> Self {
        Self {
            installation,
            downstream,
        }
    }
}

impl ChromiumFrameEventObserver for Observer {
    fn observes_page(&self, page: ObjectId) -> bool {
        self.downstream.observes_page(page)
    }

    fn retains_service_worker_targets(&self) -> bool {
        true
    }

    fn enables_service_worker_network(&self) -> bool {
        true
    }

    fn prepare_service_worker(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
    ) -> FrameObservationResult {
        self.downstream.prepare_service_worker(topology, event)?;
        if let Some((session, context, ownership)) = service_worker_scope(topology, event)? {
            self.installation
                .remember_service_worker_session(&session, context, ownership)?;
        }
        Ok(())
    }

    fn service_worker_initialization_commands(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
        commands: &mut Vec<ServiceWorkerInitializationCommand>,
    ) -> FrameObservationResult {
        self.downstream
            .service_worker_initialization_commands(topology, event, commands)?;
        #[cfg(feature = "page-routes")]
        {
            let _ = (topology, event);
            Ok(())
        }
        #[cfg(not(feature = "page-routes"))]
        {
            let Some((_, context, ownership)) = service_worker_scope(topology, event)? else {
                return Ok(());
            };
            if !self.installation.enabled(context, ownership)? {
                return Ok(());
            }
            commands
                .try_reserve(2)
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?;
            commands.push(ServiceWorkerInitializationCommand::new(
                "Network.setCacheDisabled",
                json!({ "cacheDisabled": true }),
            ));
            commands.push(ServiceWorkerInitializationCommand::new(
                "Fetch.enable",
                json!({
                    "handleAuthRequests": true,
                    "patterns": [{ "urlPattern": "*", "requestStage": "Request" }]
                }),
            ));
            Ok(())
        }
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
        self.downstream.initialize_frame(topology, session, frame)
    }

    fn reconcile(&self, topology: &CdpTopology) -> FrameObservationResult {
        self.downstream.reconcile(topology)?;
        self.installation
            .reconcile_initializing_sessions(topology)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
    }

    fn event(&self, topology: &CdpTopology, event: &CdpEvent) -> FrameObservationResult {
        self.downstream.event(topology, event)
    }

    fn seal(&self) {
        self.installation.close();
        self.downstream.seal();
    }
}

fn remember_service_worker_session_in_state(
    state: &mut State,
    session_id: &str,
    context: ObjectId,
    ownership: CdpContextOwnership,
) -> Result<(), HostError> {
    validate_native_id("sessionId", session_id)?;
    let scope = SessionScope {
        page: None,
        context,
        ownership,
    };
    if let Some(entry) = state
        .service_worker_sessions
        .iter_mut()
        .find(|entry| entry.session_id.as_ref() == session_id)
    {
        entry.scope = scope;
        return Ok(());
    }
    state
        .service_worker_sessions
        .try_reserve(1)
        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
    state.service_worker_sessions.push(ServiceWorkerSession {
        session_id: session_id.to_owned().into_boxed_str(),
        scope,
    });
    Ok(())
}

fn service_worker_scope(
    topology: &CdpTopology,
    event: &CdpEvent,
) -> Result<Option<(String, ObjectId, CdpContextOwnership)>, HostError> {
    let params: target::AttachedToTargetEvent =
        serde_json::from_value(event.params.clone()).map_err(HostError::backend)?;
    if params.target_info.r#type != "service_worker" {
        return Ok(None);
    }
    validate_native_id("service worker sessionId", &params.session_id)?;
    let native_context = params
        .target_info
        .browser_context_id
        .as_deref()
        .unwrap_or("");
    let context = if native_context.is_empty() {
        topology
            .default_context_route()
            .map(|route| route.object_id())
    } else {
        topology.context_object_id(native_context).or_else(|| {
            topology
                .default_context_route()
                .map(|route| route.object_id())
        })
    };
    let Some(context) = context else {
        return Ok(None);
    };
    let Some(route) = topology.browser_context_route(context) else {
        return Ok(None);
    };
    Ok(Some((params.session_id, context, route.ownership())))
}

fn invalid(field: &str) -> Box<dyn Error + Send + Sync> {
    Box::new(HostError::Backend(format!(
        "HTTP authentication event is missing {field}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials(origin: Option<&str>) -> ContextHttpCredentials {
        ContextHttpCredentials::new(
            "user".to_owned(),
            "pass".to_owned(),
            origin.map(str::to_owned),
        )
        .expect("valid credentials")
    }

    fn state() -> State {
        State {
            default_credentials: None,
            entries: Vec::new(),
            pending: VecDeque::new(),
            attempted: Vec::new(),
            requests: Vec::new(),
            initializing_sessions: Vec::new(),
            service_worker_sessions: Vec::new(),
            closed: false,
        }
    }

    #[test]
    fn origin_matching_is_exactly_scoped() {
        // Arrange
        let credentials = credentials(Some("http://example.test"));

        // Act
        let matching = credentials.matches_url("http://example.test/private");
        let nonmatching = credentials.matches_url("http://other.test/private");

        // Assert
        assert!(matching);
        assert!(!nonmatching);
    }

    #[test]
    fn request_send_policy_remains_explicit() {
        // Arrange
        let credentials = ContextHttpCredentials::new_with_send(
            "user".to_owned(),
            "pass".to_owned(),
            Some("https://example.test".to_owned()),
            true,
        )
        .expect("valid credentials");

        // Act
        let observed = (
            credentials.username(),
            credentials.password(),
            credentials.origin(),
            credentials.send_always(),
        );

        // Assert
        assert_eq!(
            observed,
            ("user", "pass", Some("https://example.test"), true,)
        );
    }

    #[test]
    fn runtime_credentials_replace_owned_context_value() {
        // Arrange
        let context = ObjectId::new(7);
        let mut state = state();
        state.entries.push(ContextEntry {
            context,
            credentials: ContextHttpCredentials::new(
                "old-user".to_owned(),
                "old-pass".to_owned(),
                None,
            )
            .expect("valid old credentials"),
        });
        let replacement =
            ContextHttpCredentials::new("new-user".to_owned(), "new-pass".to_owned(), None)
                .expect("valid replacement credentials");

        // Act
        set_credentials(
            &mut state,
            context,
            CdpContextOwnership::Owned,
            Some(replacement.clone()),
        )
        .expect("replace credentials");

        // Assert
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].credentials, replacement);
    }

    #[test]
    fn runtime_credentials_support_external_contexts() {
        // Arrange
        let context = ObjectId::new(11);
        let mut state = state();
        let credentials = credentials(None);

        // Act
        set_credentials(
            &mut state,
            context,
            CdpContextOwnership::External,
            Some(credentials.clone()),
        )
        .expect("install external credentials");

        // Assert
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].context, context);
        assert_eq!(state.entries[0].credentials, credentials);
    }

    #[test]
    fn runtime_credentials_clear_context_value() {
        // Arrange
        let context = ObjectId::new(13);
        let mut state = state();
        state.entries.push(ContextEntry {
            context,
            credentials: credentials(None),
        });

        // Act
        set_credentials(&mut state, context, CdpContextOwnership::Owned, None)
            .expect("clear credentials");

        // Assert
        assert!(state.entries.is_empty());
    }

    #[test]
    fn runtime_credentials_replace_default_context_value() {
        // Arrange
        let mut state = state();
        state.default_credentials = Some(credentials(None));
        let replacement =
            ContextHttpCredentials::new("new-user".to_owned(), "new-pass".to_owned(), None)
                .expect("valid replacement credentials");

        // Act
        set_credentials(
            &mut state,
            ObjectId::new(17),
            CdpContextOwnership::Default,
            Some(replacement.clone()),
        )
        .expect("replace default credentials");

        // Assert
        assert_eq!(state.default_credentials, Some(replacement));
    }

    #[test]
    fn debug_output_redacts_credential_values() {
        // Arrange
        let credentials = ContextHttpCredentials::new_with_send(
            "private-user".to_owned(),
            "private-password".to_owned(),
            Some("https://example.test".to_owned()),
            true,
        )
        .expect("valid credentials");

        // Act
        let output = format!("{credentials:?}");

        // Assert
        assert!(!output.contains("private-user"));
        assert!(!output.contains("private-password"));
        assert!(output.contains("has_username: true"));
        assert!(output.contains("has_password: true"));
        assert!(output.contains("https://example.test"));
    }

    #[test]
    fn first_matching_challenge_provides_credentials() {
        // Arrange
        let mut state = state();

        // Act
        let kind = authentication_kind(&mut state, Some(credentials(None)), "session", "request")
            .expect("authenticate challenge");

        // Assert
        assert_eq!(
            kind,
            PendingActionKind::ContinueWithAuth {
                response: "ProvideCredentials",
                username: Some("user".into()),
                password: Some("pass".into()),
            }
        );
    }

    #[test]
    fn repeated_challenge_cancels_authentication() {
        // Arrange
        let mut state = state();
        authentication_kind(&mut state, Some(credentials(None)), "session", "request")
            .expect("first challenge");

        // Act
        let kind = authentication_kind(&mut state, Some(credentials(None)), "session", "request")
            .expect("repeated challenge");

        // Assert
        assert_eq!(
            kind,
            PendingActionKind::ContinueWithAuth {
                response: "CancelAuth",
                username: None,
                password: None,
            }
        );
    }

    #[test]
    fn cancel_auth_payload_omits_optional_credential_fields() {
        // Arrange
        let response = "CancelAuth";

        // Act
        let payload = auth_challenge_response(response, None, None)
            .expect("credential-free response should serialize");

        // Assert
        assert_eq!(payload, json!({ "response": "CancelAuth" }));
    }

    #[test]
    fn origin_mismatch_defers_to_chromium() {
        // Arrange
        let mut state = state();

        // Act
        let kind = authentication_kind(&mut state, None, "session", "request")
            .expect("mismatched challenge");

        // Assert
        assert_eq!(
            kind,
            PendingActionKind::ContinueWithAuth {
                response: "Default",
                username: None,
                password: None,
            }
        );
    }

    #[test]
    fn completed_network_request_releases_authentication_attempt() {
        // Arrange
        let mut state = state();
        state.requests.push(RequestMapping {
            session_id: "session".into(),
            request_id: "fetch-request".into(),
            network_id: "network-request".into(),
        });
        authentication_kind(
            &mut state,
            Some(credentials(None)),
            "session",
            "fetch-request",
        )
        .expect("first challenge");

        // Act
        retire_request_state(&mut state, "session", "network-request");

        // Assert
        assert!(state.attempted.is_empty());
        assert!(state.requests.is_empty());
    }

    #[test]
    fn authentication_state_grows_past_the_retired_fixed_capacity() {
        // Arrange
        const RETIRED_FIXED_CAPACITY: usize = 64;
        let mut state = state();
        let scope = SessionScope {
            page: Some(ObjectId::new(8)),
            context: ObjectId::new(7),
            ownership: CdpContextOwnership::Owned,
        };

        // Act
        for index in 0..=RETIRED_FIXED_CAPACITY {
            let session_id = format!("session-{index}");
            let request_id = format!("request-{index}");
            let network_id = format!("network-{index}");
            observe_request_state(&mut state, &session_id, &request_id, &network_id)
                .expect("remember request mapping");
            let kind = authentication_kind(
                &mut state,
                Some(credentials(None)),
                &session_id,
                &request_id,
            )
            .expect("remember authentication attempt");
            enqueue_action_state(
                &mut state,
                PendingAction {
                    session_id: session_id.clone().into_boxed_str(),
                    request_id: request_id.into_boxed_str(),
                    kind,
                },
            )
            .expect("enqueue authentication action");
            remember_initializing_session_in_state(&mut state, &session_id, scope)
                .expect("remember initializing session");
        }

        // Assert
        assert_eq!(state.requests.len(), RETIRED_FIXED_CAPACITY + 1);
        assert_eq!(state.attempted.len(), RETIRED_FIXED_CAPACITY + 1);
        assert_eq!(state.pending.len(), RETIRED_FIXED_CAPACITY + 1);
        assert_eq!(
            state.initializing_sessions.len(),
            RETIRED_FIXED_CAPACITY + 1
        );
    }

    #[test]
    fn service_worker_registry_is_allocator_backed_past_initialization_capacity() {
        // Arrange
        const RETIRED_FIXED_CAPACITY: usize = 64;
        let mut state = state();
        let context = ObjectId::new(7);

        // Act
        for index in 0..=RETIRED_FIXED_CAPACITY {
            remember_service_worker_session_in_state(
                &mut state,
                &format!("worker-{index}"),
                context,
                CdpContextOwnership::Owned,
            )
            .expect("remember live Service Worker");
        }

        // Assert
        assert_eq!(
            state.service_worker_sessions.len(),
            RETIRED_FIXED_CAPACITY + 1
        );
        assert_eq!(
            state
                .service_worker_sessions
                .last()
                .map(|entry| entry.scope),
            Some(SessionScope {
                page: None,
                context,
                ownership: CdpContextOwnership::Owned,
            })
        );
    }
}
