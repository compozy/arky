//! Route handlers for the runtime server.

use arky_types::AgentEvent;

pub mod chat;
pub mod events;
pub mod health;
pub mod models;
pub mod replay;
pub mod sessions;
pub mod sse;

const fn sse_event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::AgentStart { .. } => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart { .. } => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::ReasoningStart { .. } => "reasoning_start",
        AgentEvent::ReasoningDelta { .. } => "reasoning_delta",
        AgentEvent::ReasoningComplete { .. } => "reasoning_complete",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
        AgentEvent::Custom { .. } => "custom",
        _ => "unknown",
    }
}
