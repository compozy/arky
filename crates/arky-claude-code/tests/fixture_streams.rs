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
use tokio::time::{
    Duration,
    timeout,
};

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

fn fixture_provider_with_verbose(mode: &str, verbose: bool) -> ClaudeCodeProvider {
    let mut env = BTreeMap::new();
    env.insert("CLAUDE_FIXTURE_MODE".to_owned(), mode.to_owned());

    ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
        binary: fixture_binary(),
        env,
        verbose,
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

#[tokio::test]
async fn provider_should_close_fixture_stdin_for_one_shot_runs() {
    let provider = fixture_provider("wait_for_stdin_close");
    let stream = provider
        .stream(request())
        .await
        .expect("fixture stream should start");

    let completion = timeout(Duration::from_secs(2), async move {
        let mut stream = stream;
        while let Some(item) = stream.next().await {
            item.expect("fixture event should be valid");
        }
    })
    .await;

    assert!(
        completion.is_ok(),
        "provider should not wait indefinitely on stdin"
    );
}

#[tokio::test]
async fn provider_should_keep_stream_json_compatible_when_verbose_is_disabled() {
    let provider = fixture_provider_with_verbose("contract_basic", false);
    let mut stream = provider
        .stream(request())
        .await
        .expect("stream should still start when verbose is disabled");

    let mut saw_turn_end = false;
    while let Some(item) = stream.next().await {
        if let AgentEvent::TurnEnd { .. } = item.expect("fixture event should be valid") {
            saw_turn_end = true;
            break;
        }
    }

    assert!(
        saw_turn_end,
        "provider should still complete the Claude turn"
    );
}

#[tokio::test]
async fn provider_should_surface_structured_auth_failures_instead_of_process_crashes() {
    let provider = fixture_provider("auth_failed");
    let error = provider
        .generate(request())
        .await
        .expect_err("auth failure should be surfaced as a provider error");

    match error {
        ProviderError::AuthFailed { message } => {
            assert!(message.contains("Failed to authenticate"));
        }
        other => panic!("expected auth failure, got {other:?}"),
    }
}

#[tokio::test]
async fn provider_should_continue_after_rate_limit_metadata_events() {
    let provider = fixture_provider("rate_limit_event");
    let mut stream = provider
        .stream(request())
        .await
        .expect("rate-limit fixture stream should start");

    let mut saw_rate_limit = false;
    let mut saw_turn_end = false;
    while let Some(item) = stream.next().await {
        match item.expect("fixture event should be valid") {
            AgentEvent::Custom {
                event_type,
                payload,
                ..
            } if event_type == "claude_code.rate_limit" => {
                saw_rate_limit = payload["rate_limit_info"]["status"] == "allowed";
            }
            AgentEvent::TurnEnd { .. } => {
                saw_turn_end = true;
                break;
            }
            _ => {}
        }
    }

    assert!(saw_rate_limit);
    assert!(saw_turn_end);
}
