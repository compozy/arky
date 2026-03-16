//! Real-time SSE event streaming routes.

use std::{
    convert::Infallible,
    time::Duration,
};

use arky_protocol::AgentEvent;
use async_stream::stream;
use axum::{
    extract::{
        Path,
        State,
    },
    response::sse::{
        Event,
        KeepAlive,
        Sse,
    },
};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::{
    ServerError,
    ServerState,
    middleware::parse_session_id,
};

/// Streams live events for an active session as SSE.
pub async fn stream_session_events(
    State(state): State<ServerState>,
    Path(session_id): Path<String>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ServerError> {
    let session_id = parse_session_id(&session_id)?;
    let _ = state.session_store().load(&session_id).await?;
    let mut subscription = state.agent().subscribe();

    let stream = stream! {
        loop {
            match subscription.recv().await {
                Ok(event) => {
                    if event.metadata().session_id.as_ref() != Some(&session_id) {
                        continue;
                    }

                    let payload = match serde_json::to_string(&event) {
                        Ok(payload) => payload,
                        Err(error) => {
                            let error_payload = json!({
                                "type": "stream_error",
                                "message": format!("failed to serialize event: {error}"),
                            })
                            .to_string();
                            yield Ok(Event::default().event("error").data(error_payload));
                            break;
                        }
                    };

                    yield Ok(
                        Event::default()
                            .event(event_name(&event))
                            .id(event.sequence().to_string())
                            .data(payload),
                    );
                    if matches!(event, AgentEvent::AgentEnd { .. }) {
                        break;
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    let payload = json!({
                        "type": "lagged",
                        "skipped": skipped,
                    })
                    .to_string();
                    yield Ok(Event::default().event("warning").data(payload));
                }
                Err(RecvError::Closed) => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

const fn event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::AgentStart { .. } => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart { .. } => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
        AgentEvent::Custom { .. } => "custom",
        _ => "unknown",
    }
}
