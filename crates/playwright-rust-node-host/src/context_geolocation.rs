//! Optional Context-scoped Chromium geolocation policy.
//!
//! Rust retains one coordinate triple per selected Context in allocator-backed storage and applies
//! it through Chromium's session-initialization fence before a Page or OOPIF is
//! published. Permission grants are an independent capability.

use std::error::Error;
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::CdpContextOwnership;
use playwright_rust_cdp::connection::CdpError;
use playwright_rust_cdp::{CdpConnection, CdpSession};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_session_initialization::{
    ChromiumSessionInitializationFuture, ChromiumSessionInitializationTarget,
    ChromiumSessionInitializer,
};

use crate::{HostError, HostSession};

const SCOPE: ResourceScope = ResourceScope::extension(194, 0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextGeolocation {
    longitude: f64,
    latitude: f64,
    accuracy: f64,
}

impl ContextGeolocation {
    pub fn new(longitude: f64, latitude: f64, accuracy: f64) -> Result<Self, HostError> {
        if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
            return Err(HostError::InvalidArgument(
                "geolocation.longitude: precondition -180 <= LONGITUDE <= 180 failed.".into(),
            ));
        }
        if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
            return Err(HostError::InvalidArgument(
                "geolocation.latitude: precondition -90 <= LATITUDE <= 90 failed.".into(),
            ));
        }
        if !accuracy.is_finite() || accuracy < 0.0 {
            return Err(HostError::InvalidArgument(
                "geolocation.accuracy: precondition 0 <= ACCURACY failed.".into(),
            ));
        }
        Ok(Self {
            longitude,
            latitude,
            accuracy,
        })
    }
}

struct ContextEntry {
    context: ObjectId,
    geolocation: ContextGeolocation,
}

struct State {
    default_geolocation: Option<ContextGeolocation>,
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
    gate: tokio::sync::Mutex<()>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new_with_default(
        resources: &ProductionResourceLedger,
        default_geolocation: Option<ContextGeolocation>,
    ) -> Result<Arc<Self>, HostError> {
        let plan = requirements();
        let storage = resources
            .claim(plan)
            .map_err(HostError::backend)?
            .reserve(
                SCOPE,
                ResourceClass::BackendStateBytes,
                plan.class(ResourceClass::BackendStateBytes).global,
            )
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                default_geolocation,
                entries: Vec::new(),
                closed: false,
            }),
            gate: tokio::sync::Mutex::new(()),
            _storage: storage,
        }))
    }

    pub(crate) fn install(
        &self,
        context: ObjectId,
        geolocation: ContextGeolocation,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if state.entries.iter().any(|entry| entry.context == context) {
            return Err(HostError::InvalidArgument(
                "geolocation policy is already installed for this Context".into(),
            ));
        }
        state.entries.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!("geolocation policy allocation failed: {error}"))
        })?;
        state.entries.push(ContextEntry {
            context,
            geolocation,
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

    fn set(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
        geolocation: Option<ContextGeolocation>,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        match ownership {
            CdpContextOwnership::Default => state.default_geolocation = geolocation,
            CdpContextOwnership::Owned => {
                state.entries.retain(|entry| entry.context != context);
                if let Some(geolocation) = geolocation {
                    state.entries.try_reserve(1).map_err(|error| {
                        HostError::ResourceExhausted(format!(
                            "geolocation policy allocation failed: {error}"
                        ))
                    })?;
                    state.entries.push(ContextEntry {
                        context,
                        geolocation,
                    });
                }
            }
            CdpContextOwnership::External => {
                return Err(HostError::UnsupportedOperation(
                    "geolocation cannot be changed for an externally owned Context".into(),
                ));
            }
        }
        Ok(())
    }

    fn geolocation(
        &self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<Option<ContextGeolocation>, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        Ok(match ownership {
            CdpContextOwnership::Default => state.default_geolocation,
            CdpContextOwnership::Owned => state
                .entries
                .iter()
                .find(|entry| entry.context == context)
                .map(|entry| entry.geolocation),
            CdpContextOwnership::External => None,
        })
    }

    fn close(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.closed = true;
        state.default_geolocation = None;
        state.entries.clear();
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        self.close();
    }
}

fn geolocation_payload(geolocation: ContextGeolocation) -> serde_json::Value {
    serde_json::json!({
        "longitude": geolocation.longitude,
        "latitude": geolocation.latitude,
        "accuracy": geolocation.accuracy
    })
}

fn geolocation_override_payload(geolocation: Option<ContextGeolocation>) -> serde_json::Value {
    geolocation
        .map(geolocation_payload)
        .unwrap_or_else(|| serde_json::json!({}))
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
            let _gate = self.installation.gate.lock().await;
            let Some(geolocation) = self
                .installation
                .geolocation(target.context, target.ownership)?
            else {
                return Ok(());
            };
            CdpSession::new(
                Arc::clone(&self.connection),
                target.session_id.to_owned(),
                target.target_id.to_owned(),
            )
            .send(
                "Emulation.setGeolocationOverride",
                geolocation_payload(geolocation),
            )
            .await?;
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        })
    }
}

struct NativeSession {
    session_id: String,
    target_id: String,
    is_page: bool,
}

fn retired_frame_session(session: &NativeSession, error: &CdpError) -> bool {
    !session.is_page
        && (matches!(error, CdpError::SessionDetached)
            || matches!(error, CdpError::Protocol { code: -32001, message }
                if message == "Session with given id not found."))
}

impl HostSession {
    /// Apply a Context geolocation to every current Page/OOPIF session and
    /// retain it for sessions initialized after this operation completes.
    pub async fn set_context_geolocation(
        &self,
        context_id: &str,
        geolocation: Option<ContextGeolocation>,
    ) -> Result<(), HostError> {
        let (context, _call, _operation) = self.admit_operation(context_id)?;
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .context_geolocation
                .clone()
        };
        let _gate = tokio::select! {
            biased;
            () = self.stop.cancelled() => return Err(HostError::TargetClosed),
            gate = installation.gate.lock() => gate,
        };
        let (ownership, connection, sessions) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            let topology = live.composition.topology().map_err(HostError::backend)?;
            let topology = topology.topology();
            let route = topology
                .browser_context_route(context)
                .ok_or(HostError::TargetClosed)?;
            let page_targets = topology
                .page_session_routes()
                .filter(|page| page.browser_context_object_id() == context)
                .map(|page| page.target_id())
                .collect::<Vec<_>>();
            let mut sessions = Vec::new();
            for target in topology.target_session_routes() {
                if !page_targets.contains(&target.page_target_id()) {
                    continue;
                }
                sessions
                    .try_reserve(1)
                    .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
                sessions.push(NativeSession {
                    session_id: target.session_id().to_owned(),
                    target_id: target.target_id().to_owned(),
                    is_page: target.target_id() == target.page_target_id(),
                });
            }
            (
                route.ownership(),
                live.composition.connection().clone(),
                sessions,
            )
        };

        // Playwright updates the retained Context option before broadcasting
        // to existing Pages. Keeping the same ordering ensures a concurrently
        // initialized target observes the new value after this gate releases.
        installation.set(context, ownership, geolocation)?;
        let payload = geolocation_override_payload(geolocation);
        for session in sessions {
            let result = CdpSession::new(
                connection.clone(),
                session.session_id.clone(),
                session.target_id.clone(),
            )
            .send("Emulation.setGeolocationOverride", payload.clone())
            .await;
            if let Err(error) = result {
                if retired_frame_session(&session, &error) {
                    continue;
                }
                return Err(HostError::backend(error));
            }
        }
        Ok(())
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
            let limit = required.class(class);
            limits = limits.with_class(class, limit.global, limit.per_scope);
        }
        ProductionResourceLedger::new(limits, required).expect("valid geolocation policy plan")
    }

    fn location() -> ContextGeolocation {
        ContextGeolocation::new(10.0, 20.0, 0.0).unwrap()
    }

    #[test]
    fn coordinates_are_finite_and_bounded() {
        // Arrange
        let cases = [
            (181.0, 0.0, 0.0),
            (0.0, 91.0, 0.0),
            (0.0, 0.0, -1.0),
            (f64::NAN, 0.0, 0.0),
        ];

        // Act
        let outcomes = cases.map(|(longitude, latitude, accuracy)| {
            ContextGeolocation::new(longitude, latitude, accuracy)
        });

        // Assert
        assert!(outcomes.into_iter().all(|outcome| outcome.is_err()));
    }

    #[test]
    fn policy_is_context_scoped_and_release_is_idempotent() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), None).unwrap();
        let context = ObjectId::new(7);

        // Act
        installation.install(context, location()).unwrap();
        let installed = installation
            .geolocation(context, CdpContextOwnership::Owned)
            .unwrap();
        installation.remove(context).unwrap();
        installation.remove(context).unwrap();

        // Assert
        assert_eq!(installed, Some(location()));
        assert_eq!(
            installation
                .geolocation(context, CdpContextOwnership::Owned)
                .unwrap(),
            None
        );
    }

    #[test]
    fn policy_grows_past_the_legacy_context_capacity() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), None).unwrap();
        for value in 1..=129 {
            installation
                .install(ObjectId::new(value as u64), location())
                .unwrap();
        }

        // Act
        let retained = installation
            .geolocation(ObjectId::new(129), CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(retained, Some(location()));
    }

    #[test]
    fn default_policy_is_exact_without_reducing_dynamic_capacity() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), Some(location())).unwrap();
        for value in 1..=129 {
            installation
                .install(ObjectId::new(value as u64), location())
                .unwrap();
        }
        let context = ObjectId::new(99);

        // Act
        let default = installation
            .geolocation(context, CdpContextOwnership::Default)
            .unwrap();
        let external = installation
            .geolocation(context, CdpContextOwnership::External)
            .unwrap();
        let owned = installation
            .geolocation(ObjectId::new(129), CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(default, Some(location()));
        assert_eq!(external, None);
        assert_eq!(owned, Some(location()));
    }

    #[test]
    fn protocol_payload_matches_playwright_chromium() {
        // Arrange
        let expected = serde_json::json!({
            "longitude": 10.0,
            "latitude": 20.0,
            "accuracy": 0.0
        });

        // Act
        let payload = geolocation_payload(location());

        // Assert
        assert_eq!(payload, expected);
    }

    #[test]
    fn runtime_update_replaces_and_clears_owned_policy() {
        // Arrange
        let installation = Installation::new_with_default(&resources(), None).unwrap();
        let context = ObjectId::new(7);

        // Act
        installation
            .set(context, CdpContextOwnership::Owned, Some(location()))
            .unwrap();
        let updated = installation
            .geolocation(context, CdpContextOwnership::Owned)
            .unwrap();
        installation
            .set(context, CdpContextOwnership::Owned, None)
            .unwrap();
        let cleared = installation
            .geolocation(context, CdpContextOwnership::Owned)
            .unwrap();

        // Assert
        assert_eq!(updated, Some(location()));
        assert_eq!(cleared, None);
    }

    #[test]
    fn clearing_geolocation_uses_empty_chromium_payload() {
        // Arrange
        let geolocation = None;

        // Act
        let payload = geolocation_override_payload(geolocation);

        // Assert
        assert_eq!(payload, serde_json::json!({}));
    }
}
