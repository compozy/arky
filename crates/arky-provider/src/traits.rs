//! Provider trait and stream-to-response helpers.

use std::pin::Pin;

use async_trait::async_trait;
use futures::{
    Stream,
    StreamExt,
};

use crate::{
    ProviderDescriptor,
    ProviderError,
    ProviderRequest,
    request::{
        GenerateResponse,
        SessionRef,
        TurnContext,
    },
};
use arky_protocol::{
    AgentEvent,
    Message,
};

/// Standard provider stream type that can surface mid-stream failures in-band.
pub type ProviderEventStream =
    Pin<Box<dyn Stream<Item = Result<AgentEvent, ProviderError>> + Send>>;

/// Low-level provider contract implemented by concrete provider crates.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns immutable provider metadata.
    fn descriptor(&self) -> &ProviderDescriptor;

    /// Starts streaming a provider response for the supplied request.
    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError>;

    /// Generates a full response directly.
    ///
    /// Providers may override this to use native non-streaming APIs. The
    /// default implementation drains [`Self::stream`] and synthesizes a final
    /// response from the terminal message events.
    async fn generate(
        &self,
        request: ProviderRequest,
    ) -> Result<GenerateResponse, ProviderError> {
        let session = request.session.clone();
        let turn = request.turn.clone();
        let stream = self.stream(request).await?;
        generate_response_from_stream(session, turn, stream).await
    }
}

/// Drains a provider event stream into a [`GenerateResponse`].
pub async fn generate_response_from_stream(
    session: SessionRef,
    turn: TurnContext,
    mut stream: ProviderEventStream,
) -> Result<GenerateResponse, ProviderError> {
    let mut terminal_message: Option<Message> = None;

    while let Some(item) = stream.next().await {
        let event = item?;
        if let Some(message) = terminal_message_from_event(&event) {
            terminal_message = Some(message);
        }
    }

    let message = terminal_message.ok_or_else(|| {
        ProviderError::protocol_violation(
            "provider stream completed without emitting a terminal assistant message",
            None,
        )
    })?;

    Ok(GenerateResponse::new(session, turn, message))
}

pub fn terminal_message_from_event(event: &AgentEvent) -> Option<Message> {
    match event {
        AgentEvent::TurnEnd { message, .. } | AgentEvent::MessageEnd { message, .. } => {
            Some(message.clone())
        }
        AgentEvent::AgentEnd { messages, .. } => messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, arky_protocol::Role::Assistant))
            .cloned(),
        AgentEvent::AgentStart { .. }
        | AgentEvent::TurnStart { .. }
        | AgentEvent::MessageStart { .. }
        | AgentEvent::MessageUpdate { .. }
        | AgentEvent::ToolExecutionStart { .. }
        | AgentEvent::ToolExecutionUpdate { .. }
        | AgentEvent::ToolExecutionEnd { .. }
        | AgentEvent::Custom { .. }
        | _ => None,
    }
}

#[cfg(test)]
mod tests {
    use futures::stream;
    use pretty_assertions::assert_eq;

    use super::generate_response_from_stream;
    use crate::ProviderError;
    use arky_protocol::{
        AgentEvent,
        EventMetadata,
        Message,
        SessionId,
        SessionRef,
        TurnContext,
        TurnId,
    };

    #[tokio::test]
    async fn generate_response_from_stream_should_use_terminal_message_event() {
        let session = SessionRef::new(Some(SessionId::new()));
        let turn = TurnContext::new(TurnId::new(), 1);
        let message = Message::assistant("done");
        let stream = Box::pin(stream::iter(vec![
            Ok(AgentEvent::MessageStart {
                meta: EventMetadata::new(1, 1),
                message: Message::assistant("partial"),
            }),
            Ok(AgentEvent::MessageEnd {
                meta: EventMetadata::new(2, 2),
                message: message.clone(),
            }),
        ]));

        let response = generate_response_from_stream(session, turn, stream)
            .await
            .expect("response should be synthesized");

        assert_eq!(response.message, message);
    }

    #[tokio::test]
    async fn generate_response_from_stream_should_reject_missing_terminal_message() {
        let session = SessionRef::new(Some(SessionId::new()));
        let turn = TurnContext::new(TurnId::new(), 1);
        let stream = Box::pin(stream::iter(vec![Ok(AgentEvent::TurnStart {
            meta: EventMetadata::new(1, 1),
        })]));

        let error = generate_response_from_stream(session, turn, stream)
            .await
            .expect_err("missing terminal message should fail");

        assert!(matches!(error, ProviderError::ProtocolViolation { .. }));
    }
}
