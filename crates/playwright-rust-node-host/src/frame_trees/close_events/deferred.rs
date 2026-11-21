//! Explicit deferred admission. Ordinary observer preparation is unchanged.
use super::*;
use playwright_rust_composition_chromium::{ChromiumCloseCapacity, ChromiumCloseTarget};

pub(super) struct Capacity {
    owner: Arc<FrameTrees>,
    target: ChromiumCloseTarget,
    allocator_backed: bool,
    reservations: Vec<LifecycleReservation>,
    notices: Vec<Notice>,
    storage: Storage,
}

impl Capacity {
    pub(super) fn new(
        owner: Arc<FrameTrees>,
        target: ChromiumCloseTarget,
    ) -> Result<Self, HostError> {
        let state = owner.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let allocator_backed = owner.factory.allocator_backed();
        // Finite embedders retain their complete capacity before native close
        // polling. The Playwright-compatible profile instead allocates exactly
        // the observed close receipt; a dynamic Frame index has no honest
        // artificial descendant maximum to predeclare.
        let maximum = if allocator_backed {
            0
        } else {
            match target {
                ChromiumCloseTarget::Page(_) => 1,
                ChromiumCloseTarget::BrowserContext(_) => super::super::FRAMES + 1,
            }
        };
        let storage = owner.factory.reserve_collection_storage(
            std::mem::size_of::<Self>()
                + std::mem::size_of::<Prepared>()
                + maximum
                    * (std::mem::size_of::<Notice>() + std::mem::size_of::<LifecycleReservation>()),
        )?;
        let mut reservations = Vec::new();
        reservations
            .try_reserve_exact(maximum)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        let mut notices = Vec::new();
        notices
            .try_reserve_exact(maximum)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        for _ in 0..maximum {
            reservations.push(
                owner
                    .urls
                    .publications()
                    .reserve_lifecycle(&owner.factory)?,
            );
        }
        drop(state);
        Ok(Self {
            owner,
            target,
            allocator_backed,
            reservations,
            notices,
            storage,
        })
    }

    fn plan(
        mut self,
        plan: ChromiumClosePlan<'_>,
    ) -> Result<Box<dyn ChromiumCloseReservation>, HostError> {
        if plan.target() != self.target {
            return Err(HostError::InvalidArgument(
                "deferred close plan targets another object".into(),
            ));
        }
        let root = match self.target {
            ChromiumCloseTarget::Page(id) | ChromiumCloseTarget::BrowserContext(id) => id,
        };
        let owner = self.owner.clone();
        let state = owner.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let before = state
            .collections
            .as_ref()
            .and_then(|value| value.planned.clone());
        // Native events or an ancestor can already have retired the local
        // target. A sealed native completion then has an empty latest topology
        // delta. Accept only a publication no-op when this target is no longer
        // visible; never fabricate a Dispose or hide a still-visible target.
        // This is local publication validation, not native-death authority.
        let visible = before.as_ref().is_some_and(|view| match self.target {
            ChromiumCloseTarget::Page(id) => view.pages().iter().any(|page| page.id == id),
            ChromiumCloseTarget::BrowserContext(id) => {
                view.contexts().iter().any(|context| context.id == id)
            }
        });
        if !disposed(plan.effects(), root) && (visible || !plan.effects().is_empty()) {
            return Err(HostError::InvalidArgument(
                "deferred close plan has no disposal of its admitted target".into(),
            ));
        }
        // Reject unrelated disposal effects even if their targets are invisible.
        // A Context may additionally list its known Page descendants; its compact
        // root disposal alone already covers all currently visible descendants.
        for effect in plan.effects() {
            if let CdpTopologyEffect::Lifecycle(ControlMessage::Dispose { object_id }) = effect {
                let valid = *object_id == root
                    || match self.target {
                        ChromiumCloseTarget::Page(_) => false,
                        ChromiumCloseTarget::BrowserContext(context) => {
                            before.as_ref().is_some_and(|view| {
                                view.revision()
                                    .pages()
                                    .iter()
                                    .any(|page| page.id == *object_id && page.context == context)
                            })
                        }
                    };
                if !valid {
                    return Err(HostError::InvalidArgument(
                        "deferred close plan includes an unrelated disposal".into(),
                    ));
                }
            }
        }
        if let Some(view) = &before {
            // Work exclusively from the latest planned mask. Earlier committed
            // closes and visibility changes must not be replayed or overwritten.
            let mut planned = view.clone();
            for context in view.contexts() {
                for page in view.pages().iter().filter(|page| {
                    page.context == context.id
                        && (disposed(plan.effects(), page.id)
                            || disposed(plan.effects(), context.id))
                }) {
                    let member = CollectionMember::Page(page.id);
                    planned = planned
                        .with_visibility(member, false)
                        .map_err(HostError::backend)?;
                    let reservation = self.reservation()?;
                    self.notices
                        .try_reserve(1)
                        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
                    self.notices.push(Notice {
                        member,
                        reservation,
                    });
                }
                if disposed(plan.effects(), context.id) {
                    let member = CollectionMember::Context(context.id);
                    planned = planned
                        .with_visibility(member, false)
                        .map_err(HostError::backend)?;
                    let reservation = self.reservation()?;
                    self.notices
                        .try_reserve(1)
                        .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
                    self.notices.push(Notice {
                        member,
                        reservation,
                    });
                }
            }
        }
        drop(state);
        // Finite profiles pre-admit both box layouts and vector buffers. The
        // allocator profile reserves exact notices from the observed receipt.
        Ok(Box::new(Prepared {
            owner: self.owner,
            before,
            notices: self.notices,
            _storage: self.storage,
        }))
    }

    fn reservation(&mut self) -> Result<LifecycleReservation, HostError> {
        if self.allocator_backed {
            self.owner
                .urls
                .publications()
                .reserve_lifecycle(&self.owner.factory)
        } else {
            self.reservations.pop().ok_or_else(capacity_exhausted)
        }
    }
}

fn capacity_exhausted() -> HostError {
    HostError::ResourceExhausted("deferred close exceeded the declared Page/Context bound".into())
}

impl ChromiumCloseCapacity for Capacity {
    fn prepare(
        self: Box<Self>,
        plan: ChromiumClosePlan<'_>,
    ) -> Result<Box<dyn ChromiumCloseReservation>, OperationError<()>> {
        (*self).plan(plan).map_err(operation)
    }
}
