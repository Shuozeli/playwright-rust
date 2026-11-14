//! Chromium CDP adapter for portable Page screenshots.
//!
//! This backend leaf attaches to one existing target-local CDP client. It does
//! not install DOM access, public evaluation, navigation, a runtime, a browser
//! provider, Node.js, or browser acquisition.

mod screenshot;

pub use screenshot::{
    CdpScreenshotAdmission, CdpScreenshotProductionProfile, CdpScreenshotService,
    CdpScreenshotServiceFactory,
};

use playwright_rust_backend_cdp_common::classify_cdp_failure;
use playwright_rust_browser_backend::{
    BackendDescriptor, BackendFeatureDescriptor, BackendFeatureId, BackendId,
    BackendImplementation, BackendProtocolId, BrowserBackend, BrowserEngine,
};
use playwright_rust_cap_content::{CONTENT_CAPABILITY_ID, PAGE_SCREENSHOT_COMMAND_ID};
use playwright_rust_core::ProtocolVersion;

pub const CHROMIUM_CDP_CONTENT_BACKEND_ID: BackendId =
    BackendId::new("playwright.rust.backend.chromium-cdp.content");
pub const CHROMIUM_CDP_PROTOCOL_ID: BackendProtocolId = BackendProtocolId::new("cdp");
pub const CDP_PAGE_SCREENSHOT_FEATURE: BackendFeatureId = BackendFeatureId::new("page.screenshot");
pub const CHROMIUM_CDP_CONTENT_FEATURES: &[BackendFeatureDescriptor] =
    &[BackendFeatureDescriptor::new(
        CDP_PAGE_SCREENSHOT_FEATURE,
        CONTENT_CAPABILITY_ID,
        ProtocolVersion::new(1, 0),
        PAGE_SCREENSHOT_COMMAND_ID,
        BackendImplementation::Native,
    )];

pub enum ChromiumCdpContentBackend {}

impl BrowserBackend for ChromiumCdpContentBackend {
    const DESCRIPTOR: BackendDescriptor = BackendDescriptor::new(
        CHROMIUM_CDP_CONTENT_BACKEND_ID,
        "Chromium CDP Page screenshot",
        BrowserEngine::Chromium,
        CHROMIUM_CDP_PROTOCOL_ID,
        CHROMIUM_CDP_CONTENT_FEATURES,
    );
}

#[cfg(test)]
mod tests {
    use playwright_rust_browser_backend::{BackendImplementation, BrowserBackend};

    use super::ChromiumCdpContentBackend;

    #[test]
    fn descriptor_contains_only_native_page_screenshot() {
        // Arrange
        let descriptor = ChromiumCdpContentBackend::DESCRIPTOR;

        // Act
        let features = descriptor
            .features
            .iter()
            .map(|feature| (feature.id.as_str(), feature.implementation))
            .collect::<Vec<_>>();

        // Assert
        assert_eq!(
            features,
            [("page.screenshot", BackendImplementation::Native)]
        );
        assert!(descriptor.event_features.is_empty());
    }
}
