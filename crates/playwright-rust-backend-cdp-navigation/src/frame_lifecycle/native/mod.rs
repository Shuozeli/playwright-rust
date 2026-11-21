//! Bounded CDP identity normalization for the optional lifecycle reducer.
//!
//! The embedding Rust owner publishes Frame attachments/removals, supplies an
//! exact connection epoch and authoritative session routing, and preserves CDP
//! order. Install before enabling native lifecycle/network events; this adapter
//! cannot reconstruct events that occurred before observation or after loss.
use super::{
    DocumentGeneration, DocumentLifecycle, FrameLifecycleProfile, FrameLifecycleStore,
    LifecycleError, LifecycleInput, LifecycleSnapshot, slots,
};
use playwright_rust_cdp::CdpEvent;
use playwright_rust_core::{
    ObjectId, ProductionResourceLedger, ResourceClass, ResourceLease, ResourceRequirements,
    ResourceScope,
};
use std::{fmt, mem::size_of, time::Duration};

mod events;
#[cfg(test)]
mod tests;

/// Borrowed, authoritative routing supplied by the existing Rust topology owner.
/// Worker sessions may resolve frame-less Network events via `request_frame`.
/// No fallback to the main Frame or session migration based on request ID alone.
pub trait CdpLifecycleRoutes {
    fn contains_session(&self, session: &str) -> bool;
    fn event_frame(&self, session: &str, native_frame: &str) -> Option<ObjectId>;
    fn request_frame(&self, session: &str, native_frame: Option<&str>) -> Option<ObjectId>;
    fn accepts_document_handoff(&self, frame: ObjectId, session: &str) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLifecycleError {
    Lifecycle(LifecycleError),
    InvalidEvent(&'static str),
    UnknownFrame,
    AmbiguousRequest,
}
impl fmt::Display for NativeLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native Frame lifecycle: {self:?}")
    }
}
impl std::error::Error for NativeLifecycleError {}
impl From<LifecycleError> for NativeLifecycleError {
    fn from(error: LifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}
type Result<T> = std::result::Result<T, NativeLifecycleError>;

/// Notices borrow payloads from the caller's admitted envelope. The navigation
/// operation separately admits any escaped response, URL or failure-text copy.
#[derive(Debug)]
pub enum NativeLifecycleNotice<'a> {
    Ignored,
    Committed {
        frame: ObjectId,
        document: DocumentGeneration,
        loader: &'a str,
        url: Option<&'a str>,
    },
    SameDocument {
        frame: ObjectId,
        url: Option<&'a str>,
    },
    Lifecycle {
        frame: ObjectId,
        document: DocumentGeneration,
        event: DocumentLifecycle,
    },
    RequestStarted {
        frame: ObjectId,
        request: ObjectId,
        document_loader: Option<&'a str>,
        redirected_from: Option<ObjectId>,
    },
    Response {
        frame: ObjectId,
        request: ObjectId,
        response: &'a serde_json::Value,
    },
    RequestFinished {
        frame: ObjectId,
        request: ObjectId,
    },
    RequestFailed {
        frame: ObjectId,
        request: ObjectId,
        error_text: &'a str,
        canceled: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct CdpFrameLifecycleProfile {
    lifecycle: FrameLifecycleProfile,
    scope: ResourceScope,
    id_bytes: usize,
    arena_bytes: usize,
    bytes: u64,
    allocator_backed: bool,
}
impl CdpFrameLifecycleProfile {
    pub fn new(
        lifecycle: FrameLifecycleProfile,
        scope: ResourceScope,
        id_bytes: usize,
    ) -> std::result::Result<Self, LifecycleError> {
        if id_bytes == 0 || scope == lifecycle.scope {
            return Err(LifecycleError::InvalidProfile);
        }
        let arena_bytes = lifecycle
            .requests
            .checked_mul(2)
            .and_then(|n| n.checked_add(lifecycle.frames))
            .and_then(|n| n.checked_mul(id_bytes))
            .ok_or(LifecycleError::SizeOverflow)?;
        // Conservatively also includes the inline reducer body, which its own
        // profile already counts. No dynamic native-ID allocation after new().
        let bytes = lifecycle
            .frames
            .checked_mul(size_of::<Option<NativeFrame>>())
            .and_then(|n| {
                n.checked_add(
                    lifecycle
                        .requests
                        .checked_mul(size_of::<Option<NativeRequest>>())?,
                )
            })
            .and_then(|n| n.checked_add(arena_bytes))
            .and_then(|n| n.checked_add(size_of::<CdpFrameLifecycle>()))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(LifecycleError::SizeOverflow)?;
        Ok(Self {
            lifecycle,
            scope,
            id_bytes,
            arena_bytes,
            bytes,
            allocator_backed: false,
        })
    }
    pub fn new_allocator_backed(
        lifecycle: FrameLifecycleProfile,
        scope: ResourceScope,
        id_bytes: usize,
    ) -> std::result::Result<Self, LifecycleError> {
        if !lifecycle.allocator_backed || id_bytes == 0 || scope == lifecycle.scope {
            return Err(LifecycleError::InvalidProfile);
        }
        Ok(Self {
            lifecycle,
            scope,
            id_bytes,
            arena_bytes: 0,
            bytes: 0,
            allocator_backed: true,
        })
    }
    fn native_requirements(self) -> ResourceRequirements {
        ResourceRequirements::none()
            .with_class(ResourceClass::BackendStateBytes, self.bytes, self.bytes)
            .with_metadata(1, 1)
    }
    pub fn requirements(self) -> ResourceRequirements {
        self.lifecycle
            .requirements()
            .combine_concurrent(self.native_requirements())
    }
}

struct NativeFrame {
    id: ObjectId,
    loader_len: usize,
    loader: Option<Box<str>>,
    document: Option<DocumentGeneration>,
}
struct NativeRequest {
    id: ObjectId,
    frame: ObjectId,
    request_len: usize,
    session_len: usize,
    native_id: Option<Box<str>>,
    session: Option<Box<str>>,
    document: bool,
    favicon: bool,
}

/// One Page's fixed-capacity native identity owner. Request IDs are normalized
/// to unique ObjectIds, including each redirect leg. All transition storage is
/// allocated up front; external topology, envelopes and request bodies are not
/// retained. This is not a response waiter or a public Frame.goto implementation.
pub struct CdpFrameLifecycle {
    lifecycle: FrameLifecycleStore,
    frames: Vec<Option<NativeFrame>>,
    requests: Vec<Option<NativeRequest>>,
    arena: Vec<u8>,
    id_bytes: usize,
    epoch: u64,
    next_request: u64,
    next_document: u64,
    allocator_backed: bool,
    _lease: Option<ResourceLease>,
}
impl CdpFrameLifecycle {
    pub fn new(
        profile: CdpFrameLifecycleProfile,
        epoch: u64,
        resources: &ProductionResourceLedger,
    ) -> Result<Self> {
        if epoch == 0 {
            return Err(LifecycleError::InvalidIdentity.into());
        }
        let lease = if profile.allocator_backed {
            None
        } else {
            let ledger = resources
                .claim(profile.native_requirements())
                .map_err(|_| LifecycleError::Resources)?;
            Some(
                ledger
                    .reserve(
                        profile.scope,
                        ResourceClass::BackendStateBytes,
                        profile.bytes,
                    )
                    .map_err(|_| LifecycleError::Resources)?,
            )
        };
        let lifecycle = FrameLifecycleStore::new(profile.lifecycle, resources)?;
        let mut arena = Vec::new();
        if !profile.allocator_backed {
            arena
                .try_reserve_exact(profile.arena_bytes)
                .map_err(|_| LifecycleError::Allocation)?;
            if arena.capacity() != profile.arena_bytes {
                return Err(LifecycleError::Allocation.into());
            }
            arena.resize(profile.arena_bytes, 0);
        }
        Ok(Self {
            lifecycle,
            frames: if profile.allocator_backed {
                Vec::new()
            } else {
                slots(profile.lifecycle.frames)?
            },
            requests: if profile.allocator_backed {
                Vec::new()
            } else {
                slots(profile.lifecycle.requests)?
            },
            arena,
            id_bytes: profile.id_bytes,
            epoch,
            next_request: 0,
            next_document: 0,
            allocator_backed: profile.allocator_backed,
            _lease: lease,
        })
    }
    pub fn attach(
        &mut self,
        now: Duration,
        frame: ObjectId,
        parent: Option<ObjectId>,
    ) -> Result<()> {
        self.lifecycle
            .apply(now, LifecycleInput::Attach { frame, parent })?;
        if self.frame_index(frame).is_none() {
            let index = match self.frames.iter().position(Option::is_none) {
                Some(index) => index,
                None if self.allocator_backed => {
                    self.frames
                        .try_reserve(1)
                        .map_err(|_| LifecycleError::Allocation)?;
                    self.frames.push(None);
                    self.frames.len() - 1
                }
                None => {
                    self.invalidate();
                    return Err(LifecycleError::FrameCapacity.into());
                }
            };
            self.frames[index] = Some(NativeFrame {
                id: frame,
                loader_len: 0,
                loader: None,
                document: None,
            });
        }
        Ok(())
    }
    /// Actual removal, not an OOPIF swap. Renderer swaps preserve this Frame's
    /// identity/document; the route port must expose the new session explicitly.
    pub fn detach(&mut self, now: Duration, frame: ObjectId) -> Result<()> {
        self.lifecycle
            .apply(now, LifecycleInput::Detach { frame })?;
        self.prune(None)
    }
    /// Reconcile a document from an authoritative current-renderer Frame tree.
    /// Parent-first order is required. The same loader is idempotent; a changed
    /// loader is a commit, including a replacement renderer already navigated
    /// before attachment. The caller must reject snapshots from stale sessions.
    /// This initializes observed network accounting, not pre-attachment traffic.
    pub fn initialize_frame<'a>(
        &mut self,
        now: Duration,
        frame: ObjectId,
        parent: Option<ObjectId>,
        loader: &'a str,
    ) -> Result<NativeLifecycleNotice<'a>> {
        let result = (|| {
            self.attach(now, frame, parent)?;
            self.commit_native(now, frame, loader, None)
        })();
        if result.is_err() {
            self.invalidate();
        }
        result
    }
    /// Start a new same-loader document generation for an embedding-owned
    /// `document.open()` operation. CDP does not emit `frameNavigated` for this
    /// transition, so the caller must establish this checkpoint before running
    /// the script that replaces the document. Subsequent lifecycle events keep
    /// using the current loader and are attributed to this new generation.
    pub fn reset_document(&mut self, now: Duration, frame: ObjectId) -> Result<()> {
        let result = (|| {
            let index = self
                .frame_index(frame)
                .ok_or(NativeLifecycleError::UnknownFrame)?;
            self.next_document = self
                .next_document
                .checked_add(1)
                .ok_or(LifecycleError::SizeOverflow)?;
            let document = DocumentGeneration(self.next_document);
            self.lifecycle.apply(
                now,
                LifecycleInput::Commit {
                    frame,
                    document,
                    request: None,
                },
            )?;
            let row = self.frames[index]
                .as_mut()
                .ok_or(NativeLifecycleError::UnknownFrame)?;
            row.document = Some(document);
            self.prune(Some((frame, None)))
        })();
        if result.is_err() {
            self.invalidate();
        }
        result
    }
    /// Reconcile authoritative removals without retaining a second Frame map.
    pub fn retain_frames(&mut self, now: Duration, keep: impl Fn(ObjectId) -> bool) -> Result<()> {
        self.advance(now)?;
        for index in 0..self.frames.len() {
            if let Some(frame) = self.frames[index].as_ref()
                && !keep(frame.id)
            {
                self.detach(now, frame.id)?;
            }
        }
        Ok(())
    }
    pub fn advance(&mut self, now: Duration) -> Result<()> {
        self.lifecycle.advance(now).map_err(Into::into)
    }
    pub fn snapshot(&self, frame: ObjectId) -> Result<Option<LifecycleSnapshot>> {
        self.lifecycle.snapshot(frame).map_err(Into::into)
    }
    pub fn next_deadline(&self) -> Result<Option<Duration>> {
        self.lifecycle.next_deadline().map_err(Into::into)
    }
    pub fn invalidate(&mut self) {
        self.lifecycle.invalidate();
    }
    pub fn apply<'event>(
        &mut self,
        now: Duration,
        epoch: u64,
        event: &'event CdpEvent,
        routes: &impl CdpLifecycleRoutes,
    ) -> Result<NativeLifecycleNotice<'event>> {
        self.lifecycle.advance(now)?;
        let Some(session) = event
            .session_id
            .as_deref()
            .filter(|s| epoch == self.epoch && routes.contains_session(s))
        else {
            return Ok(NativeLifecycleNotice::Ignored);
        };
        let result = self.event(now, session, event, routes);
        if result.is_err() {
            self.invalidate();
        }
        result
    }
    fn frame_index(&self, frame: ObjectId) -> Option<usize> {
        self.frames
            .iter()
            .position(|row| row.as_ref().is_some_and(|r| r.id == frame))
    }
    fn validate_id(&self, value: &str) -> Result<()> {
        if value.is_empty() || value.len() > self.id_bytes {
            return Err(LifecycleError::InvalidIdentity.into());
        }
        Ok(())
    }
    fn stored_id(&self, cell: usize, len: usize) -> &str {
        let start = cell * self.id_bytes;
        std::str::from_utf8(&self.arena[start..start + len])
            .expect("only validated UTF-8 is inserted")
    }
    fn write_id(&mut self, cell: usize, value: &str) {
        let start = cell * self.id_bytes;
        self.arena[start..start + value.len()].copy_from_slice(value.as_bytes());
    }
    fn request_cell(&self, index: usize) -> usize {
        self.frames.len() + 2 * index
    }
    fn owned_id(value: &str) -> Result<Box<str>> {
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| LifecycleError::Allocation)?;
        owned.push_str(value);
        Ok(owned.into_boxed_str())
    }
    fn frame_loader<'a>(&'a self, index: usize, row: &'a NativeFrame) -> &'a str {
        row.loader
            .as_deref()
            .unwrap_or_else(|| self.stored_id(index, row.loader_len))
    }
    fn request_native_id<'a>(&'a self, index: usize, row: &'a NativeRequest) -> &'a str {
        row.native_id
            .as_deref()
            .unwrap_or_else(|| self.stored_id(self.request_cell(index), row.request_len))
    }
    fn request_session<'a>(&'a self, index: usize, row: &'a NativeRequest) -> &'a str {
        row.session
            .as_deref()
            .unwrap_or_else(|| self.stored_id(self.request_cell(index) + 1, row.session_len))
    }
    fn exact_request(&self, session: &str, native_id: &str) -> Option<usize> {
        self.requests.iter().enumerate().find_map(|(i, row)| {
            row.as_ref()
                .filter(|row| {
                    self.request_native_id(i, row) == native_id
                        && self.request_session(i, row) == session
                })
                .map(|_| i)
        })
    }
    fn request_index(
        &self,
        session: &str,
        native_id: &str,
        routes: &impl CdpLifecycleRoutes,
    ) -> Result<Option<usize>> {
        if let Some(index) = self.exact_request(session, native_id) {
            return Ok(Some(index));
        }
        let mut found = None;
        for (i, row) in self
            .requests
            .iter()
            .enumerate()
            .filter_map(|(i, row)| row.as_ref().map(|r| (i, r)))
        {
            if row.document
                && self.request_native_id(i, row) == native_id
                && routes.accepts_document_handoff(row.frame, session)
            {
                if found.is_some() {
                    return Err(NativeLifecycleError::AmbiguousRequest);
                }
                found = Some(i);
            }
        }
        Ok(found)
    }
    fn prune(&mut self, commit: Option<(ObjectId, Option<ObjectId>)>) -> Result<()> {
        for slot in &mut self.frames {
            if let Some(row) = slot
                && self.lifecycle.snapshot(row.id)?.is_none()
            {
                *slot = None;
            }
        }
        for i in 0..self.requests.len() {
            if self.requests[i].as_ref().is_some_and(|row| {
                self.frame_index(row.frame).is_none()
                    || commit
                        .is_some_and(|(frame, keep)| frame == row.frame && Some(row.id) != keep)
            }) {
                self.requests[i] = None;
            }
        }
        Ok(())
    }
}
