//! Integration tests for the runtime HTTP/SSE server.
#![cfg(feature = "server")]

use std::sync::Arc;

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
use arky_server::{
    ModelCard,
    ProviderHealthSnapshot,
    ServerState,
    serve,
};
use arky_session::InMemorySessionStore;
use async_trait::async_trait;
use futures::stream;
use pretty_assertions::assert_eq;
use reqwest::Client;
use serde_json::Value;

#[derive(Clone)]
struct EchoProvider {
    descriptor: ProviderDescriptor,
}

impl EchoProvider {
    fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("mock-server"),
                ProviderFamily::Custom("mock-server".to_owned()),
                ProviderCapabilities::new()
                    .with_streaming(true)
                    .with_generate(true)
                    .with_tool_calls(true)
                    .with_session_resume(true)
                    .with_steering(true)
                    .with_follow_up(true),
            ),
        }
    }
}

#[async_trait]
impl Provider for EchoProvider {
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
            .expect("test requests always carry session ids");
        let message_text = request
            .messages
            .iter()
            .rev()
            .find_map(|message| {
                if message.role != arky_protocol::Role::User {
                    return None;
                }

                message.content.iter().find_map(|block| match block {
                    arky_protocol::ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .unwrap_or_else(|| "empty".to_owned());
        let message = Message::assistant(format!("echo: {message_text}"));
        let provider_id = self.descriptor.id.clone();
        let turn_id = request.turn.id;

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

async fn spawn_server() -> (
    Arc<Agent>,
    Arc<InMemorySessionStore>,
    arky_server::ServerHandle,
    Client,
) {
    spawn_server_with_auth(None).await
}

async fn spawn_server_with_auth(
    token: Option<&str>,
) -> (
    Arc<Agent>,
    Arc<InMemorySessionStore>,
    arky_server::ServerHandle,
    Client,
) {
    let store = Arc::new(InMemorySessionStore::default());
    let agent = Arc::new(
        Agent::builder()
            .provider(EchoProvider::new())
            .session_store_arc(store.clone())
            .model("mock-model")
            .build()
            .expect("agent should build"),
    );
    let mut state = ServerState::new(agent.clone(), store.clone());
    if let Some(token) = token {
        state = state.with_bearer_token(token);
    }
    state
        .set_models(vec![ModelCard::new(
            "mock-model",
            "mock-server",
            ProviderId::new("mock-server"),
        )])
        .await;
    state
        .health()
        .set_provider_health(ProviderHealthSnapshot::healthy(ProviderId::new(
            "mock-server",
        )))
        .await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let handle = serve(listener, state).expect("server should start");
    let client = Client::builder().build().expect("client should build");

    (agent, store, handle, client)
}

#[derive(Debug, Default)]
struct SseFrame {
    event: Option<String>,
    data: String,
    id: Option<String>,
}

fn parse_sse_frames(payload: &str) -> Vec<SseFrame> {
    let mut frames = Vec::new();
    for frame in payload.split("\n\n") {
        if frame.starts_with(':') || frame.trim().is_empty() {
            continue;
        }

        let mut parsed = SseFrame::default();
        for line in frame.lines() {
            if let Some(value) = line.strip_prefix("event: ") {
                parsed.event = Some(value.to_owned());
            } else if let Some(value) = line.strip_prefix("data: ") {
                if !parsed.data.is_empty() {
                    parsed.data.push('\n');
                }
                parsed.data.push_str(value);
            } else if let Some(value) = line.strip_prefix("id: ") {
                parsed.id = Some(value.to_owned());
            }
        }

        frames.push(parsed);
    }

    frames
}

#[tokio::test]
async fn http_round_trip_should_expose_health_sessions_and_cors() {
    let (agent, _store, handle, client) = spawn_server().await;
    let session_id = agent
        .new_session()
        .await
        .expect("session should be created");
    let _response = agent
        .prompt("hello world")
        .await
        .expect("prompt should succeed");

    let health = client
        .get(format!("{}/health", handle.base_url()))
        .header("origin", "https://example.com")
        .send()
        .await
        .expect("health request should succeed");
    assert_eq!(health.status(), reqwest::StatusCode::OK);
    assert_eq!(
        health
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );
    let health_body: Value = health.json().await.expect("health should deserialize");
    assert_eq!(health_body["status"], "ok");

    let ready = client
        .get(format!("{}/ready", handle.base_url()))
        .send()
        .await
        .expect("ready request should succeed");
    assert_eq!(ready.status(), reqwest::StatusCode::OK);

    let providers: Value = client
        .get(format!("{}/providers/health", handle.base_url()))
        .send()
        .await
        .expect("providers request should succeed")
        .json()
        .await
        .expect("providers should deserialize");
    assert_eq!(providers["providers"][0]["provider_id"], "mock-server");

    let sessions: Value = client
        .get(format!("{}/sessions", handle.base_url()))
        .send()
        .await
        .expect("sessions request should succeed")
        .json()
        .await
        .expect("sessions should deserialize");
    assert_eq!(sessions["sessions"][0]["id"], session_id.to_string());

    let detail: Value = client
        .get(format!("{}/sessions/{}", handle.base_url(), session_id))
        .send()
        .await
        .expect("detail request should succeed")
        .json()
        .await
        .expect("detail should deserialize");
    assert_eq!(detail["session"]["metadata"]["id"], session_id.to_string());

    let messages: Value = client
        .get(format!(
            "{}/sessions/{}/messages",
            handle.base_url(),
            session_id
        ))
        .send()
        .await
        .expect("messages request should succeed")
        .json()
        .await
        .expect("messages should deserialize");
    assert_eq!(messages["messages"].as_array().map(Vec::len), Some(2));

    let missing = client
        .get(format!(
            "{}/sessions/{}",
            handle.base_url(),
            arky_protocol::SessionId::new()
        ))
        .header("origin", "https://example.com")
        .send()
        .await
        .expect("missing request should succeed");
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        missing
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );

    handle.shutdown().await.expect("server should stop cleanly");
}

#[tokio::test]
async fn models_route_should_return_openai_compatible_payload() {
    let (_agent, _store, handle, client) = spawn_server().await;

    let models: Value = client
        .get(format!("{}/v1/models", handle.base_url()))
        .send()
        .await
        .expect("models request should succeed")
        .json()
        .await
        .expect("models should deserialize");

    assert_eq!(models["object"], "list");
    assert_eq!(models["data"][0]["id"], "mock-model");
    assert_eq!(models["data"][0]["object"], "model");
    assert_eq!(models["data"][0]["compozy"]["provider_id"], "mock-server");

    handle.shutdown().await.expect("server should stop cleanly");
}

#[tokio::test]
async fn chat_stream_route_should_validate_auth_and_stream_sse() {
    let (_agent, _store, handle, client) =
        spawn_server_with_auth(Some("secret-token")).await;

    let missing = client
        .post(format!("{}/v1/chat/stream", handle.base_url()))
        .json(&serde_json::json!({
            "messages": [{"role":"user","content":[{"type":"text","text":"hello"}]}],
            "model": "mock-model"
        }))
        .send()
        .await
        .expect("missing auth request should return");
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);

    let invalid = client
        .post(format!("{}/v1/chat/stream", handle.base_url()))
        .bearer_auth("wrong-token")
        .json(&serde_json::json!({
            "messages": [{"role":"user","content":[{"type":"text","text":"hello"}]}],
            "model": "mock-model"
        }))
        .send()
        .await
        .expect("invalid auth request should return");
    assert_eq!(invalid.status(), reqwest::StatusCode::FORBIDDEN);

    let response = client
        .post(format!("{}/v1/chat/stream", handle.base_url()))
        .bearer_auth("secret-token")
        .json(&serde_json::json!({
            "messages": [{"role":"user","content":[{"type":"text","text":"hello"}]}],
            "model": "mock-model",
            "session_key": "chat-1",
            "resume_session": true
        }))
        .send()
        .await
        .expect("chat stream should connect");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.starts_with("text/event-stream")),
        Some(true)
    );

    let body = response.text().await.expect("sse body should read");
    let frames = parse_sse_frames(&body);

    assert!(frames.iter().any(|frame| frame.data == "[DONE]"));
    let ids = frames
        .iter()
        .filter_map(|frame| frame.id.as_deref())
        .filter_map(|id| id.parse::<u64>().ok())
        .collect::<Vec<_>>();
    assert_eq!(ids.windows(2).all(|window| window[0] < window[1]), true);
    assert!(
        frames
            .iter()
            .any(|frame| frame.event.as_deref() == Some("turn_end"))
    );

    handle.shutdown().await.expect("server should stop cleanly");
}

#[tokio::test]
async fn sse_endpoint_should_stream_live_session_events() {
    let (agent, _store, handle, client) = spawn_server().await;
    let session_id = agent
        .new_session()
        .await
        .expect("session should be created");

    let dropped = client
        .get(format!(
            "{}/sessions/{}/events",
            handle.base_url(),
            session_id
        ))
        .send()
        .await
        .expect("sse request should connect");
    drop(dropped);

    let response = client
        .get(format!(
            "{}/sessions/{}/events",
            handle.base_url(),
            session_id
        ))
        .send()
        .await
        .expect("sse request should connect");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.starts_with("text/event-stream")),
        Some(true)
    );

    let agent_task = {
        let agent = agent.clone();
        tokio::spawn(async move {
            agent
                .prompt("stream this")
                .await
                .expect("prompt should succeed");
        })
    };
    agent_task.await.expect("prompt task should join");
    let body = tokio::time::timeout(std::time::Duration::from_secs(5), response.text())
        .await
        .expect("sse response should complete")
        .expect("sse body should read");
    let frames = parse_sse_frames(&body);

    assert!(
        frames
            .iter()
            .any(|frame| frame.event.as_deref() == Some("turn_start"))
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.event.as_deref() == Some("turn_end"))
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.event.as_deref() == Some("agent_end"))
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.data.contains("\"type\":\"turn_end\""))
    );
    assert_eq!(
        frames.last().map(|frame| frame.data.as_str()),
        Some("[DONE]")
    );
    let ids = frames
        .iter()
        .filter_map(|frame| frame.id.as_deref())
        .filter_map(|id| id.parse::<u64>().ok())
        .collect::<Vec<_>>();
    assert_eq!(ids.windows(2).all(|window| window[0] < window[1]), true);
    assert_eq!(ids.first().copied(), Some(1));

    handle.shutdown().await.expect("server should stop cleanly");
}

#[tokio::test]
async fn replay_endpoint_should_return_persisted_event_sequence() {
    let (agent, _store, handle, client) = spawn_server().await;
    let session_id = agent
        .new_session()
        .await
        .expect("session should be created");
    let _response = agent
        .prompt("persist this")
        .await
        .expect("prompt should succeed");

    let replay: Value = client
        .get(format!(
            "{}/sessions/{}/replay",
            handle.base_url(),
            session_id
        ))
        .send()
        .await
        .expect("replay request should succeed")
        .json()
        .await
        .expect("replay should deserialize");

    let events = replay["events"]
        .as_array()
        .expect("events should be an array");
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .any(|event| event["event"]["type"] == "turn_end")
    );
    let sequences = events
        .iter()
        .map(|event| {
            event["sequence"]
                .as_u64()
                .expect("sequence should be present")
        })
        .collect::<Vec<_>>();
    assert!(sequences.windows(2).all(|window| window[0] < window[1]));

    handle.shutdown().await.expect("server should stop cleanly");
}
