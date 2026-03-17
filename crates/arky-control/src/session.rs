//! Server-facing session storage port.

use std::sync::Arc;

use arky_storage::{
    PersistedEvent,
    SessionError,
    SessionFilter,
    SessionMetadata,
    SessionSnapshot,
};
use arky_types::SessionId;
use async_trait::async_trait;

/// Read-oriented session storage contract for adapters.
#[async_trait]
pub trait SessionStoreHandle: Send + Sync {
    /// Loads the current session snapshot.
    async fn load(&self, id: &SessionId) -> Result<SessionSnapshot, SessionError>;

    /// Lists stored sessions for a filter.
    async fn list(
        &self,
        filter: SessionFilter,
    ) -> Result<Vec<SessionMetadata>, SessionError>;

    /// Replays persisted session events.
    async fn replay_events(
        &self,
        id: &SessionId,
        after_sequence: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<PersistedEvent>, SessionError>;
}

/// Adapter that promotes a dynamic execution session store to the control port.
#[derive(Clone)]
pub struct SessionStoreAdapter {
    store: Arc<dyn arky_storage::SessionStore>,
}

impl SessionStoreAdapter {
    /// Creates an adapter for a dynamic execution session store.
    #[must_use]
    pub fn new(store: Arc<dyn arky_storage::SessionStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl<T> SessionStoreHandle for T
where
    T: arky_storage::SessionStore + Send + Sync + ?Sized,
{
    async fn load(&self, id: &SessionId) -> Result<SessionSnapshot, SessionError> {
        arky_storage::SessionStore::load(self, id).await
    }

    async fn list(
        &self,
        filter: SessionFilter,
    ) -> Result<Vec<SessionMetadata>, SessionError> {
        arky_storage::SessionStore::list(self, filter).await
    }

    async fn replay_events(
        &self,
        id: &SessionId,
        after_sequence: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<PersistedEvent>, SessionError> {
        arky_storage::SessionStore::replay_events(self, id, after_sequence, limit).await
    }
}

#[async_trait]
impl SessionStoreHandle for SessionStoreAdapter {
    async fn load(&self, id: &SessionId) -> Result<SessionSnapshot, SessionError> {
        self.store.load(id).await
    }

    async fn list(
        &self,
        filter: SessionFilter,
    ) -> Result<Vec<SessionMetadata>, SessionError> {
        self.store.list(filter).await
    }

    async fn replay_events(
        &self,
        id: &SessionId,
        after_sequence: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<PersistedEvent>, SessionError> {
        self.store.replay_events(id, after_sequence, limit).await
    }
}
