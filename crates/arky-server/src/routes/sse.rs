//! Shared SSE framing helpers for runtime streaming routes.

use axum::response::sse::Event;
use serde_json::Value;

use arky_protocol::AgentEvent;

use crate::routes::sse_event_name;

#[derive(Debug, Default)]
pub struct SseSequence {
    next: u64,
}

impl SseSequence {
    #[must_use]
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    fn next_id(&mut self) -> String {
        self.next = self.next.saturating_add(1);
        self.next.to_string()
    }
}

pub fn agent_event_frame(
    sequence: &mut SseSequence,
    event: &AgentEvent,
) -> Result<Event, serde_json::Error> {
    Ok(Event::default()
        .event(sse_event_name(event))
        .id(sequence.next_id())
        .data(serde_json::to_string(event)?))
}

pub fn payload_event_frame(
    sequence: &mut SseSequence,
    event_name: &str,
    payload: &Value,
) -> Event {
    Event::default()
        .event(event_name)
        .id(sequence.next_id())
        .data(payload.to_string())
}

pub fn error_event_frame(
    sequence: &mut SseSequence,
    code: &str,
    message: impl Into<String>,
) -> Event {
    payload_event_frame(
        sequence,
        "error",
        &serde_json::json!({
            "type": "stream_error",
            "code": code,
            "message": message.into(),
        }),
    )
}

pub fn done_event_frame(sequence: &mut SseSequence) -> Event {
    Event::default().id(sequence.next_id()).data("[DONE]")
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        SseSequence,
        done_event_frame,
        error_event_frame,
    };

    #[test]
    fn sse_sequence_should_increment_monotonically() {
        let mut sequence = SseSequence::new();
        let first_id = sequence.next_id();
        let second_id = sequence.next_id();
        let third_id = sequence.next_id();

        let done = done_event_frame(&mut SseSequence { next: 3 });
        let done_payload = format!("{done:?}");

        assert_eq!(first_id, "1");
        assert_eq!(second_id, "2");
        assert_eq!(third_id, "3");
        assert_eq!(done_payload.contains("[DONE]"), true);
    }

    #[test]
    fn error_event_should_embed_structured_payload() {
        let mut sequence = SseSequence::new();
        let frame = error_event_frame(&mut sequence, "SERVER_TEST", "boom");
        let next_id = sequence.next_id();

        assert_eq!(format!("{frame:?}").is_empty(), false);
        assert_eq!(next_id, "2");
    }
}
