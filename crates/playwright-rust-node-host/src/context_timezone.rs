//! Optional Context-scoped Chromium timezone policy.
//!
//! Rust retains one timezone identifier per selected Context in allocator-backed storage and
//! applies it through Chromium's session-initialization fence before a Page or
//! out-of-process Frame is published. Chromium remains the authority for IANA
//! timezone validity; the language bindings forward strings without adding a
//! private grammar or size policy.

use std::error::Error;
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::CdpContextOwnership;
use playwright_rust_cdp::{CdpConnection, CdpSession, connection::CdpError};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_session_initialization::{
    ChromiumSessionInitializationFuture, ChromiumSessionInitializationTarget,
    ChromiumSessionInitializer,
};

use crate::HostError;

const SCOPE: ResourceScope = ResourceScope::extension(192, 0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextTimezone(Box<str>);

impl ContextTimezone {
    pub fn new(value: String) -> Result<Self, HostError> {
        Ok(Self(value.into_boxed_str()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) struct StoredTimezone {
    timezone: ContextTimezone,
}

impl StoredTimezone {
    fn as_str(&self) -> &str {
        self.timezone.as_str()
    }
}

struct ContextEntry {
    context: ObjectId,
    timezone: Arc<StoredTimezone>,
}

struct State {
    default_timezone: Option<Arc<StoredTimezone>>,
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
        default_timezone: Option<ContextTimezone>,
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
        let default_timezone = default_timezone.map(retained_timezone);
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                default_timezone,
                entries: Vec::new(),
                closed: false,
            }),
            _storage: storage,
        }))
    }

    pub(crate) fn install(
        &self,
        context: ObjectId,
        timezone: ContextTimezone,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if state.entries.iter().any(|entry| entry.context == context) {
            return Err(HostError::InvalidArgument(
                "timezone policy is already installed for this Context".into(),
            ));
        }
        state.entries.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!("timezone policy allocation failed: {error}"))
        })?;
        let timezone = retained_timezone(timezone);
        state.entries.push(ContextEntry { context, timezone });
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

    fn timezone(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<Option<Arc<StoredTimezone>>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        Ok(match ownership {
            CdpContextOwnership::Default => state.default_timezone.as_ref().map(Arc::clone),
            CdpContextOwnership::Owned => state
                .entries
                .iter()
                .find(|entry| entry.context == context)
                .map(|entry| Arc::clone(&entry.timezone)),
            CdpContextOwnership::External => None,
        })
    }

    fn close(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.closed = true;
        state.default_timezone = None;
        state.entries.clear();
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        self.close();
    }
}

fn retained_timezone(timezone: ContextTimezone) -> Arc<StoredTimezone> {
    Arc::new(StoredTimezone { timezone })
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

    async fn apply(
        &self,
        session_id: &str,
        target_id: &str,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let Some(timezone) = self.installation.timezone(context, ownership)? else {
            return Ok(());
        };
        if timezone.as_str().is_empty() {
            return Ok(());
        }
        let session = CdpSession::new(
            Arc::clone(&self.connection),
            session_id.to_owned(),
            target_id.to_owned(),
        );
        match session
            .send(
                "Emulation.setTimezoneOverride",
                timezone_override_params(timezone.as_str()),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if shared_renderer_timezone_is_already_applied(&error) => Ok(()),
            Err(error) if invalid_timezone(&error) => Err(HostError::InvalidArgument(format!(
                "Invalid timezone ID: {}",
                timezone.as_str()
            ))
            .into()),
            Err(error) => Err(error.into()),
        }
    }
}

fn shared_renderer_timezone_is_already_applied(error: &CdpError) -> bool {
    matches!(
        error,
        CdpError::Protocol { message, .. }
            if message.contains("Timezone override is already in effect")
    )
}

fn invalid_timezone(error: &CdpError) -> bool {
    matches!(
        error,
        CdpError::Protocol { message, .. } if message.contains("Invalid timezone")
    )
}

fn timezone_override_params(timezone_id: &str) -> serde_json::Value {
    serde_json::json!({ "timezoneId": timezone_id })
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
            self.downstream.initialize_scoped(target).await?;
            self.apply(
                target.session_id,
                target.target_id,
                target.context,
                target.ownership,
            )
            .await?;
            Ok(())
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
        ProductionResourceLedger::new(limits, required).expect("valid timezone resource plan")
    }

    #[test]
    fn timezone_is_context_scoped_and_release_is_idempotent() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), None).unwrap();
        let context = ObjectId::new(7);
        let timezone = ContextTimezone::new("America/Jamaica".to_owned()).unwrap();

        // Act
        installation.install(context, timezone).unwrap();
        let observed = installation
            .timezone(context, CdpContextOwnership::Owned)
            .unwrap()
            .unwrap();
        installation.remove(context).unwrap();
        installation.remove(context).unwrap();

        // Assert
        assert_eq!(observed.as_str(), "America/Jamaica");
        assert!(
            installation
                .timezone(context, CdpContextOwnership::Owned)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn default_timezone_is_selected_only_for_the_default_context() {
        // Arrange
        let timezone = ContextTimezone::new("America/Jamaica".to_owned()).unwrap();
        let installation = Installation::new_with_default(&resources(), Some(timezone)).unwrap();
        let context = ObjectId::new(7);

        // Act
        let default = installation
            .timezone(context, CdpContextOwnership::Default)
            .unwrap();
        let owned = installation
            .timezone(context, CdpContextOwnership::Owned)
            .unwrap();
        let external = installation
            .timezone(context, CdpContextOwnership::External)
            .unwrap();

        // Assert
        assert_eq!(default.unwrap().as_str(), "America/Jamaica");
        assert!(owned.is_none());
        assert!(external.is_none());
    }

    #[test]
    fn default_timezone_does_not_reduce_dynamic_context_capacity() {
        // Arrange
        let timezone = ContextTimezone::new("UTC".to_owned()).unwrap();
        let installation = Installation::new_with_default(&resources(), Some(timezone)).unwrap();
        for value in 1..=129 {
            installation
                .install(
                    ObjectId::new(value as u64),
                    ContextTimezone::new("UTC".to_owned()).unwrap(),
                )
                .unwrap();
        }

        // Act
        let retained = installation
            .timezone(ObjectId::new(129), CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(retained.unwrap().as_str(), "UTC");
    }

    #[test]
    fn timezone_identifier_domain_and_protocol_payload_match_playwright() {
        // Arrange
        let large_input = "a".repeat(4096);

        // Act
        let empty = ContextTimezone::new(String::new());
        let control = ContextTimezone::new("UTC\n".to_owned());
        let large = ContextTimezone::new(large_input.clone());
        let payload = timezone_override_params("America/Jamaica");

        // Assert
        assert_eq!(empty.unwrap().as_str(), "");
        assert_eq!(control.unwrap().as_str(), "UTC\n");
        assert_eq!(large.unwrap().as_str(), large_input);
        assert_eq!(
            payload,
            serde_json::json!({ "timezoneId": "America/Jamaica" })
        );
    }
}
