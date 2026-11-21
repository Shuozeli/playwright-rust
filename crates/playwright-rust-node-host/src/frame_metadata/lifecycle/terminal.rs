//! Terminal cause production, admitted before the native lifecycle starts.
//! This consumes a coherent planned view, not a diff of foreign collections.
use super::*;
use crate::frame_metadata::FrameClosePermit;
use crate::frame_tree::FrameTree;

/// Terminal Page/Context groups are admitted while the collection is still
/// live. Teardown only consumes this pool and therefore never allocates.
pub(crate) struct TerminalLifecycle {
    reserved: Vec<LifecycleReservation>,
    _storage: Storage,
}

impl TerminalLifecycle {
    pub(crate) fn reserve(
        _book: &FramePublications,
        factory: &FrameTreeFactory,
    ) -> Result<Box<Self>, HostError> {
        let storage = factory.reserve_collection_storage(std::mem::size_of::<Self>())?;
        Ok(Box::new(Self {
            reserved: Vec::new(),
            _storage: storage,
        }))
    }

    pub(crate) fn len(&self) -> usize {
        self.reserved.len()
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        self.reserved.truncate(len);
    }

    pub(crate) fn replenish(
        &mut self,
        target: usize,
        book: &FramePublications,
        factory: &FrameTreeFactory,
    ) -> Result<(), HostError> {
        let before = self.len();
        self.reserved
            .try_reserve(target.saturating_sub(before))
            .map_err(|error| HostError::ResourceExhausted(error.to_string()))?;
        while self.len() < target {
            match book.reserve_lifecycle(factory) {
                Ok(reserved) => self.reserved.push(reserved),
                Err(error) => {
                    self.truncate(before);
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn append(&mut self, state: &mut MetadataState, prepared: PreparedLifecycle) {
        // Collection reconciliation admits one group per visible member before
        // native teardown can begin.
        let mut reserved = self
            .reserved
            .pop()
            .expect("one admitted terminal group per member");
        reserved.group.prepared = Some(prepared);
        state.pending.push_back(FrameChange {
            sequence: u64::MAX,
            value: FrameChangeValue::Lifecycle(reserved.group),
            _event: reserved.event,
        });
    }

    /// The native owner calls this once after stopping the driver. This path
    /// reserves/allocates nothing and recovers poisoned native metadata locks,
    /// matching the existing infallible teardown path. No foreign code executes.
    pub(crate) fn stage<'a>(
        mut self,
        book: &FramePublications,
        view: CollectionView,
        pages: impl Iterator<Item = (&'a FrameTree, &'a mut Option<FrameClosePermit>)>,
    ) {
        let mut state = book
            .0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(start) = state.collection_batch.take() {
            // Failed reconciliation may have staged some raw close flags, but
            // its batch gate has never made them publishable. The terminal cut
            // owns those still-visible Pages now: transfer their publication to
            // the independent terminal groups rather than exposing every flag
            // before the first callback. Preserve all previously committed work.
            state.pending.retain(|change| {
                !(change.sequence > start
                    && matches!(&change.value, FrameChangeValue::Close(close)
                        if view.pages().iter().any(|page| close.is_for(&page.tree))))
            });
        }
        state.sequence = u64::MAX;
        // Also close native metadata for Pages not yet in the coherent public
        // collection. They receive no invented Page/Context notification.
        for (tree, permit) in pages {
            let close = tree.close_for_teardown();
            let visible = view.pages().iter().any(|page| close.is_for(&page.tree));
            if let Some(reserved) = permit.take()
                && !state.discarded
                && !visible
            {
                state.pending.push_back(FrameChange {
                    sequence: u64::MAX,
                    value: FrameChangeValue::Close(close),
                    _event: reserved.event,
                });
            }
        }
        if !state.discarded {
            let mut before = view.clone();
            for context in view.contexts() {
                for page in view
                    .pages()
                    .iter()
                    .filter(|page| page.context == context.id)
                {
                    let after = before
                        .with_visibility(CollectionMember::Page(page.id), false)
                        .expect("validated visible Page is removed once");
                    self.append(
                        &mut state,
                        PreparedLifecycle {
                            before,
                            after: after.clone(),
                            next: LifecycleNotice::PageClose {
                                context: context.id,
                                page: page.id,
                            },
                            dependent: None,
                            close: Some(page.tree.close_for_teardown()),
                            awaiting: false,
                            creation: None,
                            #[cfg(feature = "frame-detach-events")]
                            detached_frame: None,
                        },
                    );
                    before = after;
                }
                let after = before
                    .with_visibility(CollectionMember::Context(context.id), false)
                    .expect("validated Context is removed after its visible Pages");
                self.append(
                    &mut state,
                    PreparedLifecycle {
                        before,
                        after: after.clone(),
                        next: LifecycleNotice::ContextClose {
                            context: context.id,
                        },
                        dependent: None,
                        close: None,
                        awaiting: false,
                        creation: None,
                        #[cfg(feature = "frame-detach-events")]
                        detached_frame: None,
                    },
                );
                before = after;
            }
        }
        drop(state);
        book.0.changed.notify_one();
    }
}

#[cfg(test)]
mod tests;
