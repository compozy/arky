//! Integration tests covering the public `arky-core` agent API.

use std::sync::{
    Arc,
    Mutex,
    atomic::{
        AtomicBool,
        Ordering,
    },
};

use arky_core::{
    Agent,
    CoreError,
    EventSubscription,
};
use arky_protocol::{
    AgentEvent,
    ContentBlock,
    EventMetadata,
    Message,
    ProviderId,
    Role,
    SessionRef,
    ToolCall,
    ToolContent,
    ToolResult,
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
use arky_session::{
    InMemorySessionStore,
    SessionStore,
};
use arky_tools::{
    Tool,
    ToolDescriptor,
    ToolOrigin,
};
use async_trait::async_trait;
use futures::{
    Stream,
    StreamExt,
    stream,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::{
    sync::{
        Notify,
        mpsc,
    },
    time::{
        Duration,
        timeout,
    },
};
use tokio_util::sync::CancellationToken;

type ProviderHandler =
    dyn Fn(ProviderRequest) -> ProviderEventStream + Send + Sync + 'static;

struct RecordingProvider {
    descriptor: ProviderDescriptor,
    handler: Arc<ProviderHandler>,
    requests: Arc<Mutex<Vec<ProviderRequest>>>,
}

impl RecordingProvider {
    fn new<F>(handler: F) -> (Self, Arc<Mutex<Vec<ProviderRequest>>>)
    where
        F: Fn(ProviderRequest) -> ProviderEventStream + Send + Sync + 'static,
    {
        Self::new_with_descriptor(test_provider_descriptor(), handler)
    }

    fn new_with_descriptor<F>(
        descriptor: ProviderDescriptor,
        handler: F,
    ) -> (Self, Arc<Mutex<Vec<ProviderRequest>>>)
    where
        F: Fn(ProviderRequest) -> ProviderEventStream + Send + Sync + 'static,
    {
        let requests = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                descriptor,
                handler: Arc::new(handler),
                requests: Arc::clone(&requests),
            },
            requests,
        )
    }
}

#[async_trait]
impl Provider for RecordingProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError> {
        self.requests
            .lock()
            .expect("requests mutex should not be poisoned")
            .push(request.clone());
        Ok((self.handler)(request))
    }
}

struct EchoTool {
    started: Option<Arc<Notify>>,
    started_flag: Option<Arc<AtomicBool>>,
    release: Option<Arc<Notify>>,
}

#[async_trait]
impl Tool for EchoTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor::new(
            "mcp/test/echo",
            "Echo",
            "Echoes the provided payload.",
            json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                },
                "required": ["text"]
            }),
            ToolOrigin::Local,
        )
        .expect("test tool descriptor should be valid")
    }

    async fn execute(
        &self,
        call: ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolResult, arky_tools::ToolError> {
        if let Some(started_flag) = &self.started_flag {
            started_flag.store(true, Ordering::SeqCst);
        }
        if let Some(started) = &self.started {
            started.notify_waiters();
        }
        if let Some(release) = &self.release {
            tokio::select! {
                () = cancel.cancelled() => {
                    return Err(arky_tools::ToolError::cancelled(
                        "echo tool cancelled",
                        Some(call.name.clone()),
                    ));
                }
                () = release.notified() => {}
            }
        }

        Ok(ToolResult::success(
            call.id,
            call.name,
            vec![ToolContent::json(call.input)],
        ))
    }
}

fn test_provider_descriptor() -> ProviderDescriptor {
    ProviderDescriptor::new(
        ProviderId::new("mock-core"),
        ProviderFamily::Custom("mock-core".to_owned()),
        ProviderCapabilities::new()
            .with_streaming(true)
            .with_generate(true)
            .with_tool_calls(true)
            .with_session_resume(true)
            .with_steering(true)
            .with_follow_up(true),
    )
}

fn stream_from_receiver(
    mut receiver: mpsc::UnboundedReceiver<Result<AgentEvent, ProviderError>>,
) -> ProviderEventStream {
    Box::pin(futures::stream::poll_fn(move |cx| receiver.poll_recv(cx)))
}

fn final_message_stream(
    request: &ProviderRequest,
    message: Message,
) -> ProviderEventStream {
    let provider_id = test_provider_descriptor().id;
    Box::pin(stream::iter(vec![
        Ok(AgentEvent::MessageEnd {
            meta: EventMetadata::new(1, 1)
                .with_session_id(
                    request
                        .session
                        .id
                        .clone()
                        .expect("test requests always include a session id"),
                )
                .with_turn_id(request.turn.id.clone())
                .with_provider_id(provider_id.clone()),
            message: message.clone(),
        }),
        Ok(AgentEvent::TurnEnd {
            meta: EventMetadata::new(2, 2)
                .with_session_id(
                    request
                        .session
                        .id
                        .clone()
                        .expect("test requests always include a session id"),
                )
                .with_turn_id(request.turn.id.clone())
                .with_provider_id(provider_id),
            message,
            tool_results: Vec::new(),
            usage: None,
        }),
    ]))
}

fn last_text_message(request: &ProviderRequest, role: Role) -> Option<String> {
    request.messages.iter().rev().find_map(|message| {
        if message.role != role {
            return None;
        }
        text_from_message(message)
    })
}

fn text_from_message(message: &Message) -> Option<String> {
    let mut text = String::new();
    for block in &message.content {
        if let ContentBlock::Text { text: value } = block {
            text.push_str(value);
        }
    }
    if text.is_empty() { None } else { Some(text) }
}

async fn collect_until_agent_end(
    subscription: &mut EventSubscription,
    expected_agent_ends: usize,
) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    let mut agent_end_count = 0usize;

    while agent_end_count < expected_agent_ends {
        let event = timeout(Duration::from_secs(2), subscription.recv())
            .await
            .expect("subscription should yield within timeout")
            .expect("subscription should not close");
        if matches!(event, AgentEvent::AgentEnd { .. }) {
            agent_end_count += 1;
        }
        events.push(event);
    }

    events
}

async fn collect_stream_error<S>(mut stream: S) -> CoreError
where
    S: Stream<Item = Result<AgentEvent, CoreError>> + Unpin,
{
    timeout(Duration::from_secs(2), async move {
        while let Some(item) = stream.next().await {
            if let Err(error) = item {
                return error;
            }
        }
        panic!("stream should surface an error");
    })
    .await
    .expect("stream should terminate within timeout")
}

#[tokio::test]
async fn full_turn_loop_should_execute_tools_and_assemble_response() {
    let (provider, requests) = RecordingProvider::new(|request| {
        if request
            .messages
            .iter()
            .any(|message| message.role == Role::Tool)
        {
            return final_message_stream(&request, Message::assistant("tool-complete"));
        }

        let assistant_message = Message::builder(Role::Assistant)
            .block(ContentBlock::tool_use(ToolCall::new(
                "call-1",
                "mcp/test/echo",
                json!({ "text": "hello-tool" }),
            )))
            .build();
        final_message_stream(&request, assistant_message)
    });
    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .tool(EchoTool {
            started: None,
            started_flag: None,
            release: None,
        })
        .build()
        .expect("agent should build");

    let response = agent
        .prompt("needs tool")
        .await
        .expect("prompt should complete");

    assert_eq!(
        text_from_message(&response.message).as_deref(),
        Some("tool-complete")
    );
    assert_eq!(response.tool_results.len(), 1);
    assert_eq!(response.tool_results[0].name, "mcp/test/echo");

    let recorded = requests
        .lock()
        .expect("requests mutex should not be poisoned")
        .clone();
    assert_eq!(recorded.len(), 2);
    assert!(
        recorded[1]
            .messages
            .iter()
            .any(|message| message.role == Role::Tool)
    );
}

#[tokio::test]
async fn overlapping_turns_should_return_busy_session() {
    let release = Arc::new(Notify::new());
    let (provider, _) = RecordingProvider::new({
        let release = Arc::clone(&release);
        move |request| {
            let (tx, rx) = mpsc::unbounded_channel();
            let release = Arc::clone(&release);
            tokio::spawn(async move {
                release.notified().await;
                let _ = tx.send(Ok(AgentEvent::MessageEnd {
                    meta: EventMetadata::new(1, 1)
                        .with_session_id(
                            request
                                .session
                                .id
                                .clone()
                                .expect("request should have a session id"),
                        )
                        .with_turn_id(request.turn.id.clone())
                        .with_provider_id(test_provider_descriptor().id),
                    message: Message::assistant("released"),
                }));
                let _ = tx.send(Ok(AgentEvent::TurnEnd {
                    meta: EventMetadata::new(2, 2)
                        .with_session_id(
                            request
                                .session
                                .id
                                .clone()
                                .expect("request should have a session id"),
                        )
                        .with_turn_id(request.turn.id)
                        .with_provider_id(test_provider_descriptor().id),
                    message: Message::assistant("released"),
                    tool_results: Vec::new(),
                    usage: None,
                }));
            });
            stream_from_receiver(rx)
        }
    });
    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .build()
        .expect("agent should build");

    let stream = agent.stream("hold").await.expect("stream should start");
    let error = agent
        .prompt("second")
        .await
        .expect_err("second turn should be rejected");

    assert!(matches!(error, CoreError::BusySession { .. }));
    agent
        .abort()
        .await
        .expect("abort should cancel the active turn");
    let stream_error = collect_stream_error(stream).await;
    assert!(matches!(stream_error, CoreError::Cancelled { .. }));
    release.notify_waiters();
}

#[tokio::test]
async fn event_subscription_should_receive_events_in_order() {
    let (provider, _) = RecordingProvider::new(|request| {
        final_message_stream(&request, Message::assistant("done"))
    });
    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .build()
        .expect("agent should build");
    let mut subscription = agent.subscribe();

    let response = agent.prompt("hello").await.expect("prompt should succeed");
    let events = collect_until_agent_end(&mut subscription, 1).await;

    assert_eq!(
        text_from_message(&response.message).as_deref(),
        Some("done")
    );
    assert!(matches!(
        events.first(),
        Some(AgentEvent::AgentStart { .. })
    ));
    assert!(matches!(events.last(), Some(AgentEvent::AgentEnd { .. })));
    assert!(
        events
            .windows(2)
            .all(|pair| pair[1].sequence() > pair[0].sequence())
    );
}

#[tokio::test]
async fn prompt_should_accumulate_turn_and_session_usage() {
    let (provider, _) = RecordingProvider::new(|request| {
        let session_id = request
            .session
            .id
            .clone()
            .expect("test requests always include a session id");
        let provider_id = test_provider_descriptor().id;
        let turn_id = request.turn.id;

        Box::pin(stream::iter(vec![
            Ok(AgentEvent::Custom {
                meta: EventMetadata::new(1, 1)
                    .with_session_id(session_id.clone())
                    .with_turn_id(turn_id.clone())
                    .with_provider_id(provider_id.clone()),
                event_type: "usage".to_owned(),
                payload: json!({
                    "input_tokens": 10,
                    "output_tokens": 4,
                    "total_tokens": 14,
                    "output_details": {
                        "reasoning": 2
                    }
                }),
            }),
            Ok(AgentEvent::MessageEnd {
                meta: EventMetadata::new(2, 2)
                    .with_session_id(session_id.clone())
                    .with_turn_id(turn_id.clone())
                    .with_provider_id(provider_id.clone()),
                message: Message::assistant("usage"),
            }),
            Ok(AgentEvent::TurnEnd {
                meta: EventMetadata::new(3, 3)
                    .with_session_id(session_id)
                    .with_turn_id(turn_id)
                    .with_provider_id(provider_id),
                message: Message::assistant("usage"),
                tool_results: Vec::new(),
                usage: None,
            }),
        ]))
    });
    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .build()
        .expect("agent should build");
    let mut subscription = agent.subscribe();

    let first = agent
        .prompt("first")
        .await
        .expect("first prompt should succeed");
    let first_events = collect_until_agent_end(&mut subscription, 1).await;
    let second = agent
        .prompt("second")
        .await
        .expect("second prompt should succeed");
    let second_events = collect_until_agent_end(&mut subscription, 1).await;

    assert_eq!(
        first.usage.as_ref().and_then(|usage| usage.total_tokens),
        Some(14)
    );
    assert_eq!(
        second.usage.as_ref().and_then(|usage| usage.total_tokens),
        Some(28)
    );
    let first_turn_usage = first_events.iter().find_map(|event| match event {
        AgentEvent::TurnEnd { usage, .. } => usage.clone(),
        _ => None,
    });
    let second_turn_usage = second_events.iter().find_map(|event| match event {
        AgentEvent::TurnEnd { usage, .. } => usage.clone(),
        _ => None,
    });
    assert_eq!(
        first_turn_usage
            .as_ref()
            .and_then(|usage| usage.total_tokens),
        Some(14)
    );
    assert_eq!(
        second_turn_usage
            .as_ref()
            .and_then(|usage| usage.total_tokens),
        Some(14)
    );
}

#[tokio::test]
async fn capability_validation_should_emit_warning_events_at_turn_entry() {
    let descriptor = ProviderDescriptor::new(
        ProviderId::new("mock-core-limited"),
        ProviderFamily::Custom("mock-core-limited".to_owned()),
        ProviderCapabilities::new()
            .with_streaming(true)
            .with_generate(true),
    );
    let (provider, _) = RecordingProvider::new_with_descriptor(descriptor, |request| {
        final_message_stream(&request, Message::assistant("done"))
    });
    let agent = Agent::builder()
        .provider(provider)
        .tool(EchoTool {
            started: None,
            started_flag: None,
            release: None,
        })
        .model("mock-model")
        .build()
        .expect("agent should build");
    let mut subscription = agent.subscribe();

    let _response = agent.prompt("hello").await.expect("prompt should succeed");
    let events = collect_until_agent_end(&mut subscription, 1).await;
    let warning_payloads = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Custom {
                event_type,
                payload,
                ..
            } if event_type == "capability_warning" => Some(payload.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(warning_payloads.is_empty(), false);
    assert_eq!(
        warning_payloads
            .iter()
            .any(|payload| payload["capability"] == "tool_calls"),
        true
    );
}

#[tokio::test]
async fn steering_should_inject_system_guidance_into_the_next_turn() {
    let tool_started = Arc::new(Notify::new());
    let tool_started_flag = Arc::new(AtomicBool::new(false));
    let tool_release = Arc::new(Notify::new());
    let (provider, requests) = RecordingProvider::new(|request| {
        if request
            .messages
            .iter()
            .any(|message| message.role == Role::System)
        {
            return final_message_stream(&request, Message::assistant("steered"));
        }
        if request
            .messages
            .iter()
            .any(|message| message.role == Role::Tool)
        {
            return final_message_stream(&request, Message::assistant("tool-only"));
        }

        let assistant_message = Message::builder(Role::Assistant)
            .block(ContentBlock::tool_use(ToolCall::new(
                "call-1",
                "mcp/test/echo",
                json!({ "text": "payload" }),
            )))
            .build();
        final_message_stream(&request, assistant_message)
    });
    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .tool(EchoTool {
            started: Some(Arc::clone(&tool_started)),
            started_flag: Some(Arc::clone(&tool_started_flag)),
            release: Some(Arc::clone(&tool_release)),
        })
        .build()
        .expect("agent should build");

    let mut stream = agent.stream("start").await.expect("stream should start");
    if !tool_started_flag.load(Ordering::SeqCst) {
        timeout(Duration::from_secs(2), tool_started.notified())
            .await
            .expect("tool should start");
    }
    agent
        .steer("use the override")
        .await
        .expect("steering should be accepted");
    tool_release.notify_waiters();

    let final_message = timeout(Duration::from_secs(2), async {
        let mut final_text = None;
        while let Some(item) = stream.next().await {
            let event = item.expect("streamed turn should succeed");
            if let AgentEvent::TurnEnd { message, .. } = event {
                final_text = text_from_message(&message);
            }
        }
        final_text
    })
    .await
    .expect("stream should finish");

    assert_eq!(final_message.as_deref(), Some("steered"));
    let recorded = requests
        .lock()
        .expect("requests mutex should not be poisoned")
        .clone();
    assert_eq!(recorded.len(), 2);
    assert_eq!(
        last_text_message(&recorded[1], Role::System).as_deref(),
        Some("use the override")
    );
}

#[tokio::test]
async fn follow_up_should_continue_after_a_completed_turn() {
    let (provider, requests) = RecordingProvider::new(|request| {
        let user_text = last_text_message(&request, Role::User)
            .unwrap_or_else(|| "missing".to_owned());
        final_message_stream(&request, Message::assistant(format!("reply:{user_text}")))
    });
    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .build()
        .expect("agent should build");
    let mut subscription = agent.subscribe();

    let first = agent
        .prompt("hello")
        .await
        .expect("first prompt should succeed");
    assert_eq!(
        text_from_message(&first.message).as_deref(),
        Some("reply:hello")
    );

    agent
        .follow_up("again")
        .await
        .expect("follow_up should schedule a new turn");
    let events = collect_until_agent_end(&mut subscription, 2).await;

    let agent_end_count = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::AgentEnd { .. }))
        .count();
    assert_eq!(agent_end_count, 2);

    let recorded = requests
        .lock()
        .expect("requests mutex should not be poisoned")
        .clone();
    assert_eq!(recorded.len(), 2);
    assert_eq!(
        last_text_message(&recorded[1], Role::User).as_deref(),
        Some("again")
    );
    assert!(
        recorded[1]
            .messages
            .iter()
            .any(|message| text_from_message(message).as_deref() == Some("reply:hello"))
    );
}

#[tokio::test]
async fn resume_should_restore_session_state_for_the_next_prompt() {
    let store = Arc::new(InMemorySessionStore::default());
    let (provider, requests) = RecordingProvider::new(|request| {
        let user_text = last_text_message(&request, Role::User)
            .unwrap_or_else(|| "missing".to_owned());
        final_message_stream(&request, Message::assistant(format!("seen:{user_text}")))
    });

    let agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .session_store_arc(store.clone())
        .build()
        .expect("agent should build");
    let first = agent.prompt("first").await.expect("prompt should succeed");
    let session_id = first
        .session
        .id
        .clone()
        .expect("agent response should expose a session id");
    let snapshot_before_resume = store
        .load(&session_id)
        .await
        .expect("pre-resume snapshot should load");

    let (provider, requests_after_resume) = RecordingProvider::new(|request| {
        let user_text = last_text_message(&request, Role::User)
            .unwrap_or_else(|| "missing".to_owned());
        final_message_stream(&request, Message::assistant(format!("seen:{user_text}")))
    });
    let resumed_agent = Agent::builder()
        .provider(provider)
        .model("mock-model")
        .session_store_arc(store.clone())
        .build()
        .expect("resumed agent should build");
    resumed_agent
        .resume(session_id.clone())
        .await
        .expect("resume should restore the session");
    let second = resumed_agent
        .prompt("second")
        .await
        .expect("resumed prompt should succeed");

    assert_eq!(
        text_from_message(&second.message).as_deref(),
        Some("seen:second")
    );
    let snapshot = store.load(&session_id).await.expect("snapshot should load");
    assert!(snapshot.last_checkpoint.is_some());
    assert!(snapshot.replay_cursor.is_some());

    let first_recorded = requests
        .lock()
        .expect("requests mutex should not be poisoned")
        .clone();
    let second_recorded = requests_after_resume
        .lock()
        .expect("requests mutex should not be poisoned")
        .clone();
    assert_eq!(first_recorded.len(), 1);
    assert_eq!(second_recorded.len(), 1);
    assert!(
        second_recorded[0]
            .messages
            .iter()
            .any(|message| text_from_message(message).as_deref() == Some("first"))
    );
    assert_eq!(
        second_recorded[0].session,
        SessionRef::new(Some(session_id)).with_replay_cursor(
            snapshot_before_resume
                .replay_cursor
                .expect("snapshot should retain replay cursor")
        )
    );
}
