//! Optional Page-scoped viewport overrides.
//!
//! The leaf retains one allocator-backed record per live Page and replays it
//! through the session-initialization fence after Chromium replaces a Page
//! session. Device scale and mobile semantics are composed from the separately
//! selected Context device-emulation owner; language bindings only validate
//! the public width/height shape.

use std::error::Error;
use std::sync::{Arc, Mutex};

use playwright_rust_backend_cdp_topology::CdpContextOwnership;
use playwright_rust_cdp::{CdpSession, generated::browser};
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_session_initialization::{
    ChromiumSessionInitializationFuture, ChromiumSessionInitializationTarget,
    ChromiumSessionInitializer, ChromiumSessionTargetKind,
};

use crate::{HostError, HostSession, device_emulation};

pub mod profile {
    pub const BYTES: u64 = 8 * 1024;
}

const STORAGE_BYTES: u64 = profile::BYTES;
const SCOPE: ResourceScope = ResourceScope::extension(178, 1);

pub(crate) fn requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            STORAGE_BYTES,
            STORAGE_BYTES,
        )
        .with_metadata(1, 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PageState {
    page: ObjectId,
    viewport: device_emulation::DeviceViewport,
}

fn prune_closed_pages(pages: &mut Vec<PageState>, active: &[ObjectId]) {
    pages.retain(|entry| active.contains(&entry.page));
}

pub(crate) struct Installation {
    connection: Arc<playwright_rust_cdp::CdpConnection>,
    devices: Arc<device_emulation::Installation>,
    gate: tokio::sync::Mutex<()>,
    pages: Mutex<Vec<PageState>>,
    headful: bool,
    host_platform: &'static str,
    _storage: ResourceLease,
}

impl Installation {
    pub(crate) fn new(
        connection: Arc<playwright_rust_cdp::CdpConnection>,
        devices: Arc<device_emulation::Installation>,
        resources: &ProductionResourceLedger,
        headful: bool,
        host_platform: &'static str,
    ) -> Result<Arc<Self>, HostError> {
        let ledger = resources
            .claim(requirements())
            .map_err(HostError::backend)?;
        let storage = ledger
            .reserve(SCOPE, ResourceClass::BackendStateBytes, STORAGE_BYTES)
            .map_err(HostError::backend)?;
        Ok(Arc::new(Self {
            connection,
            devices,
            gate: tokio::sync::Mutex::new(()),
            pages: Mutex::new(Vec::new()),
            headful,
            host_platform,
            _storage: storage,
        }))
    }

    fn prepare(
        &self,
        page: ObjectId,
        active: &[ObjectId],
        viewport: device_emulation::DeviceViewport,
    ) -> Result<PageState, HostError> {
        let mut pages = self.pages.lock().map_err(HostError::backend)?;
        prune_closed_pages(&mut pages, active);
        Ok(PageState { page, viewport })
    }

    fn commit(&self, state: PageState) -> Result<(), HostError> {
        let mut pages = self.pages.lock().map_err(HostError::backend)?;
        if let Some(slot) = pages.iter_mut().find(|entry| entry.page == state.page) {
            *slot = state;
        } else {
            pages.try_reserve(1).map_err(|error| {
                HostError::ResourceExhausted(format!(
                    "Page viewport registry allocation failed: {error}"
                ))
            })?;
            pages.push(state);
        }
        Ok(())
    }

    fn viewport(
        &self,
        page: ObjectId,
    ) -> Result<Option<device_emulation::DeviceViewport>, HostError> {
        Ok(self
            .pages
            .lock()
            .map_err(HostError::backend)?
            .iter()
            .find(|entry| entry.page == page)
            .map(|entry| entry.viewport))
    }

    async fn apply(
        &self,
        session_id: &str,
        target_id: &str,
        context: ObjectId,
        ownership: CdpContextOwnership,
        viewport: device_emulation::DeviceViewport,
    ) -> Result<(), HostError> {
        let session = CdpSession::new(
            Arc::clone(&self.connection),
            session_id.to_owned(),
            target_id.to_owned(),
        );
        let window = session
            .send(
                "Browser.getWindowForTarget",
                serde_json::to_value(browser::GetWindowForTargetParams::default())
                    .map_err(HostError::backend)?,
            )
            .await
            .map_err(HostError::backend)?;
        let window: browser::GetWindowForTargetReturns =
            serde_json::from_value(window).map_err(HostError::backend)?;
        let (inset_width, inset_height) =
            device_emulation::window_insets(self.headful, self.host_platform);
        session
            .send(
                "Browser.setWindowBounds",
                serde_json::to_value(browser::SetWindowBoundsParams {
                    window_id: window.window_id,
                    bounds: browser::Bounds {
                        width: Some(i64::from(viewport.width() + inset_width)),
                        height: Some(i64::from(viewport.height() + inset_height)),
                        ..Default::default()
                    },
                })
                .map_err(HostError::backend)?,
            )
            .await
            .map_err(HostError::backend)?;
        let metrics = self.devices.page_metrics(context, ownership, viewport)?;
        session
            .send(
                "Emulation.setDeviceMetricsOverride",
                serde_json::to_value(metrics).map_err(HostError::backend)?,
            )
            .await
            .map(|_| ())
            .map_err(HostError::backend)
    }

    async fn initialize_target(
        &self,
        target: ChromiumSessionInitializationTarget<'_>,
    ) -> Result<(), HostError> {
        if target.kind != ChromiumSessionTargetKind::Page {
            return Ok(());
        }
        let Some(viewport) = self.viewport(target.page)? else {
            return Ok(());
        };
        self.apply(
            target.session_id,
            target.target_id,
            target.context,
            target.ownership,
            viewport,
        )
        .await
    }
}

const _: () = assert!(std::mem::size_of::<Installation>() < STORAGE_BYTES as usize);

pub(crate) struct Initializer<Downstream> {
    downstream: Downstream,
    installation: Arc<Installation>,
}

impl<Downstream> Initializer<Downstream> {
    pub(crate) fn new(downstream: Downstream, installation: Arc<Installation>) -> Self {
        Self {
            downstream,
            installation,
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
        ownership: playwright_rust_backend_cdp_topology::CdpContextOwnership,
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
            self.installation
                .initialize_target(target)
                .await
                .map_err(|error| -> Box<dyn Error + Send + Sync> { Box::new(error) })
        })
    }
}

impl HostSession {
    pub async fn page_set_viewport_size(
        &self,
        page_id: &str,
        width: u32,
        height: u32,
    ) -> Result<(), HostError> {
        let viewport = device_emulation::DeviceViewport::new(width, height)?;
        let (page, _call, _operation) = self.admit_operation(page_id)?;
        let installation = {
            let state = self.state.lock().map_err(HostError::backend)?;
            state
                .as_ref()
                .ok_or(HostError::TargetClosed)?
                .page_viewport
                .clone()
        };
        let _gate = tokio::select! {
            biased;
            () = self.stop.cancelled() => return Err(HostError::TargetClosed),
            gate = installation.gate.lock() => gate,
        };
        let (session_id, target_id, context, active) = {
            let state = self.state.lock().map_err(HostError::backend)?;
            let live = state.as_ref().ok_or(HostError::TargetClosed)?;
            let topology = live.composition.topology().map_err(HostError::backend)?;
            let topology = topology.topology();
            let route = topology
                .page_session_routes()
                .find(|route| route.object_id() == page)
                .ok_or(HostError::TargetClosed)?;
            let active = topology
                .page_session_routes()
                .map(|route| route.object_id())
                .collect::<Vec<_>>();
            (
                route.session_id().to_owned(),
                route.target_id().to_owned(),
                route.browser_context_object_id(),
                active,
            )
        };
        let next = installation.prepare(page, &active, viewport)?;
        installation
            .apply(
                &session_id,
                &target_id,
                context,
                CdpContextOwnership::Owned,
                viewport,
            )
            .await?;
        installation.commit(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_registry_prunes_closed_pages_and_grows_dynamically() {
        // Arrange
        let viewport = device_emulation::DeviceViewport::new(500, 500).unwrap();
        let mut stored = Vec::new();
        for index in 0..129 {
            stored.push(PageState {
                page: ObjectId::new(u64::try_from(index + 1).unwrap()),
                viewport,
            });
        }
        let active = Vec::new();

        // Act
        prune_closed_pages(&mut stored, &active);

        // Assert
        assert!(stored.is_empty());
    }
}
