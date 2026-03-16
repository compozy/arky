//! Chat streaming route backed by the high-level agent runtime.

use std::{
    convert::Infallible,
    time::Duration,
};

use arky_error::ClassifiedError;
use arky_protocol::{
    ContentBlock,
    Message,
    ReasoningEffort,
    Role,
};
use async_stream::stream;
use axum::{
    Json,
    extract::State,
    response::sse::{
        Event,
        KeepAlive,
        Sse,
    },
};
use futures::StreamExt;
use serde::Deserialize;

use crate::{
    ServerError,
    ServerState,
    routes::sse::{
        SseSequence,
        agent_event_frame,
        done_event_frame,
        error_event_frame,
    },
};

/// Request body accepted by `POST /v1/chat/stream`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatStreamRequest {
    /// Conversation messages to submit.
    pub messages: Vec<Message>,
    /// Requested model identifier.
    pub model: String,
    /// Optional system prompt override.
    #[serde(default, alias = "system")]
    pub system_prompt: Option<String>,
    /// Optional caller-stable session key.
    #[serde(default, alias = "sessionKey")]
    pub session_key: Option<String>,
    /// Whether to resume the prior session for the supplied key.
    #[serde(default, alias = "resumeSession")]
    pub resume_session: Option<bool>,
    /// Optional max-steps hint.
    #[serde(default, alias = "maxSteps")]
    pub max_steps: Option<u32>,
    /// Optional reasoning effort hint.
    #[serde(default, alias = "reasoningEffort")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl ChatStreamRequest {
    fn validate(&self) -> Result<(), ServerError> {
        if self.messages.is_empty() {
            return Err(ServerError::invalid_request(
                "messages must contain at least one item",
            ));
        }
        if self.model.trim().is_empty() {
            return Err(ServerError::invalid_request("model is required"));
        }
        if matches!(self.max_steps, Some(0)) {
            return Err(ServerError::invalid_request(
                "max_steps must be greater than zero",
            ));
        }
        if self.messages.iter().any(|message| {
            message
                .content
                .iter()
                .any(|block| !matches!(block, ContentBlock::Text { .. }))
        }) {
            return Err(ServerError::invalid_request(
                "only text content blocks are currently supported",
            ));
        }
        if self.resume_session == Some(true)
            && self
                .session_key
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(ServerError::invalid_request(
                "session_key is required when resume_session is true",
            ));
        }

        Ok(())
    }

    fn prompt_text(&self) -> String {
        let mut sections = Vec::new();
        if let Some(system_prompt) = &self.system_prompt {
            sections.push(format!("System: {system_prompt}"));
        }
        for message in &self.messages {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
                Role::Tool => "tool",
            };
            let content = message
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !content.is_empty() {
                sections.push(format!("{role}: {content}"));
            }
        }
        sections.join("\n\n")
    }
}

/// Starts a streaming chat turn and returns SSE events.
pub async fn chat_stream(
    State(state): State<ServerState>,
    Json(request): Json<ChatStreamRequest>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ServerError> {
    request.validate()?;

    let models = state.models().await;
    if !models.is_empty() && !models.iter().any(|model| model.id == request.model) {
        return Err(ServerError::invalid_request(format!(
            "model `{}` is not registered on this server",
            request.model
        )));
    }

    let chat_start_lock = state.chat_start_lock();
    let _chat_start_guard = chat_start_lock.lock().await;

    if let Some(session_key) = &request.session_key {
        if let Some(session_id) = state.session_id_for_key(session_key).await {
            if request.resume_session.unwrap_or(true) {
                state.agent().resume(session_id.clone()).await?;
            } else {
                let new_session = state.agent().new_session().await?;
                state
                    .set_session_key(session_key.clone(), new_session)
                    .await;
            }
        } else {
            let new_session = state.agent().new_session().await?;
            state
                .set_session_key(session_key.clone(), new_session)
                .await;
        }
    }

    let agent_stream = state.agent().stream(request.prompt_text()).await?;

    let stream = stream! {
        let mut agent_stream = agent_stream;
        let mut sequence = SseSequence::new();
        while let Some(item) = agent_stream.next().await {
            match item {
                Ok(event) => {
                    match agent_event_frame(&mut sequence, &event) {
                        Ok(frame) => yield Ok(frame),
                        Err(error) => {
                            yield Ok(error_event_frame(
                                &mut sequence,
                                "SERVER_SSE_SERIALIZATION_FAILED",
                                format!("failed to serialize event: {error}"),
                            ));
                            break;
                        }
                    }
                }
                Err(error) => {
                    yield Ok(error_event_frame(
                        &mut sequence,
                        error.error_code(),
                        error.to_string(),
                    ));
                    break;
                }
            }
        }
        yield Ok(done_event_frame(&mut sequence));
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}
