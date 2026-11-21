//! Optional Chromium permission policy for a BrowserContext.
//!
//! Permission grants are deliberately separate from geolocation values:
//! callers can install either capability without pulling in the other. Rust
//! retains the per-origin Playwright permission union in allocator-backed
//! storage and owns the exact browser-session CDP commands.

use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::CdpContextOwnership;
use playwright_rust_cdp::{CdpConnection, CdpSession};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};

use crate::{HostError, HostSession};

const SCOPE: ResourceScope = ResourceScope::extension(195, 0);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContextPermissions(Vec<String>);

impl ContextPermissions {
    pub fn new(permissions: Vec<String>) -> Result<Self, HostError> {
        protocol_permissions(&permissions)?;
        let mut unique = Vec::new();
        unique.try_reserve(permissions.len()).map_err(|error| {
            HostError::ResourceExhausted(format!("permission allocation failed: {error}"))
        })?;
        for permission in permissions {
            if !unique.contains(&permission) {
                unique.push(permission);
            }
        }
        Ok(Self(unique))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn into_names(self) -> Vec<String> {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PermissionGrant {
    origin: Option<String>,
    permissions: Vec<String>,
}

struct ContextEntry {
    context: ObjectId,
    grants: Vec<PermissionGrant>,
}

struct State {
    default_permissions: Vec<String>,
    contexts: Vec<ContextEntry>,
    closed: bool,
}

impl State {
    fn admit(&mut self, context: ObjectId, permissions: Vec<String>) -> Result<(), HostError> {
        if self.closed {
            return Err(HostError::TargetClosed);
        }
        if self.contexts.iter().any(|entry| entry.context == context) {
            return Err(HostError::InvalidArgument(
                "permissions are already installed for this Context".into(),
            ));
        }
        self.contexts.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!("permission Context allocation failed: {error}"))
        })?;
        let mut grants = Vec::new();
        grants
            .try_reserve(1)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        grants.push(PermissionGrant {
            origin: None,
            permissions,
        });
        self.contexts.push(ContextEntry { context, grants });
        Ok(())
    }

    fn remove(&mut self, context: ObjectId) {
        self.contexts
            .retain(|candidate| candidate.context != context);
    }

    fn merged_permissions(
        &self,
        context: ObjectId,
        origin: Option<&str>,
        requested: &[String],
    ) -> Result<Vec<String>, HostError> {
        if self.closed {
            return Err(HostError::TargetClosed);
        }
        let retained = self
            .contexts
            .iter()
            .find(|entry| entry.context == context)
            .and_then(|entry| {
                entry
                    .grants
                    .iter()
                    .find(|grant| grant.origin.as_deref() == origin)
            })
            .map(|grant| grant.permissions.as_slice())
            .unwrap_or_default();
        let use_default_permissions = !self.default_permissions.is_empty()
            && origin.is_none()
            && retained.is_empty()
            && !self.contexts.iter().any(|entry| entry.context == context);
        let mut merged = Vec::new();
        merged
            .try_reserve(
                retained
                    .len()
                    .saturating_add(requested.len())
                    .saturating_add(if use_default_permissions {
                        self.default_permissions.len()
                    } else {
                        0
                    }),
            )
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        if use_default_permissions {
            merged.extend(self.default_permissions.iter().cloned());
        }
        for permission in retained.iter().chain(requested) {
            if !merged.contains(permission) {
                merged.push(permission.clone());
            }
        }
        Ok(merged)
    }

    fn commit_grant(
        &mut self,
        context: ObjectId,
        origin: Option<String>,
        permissions: Vec<String>,
    ) -> Result<(), HostError> {
        if self.closed {
            return Err(HostError::TargetClosed);
        }
        if let Some(entry) = self
            .contexts
            .iter_mut()
            .find(|entry| entry.context == context)
        {
            if let Some(grant) = entry.grants.iter_mut().find(|grant| grant.origin == origin) {
                grant.permissions = permissions;
                return Ok(());
            }
            entry.grants.try_reserve(1).map_err(|error| {
                HostError::ResourceExhausted(format!("permission grant allocation failed: {error}"))
            })?;
            entry.grants.push(PermissionGrant {
                origin,
                permissions,
            });
            return Ok(());
        }
        self.contexts.try_reserve(1).map_err(|error| {
            HostError::ResourceExhausted(format!("permission Context allocation failed: {error}"))
        })?;
        let mut grants = Vec::new();
        grants
            .try_reserve(1)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        grants.push(PermissionGrant {
            origin,
            permissions,
        });
        self.contexts.push(ContextEntry { context, grants });
        Ok(())
    }

    fn clear(
        &mut self,
        context: ObjectId,
        ownership: CdpContextOwnership,
    ) -> Result<(), HostError> {
        if self.closed {
            return Err(HostError::TargetClosed);
        }
        if ownership == CdpContextOwnership::Default {
            self.default_permissions.clear();
        }
        self.remove(context);
        Ok(())
    }
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
    connection: Arc<CdpConnection>,
    state: Mutex<State>,
    gate: tokio::sync::Mutex<()>,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new_with_default(
        resources: &ProductionResourceLedger,
        connection: Arc<CdpConnection>,
        default_permissions: ContextPermissions,
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
            connection,
            state: Mutex::new(State {
                default_permissions: default_permissions.into_names(),
                contexts: Vec::new(),
                closed: false,
            }),
            gate: tokio::sync::Mutex::new(()),
            _storage: storage,
        }))
    }

    pub(crate) async fn apply_default(&self) -> Result<(), HostError> {
        let permissions = {
            let state = self.state.lock().map_err(HostError::backend)?;
            if state.closed {
                return Err(HostError::TargetClosed);
            }
            state.default_permissions.clone()
        };
        if !permissions.is_empty() {
            let _gate = self.gate.lock().await;
            let protocol = protocol_permissions(&permissions)?;
            self.grant(None, None, &protocol).await?;
        }
        Ok(())
    }

    pub(crate) async fn install(
        &self,
        context: ObjectId,
        browser_context_id: &str,
        permissions: ContextPermissions,
    ) -> Result<(), HostError> {
        let _gate = self.gate.lock().await;
        let permissions = permissions.into_names();
        let protocol = protocol_permissions(&permissions)?;
        self.state
            .lock()
            .map_err(HostError::backend)?
            .admit(context, permissions)?;
        if let Err(error) = self.grant(Some(browser_context_id), None, &protocol).await {
            self.remove(context)?;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn remove(&self, context: ObjectId) -> Result<(), HostError> {
        self.state
            .lock()
            .map_err(HostError::backend)?
            .remove(context);
        Ok(())
    }

    async fn grant(
        &self,
        browser_context_id: Option<&str>,
        origin: Option<&str>,
        protocol_permissions: &[String],
    ) -> Result<(), HostError> {
        CdpSession::browser(Arc::clone(&self.connection))
            .send(
                "Browser.grantPermissions",
                grant_payload(browser_context_id, origin, protocol_permissions),
            )
            .await
            .map_err(HostError::backend)?;
        Ok(())
    }

    async fn reset(&self, browser_context_id: Option<&str>) -> Result<(), HostError> {
        CdpSession::browser(Arc::clone(&self.connection))
            .send(
                "Browser.resetPermissions",
                reset_payload(browser_context_id),
            )
            .await
            .map_err(HostError::backend)?;
        Ok(())
    }

    fn close(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.closed = true;
        state.contexts.clear();
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        self.close();
    }
}

fn grant_payload(
    browser_context_id: Option<&str>,
    origin: Option<&str>,
    permissions: &[String],
) -> serde_json::Value {
    let mut payload = serde_json::json!({ "permissions": permissions });
    if let Some(browser_context_id) = browser_context_id {
        payload.as_object_mut().expect("object literal").insert(
            "browserContextId".into(),
            serde_json::Value::String(browser_context_id.to_owned()),
        );
    }
    if let Some(origin) = origin {
        payload.as_object_mut().expect("object literal").insert(
            "origin".into(),
            serde_json::Value::String(origin.to_owned()),
        );
    }
    payload
}

fn reset_payload(browser_context_id: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({});
    if let Some(browser_context_id) = browser_context_id {
        payload.as_object_mut().expect("object literal").insert(
            "browserContextId".into(),
            serde_json::Value::String(browser_context_id.to_owned()),
        );
    }
    payload
}

fn normalized_origin(origin: Option<&str>) -> Result<Option<String>, HostError> {
    origin
        .map(|origin| {
            url::Url::parse(origin)
                .map(|url| url.origin().ascii_serialization())
                .map_err(|error| {
                    HostError::InvalidArgument(format!("invalid permission origin: {error}"))
                })
        })
        .transpose()
}

fn protocol_permissions(permissions: &[String]) -> Result<Vec<String>, HostError> {
    let mut output = Vec::new();
    for permission in permissions {
        let mapped: &[&str] = match permission.as_str() {
            "geolocation" => &["geolocation"],
            "midi" => &["midi"],
            "notifications" => &["notifications"],
            "camera" => &["videoCapture"],
            "microphone" => &["audioCapture"],
            "background-sync" => &["backgroundSync"],
            "ambient-light-sensor" | "accelerometer" | "gyroscope" | "magnetometer" => &["sensors"],
            "clipboard-read" => &["clipboardReadWrite"],
            "clipboard-write" => &["clipboardSanitizedWrite"],
            "payment-handler" => &["paymentHandler"],
            "midi-sysex" => &["midiSysex"],
            "storage-access" => &["storageAccess"],
            "local-fonts" => &["localFonts"],
            "local-network-access" => &["localNetworkAccess", "localNetwork", "loopbackNetwork"],
            _ => {
                return Err(HostError::InvalidArgument(format!(
                    "Unknown permission: {permission}"
                )));
            }
        };
        output.try_reserve(mapped.len()).map_err(|error| {
            HostError::ResourceExhausted(format!("permission mapping allocation failed: {error}"))
        })?;
        for value in mapped {
            let value = (*value).to_owned();
            if !output.contains(&value) {
                output.push(value);
            }
        }
    }
    Ok(output)
}

fn native_context_id(ownership: CdpContextOwnership, value: &str) -> Option<&str> {
    (ownership != CdpContextOwnership::Default && !value.is_empty()).then_some(value)
}

impl HostSession {
    pub async fn grant_context_permissions(
        &self,
        context_id: &str,
        permissions: &[String],
        origin: Option<&str>,
    ) -> Result<(), HostError> {
        let (context, _call, _operation) = self.admit_operation(context_id)?;
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .context_geolocation_permission
                .clone()
        };
        let origin = normalized_origin(origin)?;
        let _gate = tokio::select! {
            biased;
            () = self.stop.cancelled() => return Err(HostError::TargetClosed),
            gate = installation.gate.lock() => gate,
        };
        let (ownership, browser_context_id) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            let topology = live.composition.topology().map_err(HostError::backend)?;
            let route = topology
                .topology()
                .browser_context_route(context)
                .ok_or(HostError::TargetClosed)?;
            (route.ownership(), route.browser_context_id().to_owned())
        };
        let merged = installation
            .state
            .lock()
            .map_err(HostError::backend)?
            .merged_permissions(context, origin.as_deref(), permissions)?;
        let protocol = protocol_permissions(&merged)?;
        installation
            .grant(
                native_context_id(ownership, &browser_context_id),
                origin.as_deref(),
                &protocol,
            )
            .await?;
        installation
            .state
            .lock()
            .map_err(HostError::backend)?
            .commit_grant(context, origin, merged)
    }

    pub async fn clear_context_permissions(&self, context_id: &str) -> Result<(), HostError> {
        let (context, _call, _operation) = self.admit_operation(context_id)?;
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .context_geolocation_permission
                .clone()
        };
        let _gate = tokio::select! {
            biased;
            () = self.stop.cancelled() => return Err(HostError::TargetClosed),
            gate = installation.gate.lock() => gate,
        };
        let (ownership, browser_context_id) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            let topology = live.composition.topology().map_err(HostError::backend)?;
            let route = topology
                .topology()
                .browser_context_route(context)
                .ok_or(HostError::TargetClosed)?;
            (route.ownership(), route.browser_context_id().to_owned())
        };
        installation
            .reset(native_context_id(ownership, &browser_context_id))
            .await?;
        installation
            .state
            .lock()
            .map_err(HostError::backend)?
            .clear(context, ownership)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(default_enabled: bool) -> State {
        State {
            default_permissions: if default_enabled {
                vec!["geolocation".to_owned()]
            } else {
                Vec::new()
            },
            contexts: Vec::new(),
            closed: false,
        }
    }

    #[test]
    fn owned_permission_grows_past_legacy_capacity_and_release_is_idempotent() {
        // Arrange
        let mut state = state(false);
        for value in 1..=129 {
            state
                .admit(ObjectId::new(value as u64), vec!["geolocation".to_owned()])
                .unwrap();
        }

        // Act
        state.remove(ObjectId::new(1));
        state.remove(ObjectId::new(1));
        let replacement = state.admit(ObjectId::new(130), vec!["geolocation".to_owned()]);

        // Assert
        assert!(replacement.is_ok());
        assert_eq!(state.contexts.len(), 129);
    }

    #[test]
    fn default_permission_does_not_reduce_dynamic_context_capacity() {
        // Arrange
        let mut state = state(true);
        for value in 1..=129 {
            state
                .admit(ObjectId::new(value as u64), vec!["geolocation".to_owned()])
                .unwrap();
        }

        // Act
        let retained = state.contexts.len();

        // Assert
        assert_eq!(retained, 129);
    }

    #[test]
    fn default_payload_omits_browser_context_id() {
        // Arrange
        let expected = serde_json::json!({ "permissions": ["geolocation"] });

        // Act
        let permissions = vec!["geolocation".to_owned()];
        let payload = grant_payload(None, None, &permissions);

        // Assert
        assert_eq!(payload, expected);
    }

    #[test]
    fn owned_payload_matches_playwright_chromium() {
        // Arrange
        let expected = serde_json::json!({
            "permissions": ["geolocation"],
            "browserContextId": "context-7"
        });

        // Act
        let permissions = vec!["geolocation".to_owned()];
        let payload = grant_payload(Some("context-7"), None, &permissions);

        // Assert
        assert_eq!(payload, expected);
    }

    #[test]
    fn grants_merge_per_normalized_origin() {
        // Arrange
        let mut state = state(false);
        let context = ObjectId::new(7);
        let first = vec!["geolocation".to_owned()];
        state
            .commit_grant(context, Some("https://example.test".into()), first)
            .unwrap();
        let requested = vec!["camera".to_owned(), "geolocation".to_owned()];

        // Act
        let merged = state
            .merged_permissions(context, Some("https://example.test"), &requested)
            .unwrap();

        // Assert
        assert_eq!(merged, vec!["geolocation", "camera"]);
    }

    #[test]
    fn chromium_permission_mapping_matches_upstream() {
        // Arrange
        let requested = vec![
            "camera".to_owned(),
            "microphone".to_owned(),
            "local-network-access".to_owned(),
        ];

        // Act
        let mapped = protocol_permissions(&requested).unwrap();

        // Assert
        assert_eq!(
            mapped,
            vec![
                "videoCapture",
                "audioCapture",
                "localNetworkAccess",
                "localNetwork",
                "loopbackNetwork",
            ]
        );
    }

    #[test]
    fn context_permissions_validate_and_deduplicate_before_creation() {
        // Arrange
        let input = vec![
            "camera".to_owned(),
            "geolocation".to_owned(),
            "camera".to_owned(),
        ];

        // Act
        let permissions = ContextPermissions::new(input).unwrap();
        let unknown = ContextPermissions::new(vec!["bluetooth".to_owned()]);

        // Assert
        assert_eq!(permissions.0, vec!["camera", "geolocation"]);
        assert!(matches!(unknown, Err(HostError::InvalidArgument(_))));
    }

    #[test]
    fn default_geolocation_is_preserved_by_first_runtime_grant() {
        // Arrange
        let state = state(true);
        let requested = vec!["camera".to_owned()];

        // Act
        let merged = state
            .merged_permissions(ObjectId::new(7), None, &requested)
            .unwrap();

        // Assert
        assert_eq!(merged, vec!["geolocation", "camera"]);
    }

    #[test]
    fn complete_default_permission_set_is_preserved_for_persistent_context() {
        // Arrange
        let state = State {
            default_permissions: vec!["geolocation".to_owned(), "camera".to_owned()],
            contexts: Vec::new(),
            closed: false,
        };
        let requested = vec!["notifications".to_owned()];

        // Act
        let merged = state
            .merged_permissions(ObjectId::new(7), None, &requested)
            .unwrap();

        // Assert
        assert_eq!(merged, vec!["geolocation", "camera", "notifications"]);
    }

    #[test]
    fn clearing_default_context_removes_creation_permission() {
        // Arrange
        let mut state = state(true);
        let context = ObjectId::new(7);

        // Act
        state.clear(context, CdpContextOwnership::Default).unwrap();
        let merged = state
            .merged_permissions(context, None, &["camera".to_owned()])
            .unwrap();

        // Assert
        assert_eq!(merged, vec!["camera"]);
    }

    #[test]
    fn origin_and_reset_payloads_are_context_scoped() {
        // Arrange
        let permissions = vec!["notifications".to_owned()];

        // Act
        let grant = grant_payload(
            Some("context-7"),
            Some("https://example.test"),
            &permissions,
        );
        let reset = reset_payload(Some("context-7"));

        // Assert
        assert_eq!(
            grant,
            serde_json::json!({
                "permissions": ["notifications"],
                "browserContextId": "context-7",
                "origin": "https://example.test"
            })
        );
        assert_eq!(
            reset,
            serde_json::json!({ "browserContextId": "context-7" })
        );
    }

    #[test]
    fn unknown_permission_matches_upstream_error() {
        // Arrange
        let requested = vec!["bluetooth".to_owned()];

        // Act
        let error = protocol_permissions(&requested).unwrap_err();

        // Assert
        assert!(error.to_string().contains("Unknown permission: bluetooth"));
    }
}
