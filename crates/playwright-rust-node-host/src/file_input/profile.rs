//! Explicit selected host budgets. No ordinary Node or CDP limits are raised.
use crate::HostError;
use playwright_rust_backend_cdp_file_input::PayloadProfile;
use playwright_rust_cdp::connection::large_commands::LargeCommandProfile;
use playwright_rust_composition_chromium_file_input::UploadProfile;
use playwright_rust_core::{ResourceClass, ResourceRequirements, ResourceScope};
use playwright_rust_file_input_model::FileInputLimits;
use playwright_rust_file_input_tokio::WorkProfile;

pub const ITEMS: usize = 64;
pub const PATH_BYTES: usize = 4096;
pub const NAME_BYTES: usize = 4096;
pub const MIME_BYTES: usize = 1024;
pub const METADATA_BYTES: usize = 64 * 1024;
pub(super) const STATE_SCOPE: ResourceScope = ResourceScope::extension(148, 1);
pub(super) const STATE_BYTES: u64 = 32 * 1024;
/// Maximum inline future/control-frame size for one host operation. This is a
/// per-operation storage invariant, not a global operation-count admission.
pub(super) const CALL_BYTES: u64 = 32 * 1024;

pub fn limits() -> FileInputLimits {
    FileInputLimits::new(ITEMS, PATH_BYTES, NAME_BYTES, MIME_BYTES, METADATA_BYTES)
        .expect("positive fixed file-input limits")
}
pub(crate) fn large_commands() -> Result<LargeCommandProfile, HostError> {
    LargeCommandProfile::new_allocator_backed(
        ResourceScope::extension(148, 3),
        usize::MAX,
        64 * 1024,
    )
    .map_err(HostError::backend)
}
pub(super) fn payload() -> Result<PayloadProfile, HostError> {
    PayloadProfile::new_allocator_backed(
        ResourceScope::extension(148, 4),
        ITEMS,
        70 * 1024 * 1024,
        METADATA_BYTES,
    )
    .map(PayloadProfile::with_large_commands)
    .map_err(HostError::backend)
}
pub(super) fn upload() -> Result<UploadProfile, HostError> {
    UploadProfile::new_allocator_backed(ResourceScope::extension(148, 5), 16 * 1024)
        .map_err(HostError::backend)
}
pub(super) fn worker() -> Result<WorkProfile, HostError> {
    WorkProfile::new_allocator_backed(limits(), ResourceScope::extension(148, 6), 16 * 1024)
        .map_err(HostError::backend)
}
pub(super) fn state_requirements() -> ResourceRequirements {
    ResourceRequirements::none()
        .with_class(ResourceClass::BackendStateBytes, STATE_BYTES, STATE_BYTES)
        .with_metadata(1, 1)
}
pub(crate) fn requirements() -> Result<ResourceRequirements, HostError> {
    let required = state_requirements()
        .combine_concurrent(payload()?.requirements().map_err(HostError::backend)?)
        .combine_concurrent(upload()?.requirements())
        .combine_concurrent(worker()?.requirements().map_err(HostError::backend)?);
    #[cfg(feature = "retained-file-input")]
    let required =
        required.combine_concurrent(crate::handles::file_input::RetainedUploads::requirements()?);
    #[cfg(feature = "retained-serialized-payload-import")]
    let required =
        required.combine_concurrent(crate::handles::file_input::serialized_payload::requirements());
    #[cfg(feature = "file-input-native-paths")]
    let required = required.combine_concurrent(super::native_paths::requirements()?);
    Ok(required)
}
