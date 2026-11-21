//! Canonical session-scoped client metadata, never native-use authority.

use super::{TimeoutPolicyBudget, TimeoutSettings};
use crate::{
    HostError, HostSession,
    handles::{HandleSourceIdentity, pending::Owners},
};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceLedger,
    ResourceLimits, ResourceRequirements, ResourceScope,
};
use std::sync::{Arc, Mutex, Weak};

const INDEX_BYTES: u64 = 16 * 1024;
const SCOPE: ResourceScope = ResourceScope::extension(174, 1);

pub(super) fn index_resources() -> Result<ResourceLedger, HostError> {
    let requirements = ResourceRequirements::none()
        .with_class(ResourceClass::BackendStateBytes, u64::MAX, u64::MAX)
        .with_metadata(1, usize::MAX);
    let limits = ResourceLimits::deny_all(1, usize::MAX).with_class(
        ResourceClass::BackendStateBytes,
        u64::MAX,
        u64::MAX,
    );
    ProductionResourceLedger::new(limits, requirements)
        .and_then(|ledger| ledger.claim(requirements))
        .map_err(HostError::backend)
}

pub(super) struct SessionSlot {
    origin: Weak<Owners>,
    session: Weak<SessionInner>,
}

struct ContextEntry {
    id: ObjectId,
    settings: TimeoutSettings,
}
struct PageEntry {
    id: ObjectId,
    context: ObjectId,
    settings: TimeoutSettings,
}
struct Index {
    closed: bool,
    contexts: Vec<ContextEntry>,
    pages: Vec<PageEntry>,
}
struct SessionInner {
    origin: Weak<Owners>,
    debug: bool,
    index: Mutex<Index>,
    _storage: ResourceLease,
    // Shared administrative admission outlives the final index lease.
    budget: TimeoutPolicyBudget,
}

// Covers the fixed index, Arc header, lease request storage and one synchronous
// membership/pruning operation's detached arrays. No native routes are stored.
const _: () = assert!(
    std::mem::size_of::<SessionInner>()
        + 2 * std::mem::size_of::<usize>()
        + std::mem::size_of::<playwright_rust_core::ResourceRequest>()
        + std::mem::size_of::<Membership>()
        + std::mem::size_of::<Index>()
        < 16 * 1024
);

/// One canonical index per exact host origin in this environment budget.
/// Close releases canonical cell ownership. The fixed index admission lasts
/// until the final clone drops; retaining a closed receipt is not a free slot.
#[derive(Clone)]
pub struct TimeoutPolicySession(Arc<SessionInner>);

#[derive(Clone, Copy)]
enum Scope {
    Context,
    Page(ObjectId),
}

/// Client settings with immutable origin and Page provenance, not permission
/// to use a browser object. No strong session/index/native owner is retained.
#[derive(Clone)]
pub struct ScopedTimeoutSettings {
    origin: Weak<Owners>,
    scope: Scope,
    settings: TimeoutSettings,
    _binding: Arc<BindingStorage>,
}

struct BindingStorage {
    _storage: ResourceLease,
    _budget: TimeoutPolicyBudget,
}

const _: () = assert!(
    std::mem::size_of::<BindingStorage>()
        + std::mem::size_of::<ScopedTimeoutSettings>()
        + 2 * std::mem::size_of::<usize>()
        + std::mem::size_of::<playwright_rust_core::ResourceRequest>()
        < super::CELL_BYTES as usize
);

impl ScopedTimeoutSettings {
    pub fn settings(&self) -> &TimeoutSettings {
        &self.settings
    }

    pub fn is_page(&self) -> bool {
        matches!(self.scope, Scope::Page(_))
    }

    /// Pure immutable provenance. Generic, disposed and closed-source Handles
    /// remain valid metadata clients, but missing/wrong Page proof never does.
    pub fn validate_source(&self, source: &HandleSourceIdentity) -> Result<(), HostError> {
        let Scope::Page(page) = self.scope else {
            return Err(invalid("Handle timeout policy must belong to a Page"));
        };
        if !source.belongs_to(&self.origin) {
            return Err(invalid("Handle timeout policy belongs to another session"));
        }
        if source.creation_page_id() != Some(page) {
            return Err(invalid(
                "Handle timeout policy does not match its creation Page",
            ));
        }
        Ok(())
    }
}

fn invalid(message: &str) -> HostError {
    HostError::InvalidArgument(message.into())
}

#[derive(Default)]
struct Membership {
    contexts: Vec<ObjectId>,
    pages: Vec<(ObjectId, ObjectId)>,
}
impl Membership {
    fn contains_context(&self, id: ObjectId) -> bool {
        self.contexts.contains(&id)
    }
    fn page_context(&self, id: ObjectId) -> Option<ObjectId> {
        self.pages
            .iter()
            .find_map(|(page, context)| (*page == id).then_some(*context))
    }
}

impl HostSession {
    fn timeout_policy_origin(&self) -> Result<Weak<Owners>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        let live = state.as_ref().ok_or(HostError::TargetClosed)?;
        if self.stop.is_cancelled() || self.disconnected.is_cancelled() {
            return Err(HostError::TargetClosed);
        }
        Ok(Arc::downgrade(&live.handles.owners))
    }

    fn timeout_policy_membership(&self) -> Result<Membership, HostError> {
        let view = self
            .frame_publications()
            .published_collections()?
            .ok_or(HostError::TargetClosed)?;
        let mut result = Membership {
            contexts: Vec::new(),
            pages: Vec::new(),
        };
        result
            .contexts
            .try_reserve_exact(view.contexts().len())
            .map_err(HostError::backend)?;
        result
            .pages
            .try_reserve_exact(view.pages().len())
            .map_err(HostError::backend)?;
        result
            .contexts
            .extend(view.contexts().iter().map(|entry| entry.id));
        result
            .pages
            .extend(view.pages().iter().map(|entry| (entry.id, entry.context)));
        Ok(result)
    }
}

impl TimeoutPolicyBudget {
    pub fn session(
        &self,
        host: &HostSession,
        debug: bool,
    ) -> Result<TimeoutPolicySession, HostError> {
        self.session_for_origin(host.timeout_policy_origin()?, debug)
    }

    fn session_for_origin(
        &self,
        origin: Weak<Owners>,
        debug: bool,
    ) -> Result<TimeoutPolicySession, HostError> {
        let mut sessions = self.inner.sessions.lock().map_err(HostError::backend)?;
        sessions.retain(|slot| slot.session.strong_count() != 0);
        let existing = sessions.iter().find_map(|slot| {
            if Weak::ptr_eq(&slot.origin, &origin) {
                slot.session.upgrade()
            } else {
                None
            }
        });
        if let Some(session) = existing {
            // Drop the cache lock before a returned temporary could become
            // the final strong owner and remove its own cache slot.
            drop(sessions);
            if session.debug != debug {
                return Err(invalid("timeout session debug mode is immutable"));
            }
            return Ok(TimeoutPolicySession(session));
        }
        sessions.try_reserve(1).map_err(HostError::backend)?;
        let storage = self
            .inner
            .indexes
            .reserve(SCOPE, ResourceClass::BackendStateBytes, INDEX_BYTES)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        let session = Arc::new(SessionInner {
            origin: origin.clone(),
            debug,
            index: Mutex::new(Index {
                closed: false,
                contexts: Vec::new(),
                pages: Vec::new(),
            }),
            _storage: storage,
            budget: self.clone(),
        });
        sessions.push(SessionSlot {
            origin,
            session: Arc::downgrade(&session),
        });
        Ok(TimeoutPolicySession(session))
    }
}

impl TimeoutPolicySession {
    pub fn context(
        &self,
        host: &HostSession,
        context_id: &str,
    ) -> Result<ScopedTimeoutSettings, HostError> {
        let id = parse_id(context_id)?;
        self.check_origin(&host.timeout_policy_origin()?)?;
        self.lookup(&host.timeout_policy_membership()?, id, false)
    }

    pub fn page(
        &self,
        host: &HostSession,
        page_id: &str,
    ) -> Result<ScopedTimeoutSettings, HostError> {
        let id = parse_id(page_id)?;
        self.check_origin(&host.timeout_policy_origin()?)?;
        self.lookup(&host.timeout_policy_membership()?, id, true)
    }

    fn check_origin(&self, origin: &Weak<Owners>) -> Result<(), HostError> {
        if !Weak::ptr_eq(&self.0.origin, origin) {
            return Err(invalid("timeout policy index belongs to another session"));
        }
        Ok(())
    }

    fn lookup(
        &self,
        membership: &Membership,
        id: ObjectId,
        page: bool,
    ) -> Result<ScopedTimeoutSettings, HostError> {
        let mut index = self.0.index.lock().map_err(HostError::backend)?;
        if index.closed {
            return Err(HostError::TargetClosed);
        }
        index
            .pages
            .retain(|entry| membership.page_context(entry.id) == Some(entry.context));
        index
            .contexts
            .retain(|entry| membership.contains_context(entry.id));
        let context = if page {
            membership.page_context(id).ok_or(HostError::TargetClosed)?
        } else {
            id
        };
        if !membership.contains_context(context) {
            return Err(HostError::TargetClosed);
        }
        // Even canonical cells can produce distinct native publication Boxes.
        // Admit the returned binding before committing any new indexed cell.
        let storage = self
            .0
            .budget
            .inner
            .resources
            .reserve(
                super::SCOPE,
                ResourceClass::BackendStateBytes,
                super::CELL_BYTES,
            )
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        let binding = Arc::new(BindingStorage {
            _storage: storage,
            _budget: self.0.budget.clone(),
        });
        if page && let Some(entry) = index.pages.iter().find(|entry| entry.id == id) {
            return Ok(self.scoped(entry.settings.clone(), Scope::Page(id), binding));
        }
        let existing_context = index
            .contexts
            .iter()
            .find(|entry| entry.id == context)
            .map(|entry| entry.settings.clone());
        let new_context = existing_context.is_none();
        let settings = match existing_context {
            Some(settings) => settings,
            None => self.0.budget.root(self.0.debug)?,
        };
        // Admit both new cells before committing either index entry. A failed
        // child admission must not retain an otherwise unobserved new root.
        let page_settings = if page { Some(settings.child()?) } else { None };
        if new_context {
            index.contexts.try_reserve(1).map_err(HostError::backend)?;
            index.contexts.push(ContextEntry {
                id: context,
                settings: settings.clone(),
            });
        }
        match page_settings {
            Some(settings) => {
                index.pages.try_reserve(1).map_err(HostError::backend)?;
                index.pages.push(PageEntry {
                    id,
                    context,
                    settings: settings.clone(),
                });
                Ok(self.scoped(settings, Scope::Page(id), binding))
            }
            None => Ok(self.scoped(settings, Scope::Context, binding)),
        }
    }

    fn scoped(
        &self,
        settings: TimeoutSettings,
        scope: Scope,
        binding: Arc<BindingStorage>,
    ) -> ScopedTimeoutSettings {
        ScopedTimeoutSettings {
            origin: self.0.origin.clone(),
            scope,
            settings,
            _binding: binding,
        }
    }

    pub fn close(&self) {
        self.0.close();
    }
}

impl SessionInner {
    fn close(&self) {
        let removed = {
            let mut index = self
                .index
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            index.closed = true;
            (
                std::mem::take(&mut index.pages),
                std::mem::take(&mut index.contexts),
            )
        };
        drop(removed);
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        self.close();
        let mut sessions = self
            .budget
            .inner
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sessions.retain(|slot| !std::ptr::eq(slot.session.as_ptr(), self));
    }
}

fn parse_id(value: &str) -> Result<ObjectId, HostError> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid("invalid timeout policy object ID"));
    }
    value
        .parse::<u64>()
        .map(ObjectId::new)
        .map_err(|_| invalid("invalid timeout policy object ID"))
}

#[cfg(test)]
mod tests;
