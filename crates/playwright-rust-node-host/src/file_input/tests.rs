use super::*;
use playwright_rust_core::{ResourceClass, ResourceLimits};

#[test]
fn host_call_and_source_owners_are_allocator_backed_beyond_legacy_caps() {
    // Arrange
    let requirements = profile::state_requirements();
    let limits = ResourceLimits::deny_all(requirements.scopes, requirements.leases).with_class(
        ResourceClass::BackendStateBytes,
        profile::STATE_BYTES,
        profile::STATE_BYTES,
    );
    let resources =
        playwright_rust_core::ProductionResourceLedger::new(limits, requirements).unwrap();
    let ledger = resources.claim(requirements).unwrap();
    let state = ledger
        .reserve(
            profile::STATE_SCOPE,
            ResourceClass::BackendStateBytes,
            profile::STATE_BYTES,
        )
        .unwrap();
    let slots = Arc::new(Slots {
        calls: AtomicUsize::new(0),
        sources: AtomicUsize::new(0),
        _state: state,
    });

    // Act
    let calls: Vec<_> = (0..129).map(|_| slots.call().unwrap()).collect();
    let sources: Vec<_> = (0..129).map(|_| slots.source().unwrap()).collect();

    // Assert
    assert_eq!(slots.calls.load(Ordering::Acquire), 129);
    assert_eq!(slots.sources.load(Ordering::Acquire), 129);
    drop(calls);
    drop(sources);
    assert_eq!(slots.calls.load(Ordering::Acquire), 0);
    assert_eq!(slots.sources.load(Ordering::Acquire), 0);
}

#[cfg(feature = "file-chooser-native-paths")]
#[tokio::test]
async fn colocated_file_chooser_paths_target_the_utility_world_object() {
    // Arrange
    let client = playwright_rust_fake::FakeCdpClient::from_html("<input type=file>");
    client.set_adopted_node_response(
        47,
        73,
        serde_json::json!({
            "object": {
                "type": "object",
                "subtype": "node",
                "objectId": "utility-input",
            },
        }),
    );
    let files = vec!["/tmp/first.txt".to_owned(), "/tmp/second.txt".to_owned()];

    // Act
    dispatch_colocated_paths(&client, 47, 73, "chooser-group", &files)
        .await
        .expect("native path dispatch");

    // Assert
    assert_eq!(
        client.calls(),
        [
            (
                "DOM.resolveNode".into(),
                vec![serde_json::json!({
                    "backendNodeId": 47,
                    "executionContextId": 73,
                    "objectGroup": "chooser-group",
                })],
            ),
            (
                "DOM.setFileInputFiles".into(),
                vec![serde_json::json!({
                    "objectId": "utility-input",
                    "files": ["/tmp/first.txt", "/tmp/second.txt"],
                })],
            ),
        ]
    );
}

#[cfg(feature = "retained-file-input")]
#[test]
fn independent_adoption_and_mutation_cleanup_failures_are_both_reported() {
    // Arrange
    let adoption = Err(HostError::backend("adoption release unconfirmed"));
    let mutation = Err(HostError::backend("upload release unconfirmed"));
    // Act
    let error = combine_native_cleanup(adoption, mutation).unwrap_err();
    // Assert
    assert!(error.to_string().contains("adoption release unconfirmed"));
    assert!(error.to_string().contains("upload release unconfirmed"));
}

#[test]
fn selected_uploads_do_not_widen_ordinary_cdp_command_or_receive_limits() {
    // Arrange
    let timeout = std::time::Duration::from_secs(5);
    // Act
    let factory = crate::profile::attach_factory(timeout).unwrap();
    // Assert
    let config = factory.profile().connection().config();
    let expected_command_bytes = if cfg!(feature = "large-evaluation-inputs") {
        8 * 1024 * 1024
    } else {
        4 * 1024 * 1024
    };
    let expected_websocket_bytes = if cfg!(any(
        feature = "large-evaluation-inputs",
        feature = "large-route-post-data"
    )) {
        8 * 1024 * 1024
    } else {
        4 * 1024 * 1024
    };
    assert_eq!(config.max_command_bytes, expected_command_bytes);
    assert_eq!(config.max_response_bytes, 4 * 1024 * 1024);
    assert_eq!(config.max_websocket_bytes, expected_websocket_bytes);
    assert_eq!(profile::large_commands().unwrap().capacity(), usize::MAX);
    assert!(profile::large_commands().unwrap().allocator_backed());
}

#[test]
fn source_and_encoded_transport_have_independent_requirements() {
    // Arrange
    let limits = profile::limits();
    // Act
    let wire = profile::large_commands().unwrap().requirements();
    // Assert
    assert_eq!(limits.max_items(), 64);
    assert_eq!(wire.class(ResourceClass::ArtifactBytes).global, 0);
    assert_eq!(wire.class(ResourceClass::WireBytes).global, 0);
    assert!(wire.class(ResourceClass::BackendStateBytes).global > 0);
}
