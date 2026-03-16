//! # 10 Steering Follow-up
//!
//! Demonstrates the two mid-conversation control points exposed by `Agent`:
//! steering while a turn is in flight and follow-up scheduling after a turn
//! completes.

mod common;

use std::{
    sync::Arc,
    time::Duration,
};

use arky::{
    AgentEvent,
    ContentBlock,
    ProviderCapabilities,
    ProviderDescriptor,
    ProviderError,
    ProviderEventStream,
    ProviderFamily,
    ProviderId,
    ProviderRequest,
    Role,
    ToolCall,
    ToolContent,
    ToolOrigin,
    prelude::*,
};
use async_trait::async_trait;
use common::{
    EchoProvider,
    ExampleError,
    final_message_stream,
    text_from_message,
};
use futures::StreamExt;
use serde_json::json;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

struct PauseTool {
    descriptor: ToolDescriptor,
    started: Arc<Notify>,
    release: Arc<Notify>,
}

impl PauseTool {
    fn new(started: Arc<Notify>, release: Arc<Notify>) -> Result<Self, ToolError> {
        Ok(Self {
            descriptor: ToolDescriptor::new(
                "mcp/local/pause",
                "Pause",
                "Pause until the example releases the tool.",
                json!({
                    "type": "object",
                    "properties": {},
                }),
                ToolOrigin::Local,
            )?,
            started,
            release,
        })
    }
}

#[async_trait]
impl Tool for PauseTool {
    fn descriptor(&self) -> ToolDescriptor {
        self.descriptor.clone()
    }

    async fn execute(
        &self,
        call: ToolCall,
        cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError> {
        self.started.notify_waiters();

        tokio::select! {
            () = cancel.cancelled() => Err(ToolError::cancelled(
                "pause tool was cancelled",
                Some(call.name.clone()),
            )),
            () = self.release.notified() => Ok(ToolResult::success(
                call.id,
                call.name,
                vec![ToolContent::text("released")],
            )),
        }
    }
}

#[derive(Clone)]
struct SteeringProvider {
    descriptor: ProviderDescriptor,
}

impl SteeringProvider {
    fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("steering-demo"),
                ProviderFamily::Custom("steering-demo".to_owned()),
                ProviderCapabilities::new()
                    .with_streaming(true)
                    .with_generate(true)
                    .with_tool_calls(true)
                    .with_steering(true)
                    .with_follow_up(true),
            ),
        }
    }
}

#[async_trait]
impl Provider for SteeringProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError> {
        let message = if request
            .messages
            .iter()
            .any(|message| message.role == Role::System)
        {
            Message::assistant("steered response")
        } else if request
            .messages
            .iter()
            .any(|message| message.role == Role::Tool)
        {
            Message::assistant("tool completed without steering")
        } else {
            Message::builder(Role::Assistant)
                .block(ContentBlock::tool_use(ToolCall::new(
                    "call-1",
                    "mcp/local/pause",
                    json!({}),
                )))
                .build()
        };

        Ok(final_message_stream(&request, &self.descriptor.id, message))
    }
}

fn final_turn_message_text(event: &AgentEvent) -> Option<String> {
    match event {
        AgentEvent::TurnEnd { message, .. } => Some(text_from_message(message)),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());

    let steering_agent = Agent::builder()
        .provider(SteeringProvider::new())
        .model("demo-model")
        .tool(PauseTool::new(started.clone(), release.clone())?)
        .build()?;

    let mut stream = steering_agent.stream("start a tool-heavy turn").await?;

    tokio::time::timeout(Duration::from_secs(2), started.notified()).await?;
    steering_agent
        .steer("interrupt and answer directly")
        .await?;
    release.notify_waiters();

    let mut steering_result = None;
    while let Some(item) = stream.next().await {
        let event = item?;
        if let Some(text) = final_turn_message_text(&event) {
            steering_result = Some(text);
        }
    }

    println!(
        "steering result: {}",
        steering_result.unwrap_or_else(|| "missing turn output".to_owned())
    );

    let follow_up_agent = Agent::builder()
        .provider(EchoProvider::new("follow-up-demo", "follow-up"))
        .model("demo-model")
        .build()?;
    let first = follow_up_agent.prompt("first turn").await?;
    let mut subscription = follow_up_agent.subscribe();

    follow_up_agent.follow_up("second turn").await?;

    let mut follow_up_result = None;
    loop {
        let event = subscription.recv().await?;
        if let Some(text) = final_turn_message_text(&event) {
            follow_up_result = Some(text);
        }
        if matches!(event, AgentEvent::AgentEnd { .. }) {
            break;
        }
    }

    println!("first turn: {}", text_from_message(&first.message));
    println!(
        "follow-up result: {}",
        follow_up_result.unwrap_or_else(|| "missing follow-up output".to_owned())
    );

    Ok(())
}
