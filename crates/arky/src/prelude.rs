//! Curated imports for typical Arky applications.
//!
//! The prelude keeps the import surface intentionally small: it exposes the
//! high-level agent API, the core provider/tool/session traits, and the shared
//! protocol items that most applications touch directly.

pub use crate::{
    Agent,
    AgentBuilder,
    AgentEvent,
    ArkyError,
    ClassifiedError,
    GenerateResponse,
    HookChain,
    HookError,
    Hooks,
    Message,
    MessageBuilder,
    Provider,
    ProviderCapabilities,
    ProviderDescriptor,
    ProviderError,
    ProviderFamily,
    ProviderRequest,
    SessionId,
    SessionStore,
    Tool,
    ToolDescriptor,
    ToolError,
    ToolResult,
    TurnId,
    tool,
};
