# Technical Specification: Arky Rust AI Agent SDK

## Executive Summary

Arky is a Rust SDK for building AI agents by wrapping existing CLI runtimes
(`claude` and the Codex App Server) as subprocess-backed providers. Rather
than reimplementing LLM APIs, Arky reuses the capabilities these CLIs already
provide: MCP interoperability, tool execution, sandboxing, approvals, session
continuity, and rich event streams.

The SDK exposes a **dual-layer API**:

- A low-level provider layer for direct control over model interaction and
  event consumption
- A high-level agent layer for stateful agent execution, tool orchestration,
  steering, hooks, and session persistence

The implementation is a Cargo workspace with bounded crates for protocol,
errors, tools, sessions, hooks, MCP, providers, orchestration, and server
exposure. This document is the implementation reference for that workspace. It
specifies architecture, contracts, lifecycle invariants, and failure handling.
It does **not** prescribe internal module-by-module code shape.

The most important corrections introduced by this revision are:

- A dedicated leaf crate for shared error classification to avoid dependency
  cycles
- A streaming contract that can represent **mid-stream failures**
- A session model that supports **resume + replay**, not just message loading
- Explicit single-turn concurrency rules for `prompt`, `stream`, `steer`,
  `follow_up`, and `abort`
- Canonical tool identity and lifecycle requirements promoted to first-class
  invariants instead of incidental implementation detail

## Porting Context

This PRD is grounded in the existing TypeScript provider stack in
`../compozy-code/providers` (absolute path:
`/Users/pedronauck/Dev/compozy/compozy-code/providers`).

Before implementing any crate or task, read
`tasks/prd-rust-providers/porting-reference.md`. It maps each Rust crate/task
to the upstream TypeScript packages, directories, and files that agents should
inspect first for behavior, edge cases, and integration details.

Those upstream packages are reference material, not a ceiling on the Rust
implementation. When the Rust tech spec and ADRs define a stronger API,
cleaner crate boundary, or better failure model, follow the Rust design rather
than copying TypeScript structure or limitations mechanically.

Primary upstream packages for this port:

- `../compozy-code/providers/core` for hooks, tool bridge, MCP helpers, and
  error classification
- `../compozy-code/providers/runtime` for protocol types, orchestration,
  sessions, server behavior, and registry patterns
- `../compozy-code/providers/claude-code` for the Claude CLI wrapper provider
- `../compozy-code/providers/codex` for the Codex App Server provider
- `../compozy-code/providers/opencode` as a secondary reference for additional
  hook, streaming, and subprocess patterns

---

## System Architecture

### Repository & Workspace

**Location:** `~/dev/compozy/arky/`

Arky is a standalone Cargo workspace, decoupled from the `compozy-code`
monorepo. The workspace follows a multi-crate design because the system has
multiple independent heavy domains: providers, MCP, sessions, hooks, tools,
server exposure, and orchestration.

```
arky/
  Cargo.toml              (workspace root, shared deps and lint config)
  crates/
    arky/                 facade crate, prelude, top-level re-exports
    arky-error/           ClassifiedError trait, error codes, shared error helpers
    arky-protocol/        shared types (messages, events, IDs, request/response DTOs)
    arky-config/          configuration loading, merging, validation
    arky-tools/           Tool trait, descriptors, registry, canonical naming
    arky-tools-macros/    #[tool] proc macro
    arky-hooks/           Hooks trait, hook chain, shell hooks, merge semantics
    arky-session/         SessionStore trait, snapshots, replay log, SQLite backend
    arky-provider/        Provider trait, provider registry, provider contract tests
    arky-mcp/             MCP client, server, bidirectional bridge
    arky-claude-code/     Claude Code CLI wrapper provider
    arky-codex/           Codex App Server wrapper provider
    arky-core/            Agent orchestration, command queue, turn loop
    arky-server/          HTTP/SSE server exposing runtime state
```

**ADR references:** ADR-001 (workspace), ADR-003 (CLI wrappers), ADR-010
(naming)

### Crate Dependency Graph

The dependency graph is intentionally strict. Leaf crates must stay leaf crates.
In particular, **no foundational crate may depend on `arky-core`**.

```
arky (facade)
  └─ re-exports everything below

arky-error (leaf)
  └─ standalone

arky-protocol (leaf)
  └─ arky-error

arky-config (leaf)
  └─ arky-error

arky-tools
  ├─ arky-error
  └─ arky-protocol

arky-tools-macros
  └─ standalone (syn / quote / proc-macro2)

arky-hooks
  ├─ arky-error
  ├─ arky-protocol
  └─ arky-tools

arky-session
  ├─ arky-error
  └─ arky-protocol

arky-provider
  ├─ arky-error
  ├─ arky-protocol
  ├─ arky-tools
  ├─ arky-hooks
  └─ arky-session

arky-mcp
  ├─ arky-error
  ├─ arky-protocol
  └─ arky-tools

arky-claude-code
  ├─ arky-error
  ├─ arky-protocol
  ├─ arky-provider
  ├─ arky-tools
  └─ arky-mcp

arky-codex
  ├─ arky-error
  ├─ arky-protocol
  ├─ arky-provider
  ├─ arky-tools
  └─ arky-mcp

arky-core
  ├─ arky-error
  ├─ arky-config
  ├─ arky-protocol
  ├─ arky-provider
  ├─ arky-tools
  ├─ arky-hooks
  ├─ arky-session
  └─ arky-mcp

arky-server
  ├─ arky-error
  ├─ arky-protocol
  └─ arky-core
```

### Component Overview

| Component                | Responsibility                                                                                                             |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| **Error**                | `ClassifiedError`, shared error-code conventions, retryability classification, helper structs for logging and API mapping. |
| **Protocol**             | Shared types: `Message`, `AgentEvent`, IDs, request DTOs, tool descriptors, persisted event records.                       |
| **Config**               | Load and merge config from files, env vars, builder overrides, and provider prerequisites.                                 |
| **Tools**                | `Tool` trait, `ToolDescriptor`, `ToolRegistry`, canonical tool naming, provider-specific name codecs, lifecycle handles.   |
| **Tools Macros**         | `#[tool]` proc macro generating `Tool` implementations from annotated async functions.                                     |
| **Hooks**                | `Hooks` trait, `HookChain`, shell hooks, merge semantics, timeout and isolation rules.                                     |
| **Session**              | `SessionStore` trait, snapshots, replay/event log persistence, in-memory and SQLite implementations.                       |
| **Provider**             | `Provider` trait, `ProviderRequest`, capability descriptors, provider registry, shared contract tests.                     |
| **Claude Code Provider** | Spawns `claude`, parses the CLI event protocol, normalizes events, manages nested tools and spawn-failure cooldown.        |
| **Codex Provider**       | Spawns the Codex App Server, manages JSON-RPC over stdio, thread routing, approval flow, and notification dispatch.        |
| **MCP**                  | `McpClient`, `McpServer`, `McpToolBridge`, canonical naming and schema translation for imported/exposed tools.             |
| **Core**                 | `Agent`, command queue, single-turn execution, steering/follow-up orchestration, session replay, tool cleanup.             |
| **Server**               | Health routes, session routes, runtime exposure, SSE event delivery, replay endpoints.                                     |
| **Facade**               | `arky` crate re-exporting common types and builders, including `arky::prelude::*`.                                         |

### Architectural Invariants

These invariants are part of the contract, not optional implementation detail:

1. **Single active turn per session**
   The agent must never execute overlapping turns for the same session. Calls to
   `prompt`, `stream`, `steer`, and `follow_up` are serialized through an
   internal command queue.

2. **Streaming can fail after it starts**
   Provider streams must yield `Result<AgentEvent, ProviderError>` items. A
   provider crash after the stream is created is a normal failure path, not an
   out-of-band panic.

3. **Canonical tool identity is provider-agnostic**
   Every imported or exposed tool has a canonical ID of the form
   `mcp/<server>/<tool>`. Provider-specific names are codecs, not identity.

4. **Tool registration is call-scoped when needed**
   Temporary tools created for a specific run must be unregistered at stream
   completion, including error and cancellation paths.

5. **Session resume must restore enough state to continue safely**
   Message history alone is insufficient. Replay metadata, last turn outcome,
   provider/session identifiers, and persisted event checkpoints are part of the
   persistence contract.

6. **Foundational crates stay acyclic**
   Shared error contracts live in `arky-error`, not `arky-core`, to avoid
   cycles between `core` and leaf crates.

7. **Provider wrappers stay CLI-first for the MVP**
   Direct Anthropic/OpenAI/Google HTTP clients are explicitly out of scope for
   this MVP. Any future direct-API crates are additive follow-on work.

---

## Implementation Design

### Core Interfaces

#### Shared Error Contract (ADR-006)

Shared classification lives in `arky-error`, because all crates need it and
`arky-core` cannot sit below them in the graph.

```rust
pub trait ClassifiedError: std::error::Error + Send + Sync {
    fn error_code(&self) -> &'static str;

    fn is_retryable(&self) -> bool {
        false
    }

    fn retry_after(&self) -> Option<std::time::Duration> {
        None
    }

    fn http_status(&self) -> u16 {
        500
    }

    fn correction_context(&self) -> Option<serde_json::Value> {
        None
    }
}
```

The facade crate exposes:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ArkyError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Mcp(#[from] McpError),
    #[error(transparent)]
    Hook(#[from] HookError),
    #[error(transparent)]
    Config(#[from] ConfigError),
}
```

#### Provider Trait (ADR-002, ADR-003)

The provider trait must be rich enough for CLI-wrapper providers to receive
session, tool, hook, and turn context. Passing only `Vec<Message>` and generic
options is too weak and would force immediate redesign.

```rust
pub type ProviderEventStream =
    std::pin::Pin<
        Box<
            dyn futures::Stream<Item = Result<AgentEvent, ProviderError>>
                + Send,
        >,
    >;

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError>;

    async fn generate(
        &self,
        request: ProviderRequest,
    ) -> Result<GenerateResponse, ProviderError>;
}

pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub family: ProviderFamily,
    pub capabilities: ProviderCapabilities,
}

pub struct ProviderCapabilities {
    pub streaming: bool,
    pub generate: bool,
    pub tool_calls: bool,
    pub mcp_passthrough: bool,
    pub session_resume: bool,
    pub steering: bool,
    pub follow_up: bool,
}

pub struct ProviderRequest {
    pub session: SessionRef,
    pub turn: TurnContext,
    pub model: ModelRef,
    pub messages: Vec<Message>,
    pub tools: ToolContext,
    pub hooks: HookContext,
    pub settings: ProviderSettings,
}
```

Key contract notes:

- `stream()` yields item-level `Result` so mid-stream provider crashes,
  protocol corruption, and transport disconnects are expressible.
- `generate()` is optional in practice but not in the trait; providers that
  emulate it by draining a stream must document that behavior.
- `ProviderRequest` is the compatibility boundary for future provider growth.

#### Agent Struct (ADR-002)

The agent owns stateful orchestration and command serialization.

```rust
pub struct Agent {
    /* private fields */
}

impl Agent {
    pub fn builder() -> AgentBuilder;

    pub async fn prompt(
        &self,
        input: impl Into<String>,
    ) -> Result<AgentResponse, CoreError>;

    pub async fn stream(
        &self,
        input: impl Into<String>,
    ) -> Result<AgentEventStream, CoreError>;

    pub async fn steer(
        &self,
        message: impl Into<String>,
    ) -> Result<(), CoreError>;

    pub async fn follow_up(
        &self,
        message: impl Into<String>,
    ) -> Result<(), CoreError>;

    pub fn subscribe(&self) -> EventSubscription;

    pub async fn new_session(&self) -> Result<SessionId, CoreError>;

    pub async fn resume(&self, session_id: SessionId) -> Result<(), CoreError>;

    pub async fn abort(&self) -> Result<(), CoreError>;
}
```

`subscribe()` returns a typed subscription wrapper over a broadcast receiver or
stream abstraction. Storing arbitrary callbacks directly is not the primary
contract, because callback lifetimes and backpressure behavior become unclear.

#### Tool Trait (ADR-005)

The tool trait stays intentionally small, but descriptors and lifecycle handles
are first-class.

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn descriptor(&self) -> ToolDescriptor;

    async fn execute(
        &self,
        call: ToolCall,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<ToolResult, ToolError>;
}

pub struct ToolDescriptor {
    pub canonical_name: String,
    pub display_name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub origin: ToolOrigin,
}

pub enum ToolOrigin {
    Local,
    Mcp { server_name: String },
    ProviderScoped { provider_id: ProviderId },
}
```

`ToolRegistry` must support:

- Long-lived registrations
- Call-scoped registrations with cleanup handles
- Provider-specific name formatting through codecs
- Collision detection before a run starts

#### Hooks Trait (ADR-009)

The hook system must define composition behavior, not just signatures.

```rust
#[async_trait::async_trait]
pub trait Hooks: Send + Sync {
    async fn before_tool_call(
        &self,
        ctx: &BeforeToolCallContext,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Verdict, HookError>;

    async fn after_tool_call(
        &self,
        ctx: &AfterToolCallContext,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Option<ToolResultOverride>, HookError>;

    async fn session_start(
        &self,
        ctx: &SessionStartContext,
    ) -> Result<Option<SessionStartUpdate>, HookError>;

    async fn session_end(
        &self,
        ctx: &SessionEndContext,
    ) -> Result<(), HookError>;

    async fn on_stop(
        &self,
        ctx: &StopContext,
    ) -> Result<StopDecision, HookError>;

    async fn user_prompt_submit(
        &self,
        ctx: &PromptSubmitContext,
    ) -> Result<Option<PromptUpdate>, HookError>;
}
```

Composition rules are mandatory:

- `before_tool_call`: first `Block` wins
- `after_tool_call`: overrides merge in registration order, last write wins per
  field
- `session_start`: env/settings merge shallowly, injected messages append in
  order
- `user_prompt_submit`: last prompt rewrite wins, injected messages append in
  order
- `on_stop`: any `Continue` blocks termination for the current stop attempt

Hook execution model:

- Hooks are invoked concurrently per event, results are re-ordered into
  registration order before merge
- Shell hooks receive JSON on stdin and may return JSON or plain text
- All hooks are subject to timeouts and cancellation
- Fail-open vs fail-closed behavior must be configurable per hook chain; the
  default for shell hooks is fail-open with structured diagnostics

#### SessionStore Trait (ADR-007)

Session persistence must support **resume** and **replay**, not just transcript
loading.

```rust
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, new_session: NewSession)
        -> Result<SessionId, SessionError>;

    async fn load(&self, id: &SessionId)
        -> Result<SessionSnapshot, SessionError>;

    async fn append_messages(
        &self,
        id: &SessionId,
        messages: &[Message],
    ) -> Result<(), SessionError>;

    async fn append_events(
        &self,
        id: &SessionId,
        events: &[PersistedEvent],
    ) -> Result<(), SessionError>;

    async fn save_turn_checkpoint(
        &self,
        id: &SessionId,
        checkpoint: TurnCheckpoint,
    ) -> Result<(), SessionError>;

    async fn list(
        &self,
        filter: SessionFilter,
    ) -> Result<Vec<SessionMetadata>, SessionError>;

    async fn delete(&self, id: &SessionId) -> Result<(), SessionError>;
}

pub struct SessionSnapshot {
    pub metadata: SessionMetadata,
    pub messages: Vec<Message>,
    pub last_checkpoint: Option<TurnCheckpoint>,
    pub replay_cursor: Option<ReplayCursor>,
}
```

Important clarifications:

- `SessionMetadata` contains stable identifiers and summary fields; it is **not**
  the input type for `create()`
- Replay storage may be full event persistence or compacted checkpoints plus
  synthesized events, but the external contract must support replay semantics
- The in-memory store may disable replay persistence by configuration, but the
  interface itself must not

### Data Models

#### Event Model (ADR-004)

The event model remains a flat enum, but its metadata must be rich enough for
replay, routing, and observability.

```rust
pub struct EventMetadata {
    pub timestamp_ms: u64,
    pub sequence: u64,
    pub session_id: Option<SessionId>,
    pub turn_id: Option<String>,
    pub provider_id: Option<ProviderId>,
}

#[non_exhaustive]
pub enum AgentEvent {
    AgentStart { meta: EventMetadata },
    AgentEnd { meta: EventMetadata, messages: Vec<Message> },
    TurnStart { meta: EventMetadata },
    TurnEnd {
        meta: EventMetadata,
        message: Message,
        tool_results: Vec<ToolResult>,
    },
    MessageStart { meta: EventMetadata, message: Message },
    MessageUpdate {
        meta: EventMetadata,
        message: Message,
        delta: StreamDelta,
    },
    MessageEnd { meta: EventMetadata, message: Message },
    ToolExecutionStart {
        meta: EventMetadata,
        tool_call_id: String,
        tool_name: String,
        args: serde_json::Value,
    },
    ToolExecutionUpdate {
        meta: EventMetadata,
        tool_call_id: String,
        tool_name: String,
        partial_result: serde_json::Value,
    },
    ToolExecutionEnd {
        meta: EventMetadata,
        tool_call_id: String,
        tool_name: String,
        result: serde_json::Value,
        is_error: bool,
    },
    Custom {
        meta: EventMetadata,
        event_type: String,
        payload: serde_json::Value,
    },
}
```

Mandatory invariants for providers:

- Event `sequence` is strictly monotonic within a session
- `provider_id` is present for provider-originated events
- Tool lifecycle transitions are valid per `tool_call_id`
- Duplicate text must be deduplicated before emission when the upstream CLI
  delivers both delta and final-block representations

#### Message Types

```rust
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub metadata: Option<MessageMetadata>,
}

pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

pub enum ContentBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        id: String,
        content: Vec<ToolContent>,
        is_error: bool,
    },
    Image {
        data: Vec<u8>,
        media_type: String,
    },
}
```

#### Error Enums (ADR-006)

Each crate defines its own `thiserror` enum implementing `ClassifiedError`.

| Crate           | Error Enum      | Key Variants                                                                                            |
| --------------- | --------------- | ------------------------------------------------------------------------------------------------------- |
| `arky-core`     | `CoreError`     | BusySession, Cancelled, InvalidState, ReplayFailed                                                      |
| `arky-provider` | `ProviderError` | NotFound, BinaryNotFound, ProcessCrashed, StreamInterrupted, ProtocolViolation, AuthFailed, RateLimited |
| `arky-tools`    | `ToolError`     | InvalidArgs, ExecutionFailed, Timeout, Cancelled, NameCollision                                         |
| `arky-session`  | `SessionError`  | NotFound, StorageFailure, ReplayUnavailable, Expired                                                    |
| `arky-mcp`      | `McpError`      | ConnectionFailed, ProtocolError, AuthFailed, ServerCrashed, SchemaMismatch                              |
| `arky-hooks`    | `HookError`     | ExecutionFailed, Timeout, InvalidOutput, PanicIsolated                                                  |
| `arky-config`   | `ConfigError`   | ParseFailed, ValidationFailed, NotFound, MissingBinary                                                  |

---

## Integration Points

### Claude Code CLI Integration (ADR-003)

- **Binary:** `claude`
- **Transport:** CLI subprocess over stdin/stdout
- **Provider-specific concerns that must be preserved:**
  - Spawn-failure cooldown tracking
  - Tool lifecycle finite-state machine
  - Nested tool-call tracking
  - Text deduplication between streamed and final assistant payloads
  - Session identifier passthrough and reuse
- **Failure modes:** binary missing, spawn failure, protocol corruption,
  partial stdout/stderr, invalid tool transition

This provider must not collapse all of its complexity into a single "read lines
and map JSON" module. The analysis shows that the tool FSM, nested tool tracking,
and duplicate-text handling are core correctness requirements.

### Codex App Server Integration (ADR-003)

- **Binary:** Codex App Server from the Codex installation
- **Transport:** newline-delimited JSON-RPC over stdio
- **Components that must exist conceptually, even if organized differently:**
  - `ProcessManager` for lifecycle
  - `RpcTransport` for request/response correlation
  - `Scheduler` for serialized model access
  - `ThreadManager` for multi-conversation control
  - `NotificationRouter` for stream routing
  - Text accumulator / tool tracker for normalized output assembly
- **Failure modes:** JSON-RPC transport desync, process crash, stale thread
  routing, approval timeout, notification stream drop

### MCP Integration (ADR-008)

- **Crate:** `rmcp` v0.16+
- **Client:** connect to stdio and streamable-HTTP servers
- **Server:** expose SDK tools as MCP tools
- **Bridge:** canonical naming, schema translation, connection lifecycle
- **Auth:** bearer token and OAuth for HTTP servers

MCP is not an optional add-on. It is required both for importing external tools
and for exposing Arky-managed tools back to CLI subprocesses.

### Shared Infrastructure

The providers share infrastructure, but only where the abstraction is truly
common:

| Shared Component        | Responsibility                                                             |
| ----------------------- | -------------------------------------------------------------------------- |
| `ProcessManager`        | Subprocess spawn, restart policy, graceful shutdown, kill-on-drop fallback |
| `StdioTransport`        | Buffered stdin/stdout handling, framing, backpressure, cancellation        |
| `ToolIdCodec`           | Canonical `<->` provider-specific tool naming round-trips                  |
| `ReplayWriter`          | Persist event log or compacted checkpoints during active streams           |
| `ProviderContractTests` | Shared behavioral tests every provider implementation must pass            |

---

## Testing Approach

### Unit Tests

- **Per-crate isolation:** each crate owns its unit tests and fixtures
- **Contract tests:** shared tests for any `Provider`, `Tool`, `SessionStore`,
  and `Hooks` implementation
- **Event model:** serialization, ordering, and metadata monotonicity
- **Tool codecs:** canonical/provider-specific round-trip correctness
- **Error classification:** retryability, error codes, HTTP mapping, correction
  context
- **Replay logic:** checkpoint synthesis and event replay cursor behavior

### Integration Tests

- **Claude provider:** spawn a real `claude` binary behind an integration flag
  and verify tool lifecycle, nested tools, and deduplication behavior
- **Codex provider:** spawn the real app server and verify request correlation,
  thread routing, and approval workflow integration
- **MCP bridge:** connect to real or fixture MCP servers over stdio and HTTP,
  then expose local tools back out through MCP
- **Session persistence:** create, append, replay, resume, delete, and migrate
  SQLite-backed sessions with a real database file
- **Concurrency:** assert that overlapping turns on one session are rejected or
  queued according to the agent contract
- **Crash paths:** process crash after first event, stalled stdout, malformed
  JSON, hook timeout, and cancellation propagation

### Macro Expansion Tests

- Compile-time expansion tests for `#[tool]`
- Schema output validation for complex arg types
- Error-message tests for invalid macro usage

---

## Development Sequencing

### Phase 1: Foundations

1. **`arky-error`**: shared error contracts and conventions
2. **`arky-protocol`**: IDs, messages, events, request DTOs, persisted events
3. **`arky-config`**: config parsing and validation
4. **`arky-tools`**: tool descriptors, registry, codecs, cleanup handles
5. **`arky-tools-macros`**: `#[tool]` macro

### Phase 2: Durable Infrastructure

6. **`arky-hooks`**: hook contract, shell hooks, merge semantics, timeouts
7. **`arky-session`**: snapshot/replay store with in-memory backend first
8. **`arky-provider`**: provider trait, request/response contracts, contract
   test suite
9. **`arky-mcp`**: MCP client, server, bridge, canonical naming integration

### Phase 3: Provider Implementations

10. **`arky-claude-code`**: first concrete provider, validates event contract
11. **`arky-codex`**: second provider, validates JSON-RPC and thread routing

### Phase 4: Orchestration

12. **`arky-core`**: agent builder, command queue, turn loop, replay integration
13. **`arky-server`**: HTTP + SSE runtime exposure
14. **`arky`**: facade crate and prelude

### Phase 5: Hardening

15. Provider fixture corpus for protocol regression tests
16. CI/CD (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`)
17. Benchmarks: event throughput, spawn latency, replay overhead
18. Documentation and runnable examples

---

## Monitoring & Observability

### Tracing

- Use `tracing` across all crates
- Span hierarchy: `agent > session > turn > provider_call > tool_call`
- Required fields: `session_id`, `turn_id`, `provider_id`, `tool_name`,
  `canonical_tool_name`, `event_sequence`
- Long-lived subprocess spans must record spawn args and binary resolution path
  without logging secrets

### Metrics

- Event throughput per provider
- Tool execution latency (p50/p95/p99)
- Replay load latency
- Provider startup latency
- JSON-RPC round-trip latency for Codex
- Hook latency and timeout rate
- Subprocess restart count and uptime

### Health

- `arky-server` exposes `/health` and `/ready`
- Provider wrappers expose internal health state for binaries, transports, and
  session compatibility
- MCP connections use keepalive pings where transport supports them

---

## Technical Considerations

### Key Decisions Summary

| ADR     | Decision                              | Rationale                                                        |
| ------- | ------------------------------------- | ---------------------------------------------------------------- |
| ADR-001 | Cargo workspace multi-crate           | Enforced boundaries and incremental compilation                  |
| ADR-002 | Dual-layer API (Provider + Agent)     | Supports both infrastructure and productive agent use cases      |
| ADR-003 | CLI wrapper providers                 | Reuses MCP/tools/sandboxing/approval already present in CLIs     |
| ADR-004 | Flat enum events with rich metadata   | Single event protocol with enough context for replay and routing |
| ADR-005 | Tool trait + proc macro               | Small runtime contract plus ergonomic authoring                  |
| ADR-006 | Per-crate errors + `arky-error` leaf  | Uniform classification without dependency cycles                 |
| ADR-007 | Session store from day one            | Resume and replay are mandatory for coding agents                |
| ADR-008 | MCP client + server + bridge          | Full MCP interop and bidirectional tool exposure                 |
| ADR-009 | Hooks trait with explicit merge rules | Complete lifecycle coverage without ambiguous behavior           |
| ADR-010 | `arky-*` naming                       | Consistent crate and facade naming                               |

### Rust Dependency Stack

Core dependencies managed via `[workspace.dependencies]`:

| Crate                           | Version | Purpose                                                         |
| ------------------------------- | ------- | --------------------------------------------------------------- |
| `tokio`                         | 1.x     | Async runtime, process, sync, time, io                          |
| `serde` + `serde_json`          | 1.x     | Serialization/deserialization                                   |
| `thiserror`                     | 2.x     | Error derive macros                                             |
| `tracing`                       | 0.1.x   | Structured logging                                              |
| `async-trait`                   | 0.1.x   | Async trait methods                                             |
| `tokio-util`                    | 0.7.x   | `CancellationToken`, codecs                                     |
| `futures`                       | 0.3.x   | Stream combinators and utilities                                |
| `schemars`                      | 0.8.x   | Tool input schema generation                                    |
| `rmcp`                          | 0.16.x  | Official Rust MCP SDK                                           |
| `reqwest`                       | 0.12.x  | HTTP client for MCP HTTP transport and server integration tests |
| `dashmap`                       | 6.x     | Concurrent in-memory maps where justified                       |
| `uuid`                          | 1.x     | Session ID generation                                           |
| `regex`                         | 1.x     | Tool matching, validation, classification                       |
| `syn` + `quote` + `proc-macro2` | 2.x     | Proc macro infrastructure                                       |

Feature-gated dependencies:

| Crate                                  | Feature  | Purpose                             |
| -------------------------------------- | -------- | ----------------------------------- |
| `tokio-rusqlite` or equivalent wrapper | `sqlite` | Async-friendly SQLite session store |
| `axum`                                 | `server` | HTTP/SSE server layer               |

### Known Risks

| Risk                                        | Impact                                     | Mitigation                                                                 |
| ------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------- |
| CLI protocol changes                        | Provider wrapper breakage                  | Version pinning, fixture corpus, integration tests against pinned binaries |
| Mid-stream process crash                    | Partial output + inconsistent state        | Stream item `Result`, checkpointing, cleanup-on-error, crash-path tests    |
| Dependency cycle reintroduced               | Workspace stalls at compile/design time    | Keep shared error contract in `arky-error`, enforce crate graph in CI      |
| Tool name collision or bad codec round-trip | Wrong tool routing                         | Canonical ID validation before run, codec contract tests                   |
| Session replay log growth                   | Disk bloat or slow resume                  | Compaction policy, replay cursor, snapshot + checkpoint strategy           |
| SQLite contention                           | Resume/save failures under parallel agents | WAL mode, single-writer discipline, bounded retry strategy                 |
| Hook hang or panic                          | Agent stall or crash                       | Timeouts, cancellation, isolation policy, structured diagnostics           |
| Approval or steering race conditions        | Lost commands or inconsistent turns        | Single-turn queue, explicit state machine, concurrency integration tests   |

### Rust Idioms & Conventions

- `thiserror` for library errors; no `unwrap()` in library code
- `tokio` for async I/O and subprocesses
- Traits stay narrow; dynamic dispatch only where heterogeneity is required
- Builders enforce required configuration where practical
- `#[non_exhaustive]` on public enums and externally extended context structs
- Prefer plain functions and traits over macros except where the macro removes
  real repeated boilerplate (`#[tool]`)
- Unit tests in `#[cfg(test)]`, integration tests in `tests/`

### Standards Compliance

- Rust Edition 2024
- `[workspace.dependencies]` for version unification
- `cargo fmt`
- `cargo clippy -D warnings`
- `cargo test`
- Public protocol types implement `Debug`, `Clone` where appropriate, and
  `Serialize`/`Deserialize` when crossing process or storage boundaries
- Public async traits and trait objects are `Send + Sync`

---

## References

### Analysis Documents

| Document                     | Contents                                                                                            |
| ---------------------------- | --------------------------------------------------------------------------------------------------- |
| `analysis_core.md`           | Core abstractions: hooks, tool bridge, MCP integration, token consumption, error classification     |
| `analysis_claude_code.md`    | Claude provider pipeline, nested tool tracking, deduplication, spawn failure tracking               |
| `analysis_codex.md`          | Codex provider architecture: JSON-RPC transport, scheduler, thread manager, notification routing    |
| `analysis_runtime.md`        | Runtime orchestration, tool registry/codec, session store, usage tracking, server                   |
| `analysis_pi_agent.md`       | Agent-centric orchestration, event protocol, steering/follow-up model                               |
| `analysis_rust_ecosystem.md` | Rust ecosystem survey, dependency candidates, architecture recommendations                          |
| `analysis_codex_rs.md`       | Production Rust reference for workspace layout, SQ/EQ-style flow, MCP integration, session handling |
| `analysis_opencode.md`       | Secondary reference for SSE resilience, config loading, hook execution, and event conversion        |

### ADR Documents

ADR-001 through ADR-010 in `tasks/prd-rust-providers/adrs/`

### External References

- Claude Agent SDK / CLI
- Codex App Server reference under `.resources/codex/codex-rs/`
- `rmcp` crate
- Pi agent framework under `.resources/pi/packages/agent/`
