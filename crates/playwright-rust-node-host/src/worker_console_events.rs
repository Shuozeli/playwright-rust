//! Optional bounded Worker console publication.
//!
//! This leaf composes Worker ownership with the existing console codec. Rust
//! identifies the exact retained Worker session, bounds and owns the FIFO, and
//! releases diagnostic RemoteObjects unless the separately selectable argument
//! Handle leaf atomically transfers them into the Worker Handle registry.

use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::CdpTopology;
use playwright_rust_cdp::{CdpEvent, ServiceWorkerInitializationCommand, generated::page};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_frame_observer::{
    ChromiumFrameEventObserver, FrameObservationResult,
};
use playwright_rust_runtime::{
    CleanupFailure, CleanupReservation, CleanupSubmitError, ReservedCleanupExecutor,
};
use playwright_rust_runtime_tokio::{TokioCleanupExecutor, TokioCleanupReservation};
use serde_json::Value;
use tokio::sync::Notify;

use crate::console_events::ConsoleMessagePlan;
use crate::worker_events::{Installation as WorkerInstallation, WorkerOwner};
#[cfg(feature = "worker-console-argument-handles")]
use crate::worker_handles::{Installation as WorkerHandleInstallation, WorkerHandle};
use crate::{ConsoleMessage, HostError, HostSession};

#[cfg(any(
    test,
    not(any(feature = "handles", feature = "worker-handle-evaluation"))
))]
pub(crate) const MAX_WORKER_CONSOLE_RELEASE_JOBS: usize = 128;
const INSTALLATION_BYTES: u64 = 4096;
const INSTALLATION_SCOPE: ResourceScope = ResourceScope::extension(183, 1);
const MAX_ARGUMENTS: usize = 16;
const MAX_OBJECT_ID_BYTES: usize = 4096;

pub(crate) const fn requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            INSTALLATION_BYTES,
            INSTALLATION_BYTES,
        )
        .with_metadata(1, 1)
}

pub struct WorkerConsoleNotice {
    pub page: Option<ObjectId>,
    pub context: ObjectId,
    #[cfg(feature = "worker-console-argument-handles")]
    pub argument_handles: Vec<WorkerHandle>,
    pub owner: Arc<WorkerOwner>,
    pub message: Arc<ConsoleMessage>,
}

struct State {
    notices: VecDeque<WorkerConsoleNotice>,
    subscriptions: std::collections::BTreeMap<Box<str>, u32>,
    closed: bool,
}

pub(crate) struct Installation {
    state: Mutex<State>,
    notify: Notify,
    cleanup: TokioCleanupExecutor,
    #[cfg(feature = "worker-console-argument-handles")]
    worker_handles: Option<Arc<WorkerHandleInstallation>>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new(
        resources: &ProductionResourceLedger,
        cleanup: TokioCleanupExecutor,
    ) -> Result<Arc<Self>, HostError> {
        let plan = requirements();
        let ledger = resources.claim(plan).map_err(HostError::backend)?;
        let storage = ledger
            .reserve(
                INSTALLATION_SCOPE,
                ResourceClass::BackendStateBytes,
                INSTALLATION_BYTES,
            )
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                notices: VecDeque::new(),
                subscriptions: std::collections::BTreeMap::new(),
                closed: false,
            }),
            notify: Notify::new(),
            cleanup,
            #[cfg(feature = "worker-console-argument-handles")]
            worker_handles: None,
            _storage: storage,
        }))
    }

    #[cfg(feature = "worker-console-argument-handles")]
    pub(crate) fn new_with_argument_handles(
        resources: &ProductionResourceLedger,
        cleanup: TokioCleanupExecutor,
        worker_handles: Arc<WorkerHandleInstallation>,
    ) -> Result<Arc<Self>, HostError> {
        let mut installation = Self::new(resources, cleanup)?;
        Arc::get_mut(&mut installation)
            .expect("new Worker console installation is uniquely owned")
            .worker_handles = Some(worker_handles);
        Ok(installation)
    }

    fn record(&self, owner: Arc<WorkerOwner>, params: &Value) -> Result<(), HostError> {
        if params.get("executionContextId").and_then(Value::as_i64) == Some(0) {
            return Ok(());
        }
        let message_type = params
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| HostError::Backend("Worker console event is missing type".into()))?;
        let args = params
            .get("args")
            .and_then(Value::as_array)
            .ok_or_else(|| HostError::Backend("Worker console event is missing args".into()))?;
        let plan = ConsoleMessagePlan::new(message_type, args, params)?;
        #[cfg(feature = "worker-console-argument-handles")]
        let _storage_bytes = plan
            .storage_bytes()
            .checked_add(
                args.len()
                    .checked_mul(std::mem::size_of::<WorkerHandle>())
                    .ok_or_else(|| {
                        HostError::ResourceExhausted(
                            "Worker console argument Handle storage overflow".into(),
                        )
                    })?,
            )
            .ok_or_else(|| {
                HostError::ResourceExhausted(
                    "Worker console argument Handle storage overflow".into(),
                )
            })?;
        let page = owner.page();
        let context = owner.context();

        {
            let state = self.state.lock().map_err(HostError::backend)?;
            if state.closed {
                return Err(HostError::TargetClosed);
            }
        }

        let recovery = RemoteObjectRecovery::new(&self.cleanup, &owner, args)?;
        #[cfg(feature = "worker-console-argument-handles")]
        let (argument_handles, recovery) = {
            let mut recovery = recovery;
            let argument_handles = match &self.worker_handles {
                Some(worker_handles) => {
                    let argument_handles =
                        worker_handles.adopt_event_handles(Arc::clone(&owner), args)?;
                    recovery.disarm();
                    argument_handles
                }
                None => Vec::new(),
            };
            (argument_handles, recovery)
        };
        let message = Arc::new(plan.build()?);
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        state.notices.try_reserve(1).map_err(HostError::backend)?;
        state.notices.push_back(WorkerConsoleNotice {
            page,
            context,
            owner,
            message,
            #[cfg(feature = "worker-console-argument-handles")]
            argument_handles,
        });
        drop(state);
        drop(recovery);
        self.notify.notify_one();
        Ok(())
    }

    async fn next_notice(&self) -> Result<WorkerConsoleNotice, HostError> {
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

    pub(crate) fn close(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.closed {
            return;
        }
        state.closed = true;
        state.notices.clear();
        state.subscriptions.clear();
        drop(state);
        self.notify.notify_waiters();
    }

    pub(crate) fn update_subscription(
        &self,
        owner: &WorkerOwner,
        enabled: bool,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if enabled {
            let subscriptions = state.subscriptions.entry(owner.id().into()).or_insert(0);
            *subscriptions = subscriptions.checked_add(1).ok_or_else(|| {
                HostError::ResourceExhausted("Worker console subscription count exhausted".into())
            })?;
        } else if let Some(subscriptions) = state.subscriptions.get_mut(owner.id()) {
            *subscriptions = subscriptions.saturating_sub(1);
            if *subscriptions == 0 {
                state.subscriptions.remove(owner.id());
            }
        }
        Ok(())
    }

    fn subscribed(&self, owner: &WorkerOwner) -> Result<bool, HostError> {
        Ok(self
            .state
            .lock()
            .map_err(HostError::backend)?
            .subscriptions
            .contains_key(owner.id()))
    }
}

struct RemoteObjectRecovery {
    reservation: Option<TokioCleanupReservation>,
    owner: Arc<WorkerOwner>,
    object_ids: Vec<Box<str>>,
}

impl RemoteObjectRecovery {
    fn new(
        cleanup: &TokioCleanupExecutor,
        owner: &Arc<WorkerOwner>,
        args: &[Value],
    ) -> Result<Self, HostError> {
        if args.len() > MAX_ARGUMENTS {
            return Err(HostError::ResourceExhausted(format!(
                "Worker console argument count exceeds {MAX_ARGUMENTS}"
            )));
        }
        let ids = args
            .iter()
            .filter_map(Value::as_object)
            .filter_map(|argument| argument.get("objectId"))
            .filter_map(Value::as_str);
        let bytes = ids
            .clone()
            .try_fold(0usize, |total, id| total.checked_add(id.len()))
            .ok_or_else(|| {
                HostError::ResourceExhausted("Worker console object ID bytes overflow".into())
            })?;
        if bytes > MAX_OBJECT_ID_BYTES {
            return Err(HostError::ResourceExhausted(format!(
                "Worker console object ID bytes exceed {MAX_OBJECT_ID_BYTES}"
            )));
        }
        let mut object_ids = Vec::with_capacity(args.len());
        object_ids.extend(ids.map(Box::<str>::from));
        let reservation = (!object_ids.is_empty())
            .then(|| cleanup.try_reserve())
            .transpose()
            .map_err(cleanup_admission_error)?;
        Ok(Self {
            reservation,
            owner: Arc::clone(owner),
            object_ids,
        })
    }

    #[cfg(feature = "worker-console-argument-handles")]
    fn disarm(&mut self) {
        self.reservation.take();
        self.object_ids.clear();
    }
}

impl Drop for RemoteObjectRecovery {
    fn drop(&mut self) {
        let Some(reservation) = self.reservation.take() else {
            return;
        };
        let client = self.owner.client();
        let object_ids = std::mem::take(&mut self.object_ids);
        reservation.submit(Box::pin(async move {
            for object_id in object_ids {
                match client.runtime_release_object(&object_id).await {
                    Ok(())
                    | Err(playwright_rust_cdp::connection::CdpError::Closed)
                    | Err(playwright_rust_cdp::connection::CdpError::SessionDetached)
                    | Err(playwright_rust_cdp::connection::CdpError::Protocol { .. }) => {}
                    Err(error) => {
                        return Err(CleanupFailure::new(format!(
                            "Worker console argument release failed: {error}"
                        )));
                    }
                }
            }
            Ok(())
        }));
    }
}

fn cleanup_admission_error(error: CleanupSubmitError) -> HostError {
    match error {
        CleanupSubmitError::CapacityExceeded { .. } => {
            HostError::ResourceExhausted(error.to_string())
        }
        CleanupSubmitError::Unavailable => HostError::TargetClosed,
        CleanupSubmitError::StatePoisoned => HostError::backend(error),
    }
}

pub(crate) struct Observer {
    installation: Arc<Installation>,
    workers: Arc<WorkerInstallation>,
    subscriptions: Arc<crate::console_events::Subscriptions>,
    downstream: Arc<dyn ChromiumFrameEventObserver>,
}

impl Observer {
    pub(crate) fn new(
        installation: Arc<Installation>,
        workers: Arc<WorkerInstallation>,
        subscriptions: Arc<crate::console_events::Subscriptions>,
        downstream: Arc<dyn ChromiumFrameEventObserver>,
    ) -> Self {
        Self {
            installation,
            workers,
            subscriptions,
            downstream,
        }
    }
}

impl ChromiumFrameEventObserver for Observer {
    fn observes_page(&self, page: ObjectId) -> bool {
        self.downstream.observes_page(page)
    }

    fn retains_worker_targets(&self) -> bool {
        self.downstream.retains_worker_targets()
    }

    fn retains_service_worker_targets(&self) -> bool {
        self.downstream.retains_service_worker_targets()
    }

    fn retains_worker_session(&self, session: &str) -> bool {
        self.downstream.retains_worker_session(session)
    }

    fn enables_worker_network(&self) -> bool {
        self.downstream.enables_worker_network()
    }

    fn enables_service_worker_network(&self) -> bool {
        self.downstream.enables_service_worker_network()
    }

    fn prepare_service_worker(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
    ) -> FrameObservationResult {
        self.downstream.prepare_service_worker(topology, event)
    }

    fn service_worker_initialization_commands(
        &self,
        topology: &CdpTopology,
        event: &CdpEvent,
        commands: &mut Vec<ServiceWorkerInitializationCommand>,
    ) -> FrameObservationResult {
        self.downstream
            .service_worker_initialization_commands(topology, event, commands)
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
        self.downstream.reconcile(topology)
    }

    fn event(&self, topology: &CdpTopology, event: &CdpEvent) -> FrameObservationResult {
        self.downstream.event(topology, event)?;
        if event.method != "Runtime.consoleAPICalled" {
            return Ok(());
        }
        let Some(session_id) = event.session_id.as_deref() else {
            return Ok(());
        };
        let Some(owner) = self
            .workers
            .owner(session_id)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?
        else {
            return Ok(());
        };
        if !self
            .workers
            .notice_visible(&owner)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?
        {
            return Ok(());
        }
        if !self
            .installation
            .subscribed(&owner)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?
            && !self
                .subscriptions
                .publishes_worker_context(topology, owner.context())
                .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?
        {
            return Ok(());
        }
        self.installation
            .record(owner, &event.params)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
    }

    fn seal(&self) {
        self.installation.close();
        self.downstream.seal();
    }
}

impl HostSession {
    pub fn set_worker_console_listener(
        &self,
        owner: Arc<WorkerOwner>,
        enabled: bool,
    ) -> Result<(), HostError> {
        if let Some(page) = owner.page() {
            self.require_visible_page_id(page)?;
        } else {
            self.require_visible_context_id(owner.context())?;
        }
        self.worker_events()?.require_current_owner(&owner)?;
        self.worker_console_events()?
            .update_subscription(&owner, enabled)
    }

    pub async fn next_worker_console_notice(&self) -> Result<WorkerConsoleNotice, HostError> {
        let installation = self.worker_console_events()?;
        let workers = self.worker_events()?;
        loop {
            let notice = tokio::select! {
                result = installation.next_notice() => result,
                () = self.stop.cancelled() => Err(HostError::TargetClosed),
                () = self.disconnected.cancelled() => Err(HostError::TargetClosed),
            }?;
            if workers.notice_visible(&notice.owner)? {
                return Ok(notice);
            }
        }
    }

    fn worker_console_events(&self) -> Result<Arc<Installation>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        let live = state.as_ref().ok_or(HostError::TargetClosed)?;
        Ok(Arc::clone(&live.worker_console_events))
    }
}

#[cfg(test)]
mod tests {
    use playwright_rust_core::ResourceLimits;
    use playwright_rust_fake::FakeCdpClient;
    use serde_json::json;

    use super::*;

    fn resources() -> ProductionResourceLedger {
        let required = requirements();
        let mut limits = ResourceLimits::deny_all(required.scopes, required.leases);
        for class in ResourceClass::ALL {
            let bound = required.class(class);
            limits = limits.with_class(class, bound.global, bound.per_scope);
        }
        ProductionResourceLedger::new(limits, required).unwrap()
    }

    #[cfg(feature = "worker-console-argument-handles")]
    fn combined_resources() -> ProductionResourceLedger {
        let required = requirements().combine_concurrent(crate::worker_handles::requirements());
        let mut limits = ResourceLimits::deny_all(required.scopes, required.leases);
        for class in ResourceClass::ALL {
            let bound = required.class(class);
            limits = limits.with_class(class, bound.global, bound.per_scope);
        }
        ProductionResourceLedger::new(limits, required).unwrap()
    }

    #[cfg(feature = "worker-console-argument-handles")]
    fn combined_cleanup() -> TokioCleanupExecutor {
        TokioCleanupExecutor::new_allocator_backed()
            .with_job_timeout(std::time::Duration::from_secs(1))
            .unwrap()
    }

    #[tokio::test]
    async fn records_one_bounded_worker_console_notice_and_releases_it_on_read() {
        // Arrange
        let resources = resources();
        let cleanup = TokioCleanupExecutor::new_with_job_timeout(
            MAX_WORKER_CONSOLE_RELEASE_JOBS,
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        let installation = Installation::new(&resources, cleanup).unwrap();
        let client = Arc::new(FakeCdpClient::from_html(""));
        let owner = WorkerOwner::for_test(ObjectId::new(7), client);

        // Act
        installation
            .record(
                owner.clone(),
                &json!({
                    "executionContextId": 1,
                    "type": "log",
                    "args": [{"type": "number", "value": 42}],
                }),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();

        // Assert
        assert_eq!(notice.page, Some(ObjectId::new(7)));
        assert_eq!(notice.context, ObjectId::new(7));
        assert!(Arc::ptr_eq(&notice.owner, &owner));
        assert_eq!(notice.message.message_type(), "log");
        assert_eq!(notice.message.text(), "42");
        drop(notice);
        let metrics = resources.metrics().unwrap();
        assert_eq!(metrics.class(ResourceClass::EventItem).current, 0);
        assert_eq!(metrics.class(ResourceClass::EventBytes).current, 0);
    }

    #[tokio::test]
    async fn service_worker_console_notice_is_owned_by_context_without_a_page() {
        // Arrange
        let resources = resources();
        let cleanup = TokioCleanupExecutor::new_with_job_timeout(
            MAX_WORKER_CONSOLE_RELEASE_JOBS,
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        let installation = Installation::new(&resources, cleanup).unwrap();
        let owner = WorkerOwner::for_service_worker_test(
            ObjectId::new(9),
            Arc::new(FakeCdpClient::from_html("")),
        );

        // Act
        installation
            .record(
                owner.clone(),
                &json!({
                    "executionContextId": 1,
                    "type": "log",
                    "args": [{"type": "string", "value": "service"}],
                }),
            )
            .unwrap();
        let notice = installation.next_notice().await.unwrap();

        // Assert
        assert_eq!(notice.page, None);
        assert_eq!(notice.context, ObjectId::new(9));
        assert!(notice.owner.is_service_worker());
        assert!(Arc::ptr_eq(&notice.owner, &owner));
        assert_eq!(notice.message.text(), "service");
    }

    #[tokio::test]
    async fn close_releases_queued_notices_and_rejects_future_reads() {
        // Arrange
        let resources = resources();
        let cleanup = TokioCleanupExecutor::new_with_job_timeout(
            MAX_WORKER_CONSOLE_RELEASE_JOBS,
            std::time::Duration::from_secs(1),
        )
        .unwrap();
        let installation = Installation::new(&resources, cleanup).unwrap();
        let owner = WorkerOwner::for_test(ObjectId::new(7), Arc::new(FakeCdpClient::from_html("")));
        installation
            .record(
                owner,
                &json!({
                    "executionContextId": 1,
                    "type": "log",
                    "args": [{"type": "string", "value": "hello"}],
                }),
            )
            .unwrap();

        // Act
        installation.close();
        let result = installation.next_notice().await;

        // Assert
        assert!(matches!(result, Err(HostError::TargetClosed)));
        let metrics = resources.metrics().unwrap();
        assert_eq!(metrics.class(ResourceClass::EventItem).current, 0);
        assert_eq!(metrics.class(ResourceClass::EventBytes).current, 0);
    }

    #[test]
    fn resource_profile_has_no_public_worker_console_queue_count_or_byte_limit() {
        // Arrange
        let required = requirements();

        // Act
        let items = required.class(ResourceClass::EventItem);
        let bytes = required.class(ResourceClass::EventBytes);

        // Assert
        assert_eq!(items.global, 0);
        assert_eq!(items.per_scope, 0);
        assert_eq!(bytes.global, 0);
        assert_eq!(bytes.per_scope, 0);
    }

    #[cfg(feature = "worker-console-argument-handles")]
    #[tokio::test]
    async fn closing_queued_notice_releases_adopted_argument_object_once() {
        // Arrange
        let resources = combined_resources();
        let cleanup = combined_cleanup();
        let worker_handles =
            crate::worker_handles::Installation::new(&resources, cleanup.clone()).unwrap();
        let installation = Installation::new_with_argument_handles(
            &resources,
            cleanup.clone(),
            Arc::clone(&worker_handles),
        )
        .unwrap();
        let client = Arc::new(FakeCdpClient::from_html(""));
        let owner = WorkerOwner::for_test(ObjectId::new(7), client.clone());
        installation
            .record(
                owner,
                &json!({
                    "executionContextId": 1,
                    "type": "log",
                    "args": [{
                        "type": "object",
                        "objectId": "worker-console-object",
                        "description": "Object"
                    }],
                }),
            )
            .unwrap();

        // Act
        installation.close();
        worker_handles.close();
        let report = cleanup.shutdown().await;

        // Assert
        assert_eq!(report.metrics.failed, 0);
        let releases = client
            .calls()
            .into_iter()
            .filter(|call| call.0 == "Runtime.releaseObject")
            .count();
        assert_eq!(releases, 1);
    }

    #[cfg(feature = "worker-console-argument-handles")]
    #[tokio::test]
    async fn argument_handles_grow_past_the_legacy_count_and_release_once() {
        // Arrange
        let resources = combined_resources();
        let cleanup = combined_cleanup();
        let worker_handles =
            crate::worker_handles::Installation::new(&resources, cleanup.clone()).unwrap();
        let installation = Installation::new_with_argument_handles(
            &resources,
            cleanup.clone(),
            Arc::clone(&worker_handles),
        )
        .unwrap();
        let client = Arc::new(FakeCdpClient::from_html(""));
        let owner = WorkerOwner::for_test(ObjectId::new(7), client.clone());
        let primitive_arguments = (0..=crate::worker_handles::MAX_WORKER_HANDLES)
            .map(|value| json!({"type": "number", "value": value}))
            .collect::<Vec<_>>();
        let retained = worker_handles
            .adopt_event_handles(Arc::clone(&owner), &primitive_arguments)
            .unwrap();

        // Act
        let result = installation.record(
            owner,
            &json!({
                "executionContextId": 1,
                "type": "log",
                "args": [{
                    "type": "object",
                    "objectId": "unadopted-console-object",
                    "description": "Object"
                }],
            }),
        );
        drop(retained);
        installation.close();
        worker_handles.close();
        let report = cleanup.shutdown().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(report.metrics.failed, 0);
        let releases = client
            .calls()
            .into_iter()
            .filter(|call| call.0 == "Runtime.releaseObject")
            .count();
        assert_eq!(releases, 1);
    }
}
