//! Native close work belongs to the one topology task, not caller futures.
//! Polling never commits local state: completions park until the driver owns a
//! transaction boundary. Stop drains written commands before releasing owners.
mod profile;
use crate::lifecycle::{DriverReply, DriverRequest, ordinary_close};
use crate::{CommandRuntime, HostError, LiveSession, frame_trees::FrameTrees};
use playwright_rust_backend_cdp_topology::{
    CdpContextOwnership, transition_reservations::TransitionReservations,
};
use playwright_rust_composition_chromium::ChromiumCloseCapacityBundle;
use playwright_rust_composition_chromium::native_close_authority::scheduler::{
    NativeCloseCompletion, NativeCloseScheduler,
};
use playwright_rust_composition_chromium::{
    AttachedChromiumComposition, ChromiumCloseObserver, ChromiumCloseTarget,
};
use playwright_rust_core::ResourceLease;
use playwright_rust_runtime::disposal_reservations::DisposalReservations;
use profile::MAX_CLOSES;
pub(crate) use profile::Profile;
use std::{
    collections::VecDeque,
    future::{Future, poll_fn},
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::sync::{Notify, oneshot};
use tokio_util::sync::CancellationToken;

struct Reply {
    target: ChromiumCloseTarget,
    sender: oneshot::Sender<Result<(), HostError>>,
}
impl Reply {
    fn finish(self, result: Result<(), HostError>) -> DriverReply {
        match self.target {
            ChromiumCloseTarget::Page(_) => {
                DriverReply::ClosePage(self.sender, ordinary_close(result))
            }
            ChromiumCloseTarget::BrowserContext(_) => {
                DriverReply::CloseContext(self.sender, result)
            }
        }
    }
}
struct PendingClose {
    capacity: ChromiumCloseCapacityBundle,
    reply: Reply,
}
#[derive(Default)]
struct Waiting {
    replies: VecDeque<Reply>,
}
impl Waiting {
    fn push(&mut self, reply: Reply) -> Result<(), HostError> {
        self.replies.try_reserve(1).map_err(HostError::backend)?;
        self.replies.push_back(reply);
        Ok(())
    }
    fn pop_live(&mut self) -> Option<Reply> {
        while let Some(reply) = self.replies.pop_front() {
            if !reply.sender.is_closed() {
                return Some(reply);
            }
        }
        None
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        self.replies.len()
    }
}
struct Selected {
    native: NativeCloseScheduler<PendingClose>,
    topology: TransitionReservations,
    runtime: DisposalReservations,
    observer: Arc<dyn ChromiumCloseObserver>,
    ready: [Option<NativeCloseCompletion<PendingClose>>; MAX_CLOSES],
    head: usize,
    len: usize,
    inflight: usize,
    waiting: Waiting,
    settled: VecDeque<DriverReply>,
    wakeup: Arc<Notify>,
    _storage: ResourceLease,
}
pub(crate) struct Jobs(Option<Box<Selected>>);
pub(crate) enum Admission {
    Queued,
    Other(DriverRequest),
}
impl Jobs {
    pub(crate) fn new(
        composition: &AttachedChromiumComposition,
        runtime: &CommandRuntime,
        trees: &Arc<FrameTrees>,
        stop: CancellationToken,
    ) -> Result<Self, HostError> {
        let Some(observer) = trees.close_observer()? else {
            return Ok(Self(None));
        };
        let profile = Profile::new(
            composition
                .topology()
                .map_err(HostError::backend)?
                .profile(),
        )?;
        let storage = profile::reserve_owner(composition.resources())?;
        let topology = profile
            .topology
            .bind(
                &*composition.topology().map_err(HostError::backend)?,
                composition.resources(),
            )
            .map_err(HostError::backend)?;
        let reservations = profile
            .runtime
            .bind(&runtime.lifecycle_handle(), composition.resources())
            .map_err(HostError::backend)?;
        let native = composition
            .native_close_scheduler(
                profile.scheduler,
                profile.authority,
                profile.pages,
                crate::close_services::NativeStop(stop),
            )
            .map_err(HostError::backend)?;
        Ok(Self(Some(Box::new(Selected {
            native,
            topology,
            runtime: reservations,
            observer,
            ready: std::array::from_fn(|_| None),
            head: 0,
            len: 0,
            inflight: 0,
            waiting: Waiting::default(),
            settled: VecDeque::new(),
            wakeup: Arc::new(Notify::new()),
            _storage: storage,
        }))))
    }
    pub(crate) fn wakeup(&self) -> Option<Arc<Notify>> {
        self.0.as_ref().map(|jobs| jobs.wakeup.clone())
    }
    pub(crate) async fn drive<F: Future>(&mut self, future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        poll_fn(|cx| {
            if let Some(jobs) = &mut self.0 {
                jobs.poll_native(cx);
            }
            future.as_mut().poll(cx)
        })
        .await
    }
    pub(crate) fn admit(
        &mut self,
        state: &Mutex<Option<LiveSession>>,
        request: DriverRequest,
    ) -> Result<Admission, HostError> {
        let Some(jobs) = &mut self.0 else {
            return Ok(Admission::Other(request));
        };
        let target = match &request {
            DriverRequest::ClosePage { page, .. } => ChromiumCloseTarget::Page(*page),
            DriverRequest::CloseContext { context, .. } => {
                ChromiumCloseTarget::BrowserContext(*context)
            }
            _ => return Ok(Admission::Other(request)),
        };
        let state_guard = state.lock().map_err(HostError::backend)?;
        let live = state_guard.as_ref().ok_or(HostError::TargetClosed)?;
        let topology = live.composition.topology().map_err(HostError::backend)?;
        let context = match target {
            ChromiumCloseTarget::Page(page) => topology
                .topology()
                .page_session_routes()
                .find(|route| route.object_id() == page)
                .map(|route| route.browser_context_object_id()),
            ChromiumCloseTarget::BrowserContext(context) => Some(context),
        };
        if context
            .and_then(|id| topology.topology().browser_context_route(id))
            .is_none_or(|route| route.ownership() != CdpContextOwnership::Owned)
        {
            return Ok(Admission::Other(request));
        }
        let sender = match request {
            DriverRequest::ClosePage { reply, .. } | DriverRequest::CloseContext { reply, .. } => {
                reply
            }
            _ => unreachable!("only close requests reach admission"),
        };
        if sender.is_closed() {
            return Ok(Admission::Queued);
        }
        let reply = Reply { target, sender };
        jobs.waiting.push(reply)?;
        drop(topology);
        drop(state_guard);
        self.promote_waiting(state)?;
        Ok(Admission::Queued)
    }
    pub(crate) fn finish_next(
        &mut self,
        state: &Mutex<Option<LiveSession>>,
    ) -> Result<Option<DriverReply>, HostError> {
        if let Some(reply) = self.0.as_mut().and_then(|jobs| jobs.settled.pop_front()) {
            return Ok(Some(reply));
        }
        let Some(completion) = self.0.as_mut().and_then(|jobs| jobs.pop_ready()) else {
            self.promote_waiting(state)?;
            return Ok(self.0.as_mut().and_then(|jobs| jobs.settled.pop_front()));
        };
        let reply = completion.consume(|payload, result| {
            let evidence = match result {
                Ok(evidence) => evidence,
                Err(error) => {
                    return Ok(Some(payload.reply.finish(Err(HostError::operation(
                        playwright_rust_core::OperationError::<()>::new(
                            error.kind(),
                            error.message(),
                            None,
                        ),
                        0,
                    )))));
                }
            };
            let PendingClose { capacity, reply } = payload;
            let runtime = state
                .lock()
                .map_err(HostError::backend)?
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .runtime
                .clone();
            let committed = {
                let state = state.lock().map_err(HostError::backend)?;
                let live = state.as_ref().ok_or(HostError::TargetClosed)?;
                live.composition
                    .retire_native_close(evidence, capacity, &runtime, None)
                    .map_err(HostError::backend)?
            };
            // Publication failure after native success is terminal, never a
            // recoverable operation error followed by more lifecycle mutations.
            committed.notify().map_err(HostError::backend)?;
            Ok(Some(reply.finish(Ok(()))))
        })?;
        if let Some(jobs) = &mut self.0 {
            jobs.inflight = jobs
                .inflight
                .checked_sub(1)
                .expect("ready close completion owns one in-flight slot");
        }
        self.promote_waiting(state)?;
        Ok(reply)
    }
    fn promote_waiting(&mut self, state: &Mutex<Option<LiveSession>>) -> Result<(), HostError> {
        let Some(jobs) = &mut self.0 else {
            return Ok(());
        };
        while jobs.inflight < MAX_CLOSES {
            let Some(reply) = jobs.waiting.pop_live() else {
                break;
            };
            jobs.settled.try_reserve(1).map_err(HostError::backend)?;
            let target = reply.target;
            let capacity = {
                let state = state.lock().map_err(HostError::backend)?;
                let Some(live) = state.as_ref() else {
                    jobs.settled
                        .push_back(reply.finish(Err(HostError::TargetClosed)));
                    continue;
                };
                let topology = live.composition.topology().map_err(HostError::backend)?;
                ChromiumCloseCapacityBundle::reserve(
                    target,
                    &topology,
                    &jobs.topology,
                    &jobs.runtime,
                    jobs.observer.as_ref(),
                )
            };
            let capacity = match capacity {
                Ok(capacity) => capacity,
                Err(error) => {
                    jobs.settled
                        .push_back(reply.finish(Err(capacity_error(error))));
                    continue;
                }
            };
            let payload = PendingClose { capacity, reply };
            let result = match target {
                ChromiumCloseTarget::Page(page) => jobs.native.push_page(page, payload),
                ChromiumCloseTarget::BrowserContext(context) => {
                    jobs.native.push_context(context, payload)
                }
            };
            match result {
                Ok(()) => jobs.inflight += 1,
                Err((payload, error)) => jobs
                    .settled
                    .push_back(payload.reply.finish(Err(HostError::operation(error, 0)))),
            }
        }
        if !jobs.settled.is_empty() {
            jobs.wakeup.notify_one();
        }
        Ok(())
    }
    pub(crate) async fn drain(&mut self) {
        let Some(jobs) = &mut self.0 else {
            return;
        };
        // Stop is already signalled. Do not locally publish successful close
        // evidence during terminal teardown, but retain every written command.
        while let Some(reply) = jobs.waiting.replies.pop_front() {
            let _ = reply.finish(Err(HostError::TargetClosed)).send();
        }
        while let Some(reply) = jobs.settled.pop_front() {
            let _ = reply.send();
        }
        while jobs.pop_ready().is_some() {}
        while poll_fn(|cx| jobs.native.poll_next(cx)).await.is_some() {}
    }
}
impl Selected {
    fn poll_native(&mut self, cx: &mut Context<'_>) {
        while self.len < MAX_CLOSES {
            let Poll::Ready(Some(completion)) = self.native.poll_next(cx) else {
                break;
            };
            self.ready[(self.head + self.len) % MAX_CLOSES] = Some(completion);
            self.len += 1;
            self.wakeup.notify_one();
        }
    }
    fn pop_ready(&mut self) -> Option<NativeCloseCompletion<PendingClose>> {
        if self.len == 0 {
            return None;
        }
        let completion = self.ready[self.head].take();
        self.head = (self.head + 1) % MAX_CLOSES;
        self.len -= 1;
        if self.len != 0 {
            self.wakeup.notify_one();
        }
        completion
    }
}
pub(crate) async fn notified(wakeup: &Option<Arc<Notify>>) {
    match wakeup {
        Some(signal) => signal.notified().await,
        None => std::future::pending().await,
    }
}
fn capacity_error(
    error: playwright_rust_composition_chromium::ChromiumCloseCapacityBundleError,
) -> HostError {
    use playwright_rust_backend_cdp_topology::CdpTopologyProductionError;
    use playwright_rust_backend_cdp_topology::transition_reservations::TransitionReservationError as Topology;
    use playwright_rust_composition_chromium::ChromiumCloseCapacityBundleError as Error;
    use playwright_rust_runtime::RuntimeStateError;
    use playwright_rust_runtime::disposal_reservations::DisposalReservationError as Runtime;
    match error {
        Error::Publication(error) => HostError::operation(error, 0),
        Error::Runtime(Runtime::Runtime(RuntimeStateError::TransportClosed))
        | Error::Topology(Topology::EpochChanged { .. }) => HostError::TargetClosed,
        Error::Topology(Topology::SlotsExhausted { .. })
        | Error::Runtime(Runtime::SlotsExhausted { .. })
        | Error::Runtime(Runtime::Runtime(
            RuntimeStateError::ControlMessageCapacityExceeded { .. }
            | RuntimeStateError::LifecycleTransactionInProgress
            | RuntimeStateError::LifecycleTransactionIdExhausted
            | RuntimeStateError::Resource(_),
        ))
        | Error::Topology(Topology::Production(
            CdpTopologyProductionError::Resource(_)
            | CdpTopologyProductionError::TransactionInProgress
            | CdpTopologyProductionError::TransactionIdExhausted,
        )) => HostError::ResourceExhausted(error.to_string()),
        other => HostError::backend(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playwright_rust_composition_chromium::ChromiumCloseCapacityBundleError as Capacity;
    use playwright_rust_core::ObjectId;
    use playwright_rust_runtime::{
        RuntimeStateError, disposal_reservations::DisposalReservationError,
    };

    #[test]
    fn waiting_close_ownership_grows_past_the_execution_lane() {
        // Arrange
        let mut waiting = Waiting::default();
        let mut receivers = Vec::new();

        // Act
        for id in 1..=129 {
            let (sender, receiver) = oneshot::channel();
            waiting
                .push(Reply {
                    target: ChromiumCloseTarget::Page(ObjectId::new(id)),
                    sender,
                })
                .unwrap();
            receivers.push(receiver);
        }

        // Assert
        assert_eq!(waiting.len(), 129);
        assert_eq!(receivers.len(), 129);
        assert_eq!(
            waiting.pop_live().unwrap().target,
            ChromiumCloseTarget::Page(ObjectId::new(1))
        );
        assert_eq!(waiting.len(), 128);
    }

    #[test]
    fn cancelled_waiter_is_removed_before_native_admission() {
        // Arrange
        let mut waiting = Waiting::default();
        let (cancelled_sender, cancelled_receiver) = oneshot::channel();
        let (live_sender, _live_receiver) = oneshot::channel();
        waiting
            .push(Reply {
                target: ChromiumCloseTarget::Page(ObjectId::new(1)),
                sender: cancelled_sender,
            })
            .unwrap();
        waiting
            .push(Reply {
                target: ChromiumCloseTarget::Page(ObjectId::new(2)),
                sender: live_sender,
            })
            .unwrap();
        drop(cancelled_receiver);

        // Act
        let next = waiting.pop_live().unwrap();

        // Assert
        assert_eq!(next.target, ChromiumCloseTarget::Page(ObjectId::new(2)));
        assert_eq!(waiting.len(), 0);
    }

    #[test]
    fn full_outbox_is_an_admission_error_not_terminal_peer_loss() {
        // Arrange
        let error = Capacity::Runtime(DisposalReservationError::Runtime(
            RuntimeStateError::ControlMessageCapacityExceeded {
                required: 256,
                remaining: 0,
                limit: 256,
            },
        ));
        // Act
        let result = capacity_error(error);
        // Assert
        assert!(matches!(result, HostError::ResourceExhausted(_)));
    }

    #[test]
    fn closed_transport_keeps_its_terminal_classification() {
        // Arrange
        let error = Capacity::Runtime(DisposalReservationError::Runtime(
            RuntimeStateError::TransportClosed,
        ));
        // Act
        let result = capacity_error(error);
        // Assert
        assert!(matches!(result, HostError::TargetClosed));
    }

    #[tokio::test]
    async fn page_terminal_reply_remains_idempotent() {
        // Arrange
        let (sender, receiver) = oneshot::channel();
        let reply = Reply {
            target: ChromiumCloseTarget::Page(ObjectId::new(3)),
            sender,
        };
        // Act
        let send_result = reply.finish(Err(HostError::TargetClosed)).send();
        // Assert
        assert!(send_result.is_ok());
        assert!(receiver.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn context_terminal_reply_does_not_claim_native_success() {
        // Arrange
        let (sender, receiver) = oneshot::channel();
        let reply = Reply {
            target: ChromiumCloseTarget::BrowserContext(ObjectId::new(2)),
            sender,
        };
        // Act
        let send_result = reply.finish(Err(HostError::TargetClosed)).send();
        // Assert
        assert!(send_result.is_ok());
        assert!(matches!(
            receiver.await.unwrap(),
            Err(HostError::TargetClosed)
        ));
    }
}
