//! Exact browser resources created through one attached HostSession.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::HostError;

#[derive(Default)]
struct State {
    contexts: BTreeSet<String>,
    pages: BTreeMap<String, String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ShutdownSnapshot {
    pub(crate) contexts: Vec<String>,
    pub(crate) pages_outside_owned_contexts: Vec<String>,
}

#[derive(Default)]
pub(crate) struct SessionOwnedResources(Mutex<State>);

impl SessionOwnedResources {
    pub(crate) fn record_context(&self, context: &str) -> Result<(), HostError> {
        self.0
            .lock()
            .map_err(HostError::backend)?
            .contexts
            .insert(context.to_owned());
        Ok(())
    }

    pub(crate) fn record_page(&self, page: &str, context: &str) -> Result<(), HostError> {
        self.0
            .lock()
            .map_err(HostError::backend)?
            .pages
            .insert(page.to_owned(), context.to_owned());
        Ok(())
    }

    pub(crate) fn remove_page(&self, page: &str) -> Result<(), HostError> {
        self.0
            .lock()
            .map_err(HostError::backend)?
            .pages
            .remove(page);
        Ok(())
    }

    pub(crate) fn remove_context(&self, context: &str) -> Result<(), HostError> {
        let mut state = self.0.lock().map_err(HostError::backend)?;
        state.contexts.remove(context);
        state.pages.retain(|_, owner| owner != context);
        Ok(())
    }

    pub(crate) fn shutdown_snapshot(&self) -> Result<ShutdownSnapshot, HostError> {
        let state = self.0.lock().map_err(HostError::backend)?;
        Ok(ShutdownSnapshot {
            contexts: state.contexts.iter().cloned().collect(),
            pages_outside_owned_contexts: state
                .pages
                .iter()
                .filter(|(_, context)| !state.contexts.contains(*context))
                .map(|(page, _)| page.clone())
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_snapshot_closes_external_context_pages_individually() {
        // Arrange
        let owned = SessionOwnedResources::default();
        owned.record_context("owned-context").unwrap();
        owned.record_page("owned-page", "owned-context").unwrap();
        owned
            .record_page("default-page", "default-context")
            .unwrap();

        // Act
        let snapshot = owned.shutdown_snapshot().unwrap();

        // Assert
        assert_eq!(snapshot.contexts, ["owned-context"]);
        assert_eq!(snapshot.pages_outside_owned_contexts, ["default-page"]);
    }

    #[test]
    fn closing_a_context_retires_all_of_its_page_ownership() {
        // Arrange
        let owned = SessionOwnedResources::default();
        owned.record_context("context").unwrap();
        owned.record_page("first", "context").unwrap();
        owned.record_page("second", "context").unwrap();

        // Act
        owned.remove_context("context").unwrap();
        let snapshot = owned.shutdown_snapshot().unwrap();

        // Assert
        assert!(snapshot.contexts.is_empty());
        assert!(snapshot.pages_outside_owned_contexts.is_empty());
    }
}
