//! Explicit lifecycle and bounded navigation-registration host selection.
//! Native dispatch/execution and public Node Frame.goto remain separate work.
use crate::{HostError, HostSession};
use playwright_rust_backend_cdp_navigation::frame_lifecycle::native::NativeLifecycleNotice;
#[cfg(feature = "frame-navigation")]
use playwright_rust_backend_cdp_navigation::frame_lifecycle::navigation::FrameNavigationWaitProfile;
#[cfg(feature = "frame-navigation")]
use playwright_rust_cap_navigation::WaitUntil;
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use playwright_rust_plugin_chromium_frame_lifecycle::{
    FrameLifecycleClock, FrameLifecycleNoticeSink, FrameLifecycleRegistry,
    FrameLifecycleRegistryProfile, PageLifecycleHandle,
};
#[cfg(feature = "frame-navigation")]
use playwright_rust_plugin_chromium_frame_lifecycle::{
    NavigationRegistrationProfile, NavigationWaitHandle,
};
use playwright_rust_plugin_chromium_frame_observer::FrameObservationResult;
use std::{
    mem::size_of,
    sync::Arc,
    time::{Duration, Instant},
};

// One admitted allocation is shared through the injected clock and sink ports.
// The external sink retains no envelope. Explicitly registered navigation
// owners consume ordered notices internally under the Page lifecycle lock.
struct LifecycleSupport {
    origin: Instant,
    _lease: ResourceLease,
}
#[cfg(feature = "frame-navigation")]
const INITIAL_NAVIGATION_SLOTS_PER_PAGE: usize = 4;
impl FrameLifecycleClock for LifecycleSupport {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}
impl FrameLifecycleNoticeSink for LifecycleSupport {
    fn observe(&self, _notice: NativeLifecycleNotice<'_>) -> FrameObservationResult {
        Ok(())
    }
}
fn profile() -> Result<FrameLifecycleRegistryProfile, HostError> {
    // Page, Frame, request and native-ID ownership follows observed Chromium
    // state. Navigation registration remains a separately selected scheduler.
    let profile = FrameLifecycleRegistryProfile::new_allocator_backed(123, 256)
        .map_err(HostError::backend)?;
    #[cfg(feature = "frame-navigation")]
    let profile = profile
        .with_navigation_slots(INITIAL_NAVIGATION_SLOTS_PER_PAGE)
        .map_err(HostError::backend)?;
    Ok(profile)
}
#[cfg(feature = "frame-navigation")]
pub(crate) fn registration_profile() -> Result<NavigationRegistrationProfile, HostError> {
    registration_profile_with_url_bytes(0)
}
#[cfg(feature = "navigation-url-match")]
pub(crate) fn matched_registration_profile() -> Result<NavigationRegistrationProfile, HostError> {
    registration_profile_with_url_bytes(16 * 1024)
}
#[cfg(feature = "frame-navigation")]
fn registration_profile_with_url_bytes(
    url_bytes: usize,
) -> Result<NavigationRegistrationProfile, HostError> {
    let wait = FrameNavigationWaitProfile::new_allocator_backed(
        ResourceScope::extension(125, 0),
        // Initial capacities only; observed rows grow through the allocator.
        16,
        8,
        256,
        1024,
        64 * 1024,
    )
    .map_err(HostError::backend)?;
    let wait = if url_bytes == 0 {
        wait
    } else {
        wait.with_url_bytes(url_bytes).map_err(HostError::backend)?
    };
    Ok(NavigationRegistrationProfile::new_allocator_backed(
        wait,
        ResourceScope::extension(125, 1),
    ))
}
fn support_requirements() -> ResourceRequirements {
    let bytes = (size_of::<LifecycleSupport>() + 2 * size_of::<usize>() + 256) as u64;
    ResourceRequirements::none()
        .with_class(ResourceClass::BackendStateBytes, bytes, bytes)
        .with_metadata(1, 1)
}
pub(crate) fn requirements() -> Result<ResourceRequirements, HostError> {
    let requirements = profile()?
        .requirements()
        .combine_concurrent(support_requirements());
    // Registration indexes, wait arenas and moved-out results grow through the
    // allocator. Per-operation field validation remains independent.
    Ok(requirements)
}
pub(crate) fn install(
    resources: &ProductionResourceLedger,
    epoch: u64,
) -> Result<Arc<FrameLifecycleRegistry>, HostError> {
    let requirement = support_requirements();
    let lease = resources
        .claim(requirement)
        .map_err(HostError::backend)?
        .reserve(
            ResourceScope::extension(124, 0),
            ResourceClass::BackendStateBytes,
            requirement.class(ResourceClass::BackendStateBytes).global,
        )
        .map_err(HostError::backend)?;
    let support = Arc::new(LifecycleSupport {
        origin: Instant::now(),
        _lease: lease,
    });
    FrameLifecycleRegistry::new(profile()?, epoch, resources, support.clone(), support)
        .map(Arc::new)
        .map_err(HostError::backend)
}
impl HostSession {
    /// Rust-only registration, not Frame.goto: no native command is sent here.
    /// Timeout zero/None disables the deadline. The executor must dispatch the
    /// exact native route, deliver its receipt, and drive notified/deadline waits.
    #[cfg(feature = "frame-navigation")]
    pub async fn register_frame_navigation(
        &self,
        page_id: &str,
        frame_id: &str,
        wait: WaitUntil,
        timeout: Option<Duration>,
    ) -> Result<NavigationWaitHandle, HostError> {
        let frame = frame_id
            .parse::<u64>()
            .ok()
            .filter(|id| *id != 0)
            .map(ObjectId::new)
            .ok_or_else(|| HostError::InvalidArgument("invalid portable Frame ID".into()))?;
        let page = self.frame_lifecycle(page_id).await?;
        self.require_visible_frame_id(frame)?;
        let now = page.now().map_err(HostError::backend)?;
        let deadline = timeout
            .filter(|timeout| !timeout.is_zero())
            .map(|timeout| {
                now.checked_add(timeout).ok_or_else(|| {
                    HostError::InvalidArgument("navigation deadline overflow".into())
                })
            })
            .transpose()?;
        page.register_navigation(
            registration_profile()?,
            &self.resources,
            frame,
            wait,
            deadline,
        )
        .map_err(HostError::backend)
    }
    /// Rust-only, opt-in readiness handle. Establishes the existing host
    /// publication fence first; this does not initiate or await navigation.
    pub async fn frame_lifecycle(&self, page_id: &str) -> Result<PageLifecycleHandle, HostError> {
        let page = page_id
            .parse::<u64>()
            .ok()
            .filter(|id| *id != 0)
            .map(ObjectId::new)
            .ok_or_else(|| HostError::InvalidArgument("invalid portable Page ID".into()))?;
        self.flush_topology().await?;
        self.require_visible_page_id(page)?;
        let state = self.state.lock().map_err(HostError::backend)?;
        let live = state.as_ref().ok_or(HostError::TargetClosed)?;
        live.frame_lifecycle
            .page(page)
            .map_err(HostError::backend)?
            .ok_or(HostError::TargetClosed)
    }
}

#[cfg(all(test, feature = "frame-navigation"))]
mod tests {
    use super::*;

    #[test]
    fn allocator_backed_registration_adds_no_fabricated_global_capacity() {
        // Arrange
        let base = profile()
            .unwrap()
            .requirements()
            .combine_concurrent(support_requirements());
        #[cfg(feature = "navigation-url-match")]
        let registration = matched_registration_profile().unwrap().requirements();
        #[cfg(not(feature = "navigation-url-match"))]
        let registration = registration_profile().unwrap().requirements();

        // Act
        let selected = requirements().unwrap();
        let selected_bytes = selected.class(ResourceClass::BackendStateBytes).global;
        let base_bytes = base.class(ResourceClass::BackendStateBytes).global;

        // Assert
        assert_eq!(INITIAL_NAVIGATION_SLOTS_PER_PAGE, 4);
        assert_eq!(registration, ResourceRequirements::none());
        assert_eq!(selected_bytes, base_bytes);
    }
}
