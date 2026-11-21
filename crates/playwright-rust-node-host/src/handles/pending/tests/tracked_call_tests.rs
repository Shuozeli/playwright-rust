use super::*;
use std::{
    future::Future,
    task::{Context, Waker},
};

fn retained(
    owners: &Arc<Owners>,
    cleanup: &TokioCleanupExecutor,
    released: &Arc<AtomicUsize>,
) -> PendingHandle {
    let (initializer, ticket, owner) =
        ticket(2, owners.admit().unwrap(), cleanup, released.clone());
    owners.retain(initializer, ticket, owner).unwrap()
}

fn bytes(resources: &ProductionResourceLedger) -> u64 {
    resources
        .metrics()
        .unwrap()
        .class(ResourceClass::BackendStateBytes)
        .current
}

#[tokio::test]
async fn tracked_calls_grow_past_the_legacy_lane_without_native_or_owner_admission() {
    // Arrange
    let (owners, resources) = owners();
    let cleanup = TokioCleanupExecutor::new(1);
    let released = Arc::new(AtomicUsize::new(0));
    let pending = retained(&owners, &cleanup, &released);
    let ordinary = (0..MAX_HANDLES)
        .map(|_| owners.admit_call().unwrap())
        .collect::<Vec<_>>();

    // Act
    let tracked = owners.admit_tracked_call(&pending).unwrap();
    let admitted = resources.metrics().unwrap();
    let caller = tracked.call.clone();
    let additional_tracked = owners.admit_tracked_call(&pending).unwrap();
    let additional_read = owners.admit_tracked_read(&pending).unwrap();
    let additional_ordinary = owners.admit_call().unwrap();

    // Assert
    assert_eq!(resources.metrics().unwrap(), admitted);
    assert_eq!(
        owners.capacity.calls.load(Ordering::Acquire),
        MAX_HANDLES + 4
    );
    assert_eq!(owners.capacity.owners.load(Ordering::Acquire), 1);
    assert_eq!(admitted.class(ResourceClass::RemoteHandle).current, 0);
    assert_eq!(admitted.class(ResourceClass::RemoteHandle).peak, 0);
    assert_eq!(bytes(&resources), STATE_BYTES);
    drop(tracked);
    assert_eq!(
        owners.capacity.calls.load(Ordering::Acquire),
        MAX_HANDLES + 4
    );
    drop(caller);
    assert_eq!(
        owners.capacity.calls.load(Ordering::Acquire),
        MAX_HANDLES + 3
    );
    drop((additional_tracked, additional_read, additional_ordinary));
    drop(ordinary);
    pending.dispose().await.unwrap();
    drop(pending);
    owners.close();
    owners.wait().await;
    let report = cleanup.shutdown().await;
    assert_eq!(released.load(Ordering::Acquire), 1);
    assert_eq!(report.metrics.completed, 1);
    assert_eq!(report.metrics.failed, 0);
    assert_eq!(report.metrics.reserved, 0);
    assert_eq!(bytes(&resources), STATE_BYTES);
}

#[tokio::test]
async fn dropping_allocator_backed_tracked_calls_reclaims_exactly_one_owner() {
    // Arrange
    let (owners, resources) = owners();
    let cleanup = TokioCleanupExecutor::new(1);
    let released = Arc::new(AtomicUsize::new(0));
    let pending = retained(&owners, &cleanup, &released);
    let mut calls = (0..MAX_HANDLES)
        .map(|_| owners.admit_tracked_call(&pending).unwrap())
        .collect::<Vec<_>>();
    let full = resources.metrics().unwrap();

    // Act
    drop(calls.pop().unwrap());
    let released_slot = resources.metrics().unwrap();
    let replacement = owners.admit_tracked_call(&pending).unwrap();
    let restored = resources.metrics().unwrap();
    let additional = owners.admit_tracked_call(&pending).unwrap();

    // Assert
    assert_eq!(released_slot, full);
    assert_eq!(restored, full);
    assert_eq!(restored.class(ResourceClass::RemoteHandle).peak, 0);
    assert_eq!(owners.capacity.owners.load(Ordering::Acquire), 1);
    assert_eq!(
        owners.capacity.calls.load(Ordering::Acquire),
        MAX_HANDLES + 1
    );
    drop((calls, replacement, additional));
    pending.dispose().await.unwrap();
    drop(pending);
    owners.close();
    owners.wait().await;
    assert_eq!(cleanup.shutdown().await.metrics.completed, 1);
    assert_eq!(released.load(Ordering::Acquire), 1);
    assert_eq!(bytes(&resources), STATE_BYTES);
}

#[tokio::test]
async fn close_waits_for_tracked_call_work_after_source_cleanup_and_descriptor_drop() {
    // Arrange
    let (owners, resources) = owners();
    let cleanup = TokioCleanupExecutor::new(1);
    let released = Arc::new(AtomicUsize::new(0));
    let pending = retained(&owners, &cleanup, &released);
    let TrackedCallAdmission { call, work } = owners.admit_tracked_call(&pending).unwrap();
    pending.dispose().await.unwrap();
    drop(pending);
    drop(call);
    assert_eq!(released.load(Ordering::Acquire), 1);
    assert_eq!(bytes(&resources), STATE_BYTES);

    // Act
    owners.close();
    let mut waiting = Box::pin(owners.wait());
    let mut context = Context::from_waker(Waker::noop());
    let while_tracked = waiting.as_mut().poll(&mut context);
    drop(work);
    let after_completion = waiting.as_mut().poll(&mut context);

    // Assert
    assert!(while_tracked.is_pending());
    assert!(after_completion.is_ready());
    assert_eq!(owners.capacity.owners.load(Ordering::Acquire), 0);
    assert_eq!(owners.capacity.calls.load(Ordering::Acquire), 0);
    let metrics = resources.metrics().unwrap();
    assert_eq!(metrics.active_leases, 1);
    assert_eq!(metrics.class(ResourceClass::RemoteHandle).peak, 0);
    let report = cleanup.shutdown().await;
    assert_eq!(report.metrics.completed, 1);
    assert_eq!(report.metrics.failed, 0);
    assert_eq!(report.metrics.reserved, 0);
}

#[tokio::test]
async fn explicit_disposal_wins_and_invalidates_a_previously_admitted_source() {
    // Arrange
    let (owners, resources) = owners();
    let cleanup = TokioCleanupExecutor::new(1);
    let released = Arc::new(AtomicUsize::new(0));
    let pending = retained(&owners, &cleanup, &released);
    let tracked = owners.admit_tracked_call(&pending).unwrap();

    // Act
    pending.dispose().await.unwrap();
    let after_disposal = resources.metrics().unwrap();
    let revalidated = owners.check_arguments(std::iter::once(&pending));
    let next_call = owners.admit_tracked_call(&pending);

    // Assert
    assert_eq!(released.load(Ordering::Acquire), 1);
    assert!(
        matches!(revalidated, Err(HostError::InvalidArgument(message)) if message == "JSHandle is disposed")
    );
    assert!(
        matches!(next_call, Err(HostError::InvalidArgument(message)) if message == "JSHandle is disposed")
    );
    assert_eq!(resources.metrics().unwrap(), after_disposal);
    assert_eq!(owners.capacity.calls.load(Ordering::Acquire), 1);
    assert_eq!(after_disposal.class(ResourceClass::RemoteHandle).peak, 0);
    assert_eq!(bytes(&resources), STATE_BYTES);
    drop((tracked, pending));
    owners.close();
    owners.wait().await;
    let report = cleanup.shutdown().await;
    assert_eq!(report.metrics.completed, 1);
    assert_eq!(report.metrics.reserved, 0);
    assert_eq!(bytes(&resources), STATE_BYTES);
}

#[tokio::test]
async fn foreign_tracked_call_rejects_equal_numeric_identity_without_admission() {
    // Arrange
    let (owner, resources) = owners();
    let (foreign, foreign_resources) = owners();
    let cleanup = TokioCleanupExecutor::new(2);
    let released = Arc::new(AtomicUsize::new(0));
    let local = retained(&owner, &cleanup, &released);
    let other = retained(&foreign, &cleanup, &released);
    assert_eq!(local.initializer().object_id, other.initializer().object_id);
    let baseline = resources.metrics().unwrap();
    let foreign_baseline = foreign_resources.metrics().unwrap();

    // Act
    let result = owner.admit_tracked_call(&other);

    // Assert
    assert!(
        matches!(result, Err(HostError::InvalidArgument(message)) if message == "Handle belongs to another session")
    );
    assert_eq!(resources.metrics().unwrap(), baseline);
    assert_eq!(foreign_resources.metrics().unwrap(), foreign_baseline);
    assert_eq!(owner.capacity.calls.load(Ordering::Acquire), 0);
    local.dispose().await.unwrap();
    other.dispose().await.unwrap();
    drop((local, other));
    owner.close();
    foreign.close();
    owner.wait().await;
    foreign.wait().await;
    assert_eq!(cleanup.shutdown().await.metrics.completed, 2);
    assert_eq!(released.load(Ordering::Acquire), 2);
    assert_eq!(bytes(&resources), STATE_BYTES);
    assert_eq!(bytes(&foreign_resources), STATE_BYTES);
}

#[tokio::test]
async fn closed_owners_reject_tracked_calls_without_reviving_work_or_admission() {
    // Arrange
    let (owners, resources) = owners();
    let cleanup = TokioCleanupExecutor::new(1);
    let released = Arc::new(AtomicUsize::new(0));
    let pending = retained(&owners, &cleanup, &released);
    owners.close();
    owners.wait().await;
    let baseline = resources.metrics().unwrap();

    // Act
    let result = owners.admit_tracked_call(&pending);

    // Assert
    assert!(matches!(result, Err(HostError::TargetClosed)));
    assert_eq!(resources.metrics().unwrap(), baseline);
    assert_eq!(owners.capacity.calls.load(Ordering::Acquire), 0);
    let mut waiting = Box::pin(owners.wait());
    assert!(
        waiting
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    assert_eq!(released.load(Ordering::Acquire), 1);
    pending.dispose().await.unwrap();
    drop(pending);
    assert_eq!(cleanup.shutdown().await.metrics.completed, 1);
    assert_eq!(bytes(&resources), STATE_BYTES);
}
