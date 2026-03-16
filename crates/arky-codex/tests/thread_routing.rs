//! Session routing and crash-path tests for the Codex provider fixture.

use std::{
    path::PathBuf,
    time::Duration,
};

use arky_codex::{
    ApprovalMode,
    CodexProvider,
    CodexProviderConfig,
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
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test]
async fn codex_provider_should_resume_threads_for_the_same_session() {
    let tempdir = TempDir::new().expect("tempdir should create");
    let provider = fixture_provider(&tempdir);
    let session_id = SessionId::new();

    let first = provider
        .generate(request_with_session(session_id.clone(), "first", 1))
        .await
        .expect("first turn should succeed");
    let second = provider
        .generate(request_with_session(session_id, "second", 2))
        .await
        .expect("second turn should reuse the thread");

    assert_eq!(first.message, Message::assistant("turn=1;echo=User: first"));
    assert_eq!(
        second.message,
        Message::assistant("turn=2;echo=User: second")
    );
}

#[tokio::test]
async fn codex_provider_should_report_process_crashes_from_the_app_server() {
    let tempdir = TempDir::new().expect("tempdir should create");
    let provider = fixture_provider(&tempdir);
    let mut stream = provider
        .stream(request_with_session(
            SessionId::new(),
            "__CRASH_AFTER_TURN_START__",
            1,
        ))
        .await
        .expect("stream should construct");

    let first = stream
        .next()
        .await
        .expect("turn start event should be emitted")
        .expect("turn start should be valid");
    assert!(matches!(first, AgentEvent::TurnStart { .. }));

    let error = stream
        .next()
        .await
        .expect("crash should surface as a stream item")
        .expect_err("fixture crash should fail the stream");
    assert!(matches!(error, ProviderError::ProcessCrashed { .. }));
}

fn fixture_provider(tempdir: &TempDir) -> CodexProvider {
    let mut config = CodexProviderConfig {
        binary: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_codex_app_server.js")
            .display()
            .to_string(),
        allow_npx: false,
        request_timeout: Duration::from_secs(5),
        scheduler_timeout: Duration::from_secs(5),
        approval_mode: ApprovalMode::AutoApprove,
        ..CodexProviderConfig::default()
    };
    config
        .env
        .insert("ARKY_CODEX_FIXTURE".to_owned(), "1".to_owned());
    config.env.insert(
        "ARKY_CODEX_FIXTURE_STATE".to_owned(),
        tempdir
            .path()
            .join("fixture-state.json")
            .display()
            .to_string(),
    );

    CodexProvider::with_config(config)
}

fn request_with_session(
    session_id: SessionId,
    prompt: &str,
    turn_index: u64,
) -> ProviderRequest {
    ProviderRequest::new(
        SessionRef::new(Some(session_id)),
        TurnContext::new(TurnId::new(), turn_index),
        ModelRef::new("gpt-5"),
        vec![Message::user(prompt)],
    )
}
