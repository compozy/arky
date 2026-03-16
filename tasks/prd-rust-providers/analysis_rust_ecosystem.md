# Rust AI Ecosystem Analysis for Multi-Provider Agent SDK

## Executive Summary

The Rust AI ecosystem has matured significantly in 2025-2026, offering a viable foundation for building a multi-provider AI agent SDK. The ecosystem now includes full-featured LLM agent frameworks (Rig, AutoAgents, `llm` crate), an official MCP SDK (`rmcp` with 3.2k GitHub stars), well-established streaming patterns (Axum + tokio-stream + async-stream), and type-safe tool calling via `schemars` + `serde` derive macros. The key gap our SDK would fill is a **Compozy-specific orchestration layer** that unifies provider adapters, session management, event normalization, and tool name canonicalization in Rust -- bridging the gap between the current TypeScript Effect-TS implementation and native Rust performance. No existing Rust project provides Compozy's unique combination of canonical tool name normalization, 14-type event model, multi-provider session management with replay, and integrated MCP bridge.

---

## 1. Rust AI Agent Frameworks

### 1.1 Rig (rig-core)

- **GitHub**: [0xPlaygrounds/rig](https://github.com/0xPlaygrounds/rig) -- 6,500 stars, 704 forks, 172 contributors
- **Version**: v0.31+ (516 releases, rapid iteration, pre-1.0)
- **Providers**: 20+ model providers (OpenAI, Anthropic, Cohere, AWS Bedrock, Google Vertex AI, etc.)
- **Key design**:
  - `CompletionModel` and `EmbeddingModel` traits as the provider abstraction
  - `Agent` struct combines model + preamble + tools + context documents
  - Builder pattern for all configuration
  - Full async/await on Tokio
  - WASM compatible (core only)
  - 10+ vector store integrations
  - OpenTelemetry GenAI semantic conventions
- **Tool calling**: Trait-based with `schemars::JsonSchema` for schema generation, string-based argument passing
- **Streaming**: Multi-turn streaming completions as first-class
- **Limitations**: Pre-1.0 with frequent breaking changes; focused on LLM application building rather than raw provider SDK

### 1.2 AutoAgents

- **GitHub**: [liquidos-ai/AutoAgents](https://github.com/liquidos-ai/AutoAgents) -- 441 stars, 61 forks
- **Key design**:
  - Procedural macros: `#[agent]`, `#[tool]` for declarative definitions
  - ReAct and basic executors with streaming responses
  - WASM sandboxed runtime for tool execution
  - Sliding window memory with extensible backends
  - Typed pub/sub for multi-agent orchestration
  - OpenTelemetry tracing/metrics
  - Python bindings via PyPI
- **Providers**: OpenAI, Anthropic, Google, Azure, DeepSeek, Groq, xAI, Ollama, Mistral-rs, Llama-Cpp
- **Performance**: 5x less memory than Python frameworks, 36% more throughput, 4ms cold start vs 60-140ms

### 1.3 `llm` Crate (by graniet)

- **GitHub**: [graniet/llm](https://github.com/graniet/llm) -- 322 stars, v1.3.4
- **Key design**:
  - Builder pattern: `LLMBuilder::new().backend(LLMBackend::OpenAI).build()`
  - `ChatProvider` and `CompletionProvider` traits
  - Feature-gated providers via Cargo features
  - Sliding window memory with shared memory across providers
  - Multi-step chains with different backends per step
  - Agentic capabilities (reactive agents, shared memory, triggers)
  - REST API mode (serve any backend as OpenAI-compatible API)
- **Providers**: OpenAI, Anthropic, Ollama, DeepSeek, xAI, Phind, Groq, Google, Cohere, Mistral, HuggingFace, ElevenLabs
- **Tool calling**: Full function calling support with unified tool call types
- **Streaming**: Per-provider streaming with `normalize_response(true)` option

### 1.4 ADK-Rust

- Positioned as production-ready agent framework combining best patterns from LangChain, LangGraph, OpenAI SDK
- Vendor-agnostic, modular design
- Less documentation available; newer project

### 1.5 Other Notable Projects

- **langchain-rust** / **llm-chain**: Community ports of LangChain concepts to Rust; lower activity
- **CrustAGI**: Port of BabyAGI for task management with GPT
- **SmartGPT**: Modular LLM agent framework inspired by AutoGPT
- **Nerve**: YAML-driven Rust tool for defining multi-step agents

---

## 2. Rust LLM Provider Crates

### 2.1 async-openai

- **GitHub**: [64bit/async-openai](https://github.com/64bit/async-openai) -- v0.32.4
- **Scope**: Full OpenAI API spec implementation (Responses API, Chat Completions, Realtime, Audio, Images, etc.)
- **Design**: Builder pattern, `Config` trait for provider customization, granular feature flags
- **Streaming**: SSE streaming via `eventsource-stream`
- **Tool calling**: Via companion crate `openai-func-enums` with derive macros
- **Strengths**: Most complete OpenAI API coverage, WASM support, configurable for compatible providers
- **Limitation**: Focused on OpenAI spec; non-OpenAI-compatible providers (Anthropic native API) need separate handling

### 2.2 genai (by jeremychone)

- **GitHub**: [jeremychone/rust-genai](https://github.com/jeremychone/rust-genai) -- 690 stars, v0.5.x
- **Design**: Single `Client` with `exec_chat()` / `exec_chat_stream()`, `AdapterKind` for auto provider routing
- **Providers**: 14 natively (OpenAI, Anthropic, Gemini, xAI, Ollama, Groq, DeepSeek, Cohere, Together, Fireworks, etc.)
- **Streaming**: Full streaming via unified `EventSourceStream` and `WebStream`
- **Tool calling**: Listed as future direction -- NOT YET IMPLEMENTED
- **Strengths**: Ergonomic multi-provider API, auto model-name-to-provider routing, reasoning/thinking support
- **Limitation**: "Prioritizes ergonomics and commonality, with depth being secondary" -- not a full-depth SDK

### 2.3 turbine-llm

- **Scope**: Unified API for OpenAI, Anthropic, Gemini, Groq
- **Design**: Async/await with Tokio, type safety, proper error handling
- **Maturity**: Active but smaller community

### 2.4 llm-connector

- **Scope**: 11+ providers with protocol/provider separation
- **Features**: Multi-modal support, function calling with streaming, reasoning models
- **Design**: Clean Protocol/Provider separation pattern

### 2.5 rs-agent

- **Scope**: Single agent interface with pluggable LLM adapters
- **Design**: `LLM` trait for pluggable backends, `Tool` trait + `ToolCatalog` for tool registry
- **Features**: UTCP protocol for agent-as-a-tool, multi-agent coordination
- **Providers**: Gemini, Ollama, Anthropic, OpenAI (feature-flagged)

### 2.6 agentai

- **Scope**: Connect to major LLM providers with MCP Server support
- **Providers**: OpenAI, Anthropic, Gemini, Ollama, OpenAI-compatible APIs

---

## 3. MCP (Model Context Protocol) in Rust

### 3.1 rmcp (Official SDK)

- **GitHub**: [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) -- 3,200 stars
- **Version**: v0.16.0-1.2.0 (actively versioned)
- **Architecture**:
  - Role-typed generics: `RoleServer` / `RoleClient` prevent misuse at compile time
  - `ServerHandler` / `ClientHandler` traits
  - `ServiceExt::serve(transport)` entry point
  - `RequestContext<Role>` for per-request context with `peer` handle
- **Transport types**:
  - stdio: `(stdin(), stdout())` or `TokioChildProcess`
  - HTTP: `StreamableHttpClientTransport` / `StreamableHttpService`
  - Pluggable via `Transport` / `IntoTransport` traits
- **Tool registration**: `#[tool]`, `#[tool_handler]`, `#[tool_router]` attribute macros with `schemars::JsonSchema` derives
- **Features**: Resources, Prompts, Sampling, Roots, Logging, Completions, Notifications, OAuth
- **Dependencies**: tokio, serde, schemars, reqwest, axum, thiserror, oauth2, jsonwebtoken

### 3.2 rust-mcp-sdk

- Alternative MCP implementation with `#[mcp_tool]` macro
- Built on top of rmcp (depends on rmcp ^0.17.0)
- Latest: v0.3.5 (2026-03-02)

### 3.3 Codex's MCP Usage (Reference Architecture)

The codex-rs project already uses `rmcp` extensively:

- `codex-mcp-server`: MCP server exposing Codex tools (depends on rmcp with server features)
- `codex-rmcp-client`: MCP client with full transport support including auth, streamable HTTP, child process
- `codex-core`: Integrates rmcp with `base64`, `macros`, `schemars`, `server` features
- Key dependencies alongside rmcp: `schemars`, `serde`, `tokio`, `thiserror`, `tracing`

---

## 4. Streaming/SSE in Rust

### Standard Architecture (2025-2026)

The dominant pattern for LLM response streaming in Rust:

| Component            | Standard Choice                                                             |
| -------------------- | --------------------------------------------------------------------------- |
| **Framework**        | Axum (with Tokio runtime)                                                   |
| **Stream creation**  | `async_stream::stream!` or `try_stream!` macro                              |
| **Channel**          | `tokio::sync::mpsc` (single consumer) or `tokio::sync::broadcast` (fan-out) |
| **SSE response**     | `Sse<impl Stream<Item = Result<Event, Infallible>>>` with `KeepAlive`       |
| **Stream utilities** | `tokio-stream` wrappers (`ReceiverStream`, `BroadcastStream`)               |
| **SSE client**       | `eventsource-stream`, `rust-eventsource-client` (with reconnect)            |
| **LLM integration**  | Tokens via channel -> stream -> SSE events                                  |

### Key Crates

- **`tokio-stream`** (official Tokio): Stream wrappers and utilities, `StreamExt::next()` for iteration
- **`async-stream`** (temporary solution): `stream!` and `try_stream!` macros until async generators stabilize
- **`futures-util`**: `Stream` trait, stream combinators
- **`axum` SSE module**: `axum::response::sse::{Event, KeepAlive, Sse}`
- **`eventsource-stream`**: SSE client-side parsing (used by async-openai)
- **`llm-stream`**: Dedicated LLM streaming crate

### Patterns for LLM Streaming

1. **Channel-based**: Spawn LLM task, send tokens via `mpsc::Sender`, consume via `ReceiverStream`
2. **Broadcast for fan-out**: `broadcast::Sender` + `BroadcastStream` for multi-client streaming
3. **Connection closure detection**: `Drop` guard inside `stream!` macro
4. **SSE reconnection**: `Last-Event-Id` header for resume, exponential backoff via `rust-eventsource-client`

---

## 5. Tool Calling Patterns in Rust

### Common Pattern

```
Rust struct -> derive(serde, schemars) -> auto JSON Schema -> LLM tool definition
     ^                                                              |
     |--- deserialize LLM arguments back into typed struct <--------|
                    |
                    v
        execute via dynamic dispatch (dyn Trait or fn pointer)
```

### Key Libraries for Tool Schema

| Library        | Approach                  | Notes                                                                 |
| -------------- | ------------------------- | --------------------------------------------------------------------- |
| **schemars**   | `#[derive(JsonSchema)]`   | Foundation for most Rust LLM tools; compatible with serde annotations |
| **llm-schema** | `#[derive(LlmSchema)]`    | LLM-optimized schema generation                                       |
| **typify**     | JSON Schema -> Rust types | Reverse direction (schema to types)                                   |

### Tool Registration Patterns

1. **Trait-based** (Rig, llm-kit): Define `Tool` trait with async `execute`, register via builder
2. **Macro-based** (rmcp, rust-mcp-sdk): `#[tool]` / `#[mcp_tool]` attribute macros auto-generate routing
3. **HashMap registry** (Groq Rust Agent): `lazy_static` HashMap mapping tool names to function handlers
4. **`ToolCatalog`** (rs-agent): Implement `Tool` trait, register in catalog, bridge via UTCP

### Dynamic Dispatch Considerations

- `dyn Trait` for runtime polymorphism (small vtable overhead)
- `Arc<dyn Trait>` for thread-safe sharing across async tasks
- Static dispatch (generics) preferred in libraries; dynamic dispatch in binaries
- Async traits now natively supported (stabilized); `dynosaur` crate for dynamic dispatch of async trait methods

---

## 6. Rust SDK Design Patterns

### Builder Pattern

- Standard for configuration objects (no function overloading in Rust)
- Two approaches: by-value (consuming) or by-reference (mutable borrow)
- Enhanced with `derive_builder` crate for auto-generation
- Example: async-openai, Rig, llm crate all use builder pattern extensively

### Trait-Based Provider Abstraction

- `CompletionModel` trait (Rig): Provider implements `completion(request) -> Result<Response>`
- `ChatProvider` / `CompletionProvider` traits (llm crate): Backend-agnostic API
- `LLM` trait (rs-agent): Pluggable model adapters behind feature flags
- `Config` trait (async-openai): Configurable base URL, API key, headers for compatible providers

### Error Handling

| Pattern          | When to Use                               | Crate                                         |
| ---------------- | ----------------------------------------- | --------------------------------------------- |
| **`thiserror`**  | Libraries with typed error enums          | For caller-facing errors requiring match arms |
| **`anyhow`**     | Applications, top-level error aggregation | When callers just need to display errors      |
| **`snafu`**      | Large workspaces (like GreptimeDB)        | Combines thiserror + anyhow patterns          |
| **Combine both** | Most real-world projects                  | Internal: thiserror; application: anyhow      |

Best practice for 2025-2026: Use `thiserror` for all public error types in library crates. Use `anyhow` (or `eyre`) at the binary/application boundary only. Error types should carry context via `#[from]` and `#[source]`.

### Feature Flags (Cargo)

- **Additive**: Features must only add functionality, never remove
- **Provider gating**: `features = ["openai", "anthropic"]` -- each provider is opt-in
- **Transport gating**: rmcp uses `transport-io`, `transport-child-process`, `transport-streamable-http-*`
- **API surface gating**: async-openai uses `responses`, `chat-completion`, `byot`
- **SemVer rules**: Adding features = minor release; removing = major release
- **Testing**: Feature combinations require exponential test matrix; use CI matrix strategy

### Type-Driven API Design

- Leverage Rust's type system to prevent misuse at compile time
- Role-typed generics (rmcp's `RoleServer`/`RoleClient`)
- Branded types via newtypes for IDs, tokens, etc.
- `Pin` handling for streams and futures

---

## 7. Session Management Patterns

### Current Ecosystem Status

No Rust crate provides a turnkey "AI agent session manager" with conversation state persistence. The building blocks exist:

| Concern                   | Rust Solution                                             |
| ------------------------- | --------------------------------------------------------- |
| **Async runtime**         | Tokio                                                     |
| **Shared state**          | `Arc<Mutex<T>>`, `Arc<RwLock<T>>`, `DashMap`              |
| **Channel communication** | `tokio::sync::mpsc`, `broadcast`, `watch`                 |
| **Session storage**       | `axum_session` (web-focused), custom with SQLite/rusqlite |
| **Conversation memory**   | `llm` crate's sliding window, rs-agent's memory backends  |
| **Event persistence**     | Custom implementation needed (Codex uses SQLite)          |

### Codex's Approach (Reference)

- `codex-state`: Session state management crate
- `codex-protocol`: Protocol types for session events
- `codex-core`: Orchestrates everything with rmcp integration
- Uses SQLite via bundled `rusqlite` for persistence
- Event streaming via `eventsource-stream`

### Recommended Patterns for Our SDK

1. **Session struct**: Owns session ID, provider ID, state, event buffer
2. **Event buffer**: Ring buffer with sequence IDs (like Compozy's current EventBuffer design)
3. **Persistence driver trait**: Pluggable (in-memory, SQLite, custom)
4. **Replay context**: Serialize recent events for session resume
5. **Concurrency**: `DashMap<SessionId, SessionState>` for concurrent session access

---

## 8. Comparable Projects (Full Agent SDK in Rust)

No existing Rust project exactly matches Compozy's scope. Here's how the closest alternatives compare:

| Project          | Multi-Provider | Tool Normalization      | Session Mgmt     | Event Model          | MCP        | Streaming |
| ---------------- | -------------- | ----------------------- | ---------------- | -------------------- | ---------- | --------- |
| **Rig**          | 20+            | No canonical IDs        | No               | Raw responses        | No         | Yes       |
| **AutoAgents**   | 13+            | No                      | Sliding window   | Basic                | No         | Yes       |
| **`llm` crate**  | 12+            | No                      | Sliding window   | Unified ToolCall     | No         | Yes       |
| **genai**        | 14             | No                      | No               | Normalized responses | No         | Yes       |
| **rs-agent**     | 4              | UTCP protocol           | Memory backends  | Basic                | No         | Limited   |
| **Codex-rs**     | OpenAI only    | No                      | Yes (full)       | JSON-RPC envelopes   | Yes (rmcp) | Yes       |
| **Compozy goal** | 3+             | Canonical `mcp/<s>/<t>` | Full with replay | 14-type union        | Yes        | Yes       |

---

## 9. Recommended Dependency Stack

### Core Foundation

| Dependency             | Version | Purpose                                                   |
| ---------------------- | ------- | --------------------------------------------------------- |
| `tokio`                | 1.x     | Async runtime (multi-thread, sync, time, process, signal) |
| `serde` / `serde_json` | 1.x     | Serialization/deserialization                             |
| `thiserror`            | 2.x     | Typed error enums for library crates                      |
| `tracing`              | 0.1.x   | Structured logging and instrumentation                    |

### Provider Integration

| Dependency                 | Version   | Purpose                                           |
| -------------------------- | --------- | ------------------------------------------------- |
| `reqwest`                  | 0.12-0.13 | HTTP client with streaming, JSON, TLS             |
| `eventsource-stream`       | latest    | SSE client parsing for provider streams           |
| `futures` / `futures-util` | 0.3.x     | Stream combinators and utilities                  |
| `tokio-stream`             | 0.1.x     | Stream wrappers (ReceiverStream, BroadcastStream) |
| `async-stream`             | 0.3.x     | `stream!` / `try_stream!` macros                  |

### Schema & Tool Calling

| Dependency   | Version | Purpose                                |
| ------------ | ------- | -------------------------------------- |
| `schemars`   | 0.8.x+  | JSON Schema generation from Rust types |
| `serde_json` | 1.x     | Tool argument deserialization          |

### MCP Integration

| Dependency | Version | Purpose                        |
| ---------- | ------- | ------------------------------ |
| `rmcp`     | 0.16+   | Official MCP client/server SDK |

### Optional Providers (Feature-Gated)

| Feature              | Dependencies                     | Purpose               |
| -------------------- | -------------------------------- | --------------------- |
| `provider-openai`    | `async-openai` OR custom reqwest | OpenAI/compatible API |
| `provider-anthropic` | Custom reqwest client            | Anthropic native API  |
| `provider-google`    | Custom reqwest client            | Gemini API            |
| `provider-ollama`    | Custom reqwest client            | Local Ollama          |

### Session & State

| Dependency            | Version | Purpose                      |
| --------------------- | ------- | ---------------------------- |
| `dashmap`             | 6.x     | Concurrent session state map |
| `uuid`                | 1.x     | Session/event ID generation  |
| `chrono`              | 0.4.x   | Timestamp handling           |
| `rusqlite` (optional) | 0.38+   | SQLite-backed persistence    |

### Error & Utilities

| Dependency                          | Version | Purpose                          |
| ----------------------------------- | ------- | -------------------------------- |
| `thiserror`                         | 2.x     | Library error types              |
| `anyhow`                            | 1.x     | Application-level error handling |
| `once_cell` / `std::sync::LazyLock` | std     | Lazy initialization              |
| `regex`                             | 1.x     | Tool name parsing/normalization  |

### Server (if exposing API)

| Dependency | Version | Purpose                      |
| ---------- | ------- | ---------------------------- |
| `axum`     | 0.8+    | HTTP server with SSE support |

---

## 10. Gaps Our SDK Would Fill

### Gap 1: Canonical Tool Name Normalization

No Rust crate provides Compozy's `mcp/<server>/<tool>` canonical ID system with bidirectional provider-specific format conversion. This is a unique Compozy innovation.

### Gap 2: Rich Typed Event Model

While `llm` crate offers a unified `ToolCall` type, no Rust framework provides a 14-type discriminated union event model comparable to Compozy's `OpenResponsesStreamEvent`. Rust's enum system is ideal for this.

### Gap 3: Session Management with Event Replay

No Rust agent framework offers session resume with event replay context injection. The `llm` crate has sliding window memory, but nothing comparable to Sandbox Agent's `buildReplayText` mechanism.

### Gap 4: Provider Adapter Normalization Layer

While Rig and genai abstract providers, none provide the depth of Compozy's adapter layer (1000+ line adapters that handle 20+ native event type mappings with stateful accumulation).

### Gap 5: Integrated MCP Bridge

No multi-provider agent framework integrates MCP client/server capabilities. The `rmcp` crate handles MCP, and agent frameworks handle LLM providers, but nobody combines them.

### Gap 6: Native Rust Performance for Desktop Agent SDK

The codex-rs reference shows Rust can power a desktop agent with process management, event streaming, and MCP support. A Compozy Rust SDK would bring this to multi-provider scenarios with 5x memory reduction and near-zero cold start times.

---

## 11. Architectural Recommendations

### Approach A: Wrap Existing Crates

Use `rig-core` or `genai` as the provider abstraction layer and build Compozy-specific features on top.

- **Pro**: Faster initial development, community maintenance of provider integrations
- **Con**: Dependency on external API stability, limited control over streaming/event model

### Approach B: Custom Provider Layer (Recommended)

Build custom provider traits inspired by Rig's `CompletionModel` pattern, with Compozy-specific event normalization.

- **Pro**: Full control over event model, tool normalization, session management
- **Con**: More initial development effort
- **Mitigation**: Use `reqwest` + `eventsource-stream` directly (as codex-rs does) for each provider

### Approach C: Hybrid

Custom core traits with optional integration crates that bridge existing Rust LLM crates.

- **Pro**: Best of both worlds
- **Con**: Additional maintenance surface

### Recommended Architecture

```
compozy-sdk-core        (traits, event model, error types, tool normalization)
compozy-sdk-providers   (feature-gated: openai, anthropic, google, ollama)
compozy-sdk-mcp         (rmcp integration for MCP client/server bridge)
compozy-sdk-session     (session management, event persistence, replay)
compozy-sdk             (top-level facade crate re-exporting everything)
```

This mirrors the codex-rs workspace pattern and follows Rust ecosystem best practices for modularity via Cargo feature flags.
