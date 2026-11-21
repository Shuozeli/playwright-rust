//! Allocator-backed session hooks for already-admitted, possibly parked FFI work.
//!
//! Register only at the first native handoff, not during arbitrary language
//! preparation. This registry proves its own identity, not source/session
//! provenance; the embedding checks that provenance under its admission lock.
//!
//! The active 4 KiB lease is an admission bound, not heap-reclamation evidence.
//! External stale Weak receipts can retain dead Arc allocation backing after
//! Inner and its lease drop. That tombstone storage is outside the active lease
//! claim and is not measured or proven reclaimed by zero ledger counts.

use crate::{HostError, ffi_budget::FfiRevoker};
use playwright_rust_core::{
    ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequest, ResourceRequirements,
    ResourceScope,
};
use std::sync::{Arc, Mutex, Weak};

const STORAGE_BYTES: u64 = 4 * 1024;
const SCOPE: ResourceScope = ResourceScope::extension(169, 2);

struct Entry {
    generation: u64,
    revoker: FfiRevoker,
}

struct State {
    sealed: bool,
    next_generation: u64,
    entries: Vec<Entry>,
}

struct Inner {
    state: Mutex<State>,
    _storage: ResourceLease,
}

/// Weak hooks with no fabricated public count limit. No entry owns an FFI
/// budget, task token or native result.
pub(super) struct Invocations(Arc<Inner>);

/// A move-only weak receipt, valid only for its original registry generation.
/// Dropping it unregisters the hook; it does not cancel the admitted operation.
#[must_use = "retain the registration through every native/parked phase"]
pub(super) struct Registration {
    origin: Weak<Inner>,
    generation: u64,
}

/// Detached cancellation work. Sealing never invokes cancellation under the
/// caller's admission lock. Explicitly call `revoke` after releasing that lock.
/// Armed Drop is a fallback, so this batch must also never be dropped while
/// caller admission locks are held.
#[must_use = "invoke revoke after releasing all caller admission locks"]
pub(crate) struct RevokeBatch {
    entries: Vec<Entry>,
    // Retain administrative admission through wakeups/result destructors even
    // if the session owner disappears. Repeated empty seals retain no owner.
    _storage: Option<Arc<Inner>>,
}

// Include the fixed heap index/Mutex and Arc header, empty detached-vector
// owners, one public receipt value, one resource request, and bounded control
// temporaries. Dynamic entries use ordinary allocator ownership.
// FFI entries/results and shared ledger metadata have separate admission; weak
// hooks do not duplicate those owners. This is not a process/V8 memory bound.
const REQUIRED_BYTES: usize = std::mem::size_of::<Inner>()
    + 2 * std::mem::size_of::<usize>()
    + std::mem::size_of::<RevokeBatch>()
    + std::mem::size_of::<super::RetainedFileInputRegistration>()
    + std::mem::size_of::<ResourceRequest>()
    + 512;
const _: () = assert!(REQUIRED_BYTES <= STORAGE_BYTES as usize);

impl Invocations {
    pub(super) const fn requirements() -> ResourceRequirements {
        ResourceRequirements::none()
            .with_class(
                ResourceClass::BackendStateBytes,
                STORAGE_BYTES,
                STORAGE_BYTES,
            )
            .with_metadata(1, 1)
    }

    pub(super) fn new(resources: &ProductionResourceLedger) -> Result<Self, HostError> {
        if REQUIRED_BYTES > STORAGE_BYTES as usize {
            return Err(HostError::ResourceExhausted(
                "retained invocation registry exceeds storage admission".into(),
            ));
        }
        let storage = resources
            .claim(Self::requirements())
            .map_err(HostError::backend)?
            .reserve(SCOPE, ResourceClass::BackendStateBytes, STORAGE_BYTES)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        Ok(Self(Arc::new(Inner {
            state: Mutex::new(State {
                sealed: false,
                next_generation: 0,
                entries: Vec::new(),
            }),
            _storage: storage,
        })))
    }

    /// Does not promote work or inspect dispatcher/native liveness. The caller
    /// keeps its exact session admission guard until this registration returns.
    pub(super) fn register(&self, revoker: FfiRevoker) -> Result<Registration, HostError> {
        let mut state = self.0.state.lock().map_err(HostError::backend)?;
        if state.sealed {
            return Err(HostError::TargetClosed);
        }
        let generation = state.next_generation.checked_add(1).ok_or_else(|| {
            HostError::ResourceExhausted("retained invocation IDs exhausted".into())
        })?;
        state.entries.try_reserve(1).map_err(HostError::backend)?;
        state.next_generation = generation;
        state.entries.push(Entry {
            generation,
            revoker,
        });
        Ok(Registration {
            origin: Arc::downgrade(&self.0),
            generation,
        })
    }

    pub(super) fn check(&self, registration: &Registration) -> Result<(), HostError> {
        if !Weak::ptr_eq(&registration.origin, &Arc::downgrade(&self.0)) {
            return Err(HostError::InvalidArgument(
                "retained invocation belongs to another registry".into(),
            ));
        }
        let state = self.0.state.lock().map_err(HostError::backend)?;
        if state.sealed
            || !state
                .entries
                .iter()
                .any(|entry| entry.generation == registration.generation)
        {
            return Err(HostError::TargetClosed);
        }
        Ok(())
    }

    /// Cleanup recovers a poisoned lock so one prior panic cannot strand other
    /// hooks. There are no cancellation calls or native destructors here.
    pub(super) fn seal_and_take(&self) -> RevokeBatch {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let first = !state.sealed;
        state.sealed = true;
        let entries = std::mem::take(&mut state.entries);
        RevokeBatch {
            entries,
            _storage: first.then(|| self.0.clone()),
        }
    }
}

impl RevokeBatch {
    /// Cancellation may synchronously wake Rust futures or drop parked native
    /// owners. The embedding must hold no session/registry admission locks.
    pub(crate) fn revoke(mut self) {
        self.drain();
    }

    fn drain(&mut self) {
        for entry in self.entries.drain(..) {
            entry.revoker.revoke();
        }
    }
}

impl Drop for RevokeBatch {
    fn drop(&mut self) {
        self.drain();
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let Some(origin) = self.origin.upgrade() else {
            return;
        };
        let removed = {
            let mut state = origin
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state
                .entries
                .iter()
                .position(|entry| entry.generation == self.generation)
                .map(|index| state.entries.swap_remove(index))
        };
        // Drop the detached weak hook after releasing the registry lock; other
        // weak receipts may still retain its dead allocation backing. Stale
        // receipts cannot unregister a later generation.
        drop(removed);
    }
}

#[cfg(test)]
mod tests;
