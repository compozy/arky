//! Provider session identifier passthrough and reuse.

use std::sync::Arc;

use arky_protocol::SessionRef;
use tokio::sync::Mutex;

/// Tracks the latest provider-native Claude session identifier.
#[derive(Debug, Clone, Default)]
pub struct SessionManager {
    current: Arc<Mutex<Option<String>>>,
}

impl SessionManager {
    /// Creates an empty session manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves the provider-native session identifier for the next turn.
    pub async fn resolve(&self, session: &SessionRef) -> Option<String> {
        if let Some(provider_session_id) = &session.provider_session_id {
            return Some(provider_session_id.clone());
        }

        self.current.lock().await.clone()
    }

    /// Stores the most recently observed provider-native session identifier.
    pub async fn record(&self, provider_session_id: impl Into<String>) {
        *self.current.lock().await = Some(provider_session_id.into());
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::SessionManager;
    use arky_protocol::SessionRef;

    #[tokio::test]
    async fn session_manager_should_prefer_request_session_and_then_cached_value() {
        let manager = SessionManager::new();
        manager.record("cached-session").await;

        let cached = manager.resolve(&SessionRef::new(None)).await;
        assert_eq!(cached, Some("cached-session".to_owned()));

        let explicit = manager
            .resolve(&SessionRef::new(None).with_provider_session_id("request-session"))
            .await;
        assert_eq!(explicit, Some("request-session".to_owned()));
    }
}
