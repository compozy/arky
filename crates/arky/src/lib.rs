//! User-facing facade for the Arky SDK.
//!
//! The `arky` crate is the primary entrypoint for consumers that want a single
//! dependency and a curated public API. It provides:
//!
//! - focused top-level re-exports for the most common SDK types;
//! - namespaced access to the underlying workspace crates via modules such as
//!   [`crate::core`], [`crate::provider`], and [`crate::tools`];
//! - [`crate::prelude`] for the ergonomic `use arky::prelude::*` workflow.
//!
//! # Feature Flags
//!
//! - `claude-code`: includes the Claude Code provider surface.
//! - `codex`: includes the Codex provider surface.
//! - `sqlite`: exposes the `SQLite` session backend.
//! - `server`: exposes the HTTP/SSE runtime server surface.
//! - `full`: enables every optional provider and backend feature.
//!
//! # Example
//!
//! ```rust
//! use arky::prelude::*;
//!
//! let _builder = Agent::builder();
//! let _message = Message::user("hello");
//! let _descriptor: Option<ToolDescriptor> = None;
//! let _event: Option<AgentEvent> = None;
//!
//! fn _provider(_: Option<&dyn Provider>) {}
//! fn _tool(_: Option<&dyn Tool>) {}
//! fn _session_store(_: Option<&dyn SessionStore>) {}
//!
//! _provider(None);
//! _tool(None);
//! _session_store(None);
//! ```

pub mod error;
pub mod prelude;

pub use crate::error::ArkyError;
pub use arky_config as config;
pub use arky_config::{
    AgentConfig,
    AgentConfigBuilder,
    ArkyConfig,
    ArkyConfigBuilder,
    ConfigError,
    ConfigFormat,
    ConfigLoader,
    ProviderConfig,
    ProviderConfigBuilder,
    ValidationIssue,
    WorkspaceConfig,
    WorkspaceConfigBuilder,
};
pub use arky_core as core;
pub use arky_core::{
    Agent,
    AgentBuilder,
    AgentEventStream,
    CoreError,
    EventSubscription,
};
pub use arky_error::{
    ClassifiedError,
    ErrorLogEntry,
    HttpErrorMapping,
    classify_error,
};
pub use arky_hooks as hooks;
pub use arky_hooks::{
    FailureMode,
    HookChain,
    HookChainConfig,
    HookDiagnostic,
    HookError,
    Hooks,
    PromptUpdate,
    SessionStartUpdate,
    ShellCommandHook,
    StopDecision,
    ToolMatcher,
    ToolResultOverride,
    Verdict,
};
pub use arky_mcp as mcp;
pub use arky_mcp::{
    ConnectionState,
    McpAuth,
    McpClient,
    McpClientConfig,
    McpError,
    McpHttpClientConfig,
    McpHttpServerHandle,
    McpOAuthAuth,
    McpOAuthFlow,
    McpOAuthOptions,
    McpServer,
    McpServerHandle,
    McpServerTransport,
    McpStdioClientConfig,
    McpStdioServerHandle,
    McpToolBridge,
    McpToolBridgeBuilder,
};
pub use arky_protocol as protocol;
pub use arky_protocol::{
    AgentEvent,
    AgentResponse,
    ContentBlock,
    EventMetadata,
    GenerateResponse,
    Message,
    MessageBuilder,
    MessageMetadata,
    ModelRef,
    PersistedEvent,
    ProviderId,
    ProviderRequest,
    ProviderSettings,
    ReplayCursor,
    Role,
    SessionId,
    SessionRef,
    StreamDelta,
    ToolCall,
    ToolContent,
    ToolContext,
    ToolDefinition,
    ToolResult,
    TurnCheckpoint,
    TurnContext,
    TurnId,
    Usage,
};
pub use arky_provider as provider;
pub use arky_provider::{
    ManagedProcess,
    ProcessConfig,
    ProcessManager,
    Provider,
    ProviderCapabilities,
    ProviderContractCase,
    ProviderContractTests,
    ProviderDescriptor,
    ProviderError,
    ProviderEventStream,
    ProviderFamily,
    ProviderRegistry,
    ReplayWriter,
    ReplayWriterConfig,
    RestartPolicy,
    StdioTransport,
    StdioTransportConfig,
    generate_response_from_stream,
};
pub use arky_session as session;
pub use arky_session::{
    InMemorySessionStore,
    InMemorySessionStoreConfig,
    NewSession,
    SessionError,
    SessionFilter,
    SessionMetadata,
    SessionSnapshot,
    SessionStore,
};
pub use arky_tools as tools;
pub use arky_tools::{
    ParsedCanonicalToolName,
    ParsedProviderToolName,
    StaticToolIdCodec,
    Tool,
    ToolDescriptor,
    ToolError,
    ToolIdCodec,
    ToolOrigin,
    ToolRegistrationHandle,
    ToolRegistry,
    build_canonical_tool_name,
    create_claude_code_tool_id_codec,
    create_codex_tool_id_codec,
    create_opencode_tool_id_codec,
    parse_canonical_tool_name,
    validate_canonical_segment,
    validate_canonical_tool_name,
};
pub use arky_tools_macros as macros;
pub use arky_tools_macros::tool;
pub use arky_usage as usage;
pub use arky_usage::{
    ModelCost,
    NormalizedUsage,
    ProviderMetadata,
    ProviderMetadataExtractor,
    UsageAggregator,
};

#[cfg(feature = "claude-code")]
pub use arky_claude_code as claude_code;
#[cfg(feature = "claude-code")]
pub use arky_claude_code::{
    ClaudeCodeProvider,
    ClaudeCodeProviderConfig,
};

#[cfg(feature = "codex")]
pub use arky_codex as codex;
#[cfg(feature = "codex")]
pub use arky_codex::{
    CodexProvider,
    CodexProviderConfig,
};

#[cfg(feature = "sqlite")]
pub use arky_session::{
    SqliteSessionStore,
    SqliteSessionStoreConfig,
};

#[cfg(feature = "server")]
pub use arky_server as server;
#[cfg(feature = "server")]
pub use arky_server::{
    ComponentHealth,
    HealthStatus,
    ProviderHealthSnapshot,
    ReadinessSnapshot,
    RuntimeHealthRegistry,
    ServerError,
    ServerHandle,
    ServerState,
    SessionCompatibility,
    router,
    serve,
};
