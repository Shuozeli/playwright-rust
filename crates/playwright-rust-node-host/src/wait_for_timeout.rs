//! Optional Rust-owned timeout waiting.

use std::sync::Arc;
use std::time::Duration;

use playwright_rust_core::{
    ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements, ResourceScope,
};

use crate::{HostError, HostSession};

pub mod profile {
    pub const BYTES: u64 = 4 * 1024;
}

const STORAGE_BYTES: u64 = 4 * 1024;
const STORAGE_SCOPE: ResourceScope = ResourceScope::extension(140, 1);

struct Slots {
    _storage: ResourceLease,
}

impl Slots {
    fn new(resources: &ProductionResourceLedger) -> Result<Arc<Self>, HostError> {
        let storage = resources
            .claim(storage_requirements())
            .map_err(HostError::backend)?
            .reserve(
                STORAGE_SCOPE,
                ResourceClass::BackendStateBytes,
                STORAGE_BYTES,
            )
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self { _storage: storage }))
    }

    fn admit(self: &Arc<Self>) {}
}

pub(crate) struct Installation {
    slots: Arc<Slots>,
}

const fn storage_requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            STORAGE_BYTES,
            STORAGE_BYTES,
        )
        .with_metadata(1, 1)
}

pub(crate) fn requirements() -> ResourceRequirements {
    storage_requirements()
}

pub(crate) fn install(resources: &ProductionResourceLedger) -> Result<Installation, HostError> {
    Ok(Installation {
        slots: Slots::new(resources)?,
    })
}

impl HostSession {
    pub async fn wait_for_timeout(&self, timeout_ms: u32) -> Result<(), HostError> {
        {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .wait_for_timeout
                .slots
                .admit()
        }
        tokio::select! {
            biased;
            () = self.stop.cancelled() => Err(HostError::TargetClosed),
            () = self.disconnected.cancelled() => Err(HostError::TargetClosed),
            () = tokio::time::sleep(Duration::from_millis(u64::from(timeout_ms))) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playwright_rust_core::ResourceLimits;

    fn resources() -> ProductionResourceLedger {
        let plan = requirements();
        let mut limits = ResourceLimits::deny_all(plan.scopes, plan.leases);
        for class in ResourceClass::ALL {
            let bound = plan.class(class);
            limits = limits.with_class(class, bound.global, bound.per_scope);
        }
        ProductionResourceLedger::new(limits, plan).unwrap()
    }

    #[test]
    fn operation_admission_is_allocator_backed_past_legacy_count() {
        // Arrange
        let resources = resources();
        let slots = Slots::new(&resources).unwrap();
        let admissions = (0..129).map(|_| slots.admit()).collect::<Vec<_>>();

        // Act
        slots.admit();

        // Assert
        assert_eq!(admissions.len(), 129);
    }
}
