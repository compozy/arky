//! Typed event subscription helpers.

use tokio::sync::broadcast;

use arky_protocol::AgentEvent;

/// Broadcast subscription wrapper returned by [`crate::Agent::subscribe`].
#[derive(Debug)]
pub struct EventSubscription {
    receiver: broadcast::Receiver<AgentEvent>,
}

impl EventSubscription {
    pub(crate) const fn new(receiver: broadcast::Receiver<AgentEvent>) -> Self {
        Self { receiver }
    }

    /// Awaits the next broadcast event.
    pub async fn recv(&mut self) -> Result<AgentEvent, broadcast::error::RecvError> {
        self.receiver.recv().await
    }

    /// Attempts to receive the next broadcast event without waiting.
    pub fn try_recv(&mut self) -> Result<AgentEvent, broadcast::error::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Creates a fresh subscription from the same broadcast position.
    #[must_use]
    pub fn resubscribe(&self) -> Self {
        Self::new(self.receiver.resubscribe())
    }
}
