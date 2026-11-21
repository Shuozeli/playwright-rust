//! Host-selected Page controller inventory; native cleanup precedes transport teardown.
use super::{PageDragMoveSource, PageDragSourceProfile};
use crate::HostError;
use playwright_rust_backend_cdp_drag::{
    DragCaptureFactory, DragDataLimits, DragDataProfile, DragDataStore, DragListenerFactory,
    DragPage,
    mouse::{MouseDragDispatcher, MouseDragRetirementDriver},
};
use playwright_rust_cdp::CdpClient;
use playwright_rust_composition_chromium::ChromiumFrameWorldAccess;
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceLimits,
    ResourceRequirements, ResourceScope,
};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

const OPERATIONS: usize = crate::mouse::EXECUTING
    * playwright_rust_input_model::INPUT_MOUSE_MAX_CLICKS as usize
    + crate::keyboard::EXECUTING
    + crate::locator::keyboard::EXECUTING;
const CONTROLLER: ResourceScope = ResourceScope::extension(122, 2);
// Separate from reusable wrapper payloads. Native child futures, script/wire
// argument trees and bounded errors remain host-owned provider allowances.
const DATA_BYTES: usize = 256 * 1024;
const PROVIDER_BYTES: u64 = (OPERATIONS * (DATA_BYTES + 64 * 1024)) as u64;
fn controller_bytes() -> u64 {
    (std::mem::size_of::<PageController>() + 4096) as u64 + PROVIDER_BYTES
}
fn source_profile() -> Result<PageDragSourceProfile, HostError> {
    PageDragSourceProfile::new_allocator_backed_worlds(
        ResourceScope::extension(122, 3),
        ResourceScope::extension(122, 4),
        1,
    )
    .map_err(HostError::backend)
}
fn data_profile() -> Result<DragDataProfile, HostError> {
    DragDataProfile::new_allocator_backed_session(
        ResourceScope::extension(122, 5),
        DragDataLimits {
            items: 32,
            files: 32,
            text_bytes: DATA_BYTES,
        },
        NonZeroUsize::new(1).expect("one retained drag record"),
    )
    .map_err(HostError::backend)
}
pub(crate) fn requirements() -> Result<ResourceRequirements, HostError> {
    Ok(ResourceRequirements::none())
}
fn controller_requirements() -> Result<ResourceRequirements, HostError> {
    let storage = ResourceRequirements::none()
        .with_class(
            ResourceClass::BackendStateBytes,
            controller_bytes(),
            controller_bytes(),
        )
        .with_metadata(1, 1);
    Ok(storage.combine_concurrent(
        source_profile()?
            .requirements()
            .map_err(HostError::backend)?
            .combine_concurrent(data_profile()?.requirements())
            .combine_concurrent(DragListenerFactory::allocator_backed_requirements())
            .combine_concurrent(DragCaptureFactory::requirements())
            .combine_concurrent(DragPage::allocator_backed_requirements())
            .combine_concurrent(MouseDragDispatcher::allocator_backed_requirements()),
    ))
}
fn controller_resources() -> Result<ProductionResourceLedger, HostError> {
    let requirements = controller_requirements()?;
    let mut limits = ResourceLimits::deny_all(requirements.scopes, requirements.leases);
    for class in ResourceClass::ALL {
        let bound = requirements.class(class);
        limits = limits.with_class(class, bound.global, bound.per_scope);
    }
    ProductionResourceLedger::new(limits, requirements).map_err(HostError::backend)
}
struct Entry {
    id: ObjectId,
    client: Arc<dyn CdpClient>,
    controller: Arc<PageController>,
}
pub(crate) struct Installation {
    access: ChromiumFrameWorldAccess,
    entries: Mutex<Vec<Entry>>,
}
pub(crate) struct PageController {
    pub(crate) dispatcher: Arc<MouseDragDispatcher>,
    page: DragPage,
    driver: tokio::sync::Mutex<MouseDragRetirementDriver>,
    _resources: ProductionResourceLedger,
    _storage: ResourceLease,
}
impl PageController {
    pub(crate) async fn drain(&self) -> Result<(), HostError> {
        self.driver
            .lock()
            .await
            .retire_quarantined()
            .await
            .map_err(HostError::backend)
    }
    async fn shutdown(&self) -> Result<(), HostError> {
        self.dispatcher.stop();
        let drained = self.drain().await;
        // Preserve native cancellation ownership through its reply. The host
        // awaits this task; no caller timeout drops an in-flight cancel command.
        let cancel = self.page.cancel().map_err(HostError::backend)?;
        let cancelled = cancel.await.map(|_| ()).map_err(HostError::backend);
        drained.and(cancelled)
    }
}
impl Installation {
    pub(crate) fn new(
        access: ChromiumFrameWorldAccess,
        _resources: &ProductionResourceLedger,
    ) -> Result<Self, HostError> {
        Ok(Self {
            access,
            entries: Mutex::new(Vec::new()),
        })
    }
    pub(crate) fn bind(
        &self,
        id: ObjectId,
        client: &Arc<dyn CdpClient>,
    ) -> Result<Arc<PageController>, HostError> {
        let mut entries = self.entries.lock().map_err(HostError::backend)?;
        if let Some(entry) = entries
            .iter()
            .find(|entry| entry.id == id && Arc::ptr_eq(&entry.client, client))
        {
            return Ok(entry.controller.clone());
        }
        // Only discard a stale route after revoking its escaped dispatchers and
        // observing no native owner or active Page drag. Dirty state is retained.
        let mut index = 0;
        while index < entries.len() {
            let entry = &entries[index];
            let binding = match self.access.page_input_binding(entry.id) {
                Ok(binding) => binding,
                Err(playwright_rust_composition_chromium::FrameWorldAccessError::Topology(
                    playwright_rust_composition_chromium::CdpPageRouteError::PageCrashed { .. },
                )) => None,
                Err(error) => return Err(HostError::backend(error)),
            };
            let current =
                binding.is_some_and(|binding| Arc::ptr_eq(&binding.into_parts().0, &entry.client));
            if !current {
                entry.controller.dispatcher.stop();
            }
            if !current
                && entry.controller.dispatcher.is_drained()
                && !entry.controller.page.is_dragging()
            {
                entries.remove(index);
            } else {
                index += 1;
            }
        }
        entries.try_reserve(1).map_err(HostError::backend)?;
        let resources = controller_resources()?;
        let ledger = resources
            .claim(controller_requirements()?)
            .map_err(HostError::backend)?;
        let storage = ledger
            .reserve(
                CONTROLLER,
                ResourceClass::BackendStateBytes,
                controller_bytes(),
            )
            .map_err(HostError::backend)?;
        let native = client.clone().retained_native_session().ok_or_else(|| {
            HostError::Backend("drag runtime requires the retained native session".into())
        })?;
        let store = DragDataStore::new(data_profile()?, &resources).map_err(HostError::backend)?;
        let captures = DragCaptureFactory::new(
            native.clone(),
            store,
            ResourceScope::extension(122, 6),
            &resources,
        )
        .map_err(HostError::backend)?;
        let session = native
            .session_id()
            .ok_or_else(|| HostError::Backend("drag requires a Page session".into()))?;
        let namespace = format!("node-host-drag-{session}");
        let listeners = DragListenerFactory::new_allocator_backed(
            &namespace,
            ResourceScope::extension(122, 7),
            &resources,
        )
        .map_err(HostError::backend)?;
        let source = Arc::new(
            PageDragMoveSource::new(
                self.access.clone(),
                id,
                listeners,
                captures.clone(),
                source_profile()?,
                &resources,
            )
            .map_err(HostError::backend)?,
        );
        let page = DragPage::new_allocator_backed(
            native,
            captures,
            ResourceScope::extension(122, 8),
            &resources,
        )
        .map_err(HostError::backend)?;
        let (dispatcher, driver) = MouseDragDispatcher::new_allocator_backed(
            client.clone(),
            page.clone(),
            source,
            ResourceScope::extension(122, 9),
            &resources,
        )
        .map_err(HostError::backend)?;
        let controller = Arc::new(PageController {
            dispatcher: Arc::new(dispatcher),
            page,
            driver: tokio::sync::Mutex::new(driver),
            _resources: resources,
            _storage: storage,
        });
        entries.push(Entry {
            id,
            client: client.clone(),
            controller: controller.clone(),
        });
        Ok(controller)
    }
    pub(crate) async fn shutdown(&self) -> Result<(), HostError> {
        let mut first = None;
        // Mouse admission is sealed and all invocation workers have finished, so
        // the inventory cannot change. Never hold its std Mutex across await.
        let mut index = 0;
        loop {
            let controller = self
                .entries
                .lock()
                .map_err(HostError::backend)?
                .get(index)
                .map(|entry| entry.controller.clone());
            let Some(controller) = controller else {
                break;
            };
            if let Err(error) = controller.shutdown().await {
                first.get_or_insert(error);
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| HostError::Backend("drag controller index overflow".into()))?;
        }
        first.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_backed_controller_ownership_has_no_connection_count_plan() {
        // Arrange
        let mut controllers = Vec::new();
        controllers.try_reserve(129).unwrap();

        // Act
        for _ in 0..129 {
            let resources = controller_resources().unwrap();
            let lease = resources
                .claim(controller_requirements().unwrap())
                .unwrap()
                .reserve(
                    CONTROLLER,
                    ResourceClass::BackendStateBytes,
                    controller_bytes(),
                )
                .unwrap();
            controllers.push((resources, lease));
        }

        // Assert
        assert_eq!(requirements().unwrap(), ResourceRequirements::none());
        assert_eq!(controllers.len(), 129);
        assert!(
            controllers
                .iter()
                .all(|(resources, _)| { resources.metrics().unwrap().active_leases == 1 })
        );
    }
}
