use super::*;
use crate::{
    context_metadata::ContextMetadataLimits,
    frame_metadata::{FrameMetadata, FrameMetadataLimits},
};

fn fixture(publications: usize) -> (Arc<FrameMetadata>, FrameTrees, ContextMetadata) {
    let book = Arc::new(
        FrameMetadata::new(FrameMetadataLimits {
            entries: 128,
            url_bytes: 64,
            total_bytes: 8192,
            publications,
        })
        .unwrap(),
    );
    let trees = FrameTrees::new(book.clone()).unwrap();
    let contexts = ContextMetadata::new(ContextMetadataLimits {
        active_contexts: 16,
        states: 256,
        conversions: 128,
        base_url_bytes: 64,
        bytes: 8192,
    })
    .unwrap();
    (book, trees, contexts)
}

#[test]
fn selected_retirement_pool_grows_with_creation_and_is_reclaimed_on_native_clear() {
    // Arrange
    let (book, trees, contexts) = fixture(64);
    let factory = trees.factory.clone();
    let before = factory.metrics().unwrap();
    let trees = trees.with_collections(contexts).unwrap();
    let selected = factory.metrics().unwrap();
    let reserved = trees.reserve_creation_collection().unwrap();
    let admitted = factory.metrics().unwrap();
    reserved.commit().unwrap();
    // Act
    trees.clear();
    // Assert
    assert!(admitted.revisions > selected.revisions);
    assert_eq!(factory.metrics().unwrap(), before);
    assert!(
        book.publications()
            .published_collections()
            .unwrap()
            .is_none()
    );
    println!(
        "Selected collection pool: bytes={}, revisions={}",
        selected.bytes - before.bytes,
        selected.revisions - before.revisions
    );
}

#[test]
fn collection_selection_does_not_preallocate_a_fixed_retirement_count() {
    // Arrange
    let (book, trees, contexts) = fixture(16);
    let factory = trees.factory.clone();
    let before = factory.metrics().unwrap();
    // Act
    let result = trees.with_collections(contexts);
    // Assert
    assert!(result.is_ok());
    assert!(factory.metrics().unwrap().revisions <= before.revisions + 1);
    assert_eq!(book.metrics().unwrap().pending_publications, 0);
}

#[test]
fn abandoned_creation_rolls_back_dynamic_retirement_admission() {
    // Arrange
    let (_book, trees, contexts) = fixture(33);
    let trees = trees.with_collections(contexts).unwrap();
    let before = trees.factory.metrics().unwrap();
    // Act
    let creation = trees.reserve_creation_collection().unwrap();
    // Assert
    assert_eq!(
        trees
            .state
            .lock()
            .unwrap()
            .collections
            .as_ref()
            .unwrap()
            .retirements
            .len(),
        1
    );
    drop(creation);
    assert_eq!(trees.factory.metrics().unwrap(), before);
    assert_eq!(
        trees
            .state
            .lock()
            .unwrap()
            .collections
            .as_ref()
            .unwrap()
            .retirements
            .len(),
        0
    );
}

#[test]
fn terminal_selection_admits_groups_dynamically_and_releases_on_clear() {
    // Arrange
    let (book, trees, contexts) = fixture(96);
    let factory = trees.factory.clone();
    let baseline = factory.metrics().unwrap();
    let trees = trees.with_collections(contexts).unwrap();
    let collections_only = factory.metrics().unwrap();
    // Act
    let trees = trees.with_terminal_publication().unwrap();
    let selected = factory.metrics().unwrap();
    trees.clear();
    // Assert
    assert!(selected.revisions <= collections_only.revisions + 1);
    assert!(selected.bytes > collections_only.bytes);
    assert_eq!(factory.metrics().unwrap(), baseline);
    assert!(book.publications().is_drained().unwrap());
    eprintln!(
        "Selected terminal source: bytes={}, revisions={}",
        selected.bytes - collections_only.bytes,
        selected.revisions - collections_only.revisions
    );
}

#[test]
fn terminal_selection_succeeds_without_a_fixed_group_reservation() {
    // Arrange
    let (book, trees, contexts) = fixture(33);
    let factory = trees.factory.clone();
    let baseline = factory.metrics().unwrap();
    let trees = trees.with_collections(contexts).unwrap();
    // Act
    let result = trees.with_terminal_publication();
    // Assert
    assert!(result.is_ok());
    drop(result);
    assert_eq!(factory.metrics().unwrap(), baseline);
    assert_eq!(book.metrics().unwrap().pending_publications, 0);
}
