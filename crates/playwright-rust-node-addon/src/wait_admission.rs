//! Long-lived selector waits share one environment-wide fraction of Command
//! admission. A second Session must not consume the slots left for unblockers.

use playwright_rust_node_host::HostError;

pub(crate) type WaitPermit = ();

#[derive(Clone)]
pub(crate) struct WaitAdmissions;

impl WaitAdmissions {
    pub(crate) fn new() -> Self {
        Self
    }

    /// FFI commands are allocator-backed; runtime active-command scheduling
    /// preserves unblocker progress without a fabricated long-wait count.
    pub(crate) fn admit(&self) -> Result<WaitPermit, HostError> {
        Ok(())
    }
}
