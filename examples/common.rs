#![allow(dead_code)]

use std::error::Error;

use arky::{
    AgentEvent,
    ContentBlock,
    EventMetadata,
    Message,
    Provider,
    ProviderCapabilities,
    ProviderDescriptor,
    ProviderError,
    ProviderEventStream,
    ProviderFamily,
    ProviderId,
    ProviderRequest,
    Role,
    ToolContent,
};
use async_trait::async_trait;
use futures::stream;

pub type ExampleError = Box<dyn Error + Send + Sync>;

pub fn text_from_message(message: &Message) -> String {
    let mut text = String::new();

    for block in &message.content {
        match block {
            ContentBlock::Text { text: fragment } => text.push_str(fragment),
            ContentBlock::ToolResult { result } => {
                for item in &result.content {
                    if let ToolContent::Text { text: fragment } = item {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(fragment);
                    }
                }
            }
            ContentBlock::ToolUse { .. } | ContentBlock::Image { .. } => {}
        }
    }

    text
}

pub fn last_text_message(request: &ProviderRequest, role: Role) -> Option<String> {
    request.messages.iter().rev().find_map(|message| {
        if message.role == role {
            let text = text_from_message(message);
            if !text.is_empty() {
                return Some(text);
            }
        }

        None
    })
}

pub fn describe_event(event: &AgentEvent) -> String {
    match event {
        AgentEvent::AgentStart { meta } => format!("agent_start#{}", meta.sequence),
        AgentEvent::AgentEnd { meta, .. } => format!("agent_end#{}", meta.sequence),
        AgentEvent::TurnStart { meta } => format!("turn_start#{}", meta.sequence),
        AgentEvent::TurnEnd { meta, .. } => format!("turn_end#{}", meta.sequence),
        AgentEvent::MessageStart { meta, .. } => {
            format!("message_start#{}", meta.sequence)
        }
        AgentEvent::MessageUpdate { meta, delta, .. } => {
            format!("message_update#{}::{delta:?}", meta.sequence)
        }
        AgentEvent::MessageEnd { meta, .. } => format!("message_end#{}", meta.sequence),
        AgentEvent::ToolExecutionStart {
            meta, tool_name, ..
        } => format!("tool_execution_start#{}::{tool_name}", meta.sequence),
        AgentEvent::ToolExecutionUpdate {
            meta, tool_name, ..
        } => format!("tool_execution_update#{}::{tool_name}", meta.sequence),
        AgentEvent::ToolExecutionEnd {
            meta, tool_name, ..
        } => format!("tool_execution_end#{}::{tool_name}", meta.sequence),
        AgentEvent::Custom {
            meta, event_type, ..
        } => format!("custom#{}::{event_type}", meta.sequence),
        _ => "unknown".to_owned(),
    }
}

pub const fn agent_capabilities() -> ProviderCapabilities {
    ProviderCapabilities::new()
        .with_streaming(true)
        .with_generate(true)
        .with_tool_calls(true)
        .with_session_resume(true)
        .with_steering(true)
        .with_follow_up(true)
}

pub fn final_message_stream(
    request: &ProviderRequest,
    provider_id: &ProviderId,
    message: Message,
) -> ProviderEventStream {
    let message_end_meta = event_metadata(request, provider_id, 1, 1);
    let turn_end_meta = event_metadata(request, provider_id, 2, 2);

    Box::pin(stream::iter(vec![
        Ok(AgentEvent::MessageEnd {
            meta: message_end_meta,
            message: message.clone(),
        }),
        Ok(AgentEvent::TurnEnd {
            meta: turn_end_meta,
            message,
            tool_results: Vec::new(),
        }),
    ]))
}

fn event_metadata(
    request: &ProviderRequest,
    provider_id: &ProviderId,
    timestamp_ms: u64,
    sequence: u64,
) -> EventMetadata {
    let mut meta =
        EventMetadata::new(timestamp_ms, sequence).with_provider_id(provider_id.clone());

    if let Some(session_id) = request.session.id.clone() {
        meta = meta.with_session_id(session_id);
    }

    meta.with_turn_id(request.turn.id.clone())
}

#[derive(Clone)]
pub struct EchoProvider {
    descriptor: ProviderDescriptor,
    prefix: &'static str,
}

impl EchoProvider {
    pub fn new(id: &str, prefix: &'static str) -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new(id),
                ProviderFamily::Custom(id.to_owned()),
                agent_capabilities(),
            ),
            prefix,
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
        let user_text = last_text_message(&request, Role::User)
            .unwrap_or_else(|| "empty prompt".to_owned());
        let system_text = last_text_message(&request, Role::System);
        let assistant_text = system_text
            .into_iter()
            .map(|system_text| {
                format!("{}: {} [{}]", self.prefix, user_text, system_text)
            })
            .next()
            .unwrap_or_else(|| format!("{}: {user_text}", self.prefix));
        let message = Message::assistant(assistant_text);

        Ok(final_message_stream(&request, &self.descriptor.id, message))
    }
}
