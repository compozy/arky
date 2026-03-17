//! Server-facing runtime port.
//!
//! Adapters such as the HTTP server should depend on this contract rather than
//! a concrete runtime implementation.

use arky_runtime::{
    CoreError,
    EventSubscription,
};
use arky_types::SessionId;
use async_trait::async_trait;

/// Server-facing runtime contract used by adapters.
#[async_trait]
pub trait RuntimeHandle: Send + Sync {
    /// Starts a streaming execution for the supplied input.
    async fn stream(
        &self,
        input: String,
    ) -> Result<arky_runtime::AgentEventStream, CoreError>;

    /// Creates a new active session.
    async fn new_session(&self) -> Result<SessionId, CoreError>;

    /// Resumes the provided session.
    async fn resume(&self, session_id: SessionId) -> Result<(), CoreError>;

    /// Subscribes to runtime events.
    fn subscribe(&self) -> EventSubscription;
}

#[async_trait]
impl RuntimeHandle for arky_runtime::Agent {
    async fn stream(
        &self,
        input: String,
    ) -> Result<arky_runtime::AgentEventStream, CoreError> {
        Self::stream(self, input).await
    }

    async fn new_session(&self) -> Result<SessionId, CoreError> {
        Self::new_session(self).await
    }

    async fn resume(&self, session_id: SessionId) -> Result<(), CoreError> {
        Self::resume(self, session_id).await
    }

    fn subscribe(&self) -> EventSubscription {
        Self::subscribe(self)
    }
}
