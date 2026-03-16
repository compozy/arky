//! In-process runtime client for driving an [`arky_core::Agent`].

use std::sync::{
    Arc,
    atomic::{
        AtomicBool,
        Ordering,
    },
};

use arky_core::{
    Agent,
    AgentEventStream,
    CoreError,
};
use arky_protocol::SessionId;

/// Thin client wrapper used by higher-level runtimes that need a disposable agent handle.
#[derive(Clone)]
pub struct RuntimeClient {
    agent: Arc<Agent>,
    disposed: Arc<AtomicBool>,
}

impl RuntimeClient {
    /// Creates a new client from a shared agent instance.
    #[must_use]
    pub fn new(agent: Arc<Agent>) -> Self {
        Self {
            agent,
            disposed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts a streaming text turn against the underlying agent.
    pub async fn stream_text(
        &self,
        input: impl Into<String>,
    ) -> Result<AgentEventStream, CoreError> {
        self.ensure_not_disposed()?;
        self.agent.stream(input).await
    }

    /// Creates and activates a fresh session.
    pub async fn create_session(&self) -> Result<SessionId, CoreError> {
        self.ensure_not_disposed()?;
        self.agent.new_session().await
    }

    /// Resumes an existing session.
    pub async fn resume_session(&self, session_id: SessionId) -> Result<(), CoreError> {
        self.ensure_not_disposed()?;
        self.agent.resume(session_id).await
    }

    /// Returns the current session identifier when available.
    pub async fn current_session_id(&self) -> Option<SessionId> {
        if self.disposed.load(Ordering::SeqCst) {
            return None;
        }
        self.agent.current_session_id().await
    }

    /// Clears the agent session and permanently disposes the client.
    pub async fn dispose(&self) -> Result<(), CoreError> {
        if self.disposed.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.agent.clear_session().await?;
        self.disposed.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn ensure_not_disposed(&self) -> Result<(), CoreError> {
        if self.disposed.load(Ordering::SeqCst) {
            return Err(CoreError::invalid_state(
                "runtime client has been disposed",
                None,
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arky_error::ClassifiedError;
    use async_trait::async_trait;
    use futures::{
        StreamExt,
        stream,
    };
    use pretty_assertions::assert_eq;

    use super::RuntimeClient;
    use arky_core::Agent;
    use arky_protocol::{
        AgentEvent,
        EventMetadata,
        Message,
        ProviderId,
    };
    use arky_provider::{
        Provider,
        ProviderCapabilities,
        ProviderDescriptor,
        ProviderError,
        ProviderEventStream,
        ProviderFamily,
        ProviderRequest,
    };
    use arky_session::InMemorySessionStore;

    #[derive(Clone)]
    struct RuntimeClientProvider {
        descriptor: ProviderDescriptor,
    }

    impl RuntimeClientProvider {
        fn new() -> Self {
            Self {
                descriptor: ProviderDescriptor::new(
                    ProviderId::new("runtime-client"),
                    ProviderFamily::Custom("runtime-client".to_owned()),
                    ProviderCapabilities::new()
                        .with_streaming(true)
                        .with_generate(true),
                ),
            }
        }
    }

    #[async_trait]
    impl Provider for RuntimeClientProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        async fn stream(
            &self,
            request: ProviderRequest,
        ) -> Result<ProviderEventStream, ProviderError> {
            let session_id = request
                .session
                .id
                .clone()
                .expect("runtime client tests require a session id");
            let provider_id = self.descriptor.id.clone();
            let turn_id = request.turn.id;
            let message = Message::assistant("runtime-client response");

            Ok(Box::pin(stream::iter(vec![
                Ok(AgentEvent::MessageEnd {
                    meta: EventMetadata::new(1, 1)
                        .with_session_id(session_id.clone())
                        .with_turn_id(turn_id.clone())
                        .with_provider_id(provider_id.clone()),
                    message: message.clone(),
                }),
                Ok(AgentEvent::TurnEnd {
                    meta: EventMetadata::new(2, 2)
                        .with_session_id(session_id)
                        .with_turn_id(turn_id)
                        .with_provider_id(provider_id),
                    message,
                    tool_results: Vec::new(),
                    usage: None,
                }),
            ])))
        }
    }

    fn make_client() -> RuntimeClient {
        let store = Arc::new(InMemorySessionStore::default());
        let agent = Arc::new(
            Agent::builder()
                .provider(RuntimeClientProvider::new())
                .session_store_arc(store)
                .model("runtime-client-model")
                .build()
                .expect("agent should build"),
        );

        RuntimeClient::new(agent)
    }

    #[tokio::test]
    async fn runtime_client_should_manage_sessions_and_stream_text() {
        let client = make_client();

        let session_id = client
            .create_session()
            .await
            .expect("session should be created");
        assert_eq!(client.current_session_id().await, Some(session_id.clone()));

        let mut stream = client
            .stream_text("hello")
            .await
            .expect("stream should start");
        let first = stream
            .next()
            .await
            .expect("stream should yield")
            .expect("event should succeed");
        assert_eq!(matches!(first, AgentEvent::AgentStart { .. }), true);

        client.dispose().await.expect("dispose should succeed");
        assert_eq!(client.current_session_id().await, None);
        let Err(error) = client.stream_text("after dispose").await else {
            panic!("disposed client should reject requests");
        };
        assert_eq!(error.error_code(), "CORE_INVALID_STATE");
    }
}
