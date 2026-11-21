use super::*;
use playwright_rust_core::ResourceLimits;

fn resources() -> ProductionResourceLedger {
    let required = requirements().unwrap();
    let mut limits = ResourceLimits::deny_all(required.scopes, required.leases);
    for class in ResourceClass::ALL {
        let bound = required.class(class);
        limits = limits.with_class(class, bound.global, bound.per_scope);
    }
    ProductionResourceLedger::new(limits, required).unwrap()
}

#[test]
fn retained_invocation_keeps_fixed_installation_state_until_last_owner_drops() {
    // Arrange
    let resources = resources();
    let installation = Installation::new(&resources, "macos").unwrap();
    let lease = installation.admit(17).unwrap();
    // Act
    drop(installation);
    // Assert
    assert_eq!(resources.metrics().unwrap().active_leases, 1);
    assert_eq!(
        resources
            .metrics()
            .unwrap()
            .class(ResourceClass::BackendStateBytes)
            .current,
        STATE_BYTES
    );
    drop(lease);
    assert_eq!(resources.metrics().unwrap().active_leases, 0);
}

#[test]
fn allocator_backed_invocation_ownership_grows_past_legacy_count() {
    // Arrange
    let resources = resources();
    let installation = Installation::new(&resources, "linux").unwrap();
    let leases: Vec<_> = (0..129).map(|_| installation.admit(0).unwrap()).collect();
    // Act
    let retained = installation.slots.count.load(Ordering::Acquire);
    // Assert
    assert_eq!(retained, 129);
    assert_eq!(resources.metrics().unwrap().active_leases, 1);
    drop(leases);
    assert_eq!(installation.slots.count.load(Ordering::Acquire), 0);
    assert!(installation.admit(KEYBOARD_TEXT_MAX_BYTES).is_ok());
}

#[test]
fn oversized_argument_rejects_without_admission() {
    // Arrange
    let resources = resources();
    let installation = Installation::new(&resources, "macos").unwrap();
    // Act
    let error = installation
        .admit(KEYBOARD_TEXT_MAX_BYTES + 1)
        .err()
        .unwrap();
    // Assert
    assert!(matches!(error, HostError::ResourceExhausted(_)));
    assert_eq!(installation.slots.count.load(Ordering::Acquire), 0);
    assert_eq!(resources.metrics().unwrap().active_leases, 1);
}

#[test]
fn fixed_state_and_retained_native_binding_fit_declared_overhead() {
    // Arrange
    let native = std::mem::size_of::<playwright_rust_cdp::CdpSession>()
        + 2 * 256
        + 2 * std::mem::size_of::<usize>();
    let invocation = std::mem::size_of::<Invocation>() + 2 * std::mem::size_of::<usize>();
    let state = std::mem::size_of::<Installation>()
        + std::mem::size_of::<Slots>()
        + 4 * std::mem::size_of::<usize>();
    // Act
    let bytes = native + invocation;
    // Assert
    assert!(bytes < CALL_BYTES as usize);
    assert!(state < STATE_BYTES as usize);
}

#[test]
fn unknown_key_message_keeps_the_exact_failing_token_without_backend_prefix() {
    // Arrange
    let message = "Unknown key: \"NotAKey\"";
    let error = OperationError::new(
        playwright_rust_core::FailureKind::Backend,
        message,
        Some(KeyboardErrorDetails::InvalidKey(message.into())),
    );
    // Act
    let mapped = keyboard_error(error, 0);
    // Assert
    assert_eq!(mapped.to_string(), message);
    assert!(matches!(mapped, HostError::Backend(_)));
}
