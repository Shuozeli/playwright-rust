//! Immutable membership visibility over an already admitted revision. Changing
//! a view allocates nothing and does not mutate another reader's membership.
//! This is not an event dispatcher or a Page-closed-flag transition.
use super::{CollectionContext, CollectionPage, CollectionRevision};
use playwright_rust_core::ObjectId;
use std::{error::Error, fmt, iter::FusedIterator, ops::Index, sync::Arc};

mod rebase;
pub use rebase::CollectionRebaseError;

#[derive(Clone)]
pub struct CollectionView {
    revision: CollectionRevision,
    contexts: Arc<[bool]>,
    pages: Arc<[bool]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionMember {
    Context(ObjectId),
    Page(ObjectId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionVisibilityError {
    Unknown(CollectionMember),
    AlreadyVisible(CollectionMember),
    NotVisible(CollectionMember),
    ContextHasVisiblePages(ObjectId),
    ParentNotVisible(ObjectId),
    Allocation,
}

impl fmt::Display for CollectionVisibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid collection visibility transition: {self:?}"
        )
    }
}

impl Error for CollectionVisibilityError {}

fn mask(len: usize, visible: bool) -> Arc<[bool]> {
    vec![visible; len].into()
}

impl CollectionView {
    pub(crate) fn revision(&self) -> &CollectionRevision {
        &self.revision
    }

    pub(crate) fn same_state(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.revision.0, &other.revision.0)
            && self.contexts == other.contexts
            && self.pages == other.pages
    }

    pub(in crate::frame_metadata) fn belongs_to(
        &self,
        domain: &playwright_rust_core::ResourceLedger,
    ) -> bool {
        self.revision.0.domain.shares_budget_with(domain)
    }

    pub fn full(revision: CollectionRevision) -> Self {
        Self {
            contexts: mask(revision.contexts().len(), true),
            pages: mask(revision.pages().len(), true),
            revision,
        }
    }

    /// All descriptors remain retained/charged, but none is publicly visible.
    pub fn hidden(revision: CollectionRevision) -> Self {
        let context_count = revision.contexts().len();
        let page_count = revision.pages().len();
        Self {
            revision,
            contexts: mask(context_count, false),
            pages: mask(page_count, false),
        }
    }

    pub fn contexts(&self) -> CollectionMembers<'_, CollectionContext> {
        CollectionMembers {
            entries: self.revision.contexts(),
            visible: &self.contexts,
        }
    }

    pub fn pages(&self) -> CollectionMembers<'_, CollectionPage> {
        CollectionMembers {
            entries: self.revision.pages(),
            visible: &self.pages,
        }
    }

    /// Returns a new view or a typed error. The receiver and all retained views
    /// remain unchanged on success and failure. Native ownership is untouched.
    pub fn with_visibility(
        &self,
        member: CollectionMember,
        visible: bool,
    ) -> Result<Self, CollectionVisibilityError> {
        let (position, visible_now) = match member {
            CollectionMember::Context(id) => (
                self.revision
                    .contexts()
                    .iter()
                    .position(|entry| entry.id == id),
                self.contexts.as_ref(),
            ),
            CollectionMember::Page(id) => (
                self.revision
                    .pages()
                    .iter()
                    .position(|entry| entry.id == id),
                self.pages.as_ref(),
            ),
        };
        let position = position.ok_or(CollectionVisibilityError::Unknown(member))?;
        if visible_now[position] == visible {
            return Err(if visible {
                CollectionVisibilityError::AlreadyVisible(member)
            } else {
                CollectionVisibilityError::NotVisible(member)
            });
        }
        match member {
            CollectionMember::Context(id) if !visible => {
                if self.pages().iter().any(|page| page.context == id) {
                    return Err(CollectionVisibilityError::ContextHasVisiblePages(id));
                }
            }
            CollectionMember::Page(_) if visible => {
                let context = self.revision.pages()[position].context;
                if !self.contexts().iter().any(|entry| entry.id == context) {
                    return Err(CollectionVisibilityError::ParentNotVisible(context));
                }
            }
            CollectionMember::Context(_) | CollectionMember::Page(_) => {}
        }
        let mut flags = Vec::new();
        flags
            .try_reserve_exact(visible_now.len())
            .map_err(|_| CollectionVisibilityError::Allocation)?;
        flags.extend_from_slice(visible_now);
        flags[position] = visible;
        let mut next = self.clone();
        match member {
            CollectionMember::Context(_) => next.contexts = flags.into(),
            CollectionMember::Page(_) => next.pages = flags.into(),
        }
        Ok(next)
    }
}

/// Ordered borrowed members. The mask is private and always comes from a
/// validated revision/view, so its set bits cannot exceed the admitted slice.
pub struct CollectionMembers<'a, T> {
    entries: &'a [T],
    visible: &'a [bool],
}

impl<'a, T> CollectionMembers<'a, T> {
    pub fn len(&self) -> usize {
        self.visible.iter().filter(|visible| **visible).count()
    }
    pub fn is_empty(&self) -> bool {
        !self.visible.iter().any(|visible| *visible)
    }
    pub fn iter(&self) -> VisibleMembers<'a, T> {
        VisibleMembers {
            entries: self.entries,
            visible: self.visible,
            front: 0,
            back: self.entries.len(),
            remaining: self.len(),
        }
    }
    pub fn get(&self, index: usize) -> Option<&'a T> {
        self.iter().nth(index)
    }
}

impl<T> Index<usize> for CollectionMembers<'_, T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index)
            .expect("collection member index out of bounds")
    }
}

impl<'a, T> IntoIterator for CollectionMembers<'a, T> {
    type Item = &'a T;
    type IntoIter = VisibleMembers<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct VisibleMembers<'a, T> {
    entries: &'a [T],
    visible: &'a [bool],
    front: usize,
    back: usize,
    remaining: usize,
}

impl<'a, T> Iterator for VisibleMembers<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let index = self.front;
            self.front += 1;
            if self.visible[index] {
                self.remaining -= 1;
                return Some(&self.entries[index]);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> DoubleEndedIterator for VisibleMembers<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            self.back -= 1;
            if self.visible[self.back] {
                self.remaining -= 1;
                return Some(&self.entries[self.back]);
            }
        }
        None
    }
}

impl<T> ExactSizeIterator for VisibleMembers<'_, T> {}
impl<T> FusedIterator for VisibleMembers<'_, T> {}

#[cfg(test)]
mod tests;
