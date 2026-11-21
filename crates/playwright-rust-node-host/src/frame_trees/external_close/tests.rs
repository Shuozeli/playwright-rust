use super::*;
use crate::frame_trees::close_events::tests::Fixture;
use crate::{LifecycleDelivery, LifecycleNotice};
use playwright_rust_core::ControlMessage;

const CONTEXT: ObjectId = ObjectId::new(1000);
const FIRST: ObjectId = ObjectId::new(1);
const SECOND: ObjectId = ObjectId::new(3);

fn dispose(id: ObjectId) -> [CdpTopologyEffect; 1] {
    [CdpTopologyEffect::Lifecycle(ControlMessage::Dispose {
        object_id: id,
    })]
}

fn remaining(fixture: &Fixture) -> usize {
    fixture
        .owner
        .state
        .lock()
        .unwrap()
        .collections
        .as_ref()
        .unwrap()
        .external
        .as_ref()
        .unwrap()
        .len()
}

#[test]
fn external_close_uses_preallocated_storage_and_publishes_stepwise() {
    // Arrange
    let fixture = Fixture::external();
    let first = fixture.owner.get(FIRST).unwrap();
    let second = fixture.owner.get(SECOND).unwrap();
    let before = fixture.owner.factory.metrics().unwrap();
    let admitted = remaining(&fixture);
    // Act
    fixture.owner.external_disposals(&dispose(CONTEXT)).unwrap();
    // Assert
    assert_eq!(fixture.owner.factory.metrics().unwrap(), before);
    assert_eq!(remaining(&fixture), admitted - 3);
    assert!(!first.is_closed().unwrap());
    assert!(!second.is_closed().unwrap());
    let mut drain = fixture.book.fence().unwrap().try_begin().unwrap();
    assert_eq!(
        drain.commit_notice().unwrap(),
        LifecycleNotice::PageClose {
            context: CONTEXT,
            page: FIRST
        }
    );
    assert!(first.is_closed().unwrap());
    assert!(!second.is_closed().unwrap());
    drain
        .acknowledge_notice(LifecycleDelivery::ListenerThrew)
        .unwrap();
    assert_eq!(
        drain.commit_notice().unwrap(),
        LifecycleNotice::PageClose {
            context: CONTEXT,
            page: SECOND
        }
    );
    drain
        .acknowledge_notice(LifecycleDelivery::Delivered)
        .unwrap();
    assert_eq!(
        drain.commit_notice().unwrap(),
        LifecycleNotice::ContextClose { context: CONTEXT }
    );
    drain
        .acknowledge_notice(LifecycleDelivery::Delivered)
        .unwrap();
}

#[test]
fn external_close_does_not_need_a_new_queue_or_factory_reservation() {
    // Arrange: exhaust fresh lifecycle admission while keeping the cold pool.
    let fixture = Fixture::external();
    let mut held = Vec::new();
    while let Ok(reserved) = fixture.book.reserve_lifecycle(&fixture.owner.factory) {
        held.push(reserved);
    }
    let before = fixture.owner.factory.metrics().unwrap();
    // Act
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    // Assert
    assert_eq!(fixture.owner.factory.metrics().unwrap(), before);
    assert_eq!(
        fixture.drain(),
        vec![LifecycleNotice::PageClose {
            context: CONTEXT,
            page: FIRST
        }]
    );
}

#[test]
fn duplicate_external_receipt_does_not_consume_another_slot() {
    // Arrange
    let fixture = Fixture::external();
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    let before = remaining(&fixture);
    // Act
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    // Assert
    assert_eq!(remaining(&fixture), before);
    assert_eq!(fixture.drain().len(), 1);
}

#[test]
fn explicit_close_receipt_is_not_replayed_by_external_followup() {
    // Arrange
    let fixture = Fixture::external();
    let admitted = remaining(&fixture);
    fixture.explicit_close(&[FIRST]);
    // Act
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    // Assert
    assert_eq!(remaining(&fixture), admitted);
    assert_eq!(fixture.drain().len(), 1);
}

#[test]
fn abandoned_creation_restores_previous_external_capacity() {
    // Arrange
    let fixture = Fixture::external();
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    let prior = remaining(&fixture);
    let before = fixture.owner.factory.metrics().unwrap();
    let creation = fixture.owner.reserve_creation_collection().unwrap();
    assert_eq!(remaining(&fixture), prior + 2);
    // Act
    drop(creation);
    // Assert
    assert_eq!(remaining(&fixture), prior);
    assert_eq!(fixture.owner.factory.metrics().unwrap(), before);
}

#[test]
fn successful_creation_keeps_replenished_external_capacity() {
    // Arrange
    let fixture = Fixture::external();
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    let prior = remaining(&fixture);
    let creation = fixture.owner.reserve_creation_collection().unwrap();
    // Act
    creation.commit().unwrap();
    // Assert
    assert_eq!(remaining(&fixture), prior + 2);
}

#[test]
fn partial_external_pool_replenishment_failure_releases_all_new_storage() {
    // Arrange
    let fixture = Fixture::external();
    let mut held = Vec::new();
    while let Ok(reserved) = fixture.book.reserve_lifecycle(&fixture.owner.factory) {
        held.push(reserved);
    }
    assert!(held.len() > 2);
    for _ in 0..2 {
        held.pop();
    }
    let before = fixture.owner.factory.metrics().unwrap();
    let mut pool = ExternalClosePool::reserve(&fixture.book, &fixture.owner.factory).unwrap();
    // Act
    let result = pool.replenish(3, &fixture.book, &fixture.owner.factory);
    // Assert
    assert!(result.is_err());
    assert_eq!(pool.len(), 0);
    drop(pool);
    assert_eq!(fixture.owner.factory.metrics().unwrap(), before);
}

#[test]
fn terminal_handoff_preserves_external_close_masks() {
    // Arrange
    let fixture = Fixture::external();
    fixture.owner.external_disposals(&dispose(FIRST)).unwrap();
    // Act
    fixture.owner.clear();
    // Assert
    assert_eq!(
        fixture.drain(),
        vec![
            LifecycleNotice::PageClose {
                context: CONTEXT,
                page: FIRST
            },
            LifecycleNotice::PageClose {
                context: CONTEXT,
                page: SECOND
            },
            LifecycleNotice::ContextClose { context: CONTEXT },
        ]
    );
}

#[test]
fn exhausted_external_pool_fails_closed_without_partial_publication() {
    // Arrange: inject a violated reservation invariant, not a real native event.
    let fixture = Fixture::external();
    fixture
        .owner
        .state
        .lock()
        .unwrap()
        .collections
        .as_mut()
        .unwrap()
        .external
        .as_mut()
        .unwrap()
        .truncate(0);
    // Act
    let result = fixture.owner.external_disposals(&dispose(CONTEXT));
    // Assert
    assert!(result.is_err());
    assert!(fixture.owner.state.lock().unwrap().closed);
    assert_eq!(
        fixture.owner.urls.metrics().unwrap().pending_publications,
        0
    );
    fixture.owner.clear();
    assert_eq!(fixture.drain().len(), 3);
}
