//! Shared protocol types for the Arky SDK.
//!
//! The protocol crate defines the durable message, event, identifier, request,
//! session, and tool data structures shared across every other workspace crate.

mod event;
mod id;
mod message;
mod request;
mod session;
mod tool;

pub use crate::{
    event::{
        AgentEvent,
        EventMetadata,
        StreamDelta,
    },
    id::{
        ProviderId,
        SessionId,
        TurnId,
    },
    message::{
        ContentBlock,
        Message,
        MessageBuilder,
        MessageMetadata,
        Role,
    },
    request::{
        AgentResponse,
        ErrorPayload,
        GenerateResponse,
        HookContext,
        InputTokenDetails,
        ModelRef,
        OutputTokenDetails,
        ProviderRequest,
        ProviderSettings,
        SessionRef,
        ToolContext,
        ToolDefinition,
        TurnContext,
        Usage,
    },
    session::{
        PersistedEvent,
        ReplayCursor,
        TurnCheckpoint,
    },
    tool::{
        ToolCall,
        ToolContent,
        ToolResult,
    },
};
