//! Session-local ownership projection. Retained public trees/nodes never own
//! this index, the URL book, the host, or a browser transport.
#[cfg(feature = "frame-detach-events")]
use crate::frame_metadata::TreeDetachReservation;
use crate::frame_metadata::{FrameClosePermit, FrameMetadata};
#[cfg(test)]
use crate::frame_tree::FrameTreeLimits;
use crate::frame_tree::{
    FrameBinding, FrameConversion, FrameNode, FrameTree, FrameTreeBinding, FrameTreeFactory,
    FrameTreeMetrics, ReservedFrameBinding, ReservedPageTree, ReservedTreeBinding, Storage,
};
use crate::{FrameUrlSnapshot, HostError};
use playwright_rust_backend_cdp_topology::CdpTopologyEffect;
use playwright_rust_core::ObjectId;
use playwright_rust_plugin_chromium_frame_observer::{
    ChromiumFrameOwnership, ChromiumFrameOwnershipView, ChromiumFrameUrlObserver,
};
use std::{
    error::Error,
    sync::{Arc, Mutex},
};

const FRAMES: usize = 128;
#[cfg(test)]
const BASE_REVISIONS: usize = if cfg!(any(feature = "console-events", feature = "page-errors")) {
    512
} else {
    256
};
#[cfg(any(test, feature = "frame-detach-events"))]
const DETACH_RESERVATIONS: usize = if cfg!(feature = "frame-detach-events") {
    FRAMES - 1
} else {
    0
};
#[cfg(test)]
const FRAME_EVENT_RESERVATIONS: usize = if cfg!(feature = "frame-events") {
    2 * (FRAMES - 1)
} else {
    0
};
#[cfg(test)]
const REVISIONS: usize = BASE_REVISIONS + 2 * DETACH_RESERVATIONS + FRAME_EVENT_RESERVATIONS;
#[cfg(test)]
const STATES: usize = 256 + DETACH_RESERVATIONS;
mod close_events;
mod collections;
mod external_close;
mod ready;
use collections::CollectionState;
struct PageEntry {
    id: ObjectId,
    internal: bool,
    tree: FrameTree,
    close: Option<FrameClosePermit>,
    ready: Option<crate::frame_metadata::CreationDelivery>,
}
struct NodeEntry {
    page: ObjectId,
    node: FrameNode,
    #[cfg(feature = "frame-detach-events")]
    detach: Option<crate::LifecycleReservation>,
    #[cfg(feature = "frame-detach-events")]
    detach_tree: Option<TreeDetachReservation>,
}
#[cfg(feature = "frame-detach-events")]
struct DetachedEntry {
    page: ObjectId,
    node: FrameNode,
    reservation: crate::LifecycleReservation,
    tree: Option<TreeDetachReservation>,
}
#[cfg(feature = "frame-detach-events")]
const DETACH_SCRATCH_BYTES: usize = DETACH_RESERVATIONS * std::mem::size_of::<DetachedEntry>();
#[cfg(not(feature = "frame-detach-events"))]
const DETACH_SCRATCH_BYTES: usize = 0;
struct State {
    pages: Vec<PageEntry>,
    nodes: Vec<NodeEntry>,
    closed: bool,
    collections: Option<CollectionState>,
}

pub(crate) struct FrameTrees {
    urls: Arc<FrameMetadata>,
    factory: FrameTreeFactory,
    state: Mutex<State>,
    _storage: Arc<Storage>,
}
pub(crate) struct ReservedPage {
    tree: ReservedPageTree,
    binding: ReservedFrameBinding,
    tree_binding: ReservedTreeBinding,
    ready: Option<crate::frame_metadata::CreationDelivery>,
    close: FrameClosePermit,
}

impl FrameTrees {
    #[cfg(test)]
    pub fn new(urls: Arc<FrameMetadata>) -> Result<Self, HostError> {
        let factory = FrameTreeFactory::new(FrameTreeLimits {
            states: STATES,
            frames_per_page: FRAMES,
            revisions: REVISIONS,
            bytes: 8 * 1024 * 1024,
        })?;
        Self::with_factory(urls, factory, FRAMES)
    }

    pub fn new_allocator_backed(urls: Arc<FrameMetadata>) -> Result<Self, HostError> {
        Self::with_factory(urls, FrameTreeFactory::new_allocator_backed()?, 0)
    }

    fn with_factory(
        urls: Arc<FrameMetadata>,
        factory: FrameTreeFactory,
        initial_capacity: usize,
    ) -> Result<Self, HostError> {
        // Finite test profiles account their fixed vectors here. The
        // Playwright-compatible profile keeps only fixed owner storage in the
        // ledger and grows all projection vectors through fallible allocation.
        let index_bytes = std::mem::size_of::<Self>()
            + 2 * std::mem::size_of::<usize>()
            + initial_capacity
                * (std::mem::size_of::<NodeEntry>()
                    + std::mem::size_of::<ChromiumFrameOwnership>()
                    + std::mem::size_of::<FrameNode>())
            + if factory.allocator_backed() {
                0
            } else {
                DETACH_SCRATCH_BYTES
            };
        #[cfg(feature = "frame-events")]
        let index_bytes =
            index_bytes + initial_capacity * std::mem::size_of::<(ObjectId, FrameNode)>();
        let bytes = urls
            .publication_storage_bytes()?
            .checked_add(index_bytes)
            .ok_or_else(|| HostError::ResourceExhausted("Frame index storage overflow".into()))?;
        let storage = Arc::new(factory.reserve_index(bytes)?);
        urls.admit_page_closure(storage.clone())?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(initial_capacity)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        Ok(Self {
            urls,
            factory,
            state: Mutex::new(State {
                pages: Vec::new(),
                nodes,
                closed: false,
                collections: None,
            }),
            _storage: storage,
        })
    }

    pub fn get(&self, page: ObjectId) -> Result<FrameTree, HostError> {
        self.state
            .lock()
            .map_err(HostError::backend)?
            .pages
            .iter()
            .find(|entry| entry.id == page)
            .map(|entry| entry.tree.clone())
            .ok_or_else(|| HostError::Backend("Page Frame tree is not available".into()))
    }
    pub fn metrics(&self) -> Result<FrameTreeMetrics, HostError> {
        self.factory.metrics()
    }
    pub fn reserve_conversion(&self, bytes: usize) -> Result<FrameConversion, HostError> {
        self.factory.reserve_conversion(bytes)
    }

    #[cfg(feature = "page-events")]
    pub(crate) fn publish_page_lifecycle(
        &self,
        page: ObjectId,
        dom_content_loaded: bool,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let collection = state.collections.as_mut().ok_or_else(|| {
            HostError::Backend("Page lifecycle publication requires collections".into())
        })?;
        let before = collection.planned.clone().ok_or_else(|| {
            HostError::Backend("Page lifecycle publication has no planned projection".into())
        })?;
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        let reserved = self.urls.publications().reserve_lifecycle(&self.factory)?;
        let cause = if dom_content_loaded {
            crate::LifecycleCause::PageDomContentLoaded { page }
        } else {
            crate::LifecycleCause::PageLoad { page }
        };
        let after = self
            .urls
            .publications()
            .stage_lifecycle(reserved, before, cause)?;
        collection.planned = Some(after);
        Ok(())
    }

    #[cfg(feature = "page-events")]
    pub(crate) fn publish_main_frame_navigated(&self, page: ObjectId) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let collection = state.collections.as_mut().ok_or_else(|| {
            HostError::Backend("Page navigation publication requires collections".into())
        })?;
        let before = collection.planned.clone().ok_or_else(|| {
            HostError::Backend("Page navigation publication has no planned projection".into())
        })?;
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        let reserved = self.urls.publications().reserve_lifecycle(&self.factory)?;
        let after = self.urls.publications().stage_lifecycle(
            reserved,
            before,
            crate::LifecycleCause::PageMainFrameNavigated { page },
        )?;
        collection.planned = Some(after);
        Ok(())
    }

    #[cfg(feature = "frame-events")]
    pub(crate) fn publish_frame_navigated(
        &self,
        page: ObjectId,
        frame: ObjectId,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let Some(node) = state
            .nodes
            .iter()
            .find(|entry| entry.page == page && entry.node.id() == frame)
            .map(|entry| entry.node.clone())
        else {
            return Ok(());
        };
        let Some(collection) = state.collections.as_mut() else {
            return Ok(());
        };
        let Some(before) = collection.planned.clone() else {
            return Ok(());
        };
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        let reserved = self.urls.publications().reserve_lifecycle(&self.factory)?;
        let after = self.urls.publications().stage_lifecycle(
            reserved,
            before,
            crate::LifecycleCause::PageFrameNavigated { page, frame: node },
        )?;
        collection.planned = Some(after);
        Ok(())
    }

    #[cfg(feature = "console-events")]
    pub(crate) fn publish_page_console(
        &self,
        page: ObjectId,
        payload_bytes: usize,
        build: impl FnOnce() -> Result<Arc<crate::ConsoleMessage>, HostError>,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let collection = state.collections.as_mut().ok_or_else(|| {
            HostError::Backend("Page console publication requires collections".into())
        })?;
        let before = collection.planned.clone().ok_or_else(|| {
            HostError::Backend("Page console publication has no planned projection".into())
        })?;
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        // The producer computes the exact bounded body size without allocating
        // copied strings. Admit that body before crossing the publication fence.
        let reserved = self
            .urls
            .publications()
            .reserve_lifecycle_payload(&self.factory, payload_bytes)?;
        let message = build()?;
        let after = self.urls.publications().stage_lifecycle(
            reserved,
            before,
            crate::LifecycleCause::PageConsole { page, message },
        )?;
        collection.planned = Some(after);
        Ok(())
    }

    #[cfg(feature = "page-errors")]
    pub(crate) fn publish_page_error(
        &self,
        page: ObjectId,
        payload_bytes: usize,
        build: impl FnOnce() -> Result<Arc<crate::PageError>, HostError>,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let collection = state.collections.as_mut().ok_or_else(|| {
            HostError::Backend("Page error publication requires collections".into())
        })?;
        let before = collection.planned.clone().ok_or_else(|| {
            HostError::Backend("Page error publication has no planned projection".into())
        })?;
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        let reserved = self
            .urls
            .publications()
            .reserve_lifecycle_payload(&self.factory, payload_bytes)?;
        let error = build()?;
        let after = self.urls.publications().stage_lifecycle(
            reserved,
            before,
            crate::LifecycleCause::PageError { page, error },
        )?;
        collection.planned = Some(after);
        Ok(())
    }

    #[cfg(feature = "page-dialogs")]
    pub(crate) fn publish_page_dialog(
        &self,
        page: ObjectId,
        payload_bytes: usize,
        build: impl FnOnce() -> Result<crate::PageDialog, HostError>,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let collection = state.collections.as_mut().ok_or_else(|| {
            HostError::Backend("Page dialog publication requires collections".into())
        })?;
        let before = collection.planned.clone().ok_or_else(|| {
            HostError::Backend("Page dialog publication has no planned projection".into())
        })?;
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        let reserved = self
            .urls
            .publications()
            .reserve_lifecycle_payload(&self.factory, payload_bytes)?;
        let dialog = Arc::new(build()?);
        let after = self.urls.publications().stage_lifecycle(
            reserved,
            before,
            crate::LifecycleCause::PageDialog { page, dialog },
        )?;
        collection.planned = Some(after);
        Ok(())
    }

    #[cfg(feature = "file-chooser-events")]
    pub(crate) fn publish_page_file_chooser(
        &self,
        page: ObjectId,
        payload_bytes: usize,
        build: impl FnOnce() -> Result<crate::PageFileChooser, HostError>,
    ) -> Result<(), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let collection = state.collections.as_mut().ok_or_else(|| {
            HostError::Backend("Page file chooser publication requires collections".into())
        })?;
        let before = collection.planned.clone().ok_or_else(|| {
            HostError::Backend("Page file chooser publication has no planned projection".into())
        })?;
        if !before.pages().iter().any(|entry| entry.id == page) {
            return Ok(());
        }
        let reserved = self
            .urls
            .publications()
            .reserve_lifecycle_payload(&self.factory, payload_bytes)?;
        let chooser = Arc::new(build()?);
        let after = self.urls.publications().stage_lifecycle(
            reserved,
            before,
            crate::LifecycleCause::PageFileChooser { page, chooser },
        )?;
        collection.planned = Some(after);
        Ok(())
    }

    pub fn reserve_page(&self, url: FrameUrlSnapshot) -> Result<ReservedPage, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if !self.factory.allocator_backed() && state.nodes.len() == FRAMES {
            return Err(HostError::ResourceExhausted(
                "Frame index capacity exhausted".into(),
            ));
        }
        // Only the serial lifecycle driver creates Pages; topology observations
        // remain paused through activation or compensation.
        Ok(ReservedPage {
            close: self.urls.publications().reserve_close()?,
            tree: self.factory.reserve_page(url)?,
            binding: self.factory.reserve_binding()?,
            tree_binding: self.factory.reserve_tree_binding()?,
            ready: self.reserve_ready_delivery(&state)?,
        })
    }

    pub fn activate_page(
        &self,
        reservation: ReservedPage,
        page: ObjectId,
        main: ObjectId,
        internal: bool,
    ) -> Result<(FrameTree, FrameBinding, FrameTreeBinding), HostError> {
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        if !self.factory.allocator_backed() && state.nodes.len() == FRAMES {
            return Err(HostError::ResourceExhausted(
                "Frame index capacity exhausted".into(),
            ));
        }
        if state.pages.iter().any(|entry| entry.id == page)
            || state.nodes.iter().any(|entry| entry.node.id() == main)
        {
            return Err(HostError::Backend(
                "Created Page Frame identity is already indexed".into(),
            ));
        }
        let (node, tree) = reservation.tree.activate(main);
        let binding = reservation.binding.activate(node.clone());
        let tree_binding = reservation.tree_binding.activate(tree.clone());
        state
            .nodes
            .try_reserve(1)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        state
            .pages
            .try_reserve(1)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        state.nodes.push(NodeEntry {
            page,
            node,
            #[cfg(feature = "frame-detach-events")]
            detach: None,
            #[cfg(feature = "frame-detach-events")]
            detach_tree: None,
        });
        state.pages.push(PageEntry {
            id: page,
            internal,
            tree: tree.clone(),
            close: Some(reservation.close),
            ready: reservation.ready,
        });
        Ok((tree, binding, tree_binding))
    }

    #[cfg(any(feature = "context-storage-state", feature = "network-events", test))]
    pub(crate) fn is_internal_page(&self, page: ObjectId) -> Result<bool, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        Ok(state
            .pages
            .iter()
            .any(|entry| entry.id == page && entry.internal))
    }

    /// Public direct-ID operations require both an indexed non-internal Page
    /// and its committed visibility, not merely native Context membership.
    pub(crate) fn is_published_public_page(&self, page: ObjectId) -> Result<bool, HostError> {
        let state = self.state.lock().map_err(HostError::backend)?;
        if state.closed
            || !state
                .pages
                .iter()
                .any(|entry| entry.id == page && !entry.internal)
        {
            return Ok(false);
        }
        drop(state);
        Ok(self
            .urls
            .publications()
            .published_collections()?
            .is_some_and(|view| view.pages().iter().any(|entry| entry.id == page)))
    }

    pub fn clear(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        let terminal = Self::terminal_publication(&mut state);
        let pages = state
            .pages
            .iter_mut()
            .map(|page| (&page.tree, &mut page.close));
        if let Some((terminal, view)) = terminal {
            (*terminal).stage(&self.urls.publications(), view, pages);
        } else {
            self.urls.publications().close_for_teardown(pages);
        }
        state.pages.clear();
        #[cfg(feature = "frame-detach-events")]
        for entry in &state.nodes {
            entry.node.release_retired_binding();
        }
        state.nodes.clear();
        state.collections = None;
    }

    #[cfg(test)]
    pub(crate) fn reconcile(
        &self,
        rows: impl Iterator<Item = ChromiumFrameOwnership>,
        contains_page: impl Fn(ObjectId) -> bool,
    ) -> Result<(), HostError> {
        self.reconcile_with_effects(rows, contains_page, &[])
    }

    fn reconcile_with_effects(
        &self,
        rows: impl Iterator<Item = ChromiumFrameOwnership>,
        contains_page: impl Fn(ObjectId) -> bool,
        effects: &[CdpTopologyEffect],
    ) -> Result<(), HostError> {
        #[cfg(not(feature = "frame-detach-events"))]
        let _ = effects;
        let mut state = self.state.lock().map_err(HostError::backend)?;
        if state.closed {
            return Err(HostError::TargetClosed);
        }
        let initial_capacity = if self.factory.allocator_backed() {
            0
        } else {
            FRAMES
        };
        let mut snapshot = Vec::new();
        snapshot
            .try_reserve_exact(initial_capacity)
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        for row in rows {
            if !self.factory.allocator_backed() && snapshot.len() == FRAMES {
                return Err(HostError::ResourceExhausted(
                    "Frame projection exceeds index capacity".into(),
                ));
            }
            snapshot
                .try_reserve(1)
                .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
            snapshot.push(row);
        }
        snapshot.sort_unstable_by_key(|row| (row.page, row.parent.is_some(), row.frame));
        if self.factory.allocator_backed() {
            state
                .nodes
                .try_reserve(snapshot.len())
                .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
            state
                .pages
                .try_reserve(snapshot.len())
                .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        }
        let mut desired = Vec::new();
        desired
            .try_reserve(snapshot.len())
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        #[cfg(feature = "frame-events")]
        let mut newly_attached = Vec::new();
        #[cfg(feature = "frame-events")]
        newly_attached
            .try_reserve_exact(if self.factory.allocator_backed() {
                snapshot.len()
            } else {
                initial_capacity
            })
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        for page in &mut state.pages {
            if !contains_page(page.id) && page.close.is_some() {
                self.urls
                    .publications()
                    .stage_close(&page.tree, &mut page.close)?;
            }
        }
        state.pages.retain(|entry| contains_page(entry.id));
        // Retired FrameNodes remain in staged tree revisions until publication.
        #[cfg(feature = "frame-detach-events")]
        let mut removed = Vec::new();
        #[cfg(feature = "frame-detach-events")]
        removed
            .try_reserve_exact(if self.factory.allocator_backed() {
                state.nodes.len()
            } else {
                DETACH_RESERVATIONS
            })
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        #[cfg(feature = "frame-detach-events")]
        let mut missing_reservation = false;
        state.nodes.retain_mut(|entry| {
            if snapshot.iter().any(|row| row.frame == entry.node.id()) {
                return true;
            }
            #[cfg(feature = "frame-detach-events")]
            if entry.node.parent_id().is_some()
                && effects.iter().any(|effect| {
                    matches!(effect, CdpTopologyEffect::FrameRemoved { page, frame }
                            if *page == entry.page && *frame == entry.node.id())
                })
            {
                if let (Some(reservation), Some(tree)) =
                    (entry.detach.take(), entry.detach_tree.take())
                {
                    removed.push(DetachedEntry {
                        page: entry.page,
                        node: entry.node.clone(),
                        reservation,
                        tree: Some(tree),
                    });
                    return false;
                } else {
                    missing_reservation = true;
                }
            }
            #[cfg(feature = "frame-detach-events")]
            entry.node.release_retired_binding();
            false
        });
        #[cfg(feature = "frame-detach-events")]
        if missing_reservation {
            return Err(HostError::Backend(
                "live child Frame lacks pre-admitted detach publication".into(),
            ));
        }
        for row in &snapshot {
            if let Some(prior) = state
                .nodes
                .iter()
                .find(|entry| entry.node.id() == row.frame)
            {
                if prior.page != row.page || prior.node.parent_id() != row.parent {
                    return Err(HostError::Backend(
                        "Retained Frame ownership changed identity".into(),
                    ));
                }
            } else {
                let url = self.urls.ensure_attached(row.frame)?;
                #[cfg(feature = "frame-detach-events")]
                let detach = row
                    .parent
                    .map(|_| self.urls.publications().reserve_lifecycle(&self.factory))
                    .transpose()?;
                #[cfg(feature = "frame-detach-events")]
                let detach_tree = row
                    .parent
                    .map(|_| self.urls.publications().reserve_tree_detach(&self.factory))
                    .transpose()?;
                #[cfg(feature = "frame-detach-events")]
                let retired_binding = row
                    .parent
                    .map(|_| self.factory.reserve_detach_binding())
                    .transpose()?;
                let node = self.factory.frame(row.frame, row.parent, url)?;
                #[cfg(feature = "frame-events")]
                if row.parent.is_some() {
                    newly_attached.push((row.page, node.clone()));
                }
                #[cfg(feature = "frame-detach-events")]
                if let Some(binding) = retired_binding {
                    node.install_retired_binding(binding)?;
                }
                state.nodes.push(NodeEntry {
                    page: row.page,
                    node,
                    #[cfg(feature = "frame-detach-events")]
                    detach,
                    #[cfg(feature = "frame-detach-events")]
                    detach_tree,
                });
            }
        }
        for row in snapshot.iter().filter(|row| row.parent.is_none()) {
            if !state.pages.iter().any(|entry| entry.id == row.page) {
                let root = state
                    .nodes
                    .iter()
                    .find(|entry| entry.node.id() == row.frame)
                    .ok_or_else(|| {
                        HostError::Backend("Projected main Frame was not indexed".into())
                    })?
                    .node
                    .clone();
                let close = self.urls.publications().reserve_close()?;
                let tree = self.factory.page(root)?;
                let ready = self.reserve_ready_delivery(&state)?;
                state
                    .pages
                    .try_reserve(1)
                    .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
                state.pages.push(PageEntry {
                    id: row.page,
                    internal: false,
                    tree,
                    close: Some(close),
                    ready,
                });
            }
        }
        for page in &state.pages {
            desired.clear();
            for row in snapshot.iter().filter(|row| row.page == page.id) {
                let node = state
                    .nodes
                    .iter()
                    .find(|entry| entry.node.id() == row.frame)
                    .ok_or_else(|| HostError::Backend("Projected Frame was not indexed".into()))?;
                desired.push(node.node.clone());
            }
            if !page.tree.matches_native(&desired)? {
                #[cfg(feature = "frame-detach-events")]
                if let Some(reserved) = removed
                    .iter_mut()
                    .find(|entry| entry.page == page.id)
                    .and_then(|entry| entry.tree.take())
                {
                    self.urls
                        .publications()
                        .stage_tree_detach(&page.tree, &desired, reserved)?;
                } else {
                    self.urls.publications().stage_tree(&page.tree, &desired)?;
                }
                #[cfg(not(feature = "frame-detach-events"))]
                self.urls.publications().stage_tree(&page.tree, &desired)?;
            }
        }
        #[cfg(feature = "frame-events")]
        for (page, frame) in newly_attached {
            let Some(collection) = state.collections.as_mut() else {
                break;
            };
            let Some(before) = collection.planned.clone() else {
                continue;
            };
            if !before.pages().iter().any(|entry| entry.id == page) {
                continue;
            }
            let reserved = self.urls.publications().reserve_lifecycle(&self.factory)?;
            let after = self.urls.publications().stage_lifecycle(
                reserved,
                before,
                crate::LifecycleCause::PageFrameAttached { page, frame },
            )?;
            collection.planned = Some(after);
        }
        #[cfg(feature = "frame-detach-events")]
        self.stage_frame_detaches(&mut state, &mut removed, effects)?;
        Ok(())
    }

    #[cfg(feature = "frame-detach-events")]
    fn stage_frame_detaches(
        &self,
        state: &mut State,
        removed: &mut Vec<DetachedEntry>,
        effects: &[CdpTopologyEffect],
    ) -> Result<(), HostError> {
        for effect in effects {
            let CdpTopologyEffect::FrameRemoved { page, frame } = effect else {
                continue;
            };
            let Some(index) = removed
                .iter()
                .position(|entry| entry.page == *page && entry.node.id() == *frame)
            else {
                if state
                    .pages
                    .iter()
                    .any(|entry| entry.id == *page && !entry.internal)
                {
                    return Err(HostError::Backend(
                        "detached Frame has no pre-admitted canonical identity".into(),
                    ));
                }
                continue;
            };
            let retired = removed.swap_remove(index);
            let Some(collection) = state.collections.as_mut() else {
                retired.node.release_retired_binding();
                continue;
            };
            let Some(before) = collection.planned.clone() else {
                retired.node.release_retired_binding();
                continue;
            };
            if !before.pages().iter().any(|entry| entry.id == *page) {
                retired.node.release_retired_binding();
                continue;
            }
            let after = self.urls.publications().stage_lifecycle(
                retired.reservation,
                before,
                crate::LifecycleCause::PageFrameDetached {
                    page: *page,
                    frame: retired.node.clone(),
                },
            );
            if after.is_err() {
                retired.node.release_retired_binding();
            }
            let after = after?;
            collection.planned = Some(after);
        }
        Ok(())
    }
}

#[cfg(test)]
mod live;
#[cfg(test)]
mod terminal_live;
#[cfg(test)]
mod tests;

impl ChromiumFrameUrlObserver for FrameTrees {
    fn reconcile_committed(
        &self,
        publication: playwright_rust_plugin_chromium_frame_observer::ChromiumCommittedFrameOwnership<'_>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let result = (|| {
            self.external_disposals(publication.effects())?;
            self.reconcile_collections(publication.ownership(), publication.effects())?;
            self.publish_ready_causes(publication.effects())
        })();
        #[cfg(any(feature = "frame-detach-events", feature = "frame-events"))]
        if result.is_err() {
            self.state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .closed = true;
        }
        result.map_err(Into::into)
    }

    fn update_navigation(
        &self,
        frame: ObjectId,
        url: &str,
        fragment: Option<&str>,
        name: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.urls.update_navigation(frame, url, fragment, name)
    }

    fn update(
        &self,
        frame: ObjectId,
        url: &str,
        fragment: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.urls.update(frame, url, fragment)
    }
    fn retain_live(
        &self,
        is_live: &dyn Fn(ObjectId) -> bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.urls.retain_live(is_live)
    }
    fn reconcile_ownership(
        &self,
        view: ChromiumFrameOwnershipView<'_>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.reconcile_collections(view, &[])?;
        Ok(())
    }
}
