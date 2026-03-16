# Deep Analysis Report: codex-rs Rust Implementation

## 1. Executive Summary

codex-rs is OpenAI's official Codex CLI rewritten entirely in Rust. It is a production-grade, native AI agent runtime that communicates with OpenAI's Responses API (both SSE and WebSocket transports). The workspace contains **71 crates** organized in a Cargo workspace using Rust Edition 2024. It implements a full agent loop with tool execution, sandboxing, MCP client/server support, multi-agent orchestration, and a terminal UI (via Ratatui).

**Key architectural patterns:**

- **SQ/EQ (Submission Queue / Event Queue)** protocol for async user-agent communication
- **Session-based agent loop** with per-turn streaming via SSE or WebSocket
- **Pluggable provider system** with built-in OpenAI + OSS (Ollama, LM Studio) support
- **First-class MCP integration** via the `rmcp` crate (both client and server)
- **Multi-agent orchestration** with agent spawning, control planes, and inter-agent messaging
- **OS-level sandboxing** (macOS Seatbelt, Linux Landlock/seccomp, Windows sandbox)
- **Comprehensive tool system** with a registry, router, parallel execution, and approval workflows

---

## 2. Workspace Structure

### Core Crates (Agent Logic)

| Crate            | Path            | Purpose                                                                                               |
| ---------------- | --------------- | ----------------------------------------------------------------------------------------------------- |
| `codex-core`     | `core/`         | Main agent business logic: session loop, tool dispatch, streaming, MCP management, config, sandboxing |
| `codex-protocol` | `protocol/`     | Shared protocol types: Op/Event enums, models, items, approvals, permissions                          |
| `codex-api`      | `codex-api/`    | API client layer: SSE streaming, WebSocket, Responses API, Realtime API                               |
| `codex-client`   | `codex-client/` | Low-level HTTP transport: reqwest wrapper, retry, SSE parsing, custom CA                              |
| `codex-config`   | `config/`       | Configuration loading/merging from TOML layers (user, project, cloud requirements)                    |
| `codex-state`    | `state/`        | SQLite-backed persistence: rollout metadata, thread state, agent jobs, memories                       |

### Infrastructure Crates

| Crate                       | Path                   | Purpose                                                                     |
| --------------------------- | ---------------------- | --------------------------------------------------------------------------- |
| `codex-app-server`          | `app-server/`          | JSON-RPC server (stdio + WebSocket) bridging IDE extensions to codex-core   |
| `codex-app-server-protocol` | `app-server-protocol/` | JSON-RPC message types for the app server                                   |
| `codex-rmcp-client`         | `rmcp-client/`         | MCP client wrapper: stdio + streamable HTTP transports, OAuth, tool listing |
| `codex-exec`                | `exec/`                | Headless CLI for non-interactive `codex exec` automation                    |
| `codex-hooks`               | `hooks/`               | Hook system: session-start, stop, after-agent, after-tool-use events        |
| `codex-connectors`          | `connectors/`          | App/connector directory listing and caching for ChatGPT apps                |
| `codex-shell-command`       | `shell-command/`       | Shell command parsing, safety classification                                |
| `codex-execpolicy`          | `execpolicy/`          | Execution policy engine for command allow/deny rules                        |

### User-Facing Crates

| Crate              | Path          | Purpose                                                                       |
| ------------------ | ------------- | ----------------------------------------------------------------------------- |
| `codex-cli`        | `cli/`        | CLI multitool with subcommands (interactive, exec, mcp-server, sandbox, etc.) |
| `codex-tui`        | `tui/`        | Full-screen terminal UI built on Ratatui                                      |
| `codex-mcp-server` | `mcp-server/` | Run Codex as an MCP server for other agents                                   |

### Provider/Integration Crates

| Crate                  | Path              | Purpose                                         |
| ---------------------- | ----------------- | ----------------------------------------------- |
| `codex-ollama`         | `ollama/`         | Ollama-specific model listing and configuration |
| `codex-lmstudio`       | `lmstudio/`       | LM Studio integration                           |
| `codex-backend-client` | `backend-client/` | Backend API client for OpenAI services          |

### Utility Crates (20+ under `utils/`)

`absolute-path`, `cache`, `cargo-bin`, `cli`, `elapsed`, `fuzzy-match`, `git`, `home-dir`, `image`, `json-to-toml`, `oss`, `pty`, `readiness`, `rustls-provider`, `sandbox-summary`, `sleep-inhibitor`, `stream-parser`, `string`, `approval-presets`

---

## 3. Core Agent Architecture

### 3.1 The `Codex` Struct (Session Owner)

**File**: `core/src/codex.rs`

The `Codex` struct is the central runtime. It operates as a **queue pair**:

```
User -> Sender<Submission> -> [Session Loop] -> Receiver<Event> -> User
```

Key fields:

- `tx_sub: Sender<Submission>` -- inbound operation queue
- `rx_event: Receiver<Event>` -- outbound event queue
- `agent_status: watch::Receiver<AgentStatus>` -- reactive status
- `session: Arc<Session>` -- shared session state
- `session_loop_termination: Shared<BoxFuture<'static, ()>>` -- shutdown handle

### 3.2 Session Lifecycle

1. **Spawn**: `Codex::spawn(CodexSpawnArgs)` creates a new session with config, auth, models manager, skills manager, plugins manager, MCP manager, file watcher, and initial history.
2. **Session Loop**: Background tokio task processes submissions sequentially, handling `Op::UserInput`, `Op::UserTurn`, `Op::ExecApproval`, `Op::Interrupt`, etc.
3. **Turn Execution**: Each turn creates a `TurnContext`, builds a `Prompt` (with tools, instructions, conversation history), streams a response from the model, and processes tool calls.
4. **Tool Dispatch**: Tool calls are routed through `ToolRouter` -> `ToolRegistry` -> individual handlers (shell exec, MCP tools, function tools, custom tools).
5. **Event Emission**: Results are emitted as `Event` messages through the event queue.

### 3.3 Thread Management

**File**: `core/src/thread_manager.rs`

`ThreadManager` manages multiple concurrent agent threads:

- Creates `CodexThread` instances (each wrapping a `Codex`)
- Handles thread lifecycle (create, resume, shutdown)
- Provides `NewThread` struct with thread ID, thread handle, and initial `SessionConfiguredEvent`
- Supports multi-agent spawning via `AgentControl`

### 3.4 Multi-Agent Control

**File**: `core/src/agent/control.rs`

`AgentControl` provides the multi-agent orchestration plane:

- `spawn_agent()` -- creates a new sub-agent thread with its own config
- Uses `Weak<ThreadManagerState>` to avoid reference cycles
- `Guards` struct enforces thread spawn depth limits
- Agent nicknames assigned from a list for identification
- Sub-agents can fork parent conversation history

---

## 4. Protocol and API Types

### 4.1 Submission/Event Protocol

**File**: `protocol/src/protocol.rs`

Uses a **SQ/EQ (Submission Queue / Event Queue)** pattern:

**Submissions (User -> Agent):**

```rust
pub struct Submission {
    pub id: String,
    pub op: Op,
    pub trace: Option<W3cTraceContext>,
}

pub enum Op {
    Interrupt,
    UserInput { items: Vec<UserInput>, final_output_json_schema: Option<Value> },
    UserTurn { items, cwd, approval_policy, sandbox_policy, model, effort, ... },
    OverrideTurnContext { cwd, approval_policy, sandbox_policy, model, ... },
    ExecApproval { id, turn_id, decision },
    PatchApproval { id, decision },
    ResolveElicitation { server_name, request_id, decision, content, meta },
    UserInputAnswer { id, response },
    RequestPermissionsResponse { id, response },
    RealtimeConversationStart(ConversationStartParams),
    RealtimeConversationAudio(ConversationAudioParams),
    // ... more ops
}
```

**Events (Agent -> User):**

- `SessionConfigured`, `TurnStarted`, `TurnCompleted`, `TurnAborted`
- `AgentMessageContentDelta`, `AgentReasoning`, `PlanDelta`
- `ExecApprovalRequest`, `PatchApprovalRequest`, `ElicitationRequest`
- `TokenCount`, `Error`, `Warning`, `TurnDiff`, `FileChange`
- `McpServerStatus`, `WebSearchEnd`, `ImageGenerationEnd`

### 4.2 Response Items (Model Types)

**File**: `protocol/src/models.rs`

Core model types for LLM communication:

- `ResponseItem` -- union of all item types the model can emit (Message, FunctionCall, FunctionCallOutput, LocalShellCall, CustomToolCall, WebSearchCall, etc.)
- `ResponseInputItem` -- items that go INTO the model (Message, FunctionCallOutput, etc.)
- `ContentItem` -- text, image, file content items
- `BaseInstructions` -- developer instructions for the model
- `SandboxPermissions` -- per-command sandbox override requests

### 4.3 Turn Items (UI Types)

**File**: `protocol/src/items.rs`

High-level turn items for UI rendering:

```rust
pub enum TurnItem {
    UserMessage(UserMessageItem),
    AgentMessage(AgentMessageItem),
    Plan(PlanItem),
    Reasoning(ReasoningItem),
    WebSearch(WebSearchItem),
    ImageGeneration(ImageGenerationItem),
    ContextCompaction(ContextCompactionItem),
}
```

---

## 5. Streaming Architecture

### 5.1 Transport Layer

**File**: `codex-client/src/transport.rs`

The `HttpTransport` trait abstracts HTTP communication:

```rust
#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn execute(&self, req: Request) -> Result<Response, TransportError>;
    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError>;
}
```

`ReqwestTransport` implements this with:

- Request body compression (zstd)
- Custom CA certificate support
- Streaming via `bytes_stream()` -> `BoxStream<Result<Bytes, TransportError>>`

### 5.2 SSE Streaming

**Files**: `codex-api/src/sse/responses.rs`, `codex-api/src/endpoint/responses.rs`

The `ResponsesClient<T: HttpTransport, A: AuthProvider>` handles SSE:

1. Builds JSON request body from `ResponsesApiRequest`
2. Sends POST to `/responses` with `Accept: text/event-stream`
3. `spawn_response_stream()` creates a background task that parses SSE events
4. Returns `ResponseStream` (wrapper around `mpsc::Receiver<Result<ResponseEvent>>`)

### 5.3 WebSocket Streaming

**File**: `codex-api/src/endpoint/responses_websocket.rs`

`ResponsesWebsocketClient` manages WebSocket connections:

- Uses `tokio-tungstenite` with TLS and compression (deflate)
- `WsStream` wraps the WebSocket with command/message channels
- Supports WebSocket prewarm (`response.create` with `generate=false`)
- Per-turn `WsCommand::Send` / `rx_message` pattern
- Sticky routing via `x-codex-turn-state` header

### 5.4 Realtime WebSocket (Audio)

**File**: `codex-api/src/endpoint/realtime_websocket/`

Separate WebSocket client for real-time audio conversations:

- Protocol v1 and v2 support
- `RealtimeSessionConfig` for session parameters
- Audio frame streaming (`RealtimeAudioFrame`)
- Transcript delta events
- Handoff mechanism for transitioning between audio and text

### 5.5 Response Event Processing

**File**: `core/src/client_common.rs`

`ResponseStream` implements `futures::Stream`:

```rust
pub struct ResponseStream {
    rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}
impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;
    fn poll_next(...) -> Poll<Option<Self::Item>> { self.rx_event.poll_recv(cx) }
}
```

---

## 6. MCP Integration

### 6.1 MCP Client (`rmcp-client`)

**File**: `rmcp-client/src/rmcp_client.rs`

`RmcpClient` wraps the `rmcp` crate (version 0.15.0):

**Transport modes:**

- **stdio**: Spawns MCP server as a child process, communicates via stdin/stdout
- **Streamable HTTP**: Connects to HTTP-based MCP servers with SSE responses

**Key capabilities:**

- `list_tools()` -> `ListToolsResult` with connector IDs
- `call_tool()` -> `CallToolResult`
- `list_resources()` / `read_resource()`
- `list_resource_templates()`
- OAuth authentication flow for HTTP MCP servers
- Elicitation request/response handling

**OAuth flow:**

- `auth_status.rs`: Discovers OAuth capabilities via `determine_streamable_http_auth_status()`
- `oauth.rs`: Token persistence with `OAuthCredentialsStoreMode` (keyring or file)
- `perform_oauth_login.rs`: Full OAuth login flow with browser redirect

### 6.2 MCP Connection Manager

**File**: `core/src/mcp_connection_manager.rs`

Manages multiple MCP server connections:

- Starts/stops MCP server processes
- Caches tool listings per server
- Handles tool refresh (soft and hard)
- Sandbox state propagation to MCP servers
- Tool plugin provenance tracking (which connector provides which tool)

### 6.3 MCP Server Mode

**Crate**: `mcp-server/`

Codex can run as an MCP server itself (`codex mcp-server`), exposing its capabilities as tools for other agents.

---

## 7. Tool System

### 7.1 Tool Specification

**File**: `core/src/client_common.rs` (tools module)

Tool types sent to the model:

```rust
enum ToolSpec {
    Function(ResponsesApiTool),       // Standard function tools
    ToolSearch { execution, description, parameters },
    LocalShell {},                     // Shell execution
    ImageGeneration { output_format }, // Image generation
    WebSearch { external_web_access, filters, ... },
    Freeform(FreeformTool),           // Custom format tools (e.g., apply_patch)
}
```

### 7.2 Tool Router

**File**: `core/src/tools/router.rs`

`ToolRouter` is the central dispatch:

```rust
pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
    model_visible_specs: Vec<ToolSpec>,
}
```

**Dispatch flow:**

1. `build_tool_call(session, item)` -- parses `ResponseItem` into `ToolCall` with payload
2. `dispatch_tool_call(session, turn, tracker, call, source)` -- routes to handler
3. Returns `ResponseInputItem` (function call output to feed back to model)

**Tool payloads:**

- `ToolPayload::Function { arguments }` -- standard function call
- `ToolPayload::Mcp { server, tool, raw_arguments }` -- MCP server tool
- `ToolPayload::LocalShell { params }` -- shell execution
- `ToolPayload::Custom { input }` -- freeform tool
- `ToolPayload::ToolSearch { arguments }` -- tool discovery

### 7.3 Tool Modules

```
tools/
  code_mode.rs          -- Code mode (JS REPL) tools
  context.rs            -- ToolInvocation, ToolPayload, SharedTurnDiffTracker
  discoverable.rs       -- Discoverable tools (dynamic tool search)
  events.rs             -- Tool execution events
  handlers/             -- Individual tool handlers
  js_repl.rs            -- JavaScript REPL integration
  network_approval.rs   -- Network access approval service
  orchestrator.rs       -- Tool orchestration
  parallel.rs           -- Parallel tool execution runtime
  registry.rs           -- ToolRegistry: maps tool names to handlers
  router.rs             -- ToolRouter: dispatch layer
  runtimes.rs           -- Execution runtimes
  sandboxing.rs         -- Sandbox enforcement, ApprovalStore
  spec.rs               -- ToolsConfig, tool spec building
```

---

## 8. Configuration

### 8.1 Config Loading

**File**: `config/src/lib.rs`

Multi-layer TOML configuration with precedence:

1. **Built-in defaults** (compiled in)
2. **User config** (`~/.codex/config.toml`)
3. **Project config** (`.codex/config.toml` in project root)
4. **Cloud requirements** (fetched from OpenAI servers)
5. **CLI overrides** (`-c key=value`)

Key types:

- `ConfigLayerStack` -- ordered stack of config layers
- `ConfigRequirements` -- constraints from cloud/organization
- `Constrained<T>` -- value with constraint enforcement
- `ConfigError` with diagnostic positions for error reporting

### 8.2 Model Provider Info

**File**: `core/src/model_provider_info.rs`

```rust
pub struct ModelProviderInfo {
    pub name: String,
    pub base_url: Option<String>,
    pub env_key: Option<String>,
    pub wire_api: WireApi,           // Only "responses" supported now
    pub query_params: Option<HashMap<String, String>>,
    pub http_headers: Option<HashMap<String, String>>,
    pub env_http_headers: Option<HashMap<String, String>>,
    pub request_max_retries: Option<u64>,
    pub stream_max_retries: Option<u64>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub requires_openai_auth: bool,
    pub supports_websockets: bool,
}
```

**Built-in providers:**

- `openai` -- OpenAI API (default, with auth and WebSocket support)
- `ollama` -- Local Ollama (port 11434)
- `lmstudio` -- Local LM Studio (port 1234)

Users can add custom providers in `config.toml` under `model_providers`.

---

## 9. State Management

### 9.1 Session State

**File**: `core/src/state.rs`

`SessionState` holds per-session mutable state:

- Active turn tracking (`ActiveTurn`)
- Conversation history
- Token usage counters
- Exec policy state
- Shell snapshots

`SessionServices` holds shared services:

- `ModelClient` for API calls
- `AgentControl` for multi-agent ops
- `AnalyticsEventsClient`
- `AuthManager`

### 9.2 Persistent State (SQLite)

**File**: `state/src/lib.rs`

`StateRuntime` manages SQLite-backed persistence:

- **Thread metadata**: thread IDs, names, summaries, timestamps
- **Agent jobs**: long-running task tracking
- **Backfill**: scanning JSONL rollouts into SQLite
- **Logs database**: structured logging
- **Memories**: persistent agent memories

---

## 10. Hook System

**File**: `hooks/src/lib.rs`

Event-driven hook system for extensibility:

```rust
pub enum HookEvent {
    SessionStart(SessionStartRequest),
    Stop(StopRequest),
    AfterAgent(HookEventAfterAgent),
    AfterToolUse(HookEventAfterToolUse),
}
```

- Hooks are shell commands configured in `config.toml`
- Receive JSON payloads via stdin
- Can return structured responses to influence agent behavior
- `HookResult` carries success/failure status

---

## 11. Error Handling Patterns

### 11.1 Core Error Type

**File**: `core/src/error.rs`

`CodexErr` is an exhaustive tagged enum using `thiserror`:

```rust
pub enum CodexErr {
    TurnAborted,
    Stream(String, Option<Duration>),
    ContextWindowExceeded,
    ThreadNotFound(ThreadId),
    AgentLimitReached { max_threads },
    Timeout,
    Spawn,
    Interrupted,
    UnexpectedStatus(UnexpectedResponseError),
    InvalidRequest(String),
    UsageLimitReached(UsageLimitReachedError),
    ServerOverloaded,
    ConnectionFailed(ConnectionFailedError),
    QuotaExceeded,
    RetryLimit(RetryLimitReachedError),
    Sandbox(SandboxErr),
    Fatal(String),
    Io(io::Error),
    Json(serde_json::Error),
    // ... platform-specific variants
}
```

Key patterns:

- `is_retryable()` method classifies errors for retry logic
- `to_codex_protocol_error()` maps to client-facing `CodexErrorInfo` enum
- `to_error_event()` creates protocol-level error events
- Separate `SandboxErr` for sandbox-specific failures

### 11.2 API Error Type

**File**: `codex-api/src/error.rs`

```rust
pub enum ApiError {
    Transport(TransportError),
    Api { status, message },
    Stream(String),
    ContextWindowExceeded,
    QuotaExceeded,
    UsageNotIncluded,
    Retryable { message, delay },
    RateLimit(String),
    InvalidRequest { message },
    ServerOverloaded,
}
```

### 11.3 Transport Error

**File**: `codex-client/src/error.rs`

```rust
pub enum TransportError {
    Network(String),
    Timeout,
    Http { status, url, headers, body },
    Build(String),
}
```

---

## 12. Provider Connectors

### 12.1 API Provider

**File**: `codex-api/src/provider.rs`

```rust
pub struct Provider {
    pub name: String,
    pub base_url: String,
    pub query_params: Option<HashMap<String, String>>,
    pub headers: HeaderMap,
    pub retry: RetryConfig,
    pub stream_idle_timeout: Duration,
}
```

Methods:

- `url_for_path(path)` -- constructs full URL with query params
- `build_request(method, path)` -- creates `Request` struct
- `websocket_url_for_path(path)` -- converts HTTP URL to WebSocket URL
- `is_azure_responses_endpoint()` -- detects Azure OpenAI deployments

### 12.2 Auth Modes

- **API Key**: via `env_key` environment variable
- **OpenAI Auth (ChatGPT)**: OAuth-based login with token refresh
- **Bearer Token**: direct token in config (for programmatic use)
- **Azure**: automatic detection of Azure OpenAI endpoints

### 12.3 Wire API

Only `WireApi::Responses` is supported. The former `chat` wire API has been removed with a clear migration error message.

---

## 13. Key Dependencies

| Dependency             | Version | Purpose                            |
| ---------------------- | ------- | ---------------------------------- |
| `tokio`                | 1       | Async runtime                      |
| `reqwest`              | 0.12    | HTTP client                        |
| `serde` / `serde_json` | 1       | Serialization                      |
| `thiserror`            | 2.0.17  | Error derive macros                |
| `anyhow`               | 1       | Error context in some crates       |
| `futures`              | 0.3     | Async stream utilities             |
| `tokio-tungstenite`    | 0.28.0  | WebSocket client                   |
| `rmcp`                 | 0.15.0  | MCP protocol (Rust implementation) |
| `axum`                 | 0.8     | HTTP server framework (app-server) |
| `ratatui`              | 0.29.0  | Terminal UI framework              |
| `clap`                 | 4       | CLI argument parsing               |
| `tracing`              | 0.1.44  | Structured logging/tracing         |
| `opentelemetry`        | 0.31.0  | Observability/telemetry            |
| `sqlx`                 | 0.8.6   | SQLite database driver             |
| `schemars`             | 0.8.22  | JSON Schema generation             |
| `ts-rs`                | 11      | TypeScript type generation         |
| `crossterm`            | 0.28.1  | Terminal I/O                       |
| `globset`              | 0.4     | Glob pattern matching              |
| `tree-sitter`          | 0.25.10 | Code parsing                       |
| `rustls`               | 0.23    | TLS implementation                 |
| `zstd`                 | 0.13    | Request body compression           |
| `starlark`             | 0.13.0  | Exec policy language               |
| `landlock`             | 0.4.4   | Linux sandbox                      |

---

## 14. Reusability Assessment

### 14.1 Directly Reusable

| Component           | Path                            | Reusability                                                                              |
| ------------------- | ------------------------------- | ---------------------------------------------------------------------------------------- |
| **Protocol types**  | `protocol/`                     | HIGH -- `Op`, `Event`, `ResponseItem`, `TurnItem` are well-defined and provider-agnostic |
| **Transport trait** | `codex-client/src/transport.rs` | HIGH -- `HttpTransport` trait is clean and generic                                       |
| **SSE streaming**   | `codex-client/src/sse.rs`       | HIGH -- SSE parser is provider-agnostic                                                  |
| **Retry logic**     | `codex-client/src/retry.rs`     | HIGH -- Generic retry with backoff                                                       |
| **MCP client**      | `rmcp-client/`                  | MEDIUM-HIGH -- Uses rmcp 0.15.0, good patterns for OAuth + tool listing                  |
| **Hook system**     | `hooks/`                        | HIGH -- Clean event-driven design, easily adaptable                                      |
| **Config layering** | `config/`                       | MEDIUM -- Good TOML merge pattern, but tied to Codex-specific types                      |
| **Error hierarchy** | `core/src/error.rs`             | HIGH -- Pattern of retryable/non-retryable classification is excellent                   |

### 14.2 Needs Adaptation

| Component             | Path                              | What to Change                                                                           |
| --------------------- | --------------------------------- | ---------------------------------------------------------------------------------------- |
| **ModelProviderInfo** | `core/src/model_provider_info.rs` | Extend for Anthropic, Google, etc. -- currently only Responses API wire format           |
| **Tool system**       | `core/src/tools/`                 | Tool registry/router pattern is great but tool specs are Responses API-specific          |
| **Agent loop**        | `core/src/codex.rs`               | Core session loop logic is good but deeply coupled to OpenAI-specific types              |
| **API client**        | `codex-api/`                      | Provider struct is generic but endpoint clients assume OpenAI Responses API              |
| **Sandboxing**        | `core/src/sandboxing.rs`          | Platform-specific (Seatbelt/Landlock/Windows) -- reusable as-is if we want same approach |
| **State management**  | `state/`                          | SQLite model is reusable but schema is Codex-specific                                    |

### 14.3 Build from Scratch

| Component                       | Reason                                                                                                          |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **Anthropic API client**        | Different API format (Messages API vs Responses API)                                                            |
| **Google Gemini client**        | Different API format                                                                                            |
| **Multi-provider abstraction**  | codex-rs only supports Responses API wire format; we need a trait that abstracts across Anthropic/OpenAI/Google |
| **Provider-specific streaming** | Each provider has different SSE event formats                                                                   |

### 14.4 Key Architectural Takeaways for Our SDK

1. **SQ/EQ pattern is excellent** -- Use `async-channel` Sender/Receiver pairs for the user-agent interface. This cleanly separates concerns and enables both interactive and headless modes.

2. **Session + TurnContext separation** -- Session holds stable state (config, auth, tools); TurnContext holds per-turn ephemeral state (model, effort, sandbox policy). This is the right granularity.

3. **ToolRouter + ToolRegistry** -- Decoupling tool specification (what the model sees) from tool dispatch (how tools execute) is clean. The `ToolPayload` enum discriminating between Function/MCP/LocalShell/Custom is a good pattern.

4. **Provider as configuration, not trait** -- codex-rs uses `ModelProviderInfo` as a data struct, not a trait. The provider is just config (base_url, headers, retry). This works because they only support one wire format. For multi-provider support, we need a trait-based approach.

5. **Multi-agent via weak references** -- `AgentControl` uses `Weak<ThreadManagerState>` to avoid reference cycles. Good pattern for agent trees.

6. **Streaming is channel-based** -- All streaming goes through `mpsc` channels, allowing the stream producer (HTTP/WS) to be decoupled from the consumer (session loop). This is the right approach.

7. **Error classification** -- `is_retryable()` on the error enum is a clean pattern for retry logic. We should adopt this.

8. **Config layer stack** -- Multi-layer config with precedence ordering (defaults < user < project < cloud < CLI) is well-designed.

---

## 15. Relevant Files

### Critical Files to Study

- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/codex.rs` -- Main session loop (5000+ lines, the heart of the agent)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/lib.rs` -- Core crate module declarations
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/protocol/src/protocol.rs` -- SQ/EQ protocol (Op and Event enums)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/protocol/src/models.rs` -- ResponseItem, ContentItem, and model types
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/client.rs` -- ModelClient (API session management)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/client_common.rs` -- Prompt, ToolSpec, ResponseStream
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/tools/router.rs` -- ToolRouter dispatch
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/tools/mod.rs` -- Tool system entry point
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/codex-api/src/provider.rs` -- Provider (HTTP endpoint config)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/codex-api/src/endpoint/responses.rs` -- ResponsesClient (SSE streaming)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/codex-api/src/endpoint/responses_websocket.rs` -- WebSocket streaming
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/codex-client/src/transport.rs` -- HttpTransport trait
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/model_provider_info.rs` -- ModelProviderInfo, built-in providers
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/error.rs` -- CodexErr error hierarchy
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/rmcp-client/src/rmcp_client.rs` -- RmcpClient (MCP integration)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/agent/control.rs` -- AgentControl (multi-agent)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/core/src/thread_manager.rs` -- ThreadManager
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/hooks/src/lib.rs` -- Hooks system
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/config/src/lib.rs` -- Config management
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/state/src/lib.rs` -- State persistence
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/app-server/src/lib.rs` -- App server (JSON-RPC bridge)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/codex/codex-rs/Cargo.toml` -- Workspace dependencies
