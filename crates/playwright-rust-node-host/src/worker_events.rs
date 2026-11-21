//! Optional bounded dedicated- and service-worker publication and evaluation.
//!
//! Chromium child-session ownership, readiness, Page correlation and value
//! evaluation stay in Rust. Language bindings receive retained Worker owners;
//! they never subscribe to CDP or evaluate through JavaScript glue.

#[cfg(feature = "worker-network-events")]
use std::collections::BTreeSet;
use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_evaluation::{CdpEvaluationLimits, CdpEvaluationService};
use playwright_rust_backend_cdp_topology::{CdpContextOwnership, CdpTopology};
use playwright_rust_cdp::generated::{page, runtime, target};
use playwright_rust_cdp::{
    CdpClient, CdpEvent, ServiceWorkerInitializationCommand, SessionFactory,
};
#[cfg(feature = "worker-network-events")]
use playwright_rust_composition_chromium::ChromiumNetworkPageRegistrationHandle;
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope, Service,
};
use playwright_rust_plugin_chromium_frame_observer::{
    ChromiumFrameEventObserver, FrameObservationResult,
};
use tokio::sync::Notify;

use crate::evaluation::{
    ERROR_BYTES, FFI_ALLOWANCE, GENERATED_SCRIPT_BYTES, SOURCE_BYTES, value_limits,
};
use crate::{HostError, HostSession, context_metadata::ContextMetadata, frame_trees::FrameTrees};

pub const MAX_WORKER_NOTICES: usize = 128;
pub const MAX_WORKER_URL_BYTES: usize = 1024 * 1024;
pub const MAX_WORKER_ID_BYTES: usize = 256;
pub const MAX_CONCURRENT_WORKER_EVALUATIONS: usize = 8;
const INSTALLATION_BYTES: u64 = 4096;
const INSTALLATION_SCOPE: ResourceScope = ResourceScope::extension(182, 1);
const EVALUATION_SCOPE: ResourceScope = ResourceScope::extension(182, 2);

#[cfg(feature = "worker-network-events")]
trait ServiceWorkerNetworkRegistration: Send + Sync {
    fn register(&self, context: ObjectId, session_id: &str) -> Result<(), HostError>;
}

#[cfg(feature = "worker-network-events")]
impl ServiceWorkerNetworkRegistration for ChromiumNetworkPageRegistrationHandle {
    fn register(&self, context: ObjectId, session_id: &str) -> Result<(), HostError> {
        self.register_service_worker(context, session_id)
            .map(|_| ())
            .map_err(HostError::backend)
    }
}

pub(crate) const fn requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            INSTALLATION_BYTES + MAX_CONCURRENT_WORKER_EVALUATIONS as u64 * FFI_ALLOWANCE,
            MAX_CONCURRENT_WORKER_EVALUATIONS as u64 * FFI_ALLOWANCE,
        )
        .with_class(
            ResourceClass::EventItem,
            MAX_WORKER_NOTICES as u64,
            MAX_WORKER_NOTICES as u64,
        )
        .with_metadata(
            2,
            MAX_WORKER_NOTICES + MAX_CONCURRENT_WORKER_EVALUATIONS + 1,
        )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerNoticeKind {
    Created,
    Closed,
}

pub struct WorkerNotice {
    pub kind: WorkerNoticeKind,
    pub page: ObjectId,
    pub owner: Arc<WorkerOwner>,
    _notice: ResourceLease,
}

pub struct ServiceWorkerNotice {
    pub kind: WorkerNoticeKind,
    pub context: ObjectId,
    pub owner: Arc<WorkerOwner>,
    _notice: ResourceLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerOwnerKind {
    Dedicated,
    Service,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkerReadiness {
    unique_context_id: Option<Box<str>>,
    script_loaded: bool,
    closed: bool,
}

pub struct WorkerOwner {
    kind: WorkerOwnerKind,
    page: Option<ObjectId>,
    context: ObjectId,
    context_owned: bool,
    sequence: u64,
    session_id: Box<str>,
    target_id: Box<str>,
    url: Box<str>,
    client: Arc<dyn CdpClient>,
    readiness: Mutex<WorkerReadiness>,
    notify: Notify,
}

impl std::fmt::Debug for WorkerOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerOwner")
            .field("page", &self.page)
            .field("session_id", &self.session_id)
            .field("target_id", &self.target_id)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl WorkerOwner {
    pub fn page(&self) -> Option<ObjectId> {
        self.page
    }

    pub fn context(&self) -> ObjectId {
        self.context
    }

    pub fn is_service_worker(&self) -> bool {
        self.kind == WorkerOwnerKind::Service
    }

    pub fn id(&self) -> &str {
        &self.session_id
    }

    #[cfg(feature = "extra-headers")]
    pub(crate) fn target_id(&self) -> &str {
        &self.target_id
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    #[cfg(any(
        feature = "worker-console-events",
        feature = "worker-handle-evaluation",
        feature = "context-offline"
    ))]
    pub(crate) fn client(&self) -> Arc<dyn CdpClient> {
        Arc::clone(&self.client)
    }

    #[cfg(all(
        test,
        any(
            feature = "worker-console-events",
            feature = "worker-handle-evaluation",
            feature = "context-offline"
        )
    ))]
    pub(crate) fn for_test(page: ObjectId, client: Arc<dyn CdpClient>) -> Arc<Self> {
        Arc::new(Self {
            kind: WorkerOwnerKind::Dedicated,
            page: Some(page),
            context: page,
            context_owned: true,
            sequence: 0,
            session_id: "worker-test".into(),
            target_id: "worker-target-test".into(),
            url: "https://example.test/worker.js".into(),
            client,
            readiness: Mutex::new(WorkerReadiness {
                unique_context_id: Some("worker-test-world".into()),
                script_loaded: true,
                closed: false,
            }),
            notify: Notify::new(),
        })
    }

    #[cfg(all(
        test,
        any(
            feature = "worker-console-events",
            feature = "worker-handle-evaluation",
            feature = "context-offline"
        )
    ))]
    pub(crate) fn for_service_worker_test(
        context: ObjectId,
        client: Arc<dyn CdpClient>,
    ) -> Arc<Self> {
        Arc::new(Self {
            kind: WorkerOwnerKind::Service,
            page: None,
            context,
            context_owned: true,
            sequence: 0,
            session_id: "service-worker-test".into(),
            target_id: "service-worker-target-test".into(),
            url: "https://example.test/scope/sw.js".into(),
            client,
            readiness: Mutex::new(WorkerReadiness {
                unique_context_id: Some("service-worker-test-world".into()),
                script_loaded: true,
                closed: false,
            }),
            notify: Notify::new(),
        })
    }

    fn mark_execution_context(&self, unique_context_id: String) -> Result<(), HostError> {
        bounded(
            &unique_context_id,
            MAX_WORKER_ID_BYTES,
            "Worker execution context ID",
        )?;
        self.readiness
            .lock()
            .map_err(HostError::backend)?
            .unique_context_id = Some(unique_context_id.into_boxed_str());
        self.notify.notify_waiters();
        Ok(())
    }

    fn mark_script_loaded(&self) -> Result<(), HostError> {
        self.readiness
            .lock()
            .map_err(HostError::backend)?
            .script_loaded = true;
        self.notify.notify_waiters();
        Ok(())
    }

    pub(crate) fn close(&self) -> Result<(), HostError> {
        self.readiness.lock().map_err(HostError::backend)?.closed = true;
        self.notify.notify_waiters();
        Ok(())
    }

    pub(crate) async fn wait_until_ready(&self) -> Result<(), HostError> {
        loop {
            let notified = self.notify.notified();
            let readiness = self.readiness.lock().map_err(HostError::backend)?.clone();
            if readiness.closed {
                return Err(HostError::TargetClosed);
            }
            if readiness.unique_context_id.is_some()
                && (self.kind == WorkerOwnerKind::Service || readiness.script_loaded)
            {
                return Ok(());
            }
            notified.await;
        }
    }

    #[cfg(feature = "worker-handle-evaluation")]
    pub(crate) async fn ready_unique_context_id(&self) -> Result<Box<str>, HostError> {
        self.wait_until_ready().await?;
        let readiness = self.readiness.lock().map_err(HostError::backend)?;
        if readiness.closed {
            return Err(HostError::TargetClosed);
        }
        readiness
            .unique_context_id
            .clone()
            .ok_or_else(|| HostError::Backend("Worker execution context is not ready".into()))
    }

    #[cfg(feature = "worker-handle-evaluation")]
    pub(crate) fn is_closed(&self) -> Result<bool, HostError> {
        Ok(self.readiness.lock().map_err(HostError::backend)?.closed)
    }
}

struct EvaluationSlot<'a> {
    active: &'a AtomicUsize,
    _resource: ResourceLease,
}

impl Drop for EvaluationSlot<'_> {
    fn drop(&mut self) {
        let previous = self.active.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0);
    }
}

struct State {
    workers: BTreeMap<Box<str>, Arc<WorkerOwner>>,
    notices: VecDeque<WorkerNotice>,
    service_worker_notices: VecDeque<ServiceWorkerNotice>,
    next_sequence: u64,
    closed: bool,
}

pub(crate) struct Installation {
    resources: ProductionResourceLedger,
    sessions: Arc<dyn SessionFactory>,
    context_metadata: ContextMetadata,
    frame_trees: Arc<FrameTrees>,
    state: Mutex<State>,
    notify: Notify,
    service_worker_notify: Notify,
    active_evaluations: AtomicUsize,
    #[cfg(feature = "worker-network-events")]
    network_registration: Mutex<Option<Arc<dyn ServiceWorkerNetworkRegistration>>>,
    #[cfg(feature = "worker-network-events")]
    prepared_service_worker_network: Mutex<BTreeSet<Box<str>>>,
    #[cfg(all(feature = "service-worker-events", feature = "context-offline"))]
    context_offline: Mutex<Option<Arc<crate::context_offline::Installation>>>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new(
        resources: &ProductionResourceLedger,
        sessions: Arc<dyn SessionFactory>,
        context_metadata: ContextMetadata,
        frame_trees: Arc<FrameTrees>,
    ) -> Result<Arc<Self>, HostError> {
        let storage = resources
            .claim(requirements())
            .map_err(HostError::backend)?
            .reserve(
                INSTALLATION_SCOPE,
                ResourceClass::BackendStateBytes,
                INSTALLATION_BYTES,
            )
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self {
            resources: resources.clone(),
            sessions,
            context_metadata,
            frame_trees,
            state: Mutex::new(State {
                workers: BTreeMap::new(),
                notices: VecDeque::new(),
                service_worker_notices: VecDeque::new(),
                next_sequence: 0,
                closed: false,
            }),
            notify: Notify::new(),
            service_worker_notify: Notify::new(),
            active_evaluations: AtomicUsize::new(0),
            #[cfg(feature = "worker-network-events")]
            network_registration: Mutex::new(None),
            #[cfg(feature = "worker-network-events")]
            prepared_service_worker_network: Mutex::new(BTreeSet::new()),
            #[cfg(all(feature = "service-worker-events", feature = "context-offline"))]
            context_offline: Mutex::new(None),
            _storage: storage,
        }))
    }

    #[cfg(all(feature = "service-worker-events", feature = "context-offline"))]
    pub(crate) fn set_context_offline(
        &self,
        installation: Arc<crate::context_offline::Installation>,
    ) -> Result<(), HostError> {
        let mut selected = self.context_offline.lock().map_err(HostError::backend)?;
        if selected.is_some() {
            return Err(HostError::Backend(
                "Service Worker offline policy is already installed".into(),
            ));
        }
        *selected = Some(installation);
        Ok(())
    }

    #[cfg(feature = "worker-network-events")]
    pub(crate) fn set_network_registration(
        &self,
        registration: ChromiumNetworkPageRegistrationHandle,
    ) -> Result<(), HostError> {
        let mut selected = self
            .network_registration
            .lock()
            .map_err(HostError::backend)?;
        if selected.is_some() {
            return Err(HostError::Backend(
                "Service Worker network registration is already installed".into(),
            ));
        }
        *selected = Some(Arc::new(registration));
        Ok(())
    }

    fn notice_lease(&self, page: ObjectId) -> Result<ResourceLease, HostError> {
        self.resources
            .claim(requirements())
            .map_err(HostError::backend)?
            .reserve(ResourceScope::Object(page), ResourceClass::EventItem, 1)
            .map_err(|error| {
                HostError::ResourceExhausted(format!(
                    "Worker notice capacity exceeded ({MAX_WORKER_NOTICES}): {error}"
                ))
            })
    }

    fn context_notice_lease(&self, context: ObjectId) -> Result<ResourceLease, HostError> {
        self.resources
            .claim(requirements())
            .map_err(HostError::backend)?
            .reserve(ResourceScope::Object(context), ResourceClass::EventItem, 1)
            .map_err(|error| {
                HostError::ResourceExhausted(format!(
                    "Service Worker notice capacity exceeded ({MAX_WORKER_NOTICES}): {error}"
                ))
            })
    }

    fn attach(
        &self,
        topology: &CdpTopology,
        event_session: Option<&str>,
        params: target::AttachedToTargetEvent,
    ) -> Result<(), HostError> {
        if params.target_info.r#type == "service_worker" {
            return self.attach_service_worker(topology, event_session, params);
        }
        if params.target_info.r#type != "worker" {
            return Ok(());
        }
        bounded(&params.session_id, MAX_WORKER_ID_BYTES, "Worker session ID")?;
        bounded(
            &params.target_info.target_id,
            MAX_WORKER_ID_BYTES,
            "Worker target ID",
        )?;
        bounded(&params.target_info.url, MAX_WORKER_URL_BYTES, "Worker URL")?;
        let Some(parent_session) = event_session else {
            return Ok(());
        };
        let page = topology
            .page_session_routes()
            .find_map(|route| (route.session_id() == parent_session).then_some(route.object_id()))
            .or_else(|| {
                self.state
                    .lock()
                    .ok()?
                    .workers
                    .get(parent_session)
                    .and_then(|owner| owner.page())
            });
        let Some(page) = page else {
            return Ok(());
        };
        let Some(context) = topology
            .materialized_page_memberships()
            .find_map(|(candidate, context)| (candidate == page).then_some(context))
        else {
            return Ok(());
        };
        let Some(context_route) = topology.browser_context_route(context) else {
            return Ok(());
        };
        let context_owned = context_route.ownership() == CdpContextOwnership::Owned;
        // Chromium's default Page is native but not public in a fresh owned
        // Browser. Discard its Workers before they can consume bounded owner
        // or notice capacity needed by a later public Context.
        if !self.page_visible(page, context, context_owned)? {
            return Ok(());
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed || state.workers.contains_key(params.session_id.as_str()) {
            return Ok(());
        }
        if state.notices.len() >= MAX_WORKER_NOTICES {
            return Err(HostError::ResourceExhausted(format!(
                "Worker notice capacity exceeded ({MAX_WORKER_NOTICES})"
            )));
        }
        let sequence = state.next_sequence;
        state.next_sequence = sequence.checked_add(1).ok_or_else(|| {
            HostError::ResourceExhausted("Worker creation sequence exhausted".into())
        })?;
        let session_id: Box<str> = params.session_id.into_boxed_str();
        let target_id: Box<str> = params.target_info.target_id.into_boxed_str();
        let owner = Arc::new(WorkerOwner {
            kind: WorkerOwnerKind::Dedicated,
            page: Some(page),
            context,
            context_owned,
            sequence,
            client: self
                .sessions
                .create_session(session_id.to_string(), target_id.to_string()),
            session_id: session_id.clone(),
            target_id,
            url: params.target_info.url.into_boxed_str(),
            readiness: Mutex::new(WorkerReadiness {
                unique_context_id: None,
                script_loaded: false,
                closed: false,
            }),
            notify: Notify::new(),
        });
        let notice = WorkerNotice {
            kind: WorkerNoticeKind::Created,
            page,
            owner: Arc::clone(&owner),
            _notice: self.notice_lease(page)?,
        };
        state.workers.insert(session_id, owner);
        state.notices.push_back(notice);
        drop(state);
        self.notify.notify_one();
        Ok(())
    }

    fn attach_service_worker(
        &self,
        topology: &CdpTopology,
        _event_session: Option<&str>,
        params: target::AttachedToTargetEvent,
    ) -> Result<(), HostError> {
        self.validate_service_worker_target(&params)?;
        let Some((context, context_owned)) =
            self.visible_service_worker_context(topology, &params)?
        else {
            return Ok(());
        };
        {
            let state = self.state.lock().map_err(HostError::backend)?;
            if state.closed || state.workers.contains_key(params.session_id.as_str()) {
                return Ok(());
            }
        }
        #[cfg(feature = "worker-network-events")]
        self.claim_service_worker_network(context, &params.session_id)?;
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed || state.workers.contains_key(params.session_id.as_str()) {
            return Ok(());
        }
        if state.service_worker_notices.len() >= MAX_WORKER_NOTICES {
            return Err(HostError::ResourceExhausted(format!(
                "Service Worker notice capacity exceeded ({MAX_WORKER_NOTICES})"
            )));
        }
        let sequence = state.next_sequence;
        state.next_sequence = sequence.checked_add(1).ok_or_else(|| {
            HostError::ResourceExhausted("Worker creation sequence exhausted".into())
        })?;
        let session_id: Box<str> = params.session_id.into_boxed_str();
        let target_id: Box<str> = params.target_info.target_id.into_boxed_str();
        let owner = Arc::new(WorkerOwner {
            kind: WorkerOwnerKind::Service,
            page: None,
            context,
            context_owned,
            sequence,
            client: self
                .sessions
                .create_session(session_id.to_string(), target_id.to_string()),
            session_id: session_id.clone(),
            target_id,
            url: params.target_info.url.into_boxed_str(),
            readiness: Mutex::new(WorkerReadiness {
                unique_context_id: None,
                script_loaded: false,
                closed: false,
            }),
            notify: Notify::new(),
        });
        let notice = ServiceWorkerNotice {
            kind: WorkerNoticeKind::Created,
            context,
            owner: Arc::clone(&owner),
            _notice: self.context_notice_lease(context)?,
        };
        state.workers.insert(session_id, owner);
        state.service_worker_notices.push_back(notice);
        drop(state);
        self.service_worker_notify.notify_one();
        Ok(())
    }

    fn validate_service_worker_target(
        &self,
        params: &target::AttachedToTargetEvent,
    ) -> Result<(), HostError> {
        bounded(
            &params.session_id,
            MAX_WORKER_ID_BYTES,
            "Service Worker session ID",
        )?;
        bounded(
            &params.target_info.target_id,
            MAX_WORKER_ID_BYTES,
            "Service Worker target ID",
        )?;
        bounded(
            &params.target_info.url,
            MAX_WORKER_URL_BYTES,
            "Service Worker URL",
        )?;
        Ok(())
    }

    fn visible_service_worker_context(
        &self,
        topology: &CdpTopology,
        params: &target::AttachedToTargetEvent,
    ) -> Result<Option<(ObjectId, bool)>, HostError> {
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
                // The Node host installs Playwright-style context fallback:
                // an unknown native context is projected through the default
                // context. Chromium may still report a native ID for that
                // default context on service-worker targets even though the
                // portable topology intentionally stores its route under the
                // empty key.
                topology
                    .default_context_route()
                    .map(|route| route.object_id())
            })
        };
        let Some(context) = context else {
            return Ok(None);
        };
        let Some(context_route) = topology.browser_context_route(context) else {
            return Ok(None);
        };
        let context_owned = context_route.ownership() == CdpContextOwnership::Owned;
        if self
            .context_metadata
            .visible(context, context_owned)?
            .is_none()
        {
            return Ok(None);
        }
        Ok(Some((context, context_owned)))
    }

    #[cfg(feature = "worker-network-events")]
    fn register_service_worker_network(
        &self,
        context: ObjectId,
        session_id: &str,
    ) -> Result<(), HostError> {
        self.network_registration
            .lock()
            .map_err(HostError::backend)?
            .as_ref()
            .ok_or_else(|| {
                HostError::Backend("Service Worker network registration is not installed".into())
            })?
            .register(context, session_id)?;
        Ok(())
    }

    #[cfg(feature = "worker-network-events")]
    fn claim_service_worker_network(
        &self,
        context: ObjectId,
        session_id: &str,
    ) -> Result<(), HostError> {
        if self
            .prepared_service_worker_network
            .lock()
            .map_err(HostError::backend)?
            .remove(session_id)
        {
            return Ok(());
        }
        self.register_service_worker_network(context, session_id)
    }

    fn prepare_service_worker_network(
        &self,
        topology: &CdpTopology,
        params: &target::AttachedToTargetEvent,
    ) -> Result<(), HostError> {
        #[cfg(feature = "worker-network-events")]
        {
            self.validate_service_worker_target(params)?;
            if let Some((context, _)) = self.visible_service_worker_context(topology, params)? {
                self.register_service_worker_network(context, &params.session_id)?;
                self.prepared_service_worker_network
                    .lock()
                    .map_err(HostError::backend)?
                    .insert(params.session_id.clone().into_boxed_str());
            }
        }
        #[cfg(not(feature = "worker-network-events"))]
        let _ = (topology, params);
        Ok(())
    }

    #[cfg(all(feature = "service-worker-events", feature = "context-offline"))]
    fn service_worker_offline_commands(
        &self,
        topology: &CdpTopology,
        params: &target::AttachedToTargetEvent,
    ) -> Result<Option<[ServiceWorkerInitializationCommand; 1]>, HostError> {
        self.validate_service_worker_target(params)?;
        let Some((context, _)) = self.visible_service_worker_context(topology, params)? else {
            return Ok(None);
        };
        let ownership = topology
            .browser_context_route(context)
            .ok_or(HostError::TargetClosed)?
            .ownership();
        self.context_offline
            .lock()
            .map_err(HostError::backend)?
            .as_ref()
            .ok_or_else(|| {
                HostError::Backend("Service Worker offline policy is not installed".into())
            })?
            .service_worker_initialization_commands(context, ownership)
    }

    fn close_worker(&self, session_id: &str) -> Result<(), HostError> {
        #[cfg(feature = "worker-network-events")]
        self.prepared_service_worker_network
            .lock()
            .map_err(HostError::backend)?
            .remove(session_id);
        let owner = {
            let mut state = self.state.lock().map_err(HostError::backend)?;
            let Some(owner) = state.workers.remove(session_id) else {
                return Ok(());
            };
            owner
        };
        owner.close()?;
        if !self.notice_visible(&owner)? {
            return Ok(());
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if !state.closed {
            if let Some(page) = owner.page() {
                if state.notices.len() >= MAX_WORKER_NOTICES {
                    return Err(HostError::ResourceExhausted(format!(
                        "Worker notice capacity exceeded ({MAX_WORKER_NOTICES})"
                    )));
                }
                state.notices.push_back(WorkerNotice {
                    kind: WorkerNoticeKind::Closed,
                    page,
                    _notice: self.notice_lease(page)?,
                    owner,
                });
                drop(state);
                self.notify.notify_one();
            } else {
                if state.service_worker_notices.len() >= MAX_WORKER_NOTICES {
                    return Err(HostError::ResourceExhausted(format!(
                        "Service Worker notice capacity exceeded ({MAX_WORKER_NOTICES})"
                    )));
                }
                let context = owner.context();
                state.service_worker_notices.push_back(ServiceWorkerNotice {
                    kind: WorkerNoticeKind::Closed,
                    context,
                    _notice: self.context_notice_lease(context)?,
                    owner,
                });
                drop(state);
                self.service_worker_notify.notify_one();
            }
        }
        Ok(())
    }

    fn mark_execution_context(
        &self,
        session_id: &str,
        unique_context_id: String,
    ) -> Result<(), HostError> {
        if let Some(owner) = self
            .state
            .lock()
            .map_err(HostError::backend)?
            .workers
            .get(session_id)
            .cloned()
        {
            owner.mark_execution_context(unique_context_id)?;
        }
        Ok(())
    }

    fn mark_script_loaded(&self, session_id: &str) -> Result<(), HostError> {
        if let Some(owner) = self
            .state
            .lock()
            .map_err(HostError::backend)?
            .workers
            .get(session_id)
            .cloned()
        {
            owner.mark_script_loaded()?;
        }
        Ok(())
    }

    fn contains(&self, session_id: &str) -> bool {
        self.state
            .lock()
            .map(|state| state.workers.contains_key(session_id))
            .unwrap_or(false)
    }

    #[cfg(any(feature = "worker-console-events", feature = "worker-error-events"))]
    pub(crate) fn owner(&self, session_id: &str) -> Result<Option<Arc<WorkerOwner>>, HostError> {
        Ok(self
            .state
            .lock()
            .map_err(HostError::backend)?
            .workers
            .get(session_id)
            .cloned())
    }

    pub(crate) fn snapshot(&self, page: ObjectId) -> Result<Vec<Arc<WorkerOwner>>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let mut workers = state
            .workers
            .values()
            .filter(|owner| owner.page() == Some(page))
            .cloned()
            .collect::<Vec<_>>();
        workers.sort_unstable_by_key(|owner| owner.sequence);
        Ok(workers)
    }

    pub(crate) fn service_worker_snapshot(
        &self,
        context: ObjectId,
    ) -> Result<Vec<Arc<WorkerOwner>>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let mut workers = state
            .workers
            .values()
            .filter(|owner| owner.is_service_worker() && owner.context() == context)
            .cloned()
            .collect::<Vec<_>>();
        workers.sort_unstable_by_key(|owner| owner.sequence);
        Ok(workers)
    }

    #[cfg(feature = "extra-headers")]
    pub(crate) fn contains_service_worker(&self, session_id: &str) -> Result<bool, HostError> {
        Ok(self
            .state
            .lock()
            .map_err(HostError::backend)?
            .workers
            .get(session_id)
            .is_some_and(|owner| owner.is_service_worker()))
    }

    fn page_visible(
        &self,
        page: ObjectId,
        context: ObjectId,
        context_owned: bool,
    ) -> Result<bool, HostError> {
        if self
            .context_metadata
            .visible(context, context_owned)?
            .is_none()
        {
            return Ok(false);
        }
        // Fresh owned launches expose only committed public Pages. Native
        // storage-state scratch Pages may share a visible owned Context.
        // Attached and persistent CDP sessions retain their existing native
        // Page semantics, including the default Context.
        if self.context_metadata.owned_contexts_only() {
            return self.frame_trees.is_published_public_page(page);
        }
        Ok(true)
    }

    pub(crate) fn notice_visible(&self, owner: &WorkerOwner) -> Result<bool, HostError> {
        match owner.page {
            Some(page) => self.page_visible(page, owner.context, owner.context_owned),
            None => Ok(self
                .context_metadata
                .visible(owner.context, owner.context_owned)?
                .is_some()),
        }
    }

    pub(crate) fn require_current_owner(&self, owner: &Arc<WorkerOwner>) -> Result<(), HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed
            || !state
                .workers
                .get(owner.id())
                .is_some_and(|current| Arc::ptr_eq(current, owner))
        {
            return Err(HostError::TargetClosed);
        }
        Ok(())
    }

    pub(crate) async fn next_notice(&self) -> Result<WorkerNotice, HostError> {
        loop {
            let notified = self.notify.notified();
            {
                let mut state = self.state.lock().map_err(HostError::backend)?;
                if let Some(notice) = state.notices.pop_front() {
                    return Ok(notice);
                }
                if state.closed {
                    return Err(HostError::TargetClosed);
                }
            }
            notified.await;
        }
    }

    pub(crate) async fn next_service_worker_notice(
        &self,
    ) -> Result<ServiceWorkerNotice, HostError> {
        loop {
            let notified = self.service_worker_notify.notified();
            {
                let mut state = self.state.lock().map_err(HostError::backend)?;
                if let Some(notice) = state.service_worker_notices.pop_front() {
                    return Ok(notice);
                }
                if state.closed {
                    return Err(HostError::TargetClosed);
                }
            }
            notified.await;
        }
    }

    fn evaluation_slot(&self) -> Result<EvaluationSlot<'_>, HostError> {
        self.active_evaluations
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_CONCURRENT_WORKER_EVALUATIONS).then_some(active + 1)
            })
            .map_err(|_| {
                HostError::ResourceExhausted(format!(
                    "Worker evaluation concurrency exceeded ({MAX_CONCURRENT_WORKER_EVALUATIONS})"
                ))
            })?;
        let resource = self
            .resources
            .claim(requirements())
            .map_err(HostError::backend)?
            .reserve(
                EVALUATION_SCOPE,
                ResourceClass::BackendStateBytes,
                FFI_ALLOWANCE,
            );
        match resource {
            Ok(resource) => Ok(EvaluationSlot {
                active: &self.active_evaluations,
                _resource: resource,
            }),
            Err(error) => {
                self.active_evaluations.fetch_sub(1, Ordering::AcqRel);
                Err(HostError::ResourceExhausted(error.to_string()))
            }
        }
    }

    pub(crate) async fn evaluate(
        &self,
        owner: Arc<WorkerOwner>,
        params: crate::evaluation::EvaluateParams,
    ) -> Result<crate::evaluation::EvaluationValue, HostError> {
        if params.source().len() > SOURCE_BYTES {
            return Err(HostError::InvalidArgument(format!(
                "evaluation source exceeds {SOURCE_BYTES} UTF-8 bytes"
            )));
        }
        if let Some(argument) = params.argument() {
            argument
                .validate(crate::evaluation::input_value_limits())
                .map_err(|error| HostError::InvalidArgument(error.to_string()))?;
        }
        owner.wait_until_ready().await?;
        let _slot = self.evaluation_slot()?;
        let service = CdpEvaluationService::with_limits(
            Arc::clone(&owner.client),
            CdpEvaluationLimits::new(
                SOURCE_BYTES,
                GENERATED_SCRIPT_BYTES,
                ERROR_BYTES,
                value_limits(),
            ),
        );
        service
            .call(params)
            .await
            .map(|result| result.into_value())
            .map_err(|error| crate::evaluation::evaluation_error(error, 0))
    }

    pub(crate) fn close(&self) {
        let workers = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.closed {
                return;
            }
            state.closed = true;
            state.notices.clear();
            state.service_worker_notices.clear();
            std::mem::take(&mut state.workers)
        };
        #[cfg(feature = "worker-network-events")]
        self.prepared_service_worker_network
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        for owner in workers.into_values() {
            let _ = owner.close();
        }
        self.notify.notify_waiters();
        self.service_worker_notify.notify_waiters();
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

    fn retains_worker_targets(&self) -> bool {
        true
    }

    fn retains_service_worker_targets(&self) -> bool {
        cfg!(feature = "service-worker-events")
    }

    fn retains_worker_session(&self, session: &str) -> bool {
        self.installation.contains(session)
    }

    fn enables_worker_network(&self) -> bool {
        cfg!(feature = "worker-network-events")
    }

    fn enables_service_worker_network(&self) -> bool {
        cfg!(feature = "worker-network-events")
            || cfg!(all(
                feature = "service-worker-events",
                feature = "context-offline"
            ))
    }

    fn prepare_service_worker(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
    ) -> FrameObservationResult {
        self.downstream.prepare_service_worker(topology, event)?;
        let params = serde_json::from_value(event.params.clone())?;
        self.installation
            .prepare_service_worker_network(topology, &params)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
    }

    fn service_worker_initialization_commands(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
        commands: &mut Vec<ServiceWorkerInitializationCommand>,
    ) -> FrameObservationResult {
        self.downstream
            .service_worker_initialization_commands(topology, event, commands)?;
        #[cfg(all(feature = "service-worker-events", feature = "context-offline"))]
        {
            let params = serde_json::from_value(event.params.clone())?;
            if let Some(offline_commands) = self
                .installation
                .service_worker_offline_commands(topology, &params)
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?
            {
                commands
                    .try_reserve(offline_commands.len())
                    .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?;
                commands.extend(offline_commands);
            }
        }
        #[cfg(not(all(feature = "service-worker-events", feature = "context-offline")))]
        let _ = (topology, event, commands);
        Ok(())
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
        // Worker retirement has an exact Target.detachedFromTarget fact. Page
        // membership can be transient while a navigation transaction is being
        // published, so it must not synthesize Worker closure.
        self.downstream.reconcile(topology)
    }

    fn event(&self, topology: &CdpTopology, event: &CdpEvent) -> FrameObservationResult {
        self.downstream.event(topology, event)?;
        let result = match event.method.as_str() {
            "Target.attachedToTarget" => {
                let params = serde_json::from_value(event.params.clone())?;
                self.installation
                    .attach(topology, event.session_id.as_deref(), params)
            }
            "Target.detachedFromTarget" => {
                let params: target::DetachedFromTargetEvent =
                    serde_json::from_value(event.params.clone())?;
                self.installation.close_worker(&params.session_id)
            }
            "Runtime.executionContextCreated" => {
                let params: runtime::ExecutionContextCreatedEvent =
                    serde_json::from_value(event.params.clone())?;
                match event.session_id.as_deref() {
                    Some(session) => self
                        .installation
                        .mark_execution_context(session, params.context.unique_id),
                    None => Ok(()),
                }
            }
            "Inspector.workerScriptLoaded" => match event.session_id.as_deref() {
                Some(session) => self.installation.mark_script_loaded(session),
                None => Ok(()),
            },
            _ => Ok(()),
        };
        result.map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
    }

    fn seal(&self) {
        self.installation.close();
        self.downstream.seal();
    }
}

impl HostSession {
    pub async fn next_worker_notice(&self) -> Result<WorkerNotice, HostError> {
        let installation = self.worker_events()?;
        loop {
            let notice = tokio::select! {
                result = installation.next_notice() => result,
                () = self.stop.cancelled() => Err(HostError::TargetClosed),
                () = self.disconnected.cancelled() => Err(HostError::TargetClosed),
            }?;
            if installation.notice_visible(&notice.owner)? {
                return Ok(notice);
            }
        }
    }

    pub fn page_workers(&self, page_id: &str) -> Result<Vec<Arc<WorkerOwner>>, HostError> {
        let page = parse_object_id(page_id, "Page")?;
        self.require_visible_page_id(page)?;
        self.worker_events()?.snapshot(page)
    }

    #[cfg(feature = "service-worker-events")]
    pub async fn next_service_worker_notice(&self) -> Result<ServiceWorkerNotice, HostError> {
        let installation = self.worker_events()?;
        loop {
            let notice = tokio::select! {
                result = installation.next_service_worker_notice() => result,
                () = self.stop.cancelled() => Err(HostError::TargetClosed),
                () = self.disconnected.cancelled() => Err(HostError::TargetClosed),
            }?;
            if installation.notice_visible(&notice.owner)? {
                return Ok(notice);
            }
        }
    }

    #[cfg(feature = "service-worker-events")]
    pub fn context_service_workers(
        &self,
        context_id: &str,
    ) -> Result<Vec<Arc<WorkerOwner>>, HostError> {
        let context = parse_object_id(context_id, "BrowserContext")?;
        self.require_visible_context_id(context)?;
        self.worker_events()?.service_worker_snapshot(context)
    }

    pub async fn worker_evaluate(
        &self,
        owner: Arc<WorkerOwner>,
        params: crate::evaluation::EvaluateParams,
    ) -> Result<crate::evaluation::EvaluationValue, HostError> {
        if let Some(page) = owner.page() {
            self.require_visible_page_id(page)?;
        } else {
            self.require_visible_context_id(owner.context())?;
        }
        let installation = self.worker_events()?;
        installation.require_current_owner(&owner)?;
        tokio::select! {
            result = installation.evaluate(owner, params) => result,
            () = self.stop.cancelled() => Err(HostError::TargetClosed),
            () = self.disconnected.cancelled() => Err(HostError::TargetClosed),
        }
    }

    pub(crate) fn worker_events(&self) -> Result<Arc<Installation>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        let live = state.as_ref().ok_or(HostError::TargetClosed)?;
        Ok(Arc::clone(&live.worker_events))
    }
}

fn bounded(value: &str, limit: usize, field: &str) -> Result<(), HostError> {
    if value.len() > limit {
        return Err(HostError::ResourceExhausted(format!(
            "{field} exceeds {limit} UTF-8 bytes"
        )));
    }
    Ok(())
}

fn parse_object_id(value: &str, kind: &str) -> Result<ObjectId, HostError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value != 0)
        .map(ObjectId::new)
        .ok_or_else(|| HostError::InvalidArgument(format!("invalid portable {kind} ID")))
}

#[cfg(test)]
mod tests {
    use playwright_rust_backend_cdp_topology::{
        CdpPageObjectIds, CdpPageTarget, CdpTopologyLimits, ConnectionEpoch,
    };
    use playwright_rust_core::ResourceLimits;
    use playwright_rust_fake::FakeCdpClient;
    use playwright_rust_plugin_chromium_frame_observer::ChromiumFrameOwnership;
    use serde_json::json;

    use crate::frame_metadata::{FrameMetadata, FrameMetadataLimits};

    use super::*;

    struct FakeSessions(Arc<FakeCdpClient>);

    impl SessionFactory for FakeSessions {
        fn create_session(&self, _session_id: String, _target_id: String) -> Arc<dyn CdpClient> {
            self.0.clone()
        }
    }

    #[cfg(feature = "worker-network-events")]
    #[derive(Default)]
    struct RecordingNetworkRegistration {
        registrations: Mutex<Vec<(ObjectId, String)>>,
    }

    #[cfg(feature = "worker-network-events")]
    impl ServiceWorkerNetworkRegistration for RecordingNetworkRegistration {
        fn register(&self, context: ObjectId, session_id: &str) -> Result<(), HostError> {
            self.registrations
                .lock()
                .map_err(HostError::backend)?
                .push((context, session_id.to_owned()));
            Ok(())
        }
    }

    #[cfg(feature = "worker-network-events")]
    fn install_test_network_registration(
        installation: &Installation,
    ) -> Arc<RecordingNetworkRegistration> {
        let registration = Arc::new(RecordingNetworkRegistration::default());
        *installation
            .network_registration
            .lock()
            .expect("test network registration should be available") = Some(registration.clone());
        registration
    }

    fn resources() -> ProductionResourceLedger {
        let required = requirements();
        let mut limits = ResourceLimits::deny_all(required.scopes, required.leases);
        for class in ResourceClass::ALL {
            let bound = required.class(class);
            limits = limits.with_class(class, bound.global, bound.per_scope);
        }
        ProductionResourceLedger::new(limits, required).unwrap()
    }

    fn context_metadata(owned_contexts_only: bool) -> ContextMetadata {
        let metadata = ContextMetadata::new(crate::context_metadata::ContextMetadataLimits {
            active_contexts: 4,
            states: 8,
            conversions: 4,
            base_url_bytes: 128,
            bytes: 4096,
        })
        .unwrap();
        let metadata = if owned_contexts_only {
            metadata.with_owned_contexts_only().unwrap()
        } else {
            metadata
        };
        metadata
            .reserve_creation(None)
            .unwrap()
            .commit(ObjectId::new(5))
            .unwrap();
        metadata
    }

    fn empty_frame_trees() -> Arc<FrameTrees> {
        let urls = Arc::new(
            FrameMetadata::new(FrameMetadataLimits {
                entries: 128,
                url_bytes: 64,
                total_bytes: 8192,
                publications: 256,
            })
            .unwrap(),
        );
        Arc::new(FrameTrees::new(urls).unwrap())
    }

    fn published_owned_page_trees(
        contexts: &ContextMetadata,
    ) -> (Arc<FrameMetadata>, Arc<FrameTrees>) {
        let urls = Arc::new(
            FrameMetadata::new(FrameMetadataLimits {
                entries: 128,
                url_bytes: 64,
                total_bytes: 8192,
                publications: 256,
            })
            .unwrap(),
        );
        let trees = Arc::new(FrameTrees::new(urls.clone()).unwrap());
        let page = ObjectId::new(6);
        trees
            .reconcile(
                std::iter::once(ChromiumFrameOwnership {
                    frame: ObjectId::new(7),
                    page,
                    parent: None,
                }),
                |candidate| candidate == page,
            )
            .unwrap();
        let snapshot = contexts.visible(ObjectId::new(5), true).unwrap().unwrap();
        let tree = trees.get(page).unwrap();
        let mut reserved = urls
            .publications()
            .reserve_collection(tree.lookup_resources())
            .unwrap();
        reserved.add_context(ObjectId::new(5), snapshot).unwrap();
        reserved.add_page(page, ObjectId::new(5), tree).unwrap();
        urls.publications().stage_collection(reserved).unwrap();
        urls.publications().fence().unwrap().publish().unwrap();
        (urls, trees)
    }

    fn topology_with_default_and_owned_pages() -> CdpTopology {
        const EPOCH: ConnectionEpoch = ConnectionEpoch::new(1);
        let mut topology = CdpTopology::new(
            ObjectId::new(1),
            EPOCH,
            CdpTopologyLimits::new(32, 4, 4, 8, 4, 64),
        )
        .unwrap();
        topology
            .register_default_context(EPOCH, ObjectId::new(2))
            .unwrap();
        topology
            .attach_page(
                EPOCH,
                CdpPageTarget::new("default-page", "default-session", ""),
                CdpPageObjectIds::new(ObjectId::new(3), ObjectId::new(4)),
            )
            .unwrap();
        topology
            .register_context(
                EPOCH,
                "owned-context",
                ObjectId::new(5),
                CdpContextOwnership::Owned,
            )
            .unwrap();
        topology
            .attach_page(
                EPOCH,
                CdpPageTarget::new("owned-page", "owned-session", "owned-context"),
                CdpPageObjectIds::new(ObjectId::new(6), ObjectId::new(7)),
            )
            .unwrap();
        topology
    }

    fn worker_event(id: &str) -> target::AttachedToTargetEvent {
        serde_json::from_value(json!({
            "sessionId": id,
            "waitingForDebugger": true,
            "targetInfo": {
                "targetId": id,
                "type": "worker",
                "title": "",
                "url": "https://example.test/worker.js",
                "attached": true,
                "canAccessOpener": false
            }
        }))
        .unwrap()
    }

    fn service_worker_event(id: &str, context: &str) -> target::AttachedToTargetEvent {
        serde_json::from_value(json!({
            "sessionId": id,
            "waitingForDebugger": true,
            "targetInfo": {
                "targetId": id,
                "type": "service_worker",
                "title": "",
                "url": "https://example.test/service-worker.js",
                "attached": true,
                "canAccessOpener": false,
                "browserContextId": context
            }
        }))
        .unwrap()
    }

    fn installation() -> Arc<Installation> {
        let installation = Installation::new(
            &resources(),
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            context_metadata(false),
            empty_frame_trees(),
        )
        .unwrap();
        #[cfg(feature = "worker-network-events")]
        install_test_network_registration(&installation);
        installation
    }

    fn owner(installation: &Installation) -> Arc<WorkerOwner> {
        let _ = installation;
        Arc::new(WorkerOwner {
            kind: WorkerOwnerKind::Dedicated,
            page: Some(ObjectId::new(7)),
            context: ObjectId::new(7),
            context_owned: true,
            sequence: 0,
            session_id: "worker-1".into(),
            target_id: "worker-target-1".into(),
            url: "https://example.test/worker.js".into(),
            client: Arc::new(FakeCdpClient::from_html("")),
            readiness: Mutex::new(WorkerReadiness {
                unique_context_id: None,
                script_loaded: false,
                closed: false,
            }),
            notify: Notify::new(),
        })
    }

    #[cfg(feature = "worker-network-events")]
    #[test]
    fn service_worker_preparation_registers_network_without_publishing_worker() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let resources = resources();
        let installation = Installation::new(
            &resources,
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees,
        )
        .unwrap();
        let registration = install_test_network_registration(&installation);
        let event = service_worker_event("service-worker-root", "owned-context");

        // Act
        installation
            .prepare_service_worker_network(&topology, &event)
            .expect("pre-resume network lane should register");

        // Assert
        assert_eq!(
            *registration.registrations.lock().unwrap(),
            vec![(ObjectId::new(5), "service-worker-root".to_owned())]
        );
        let state = installation.state.lock().unwrap();
        assert!(state.workers.is_empty());
        assert!(state.service_worker_notices.is_empty());
    }

    #[tokio::test]
    async fn root_and_parent_scoped_service_workers_are_owned_by_their_browser_context() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let resources = resources();
        let installation = Installation::new(
            &resources,
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees,
        )
        .unwrap();
        #[cfg(feature = "worker-network-events")]
        install_test_network_registration(&installation);

        // Act
        for (parent, id) in [
            (None, "service-worker-root"),
            (Some("page-session"), "service-worker-parent"),
        ] {
            installation
                .attach(&topology, parent, service_worker_event(id, "owned-context"))
                .unwrap();
        }
        let notices = [
            installation.next_service_worker_notice().await.unwrap(),
            installation.next_service_worker_notice().await.unwrap(),
        ];
        let snapshot = installation
            .service_worker_snapshot(ObjectId::new(5))
            .unwrap();

        // Assert
        for notice in notices {
            assert_eq!(notice.kind, WorkerNoticeKind::Created);
            assert_eq!(notice.context, ObjectId::new(5));
            assert!(notice.owner.is_service_worker());
            assert_eq!(notice.owner.page(), None);
        }
        assert_eq!(snapshot.len(), 2);
        #[cfg(feature = "extra-headers")]
        {
            assert!(
                installation
                    .contains_service_worker("service-worker-root")
                    .unwrap()
            );
            assert!(
                installation
                    .contains_service_worker("service-worker-parent")
                    .unwrap()
            );
            assert!(!installation.contains_service_worker("missing").unwrap());
        }
    }

    #[tokio::test]
    async fn hidden_default_service_worker_does_not_consume_public_notice_capacity() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let resources = resources();
        let installation = Installation::new(
            &resources,
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees,
        )
        .unwrap();
        #[cfg(feature = "worker-network-events")]
        install_test_network_registration(&installation);

        // Act
        installation
            .attach(
                &topology,
                None,
                service_worker_event("hidden-service-worker", ""),
            )
            .unwrap();

        // Assert
        assert!(
            installation
                .service_worker_snapshot(ObjectId::new(2))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            resources
                .metrics()
                .unwrap()
                .class(ResourceClass::EventItem)
                .current,
            0
        );
    }

    #[tokio::test]
    async fn unknown_native_service_worker_context_falls_back_to_default_context() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let installation = installation();

        // Act
        installation
            .attach(
                &topology,
                None,
                service_worker_event("default-worker", "native-default-context"),
            )
            .unwrap();
        let notice = installation.next_service_worker_notice().await.unwrap();

        // Assert
        assert_eq!(notice.context, ObjectId::new(2));
        assert_eq!(notice.owner.context(), ObjectId::new(2));
        assert_eq!(
            installation
                .service_worker_snapshot(ObjectId::new(2))
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn fresh_owned_hidden_worker_flood_preserves_visible_notice_capacity() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let resources = resources();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let installation = Installation::new(
            &resources,
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees,
        )
        .unwrap();

        // Act
        for index in 0..=MAX_WORKER_NOTICES {
            let id = format!("hidden-worker-{index}");
            installation
                .attach(&topology, Some("default-session"), worker_event(&id))
                .unwrap();
        }
        installation
            .attach(
                &topology,
                Some("owned-session"),
                worker_event("visible-worker"),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();

        // Assert
        assert_eq!(notice.kind, WorkerNoticeKind::Created);
        assert_eq!(notice.page, ObjectId::new(6));
        assert_eq!(notice.owner.id(), "visible-worker");
        assert!(installation.snapshot(ObjectId::new(3)).unwrap().is_empty());
        assert_eq!(installation.snapshot(ObjectId::new(6)).unwrap().len(), 1);
        drop(notice);
        let metrics = resources.metrics().unwrap();
        assert_eq!(metrics.class(ResourceClass::EventItem).current, 0);
        assert_eq!(metrics.class(ResourceClass::LiveObject).current, 0);
    }

    #[tokio::test]
    async fn fresh_owned_internal_scratch_page_workers_never_use_public_capacity() {
        // Arrange
        let mut topology = topology_with_default_and_owned_pages();
        topology
            .attach_page(
                ConnectionEpoch::new(1),
                CdpPageTarget::new("scratch-page", "scratch-session", "owned-context"),
                CdpPageObjectIds::new(ObjectId::new(8), ObjectId::new(9)),
            )
            .unwrap();
        let resources = resources();
        let metadata = context_metadata(true);
        let (urls, trees) = published_owned_page_trees(&metadata);
        let scratch_url = urls.reserve_new_page().unwrap();
        let reservation = trees.reserve_page(scratch_url.clone()).unwrap();
        let (_tree, _binding, _tree_binding) = trees
            .activate_page(reservation, ObjectId::new(8), ObjectId::new(9), true)
            .unwrap();
        urls.activate_new_page(ObjectId::new(9), scratch_url)
            .unwrap();
        assert!(!trees.is_published_public_page(ObjectId::new(8)).unwrap());
        let installation = Installation::new(
            &resources,
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees,
        )
        .unwrap();

        // Act
        for index in 0..=MAX_WORKER_NOTICES {
            let id = format!("scratch-worker-{index}");
            installation
                .attach(&topology, Some("scratch-session"), worker_event(&id))
                .unwrap();
        }
        installation
            .attach(
                &topology,
                Some("owned-session"),
                worker_event("visible-worker"),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();

        // Assert
        assert_eq!(notice.page, ObjectId::new(6));
        assert_eq!(notice.owner.id(), "visible-worker");
        assert!(installation.snapshot(ObjectId::new(8)).unwrap().is_empty());
        assert_eq!(installation.snapshot(ObjectId::new(6)).unwrap().len(), 1);
        drop(notice);
        let metrics = resources.metrics().unwrap();
        assert_eq!(metrics.class(ResourceClass::EventItem).current, 0);
        assert_eq!(metrics.class(ResourceClass::LiveObject).current, 0);
    }

    #[tokio::test]
    async fn worker_owner_registry_grows_past_the_legacy_count() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let installation = Installation::new(
            &resources(),
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees,
        )
        .unwrap();

        // Act
        for value in 1..=129 {
            installation
                .attach(
                    &topology,
                    Some("owned-session"),
                    worker_event(&format!("worker-{value}")),
                )
                .expect("Worker ownership should not have a fixed count limit");
            drop(installation.next_notice().await.unwrap());
        }

        // Assert
        assert_eq!(installation.snapshot(ObjectId::new(6)).unwrap().len(), 129);
    }

    #[tokio::test]
    async fn fresh_owned_queued_worker_notice_is_hidden_after_page_retirement() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let installation = Installation::new(
            &resources(),
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata,
            trees.clone(),
        )
        .unwrap();
        installation
            .attach(
                &topology,
                Some("owned-session"),
                worker_event("visible-worker"),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();
        assert!(installation.notice_visible(&notice.owner).unwrap());

        // Act
        trees.reconcile(std::iter::empty(), |_| false).unwrap();
        let visible = installation.notice_visible(&notice.owner).unwrap();

        // Assert
        assert!(!visible);
    }

    #[tokio::test]
    async fn attached_or_persistent_default_page_worker_remains_visible() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let resources = resources();
        let installation = Installation::new(
            &resources,
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            context_metadata(false),
            empty_frame_trees(),
        )
        .unwrap();

        // Act
        installation
            .attach(
                &topology,
                Some("default-session"),
                worker_event("default-worker"),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();

        // Assert
        assert_eq!(notice.page, ObjectId::new(3));
        assert_eq!(notice.owner.id(), "default-worker");
        assert!(installation.notice_visible(&notice.owner).unwrap());
    }

    #[tokio::test]
    async fn removed_context_hides_queued_worker_notice() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let metadata = context_metadata(true);
        let (_urls, trees) = published_owned_page_trees(&metadata);
        let installation = Installation::new(
            &resources(),
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            metadata.clone(),
            trees,
        )
        .unwrap();
        installation
            .attach(
                &topology,
                Some("owned-session"),
                worker_event("visible-worker"),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();

        // Act
        metadata.remove(ObjectId::new(5)).unwrap();
        let visible = installation.notice_visible(&notice.owner).unwrap();
        installation.close_worker(notice.owner.id()).unwrap();

        // Assert
        assert!(!visible);
        assert!(installation.state.lock().unwrap().notices.is_empty());
    }

    #[tokio::test]
    async fn worker_invocation_requires_the_exact_current_owner() {
        // Arrange
        let topology = topology_with_default_and_owned_pages();
        let installation = Installation::new(
            &resources(),
            Arc::new(FakeSessions(Arc::new(FakeCdpClient::from_html("")))),
            context_metadata(false),
            empty_frame_trees(),
        )
        .unwrap();
        installation
            .attach(
                &topology,
                Some("owned-session"),
                worker_event("visible-worker"),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();
        let foreign = owner(&installation);

        // Act
        let current = installation.require_current_owner(&notice.owner);
        let not_current = installation.require_current_owner(&foreign);
        installation.close_worker(notice.owner.id()).unwrap();
        let retired = installation.require_current_owner(&notice.owner);

        // Assert
        assert!(current.is_ok());
        assert!(matches!(not_current, Err(HostError::TargetClosed)));
        assert!(matches!(retired, Err(HostError::TargetClosed)));
    }

    #[tokio::test]
    async fn readiness_requires_both_execution_context_and_script_load() {
        // Arrange
        let installation = installation();
        let owner = owner(&installation);
        let waiting = owner.wait_until_ready();
        tokio::pin!(waiting);

        // Act
        owner
            .mark_execution_context("worker-world-1".to_owned())
            .unwrap();

        // Assert
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(1), &mut waiting)
                .await
                .is_err()
        );
        owner.mark_script_loaded().unwrap();
        waiting.await.unwrap();
    }

    #[tokio::test]
    async fn closing_worker_rejects_a_pending_evaluation_readiness_wait() {
        // Arrange
        let installation = installation();
        let owner = owner(&installation);

        // Act
        owner.close().unwrap();
        let result = owner.wait_until_ready().await;

        // Assert
        assert!(matches!(result, Err(HostError::TargetClosed)));
    }

    #[test]
    fn resource_profile_bounds_notices_and_concurrent_evaluations_not_worker_count() {
        // Arrange
        let required = requirements();

        // Act
        let objects = required.class(ResourceClass::LiveObject);
        let events = required.class(ResourceClass::EventItem);
        let bytes = required.class(ResourceClass::BackendStateBytes);

        // Assert
        assert_eq!(objects.global, 0);
        assert_eq!(events.global, MAX_WORKER_NOTICES as u64);
        assert!(bytes.global >= MAX_CONCURRENT_WORKER_EVALUATIONS as u64 * FFI_ALLOWANCE);
        assert_eq!(
            bytes.per_scope,
            MAX_CONCURRENT_WORKER_EVALUATIONS as u64 * FFI_ALLOWANCE
        );
    }
}
