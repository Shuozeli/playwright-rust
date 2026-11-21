//! Optional Context-scoped Chromium locale policy.
//!
//! Rust retains one locale string per owned Context in allocator-backed storage and applies Playwright's
//! Chromium ICU locale override through the existing session-initialization
//! fence before a Page or OOPIF is published. The sibling shared user-agent
//! initializer composes this policy with device emulation's user agent so
//! `acceptLanguage` never resets a selected device identity. Dedicated Workers
//! inherit the renderer's locale.

use std::error::Error;
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::CdpContextOwnership;
use playwright_rust_cdp::{CdpConnection, connection::CdpError};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_session_initialization::{
    ChromiumSessionInitializationFuture, ChromiumSessionInitializationTarget,
    ChromiumSessionInitializer,
};

use crate::HostError;

const SCOPE: ResourceScope = ResourceScope::extension(183, 0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextLocale(Box<str>);

impl ContextLocale {
    pub fn new(value: String) -> Result<Self, HostError> {
        Ok(Self(value.into_boxed_str()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) struct StoredLocale {
    locale: ContextLocale,
}

impl StoredLocale {
    pub(crate) fn as_str(&self) -> &str {
        self.locale.as_str()
    }
}

struct ContextEntry {
    context: ObjectId,
    locale: Arc<StoredLocale>,
}

struct State {
    default_locale: Option<Arc<StoredLocale>>,
    entries: Vec<ContextEntry>,
    closed: bool,
}

pub(crate) const fn requirements() -> ResourceRequirements {
    let storage_bytes = (std::mem::size_of::<Installation>() + std::mem::size_of::<State>()) as u64;
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            storage_bytes,
            storage_bytes,
        )
        .with_metadata(1, 1)
}

pub(crate) struct Installation {
    state: Mutex<State>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new_with_default(
        resources: &ProductionResourceLedger,
        default_locale: Option<ContextLocale>,
    ) -> Result<Arc<Self>, HostError> {
        let plan = requirements();
        let ledger = resources.claim(plan).map_err(HostError::backend)?;
        let storage = ledger
            .reserve(
                SCOPE,
                ResourceClass::BackendStateBytes,
                plan.class(ResourceClass::BackendStateBytes).global,
            )
            .map_err(HostError::backend)?;
        let default_locale = default_locale.map(|locale| Arc::new(StoredLocale { locale }));
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                default_locale,
                entries: Vec::new(),
                closed: false,
            }),
            _storage: storage,
        }))
    }

    pub(crate) fn install(
        &self,
        context: ObjectId,
        locale: ContextLocale,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if state.entries.iter().any(|entry| entry.context == context) {
            return Err(HostError::InvalidArgument(
                "locale is already installed for this Context".into(),
            ));
        }
        state.entries.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!("locale policy allocation failed: {error}"))
        })?;
        state.entries.push(ContextEntry {
            context,
            locale: Arc::new(StoredLocale { locale }),
        });
        Ok(())
    }

    pub(crate) fn remove(&self, context: ObjectId) -> Result<(), HostError> {
        self.state
            .lock()
            .map_err(HostError::backend)?
            .entries
            .retain(|entry| entry.context != context);
        Ok(())
    }

    pub(crate) fn locale(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<Option<Arc<StoredLocale>>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        Ok(match ownership {
            CdpContextOwnership::Default => state.default_locale.as_ref().map(Arc::clone),
            CdpContextOwnership::Owned => state
                .entries
                .iter()
                .find(|entry| entry.context == context)
                .map(|entry| Arc::clone(&entry.locale)),
            CdpContextOwnership::External => None,
        })
    }

    fn close(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.closed = true;
        state.default_locale = None;
        state.entries.clear();
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        self.close();
    }
}

pub(crate) struct Initializer<Downstream> {
    downstream: Downstream,
    installation: Arc<Installation>,
    connection: Arc<CdpConnection>,
}

impl<Downstream> Initializer<Downstream> {
    pub(crate) fn new(
        downstream: Downstream,
        installation: Arc<Installation>,
        connection: Arc<CdpConnection>,
    ) -> Self {
        Self {
            downstream,
            installation,
            connection,
        }
    }

    fn parameters(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<Option<serde_json::Value>, Box<dyn Error + Send + Sync>> {
        let Some(locale) = self.installation.locale(context, ownership)? else {
            return Ok(None);
        };
        Ok((!locale.as_str().is_empty()).then(|| locale_override_params(locale.as_str())))
    }
}

fn shared_renderer_locale_is_already_applied(error: &CdpError) -> bool {
    matches!(
        error,
        CdpError::Protocol { message, .. }
            if message.contains("Another locale override is already in effect")
    )
}

fn locale_override_params(locale: &str) -> serde_json::Value {
    serde_json::json!({ "locale": locale })
}

impl<Downstream> ChromiumSessionInitializer for Initializer<Downstream>
where
    Downstream: ChromiumSessionInitializer,
{
    fn initialize<'a>(
        &'a self,
        session_id: &'a str,
        target_id: &'a str,
        ownership: CdpContextOwnership,
        is_page: bool,
    ) -> ChromiumSessionInitializationFuture<'a> {
        self.downstream
            .initialize(session_id, target_id, ownership, is_page)
    }

    fn initialize_scoped<'a>(
        &'a self,
        target: ChromiumSessionInitializationTarget<'a>,
    ) -> ChromiumSessionInitializationFuture<'a> {
        Box::pin(async move {
            let params = self.parameters(target.context, target.ownership)?;
            let queue = params.map(|params| {
                self.connection.enqueue_command_with_session(
                    "Emulation.setLocaleOverride",
                    params,
                    Some(target.session_id),
                )
            });
            crate::session_initialization_queue::enqueue_before_downstream(
                target.resume_fence,
                queue,
                self.downstream.initialize_scoped(target),
                |command| async move {
                    match command.response().await {
                        Ok(_) => Ok(()),
                        Err(error) if shared_renderer_locale_is_already_applied(&error) => Ok(()),
                        Err(error) => Err::<(), Box<dyn Error + Send + Sync>>(error.into()),
                    }
                },
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use playwright_rust_core::ResourceLimits;

    fn resources() -> ProductionResourceLedger {
        let required = requirements();
        let mut limits = ResourceLimits::deny_all(required.scopes, required.leases);
        for class in ResourceClass::ALL {
            let bound = required.class(class);
            limits = limits.with_class(class, bound.global, bound.per_scope);
        }
        ProductionResourceLedger::new(limits, required).expect("valid locale resource plan")
    }

    #[test]
    fn locale_is_context_scoped_and_release_is_idempotent() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), None).unwrap();
        let context = ObjectId::new(7);
        let locale = ContextLocale::new("ru-RU".to_owned()).unwrap();

        // Act
        installation.install(context, locale).unwrap();
        let observed = installation
            .locale(context, CdpContextOwnership::Owned)
            .unwrap()
            .unwrap();
        installation.remove(context).unwrap();
        installation.remove(context).unwrap();

        // Assert
        assert_eq!(observed.locale.as_str(), "ru-RU");
        assert!(
            installation
                .locale(context, CdpContextOwnership::Owned)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn default_locale_is_selected_only_for_the_default_context() {
        // Arrange
        let installation = Installation::new_with_default(
            &resources(),
            Some(ContextLocale::new("fr-FR".to_owned()).unwrap()),
        )
        .unwrap();
        let context = ObjectId::new(7);

        // Act
        let default = installation
            .locale(context, CdpContextOwnership::Default)
            .unwrap();
        let external = installation
            .locale(context, CdpContextOwnership::External)
            .unwrap();
        let owned = installation
            .locale(context, CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(default.unwrap().locale.as_str(), "fr-FR");
        assert!(external.is_none());
        assert!(owned.is_none());
    }

    #[test]
    fn shared_renderer_duplicate_locale_error_is_idempotent() {
        // Arrange
        let duplicate = CdpError::Protocol {
            code: -32000,
            message: "Another locale override is already in effect".to_owned(),
        };
        let unrelated = CdpError::Protocol {
            code: -32000,
            message: "Locale is invalid".to_owned(),
        };

        // Act
        let duplicate_is_idempotent = shared_renderer_locale_is_already_applied(&duplicate);
        let unrelated_is_idempotent = shared_renderer_locale_is_already_applied(&unrelated);

        // Assert
        assert!(duplicate_is_idempotent);
        assert!(!unrelated_is_idempotent);
    }

    #[test]
    fn locale_defers_arbitrary_string_semantics_to_chromium() {
        // Arrange
        let large_input = "x".repeat(4096);

        // Act
        let empty = ContextLocale::new(String::new());
        let control = ContextLocale::new("en-US\n".to_owned());
        let large = ContextLocale::new(large_input.clone());

        // Assert
        assert_eq!(empty.unwrap().as_str(), "");
        assert_eq!(control.unwrap().as_str(), "en-US\n");
        assert_eq!(large.unwrap().as_str(), large_input);
    }

    #[test]
    fn locale_registry_grows_past_the_legacy_context_capacity() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), None).unwrap();
        for value in 1..=129 {
            installation
                .install(
                    ObjectId::new(value as u64),
                    ContextLocale::new(format!("en-{value}")).unwrap(),
                )
                .unwrap();
        }

        // Act
        let retained = installation
            .locale(ObjectId::new(129), CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(retained.unwrap().as_str(), "en-129");
    }

    #[test]
    fn default_locale_does_not_reduce_dynamic_context_capacity() {
        // Arrange
        let installation = Installation::new_with_default(
            &resources(),
            Some(ContextLocale::new("fr-FR".to_owned()).unwrap()),
        )
        .unwrap();
        for value in 1..=129 {
            installation
                .install(
                    ObjectId::new(value as u64),
                    ContextLocale::new(format!("en-{value}")).unwrap(),
                )
                .unwrap();
        }

        // Act
        let retained = installation
            .locale(ObjectId::new(129), CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(retained.unwrap().as_str(), "en-129");
    }

    #[test]
    fn locale_override_uses_the_exact_chromium_payload() {
        // Arrange
        let locale = "ru-RU";

        // Act
        let locale_params = locale_override_params(locale);

        // Assert
        assert_eq!(locale_params, serde_json::json!({ "locale": "ru-RU" }));
    }
}
