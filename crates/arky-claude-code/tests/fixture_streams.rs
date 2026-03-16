//! Fixture-backed integration tests for the Claude provider stream.

use std::{
    collections::BTreeMap,
    path::PathBuf,
};

use arky_claude_code::{
    ClaudeCodeProvider,
    ClaudeCodeProviderConfig,
};
use arky_protocol::{
    AgentEvent,
    Message,
    ModelRef,
    SessionId,
    SessionRef,
    TurnContext,
    TurnId,
};
use arky_provider::{
    Provider,
    ProviderError,
    ProviderRequest,
};
use futures::StreamExt;

fn fixture_binary() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("claude_fixture.sh")
        .display()
        .to_string()
}

fn fixture_provider(mode: &str) -> ClaudeCodeProvider {
    let mut env = BTreeMap::new();
    env.insert("CLAUDE_FIXTURE_MODE".to_owned(), mode.to_owned());

    ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
        binary: fixture_binary(),
        env,
        ..ClaudeCodeProviderConfig::default()
    })
}

fn request() -> ProviderRequest {
    ProviderRequest::new(
        SessionRef::new(Some(SessionId::new())),
        TurnContext::new(TurnId::new(), 1),
        ModelRef::new("sonnet"),
        vec![Message::user("hello")],
    )
}

#[tokio::test]
async fn fixture_stream_should_track_tool_lifecycle_end_to_end() {
    let provider = fixture_provider("tool_cycle");
    let mut stream = provider
        .stream(request())
        .await
        .expect("fixture stream should start");
    let mut saw_start = false;
    let mut saw_end = false;
    let mut saw_turn_end = false;

    while let Some(item) = stream.next().await {
        let event = item.expect("fixture event should be valid");
        match event {
            AgentEvent::ToolExecutionStart { tool_name, .. } => {
                saw_start = tool_name == "search";
            }
            AgentEvent::ToolExecutionEnd { tool_name, .. } => {
                saw_end = tool_name == "search";
            }
            AgentEvent::TurnEnd { message, .. } => {
                saw_turn_end = message.content.iter().any(|block| {
                    matches!(
                        block,
                        arky_protocol::ContentBlock::Text { text }
                            if text.contains("after tool")
                    )
                });
            }
            _ => {}
        }
    }

    assert!(saw_start);
    assert!(saw_end);
    assert!(saw_turn_end);
}

#[tokio::test]
async fn malformed_fixture_should_surface_protocol_violation_in_band() {
    let provider = fixture_provider("malformed");
    let mut stream = provider
        .stream(request())
        .await
        .expect("malformed fixture should still construct a stream");

    let mut saw_protocol_violation = false;
    while let Some(item) = stream.next().await {
        match item {
            Err(ProviderError::ProtocolViolation { .. }) => {
                saw_protocol_violation = true;
                break;
            }
            Ok(_) => {}
            Err(other) => panic!("expected protocol violation, got {other:?}"),
        }
    }

    assert!(saw_protocol_violation);
}

#[tokio::test]
async fn crash_fixture_should_surface_process_crash_in_band() {
    let provider = fixture_provider("crash_after_first_event");
    let mut stream = provider
        .stream(request())
        .await
        .expect("crash fixture should still construct a stream");

    let mut saw_process_crash = false;
    while let Some(item) = stream.next().await {
        match item {
            Err(ProviderError::ProcessCrashed { .. }) => {
                saw_process_crash = true;
                break;
            }
            Ok(_) => {}
            Err(other) => panic!("expected process crash, got {other:?}"),
        }
    }

    assert!(saw_process_crash);
}
