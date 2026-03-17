//! Real-time SSE event streaming routes.

use std::{
    convert::Infallible,
    time::Duration,
};

use arky_types::AgentEvent;
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
use tokio::sync::broadcast::error::RecvError;

use crate::{
    ServerError,
    ServerState,
    middleware::parse_session_id,
    routes::sse::{
        SseSequence,
        agent_event_frame,
        done_event_frame,
        payload_event_frame,
    },
};

/// Streams live events for an active session as SSE.
pub async fn stream_session_events(
    State(state): State<ServerState>,
    Path(session_id): Path<String>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ServerError> {
    let session_id = parse_session_id(&session_id)?;
    let _ = state.session_store().load(&session_id).await?;
    let mut subscription = state.runtime().subscribe();

    let stream = stream! {
        let mut sequence = SseSequence::new();
        loop {
            match subscription.recv().await {
                Ok(event) => {
                    if event.metadata().session_id.as_ref() != Some(&session_id) {
                        continue;
                    }

                    match agent_event_frame(&mut sequence, &event) {
                        Ok(frame) => yield Ok(frame),
                        Err(error) => {
                            yield Ok(payload_event_frame(
                                &mut sequence,
                                "error",
                                &serde_json::json!({
                                    "type": "stream_error",
                                    "code": "SERVER_SSE_SERIALIZATION_FAILED",
                                    "message": format!("failed to serialize event: {error}"),
                                }),
                            ));
                            break;
                        }
                    }
                    if matches!(event, AgentEvent::AgentEnd { .. }) {
                        break;
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    yield Ok(payload_event_frame(
                        &mut sequence,
                        "warning",
                        &serde_json::json!({
                            "type": "lagged",
                            "skipped": skipped,
                        }),
                    ));
                }
                Err(RecvError::Closed) => break,
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
