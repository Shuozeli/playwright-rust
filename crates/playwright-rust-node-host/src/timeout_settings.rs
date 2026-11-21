//! Optional Playwright-compatible timeout inheritance.
//!
//! The leaf owns allocator-backed Context/Page tables and no timer, task, queue, or
//! browser command. Language bindings publish synchronous setters and ask Rust
//! for the effective deadline before admitting an operation.

use std::sync::Mutex;

use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};

use crate::{HostError, HostSession};

const STORAGE_BYTES: u64 = 8 * 1024;
const SCOPE: ResourceScope = ResourceScope::extension(133, 1);
const DEFAULT_TIMEOUT_MS: u32 = 30_000;

pub(crate) fn requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            STORAGE_BYTES,
            STORAGE_BYTES,
        )
        .with_metadata(1, 1)
}

#[derive(Clone, Copy, Default)]
struct Settings {
    timeout_ms: Option<u32>,
    navigation_timeout_ms: Option<u32>,
}

#[derive(Clone, Copy)]
struct ContextSettings {
    id: ObjectId,
    settings: Settings,
}

#[derive(Clone, Copy)]
struct PageSettings {
    id: ObjectId,
    context: ObjectId,
    settings: Settings,
}

struct State {
    contexts: Vec<ContextSettings>,
    pages: Vec<PageSettings>,
}

#[cfg(any(feature = "page-pause", test))]
pub(crate) struct PauseTimeoutReset {
    owner: std::sync::Arc<Installation>,
    page: ObjectId,
    context: ObjectId,
    page_settings: Option<Settings>,
    context_settings: Option<Settings>,
}

#[cfg(any(feature = "page-pause", test))]
impl Drop for PauseTimeoutReset {
    fn drop(&mut self) {
        let Ok(mut state) = self.owner.state.lock() else {
            return;
        };
        if let Some(index) = state
            .contexts
            .iter()
            .position(|entry| entry.id == self.context)
        {
            match self.context_settings {
                Some(settings) => state.contexts[index].settings = settings,
                None => {
                    state.contexts.remove(index);
                }
            }
        }
        if let Some(index) = state
            .pages
            .iter()
            .position(|entry| entry.id == self.page && entry.context == self.context)
        {
            match self.page_settings {
                Some(settings) => state.pages[index].settings = settings,
                None => {
                    state.pages.remove(index);
                }
            }
        }
    }
}

pub(crate) struct Installation {
    state: Mutex<State>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new(resources: &ProductionResourceLedger) -> Result<Self, HostError> {
        let ledger = resources
            .claim(requirements())
            .map_err(HostError::backend)?;
        let storage = ledger
            .reserve(SCOPE, ResourceClass::BackendStateBytes, STORAGE_BYTES)
            .map_err(HostError::backend)?;
        Ok(Self {
            state: Mutex::new(State {
                contexts: Vec::new(),
                pages: Vec::new(),
            }),
            _storage: storage,
        })
    }

    fn reconcile(
        state: &mut State,
        active_contexts: &[ObjectId],
        active_pages: &[(ObjectId, ObjectId)],
    ) {
        state
            .contexts
            .retain(|entry| active_contexts.contains(&entry.id));
        state
            .pages
            .retain(|entry| active_pages.contains(&(entry.id, entry.context)));
    }

    fn context_mut(
        state: &mut State,
        context: ObjectId,
    ) -> Result<&mut ContextSettings, HostError> {
        if let Some(index) = state.contexts.iter().position(|entry| entry.id == context) {
            return Ok(&mut state.contexts[index]);
        }
        state.contexts.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!(
                "timeout Context registry allocation failed: {error}"
            ))
        })?;
        state.contexts.push(ContextSettings {
            id: context,
            settings: Settings::default(),
        });
        Ok(state
            .contexts
            .last_mut()
            .expect("just pushed Context settings"))
    }

    fn page_mut(
        state: &mut State,
        page: ObjectId,
        context: ObjectId,
    ) -> Result<&mut PageSettings, HostError> {
        if let Some(index) = state.pages.iter().position(|entry| entry.id == page) {
            let entry = &mut state.pages[index];
            if entry.context != context {
                return Err(HostError::TargetClosed);
            }
            return Ok(entry);
        }
        state.pages.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!(
                "timeout Page registry allocation failed: {error}"
            ))
        })?;
        state.pages.push(PageSettings {
            id: page,
            context,
            settings: Settings::default(),
        });
        Ok(state.pages.last_mut().expect("just pushed Page settings"))
    }

    fn set_context(
        &self,
        context: ObjectId,
        navigation: bool,
        timeout_ms: Option<u32>,
        active_contexts: &[ObjectId],
        active_pages: &[(ObjectId, ObjectId)],
    ) -> Result<(), HostError> {
        if !active_contexts.contains(&context) {
            return Err(HostError::TargetClosed);
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        Self::reconcile(&mut state, active_contexts, active_pages);
        let settings = &mut Self::context_mut(&mut state, context)?.settings;
        if navigation {
            settings.navigation_timeout_ms = timeout_ms;
        } else {
            settings.timeout_ms = timeout_ms;
        }
        Ok(())
    }

    fn set_page(
        &self,
        page: ObjectId,
        context: ObjectId,
        navigation: bool,
        timeout_ms: Option<u32>,
        active_contexts: &[ObjectId],
        active_pages: &[(ObjectId, ObjectId)],
    ) -> Result<(), HostError> {
        if !active_pages.contains(&(page, context)) {
            return Err(HostError::TargetClosed);
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        Self::reconcile(&mut state, active_contexts, active_pages);
        let settings = &mut Self::page_mut(&mut state, page, context)?.settings;
        if navigation {
            settings.navigation_timeout_ms = timeout_ms;
        } else {
            settings.timeout_ms = timeout_ms;
        }
        Ok(())
    }

    fn effective_page(
        &self,
        page: ObjectId,
        context: ObjectId,
        navigation: bool,
        active_contexts: &[ObjectId],
        active_pages: &[(ObjectId, ObjectId)],
    ) -> Result<u32, HostError> {
        if !active_pages.contains(&(page, context)) {
            return Err(HostError::TargetClosed);
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        Self::reconcile(&mut state, active_contexts, active_pages);
        let page = state
            .pages
            .iter()
            .find(|entry| entry.id == page)
            .map_or(Settings::default(), |entry| entry.settings);
        let context = state
            .contexts
            .iter()
            .find(|entry| entry.id == context)
            .map_or(Settings::default(), |entry| entry.settings);
        Ok(if navigation {
            page.navigation_timeout_ms
                .or(page.timeout_ms)
                .or(context.navigation_timeout_ms)
                .or(context.timeout_ms)
                .unwrap_or(DEFAULT_TIMEOUT_MS)
        } else {
            page.timeout_ms
                .or(context.timeout_ms)
                .unwrap_or(DEFAULT_TIMEOUT_MS)
        })
    }

    fn effective_context(
        &self,
        context: ObjectId,
        navigation: bool,
        active_contexts: &[ObjectId],
        active_pages: &[(ObjectId, ObjectId)],
    ) -> Result<u32, HostError> {
        if !active_contexts.contains(&context) {
            return Err(HostError::TargetClosed);
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        Self::reconcile(&mut state, active_contexts, active_pages);
        let context = state
            .contexts
            .iter()
            .find(|entry| entry.id == context)
            .map_or(Settings::default(), |entry| entry.settings);
        Ok(if navigation {
            context
                .navigation_timeout_ms
                .or(context.timeout_ms)
                .unwrap_or(DEFAULT_TIMEOUT_MS)
        } else {
            context.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)
        })
    }

    #[cfg(any(feature = "page-pause", test))]
    pub(crate) fn begin_headless_pause(
        self: &std::sync::Arc<Self>,
        page: ObjectId,
        context: ObjectId,
        active_contexts: &[ObjectId],
        active_pages: &[(ObjectId, ObjectId)],
    ) -> Result<PauseTimeoutReset, HostError> {
        if !active_contexts.contains(&context) || !active_pages.contains(&(page, context)) {
            return Err(HostError::TargetClosed);
        }
        let mut state = self.state.lock().map_err(HostError::backend)?;
        Self::reconcile(&mut state, active_contexts, active_pages);
        let context_settings = state
            .contexts
            .iter()
            .find(|entry| entry.id == context)
            .map(|entry| entry.settings);
        let page_settings = state
            .pages
            .iter()
            .find(|entry| entry.id == page && entry.context == context)
            .map(|entry| entry.settings);
        if state
            .pages
            .iter()
            .any(|entry| entry.id == page && entry.context != context)
        {
            return Err(HostError::TargetClosed);
        }
        let paused = Settings {
            timeout_ms: Some(0),
            navigation_timeout_ms: Some(0),
        };
        Self::context_mut(&mut state, context)?.settings = paused;
        Self::page_mut(&mut state, page, context)?.settings = paused;
        drop(state);
        Ok(PauseTimeoutReset {
            owner: self.clone(),
            page,
            context,
            page_settings,
            context_settings,
        })
    }
}

const _: () = assert!(
    std::mem::size_of::<Installation>()
        + std::mem::size_of::<State>()
        + 2 * std::mem::size_of::<usize>()
        + std::mem::size_of::<playwright_rust_core::ResourceRequest>()
        < STORAGE_BYTES as usize
);

pub(crate) type ActiveContexts = Vec<ObjectId>;
pub(crate) type ActivePages = Vec<(ObjectId, ObjectId)>;

impl HostSession {
    pub(crate) fn timeout_snapshot(
        &self,
    ) -> Result<(std::sync::Arc<Installation>, ActiveContexts, ActivePages), HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        let live = state.as_ref().ok_or(HostError::TargetClosed)?;
        let topology = live.composition.topology().map_err(HostError::backend)?;
        let topology = topology.topology();
        let contexts = topology
            .browser_context_routes()
            .map(|route| route.object_id())
            .collect();
        let pages = topology
            .page_session_routes()
            .map(|route| (route.object_id(), route.browser_context_object_id()))
            .collect();
        Ok((live.timeout_settings.clone(), contexts, pages))
    }

    pub(crate) fn parse_timeout_object(
        value: &str,
        kind: &'static str,
    ) -> Result<ObjectId, HostError> {
        value
            .parse::<u64>()
            .ok()
            .filter(|id| *id != 0)
            .map(ObjectId::new)
            .ok_or_else(|| HostError::InvalidArgument(format!("invalid portable {kind} ID")))
    }

    pub fn set_context_default_timeout(
        &self,
        context_id: &str,
        navigation: bool,
        timeout_ms: u32,
    ) -> Result<(), HostError> {
        self.set_context_default_timeout_override(context_id, navigation, Some(timeout_ms))
    }

    pub fn set_context_default_timeout_override(
        &self,
        context_id: &str,
        navigation: bool,
        timeout_ms: Option<u32>,
    ) -> Result<(), HostError> {
        let context = Self::parse_timeout_object(context_id, "Context")?;
        self.require_visible_context_id(context)?;
        let (installation, contexts, pages) = self.timeout_snapshot()?;
        installation.set_context(context, navigation, timeout_ms, &contexts, &pages)
    }

    pub fn set_page_default_timeout(
        &self,
        page_id: &str,
        navigation: bool,
        timeout_ms: u32,
    ) -> Result<(), HostError> {
        self.set_page_default_timeout_override(page_id, navigation, Some(timeout_ms))
    }

    pub fn set_page_default_timeout_override(
        &self,
        page_id: &str,
        navigation: bool,
        timeout_ms: Option<u32>,
    ) -> Result<(), HostError> {
        let page = Self::parse_timeout_object(page_id, "Page")?;
        self.require_visible_page_id(page)?;
        let (installation, contexts, pages) = self.timeout_snapshot()?;
        let context = pages
            .iter()
            .find_map(|(candidate, context)| (*candidate == page).then_some(*context))
            .ok_or(HostError::TargetClosed)?;
        installation.set_page(page, context, navigation, timeout_ms, &contexts, &pages)
    }

    pub fn context_default_timeout(
        &self,
        context_id: &str,
        navigation: bool,
    ) -> Result<u32, HostError> {
        let context = Self::parse_timeout_object(context_id, "Context")?;
        self.require_visible_context_id(context)?;
        let (installation, contexts, pages) = self.timeout_snapshot()?;
        installation.effective_context(context, navigation, &contexts, &pages)
    }

    pub fn page_default_timeout(&self, page_id: &str, navigation: bool) -> Result<u32, HostError> {
        let page = Self::parse_timeout_object(page_id, "Page")?;
        self.require_visible_page_id(page)?;
        let (installation, contexts, pages) = self.timeout_snapshot()?;
        let context = pages
            .iter()
            .find_map(|(candidate, context)| (*candidate == page).then_some(*context))
            .ok_or(HostError::TargetClosed)?;
        installation.effective_page(page, context, navigation, &contexts, &pages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installation() -> Installation {
        let requirements = requirements();
        let mut limits = playwright_rust_core::ResourceLimits::deny_all(
            requirements.scopes,
            requirements.leases,
        );
        for class in ResourceClass::ALL {
            let bound = requirements.class(class);
            limits = limits.with_class(class, bound.global, bound.per_scope);
        }
        let resources = ProductionResourceLedger::new(limits, requirements).unwrap();
        Installation::new(&resources).unwrap()
    }

    #[test]
    fn page_default_precedes_context_navigation_timeout() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_context(context, true, Some(20), &contexts, &pages)
            .unwrap();
        installation
            .set_page(page, context, false, Some(10), &contexts, &pages)
            .unwrap();

        // Act
        let timeout = installation
            .effective_page(page, context, true, &contexts, &pages)
            .unwrap();

        // Assert
        assert_eq!(timeout, 10);
    }

    #[test]
    fn clearing_context_navigation_timeout_restores_generic_inheritance() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_context(context, false, Some(2), &contexts, &pages)
            .unwrap();
        installation
            .set_context(context, true, Some(9), &contexts, &pages)
            .unwrap();

        // Act
        installation
            .set_context(context, true, None, &contexts, &pages)
            .unwrap();

        // Assert
        assert_eq!(
            installation
                .effective_context(context, true, &contexts, &pages)
                .unwrap(),
            2
        );
        assert_eq!(
            installation
                .effective_page(page, context, true, &contexts, &pages)
                .unwrap(),
            2
        );
    }

    #[test]
    fn clearing_page_navigation_timeout_reveals_context_navigation_timeout() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_context(context, true, Some(20), &contexts, &pages)
            .unwrap();
        installation
            .set_page(page, context, true, Some(0), &contexts, &pages)
            .unwrap();

        // Act
        installation
            .set_page(page, context, true, None, &contexts, &pages)
            .unwrap();

        // Assert
        assert_eq!(
            installation
                .effective_page(page, context, true, &contexts, &pages)
                .unwrap(),
            20
        );
    }

    #[test]
    fn zero_navigation_timeout_remains_an_explicit_override() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_context(context, true, Some(20), &contexts, &pages)
            .unwrap();
        installation
            .set_page(page, context, true, Some(0), &contexts, &pages)
            .unwrap();

        // Act
        let timeout = installation
            .effective_page(page, context, true, &contexts, &pages)
            .unwrap();

        // Assert
        assert_eq!(timeout, 0);
    }

    #[test]
    fn clearing_a_closed_context_is_rejected_without_mutating_timeout_state() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let contexts = vec![context];
        let pages = Vec::new();
        installation
            .set_context(context, true, Some(20), &contexts, &pages)
            .unwrap();
        let closed_contexts = Vec::new();

        // Act
        let result = installation.set_context(context, true, None, &closed_contexts, &pages);

        // Assert
        assert!(matches!(result, Err(HostError::TargetClosed)));
        assert_eq!(
            installation
                .effective_context(context, true, &contexts, &pages)
                .unwrap(),
            20
        );
    }

    #[test]
    fn clearing_a_closed_page_is_rejected_without_mutating_timeout_state() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_page(page, context, true, Some(20), &contexts, &pages)
            .unwrap();
        let closed_pages = Vec::new();

        // Act
        let result = installation.set_page(page, context, true, None, &contexts, &closed_pages);

        // Assert
        assert!(matches!(result, Err(HostError::TargetClosed)));
        assert_eq!(
            installation
                .effective_page(page, context, true, &contexts, &pages)
                .unwrap(),
            20
        );
    }

    #[test]
    fn headless_pause_temporarily_resets_both_timeout_kinds() {
        // Arrange
        let installation = std::sync::Arc::new(installation());
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_page(page, context, false, Some(123), &contexts, &pages)
            .unwrap();
        installation
            .set_page(page, context, true, Some(456), &contexts, &pages)
            .unwrap();

        // Act
        let reset = installation
            .begin_headless_pause(page, context, &contexts, &pages)
            .unwrap();

        // Assert
        assert_eq!(
            installation
                .effective_page(page, context, false, &contexts, &pages)
                .unwrap(),
            0
        );
        assert_eq!(
            installation
                .effective_page(page, context, true, &contexts, &pages)
                .unwrap(),
            0
        );
        drop(reset);
    }

    #[test]
    fn dropping_headless_pause_restores_custom_timeouts() {
        // Arrange
        let installation = std::sync::Arc::new(installation());
        let context = ObjectId::new(2);
        let page = ObjectId::new(3);
        let contexts = vec![context];
        let pages = vec![(page, context)];
        installation
            .set_page(page, context, false, Some(123), &contexts, &pages)
            .unwrap();
        installation
            .set_page(page, context, true, Some(456), &contexts, &pages)
            .unwrap();
        let reset = installation
            .begin_headless_pause(page, context, &contexts, &pages)
            .unwrap();

        // Act
        drop(reset);

        // Assert
        assert_eq!(
            installation
                .effective_page(page, context, false, &contexts, &pages)
                .unwrap(),
            123
        );
        assert_eq!(
            installation
                .effective_page(page, context, true, &contexts, &pages)
                .unwrap(),
            456
        );
    }

    #[test]
    fn page_timeout_registry_grows_past_the_legacy_capacity() {
        // Arrange
        let installation = installation();
        let context = ObjectId::new(2);
        let contexts = vec![context];
        let mut pages = Vec::new();
        for index in 0..129 {
            let page = ObjectId::new(index as u64 + 3);
            pages.push((page, context));
            installation
                .set_page(page, context, false, Some(1), &contexts, &pages)
                .unwrap();
        }

        // Act
        let result =
            installation.effective_page(ObjectId::new(131), context, false, &contexts, &pages);

        // Assert
        assert_eq!(result.unwrap(), 1);
    }
}
