# Technical Specification: Arky SDK Full Parity with compozy-code Providers

## Executive Summary

This techspec defines the implementation plan for achieving full feature parity between the Rust arky SDK and the TypeScript compozy-code providers. Gap analysis across 4 domains (core/runtime, claude-code, codex, server/session/usage) revealed that Rust covers 25-60% of the TS feature surface depending on the domain, with critical gaps in error classification, usage/token tracking, reasoning blocks, configuration schemas, model discovery, streaming endpoints, and Codex server lifecycle.

The implementation follows a priority-based phasing strategy (P0 -> P1 -> P2) across all domains simultaneously, with 10 architectural decisions documented as ADRs. Key decisions include: a new `arky-usage` crate for token tracking, a centralized error classifier with pattern registry, reasoning as first-class `AgentEvent` variants, a Codex server registry with ref-counting matching the TS design, and fully typed configuration schemas for all providers.

## System Architecture

### Crate Placement

Changes affect these existing crates and one new crate:

| Crate | Changes |
|-------|---------|
| `arky-error` | ErrorClassifier, ErrorPattern, ErrorCategory, format_for_agent() |
| `arky-protocol` | AgentEvent reasoning variants, ReasoningEffort enum, FinishReason enum |
| **`arky-usage`** (NEW) | NormalizedUsage, UsageAggregator, ProviderMetadataExtractor, ModelCost |
| `arky-provider` | Capability validation, model-prefix inference, generate_from_stream enhancement |
| `arky-claude-code` | Error patterns, reasoning parsing, config expansion, tool bridge, generate override |
| `arky-codex` | Server registry, event dispatcher, config expansion, model service, text accumulator |
| `arky-server` | POST /v1/chat/stream, GET /v1/models, bearer auth, SSE enhancements |
| `arky-tools` | Tool output truncation |
| `arky-core` | Usage aggregation integration, capability validation calls |
| `arky-session` | TTL/capacity for in-memory store |
| `arky-hooks` | Wiring into provider stream pipelines |

### Dependency Graph (new edges)

```
arky-usage (NEW)
  depends on: arky-error, arky-protocol

arky-error (enhanced)
  depends on: regex (new dep)

arky-provider (enhanced)
  depends on: arky-usage (new dep)

arky-claude-code (enhanced)
  depends on: arky-usage, arky-mcp, arky-tools, arky-hooks (new deps)

arky-codex (enhanced)
  depends on: arky-usage, arky-mcp, arky-tools, arky-hooks (new deps)

arky-server (enhanced)
  depends on: arky-usage (new dep), arky-core (new dep), subtle (new dep)

arky-core (enhanced)
  depends on: arky-usage (new dep)
```

### Component Overview

```
                    ┌──────────────┐
                    │  arky-server │ POST /v1/chat/stream
                    │  (axum HTTP) │ GET /v1/models
                    │  + auth      │ Bearer token middleware
                    └──────┬───────┘
                           │
                    ┌──────┴───────┐
                    │  arky-core   │ Agent loop
                    │  + usage     │ Capability validation
                    │  aggregation │ Usage accumulation
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
     ┌────────┴───┐ ┌─────┴─────┐ ┌───┴─────────┐
     │arky-claude │ │arky-codex │ │arky-provider│
     │   -code    │ │           │ │  (traits)   │
     │+classifier │ │+registry  │ │+capability  │
     │+reasoning  │ │+events    │ │ validation  │
     │+config     │ │+config    │ │+model-prefix│
     │+tool bridge│ │+model svc │ │ inference   │
     └────────────┘ └───────────┘ └─────────────┘
              │            │            │
     ┌────────┴────────────┴────────────┴──┐
     │         arky-usage (NEW)            │
     │  NormalizedUsage, UsageAggregator   │
     │  ProviderMetadataExtractor          │
     │  ModelCost                          │
     └────────────────┬───────────────────-┘
                      │
     ┌────────────────┴───────────────────┐
     │           arky-error               │
     │  ErrorClassifier + pattern registry│
     │  format_for_agent()                │
     │  ErrorCategory enum                │
     └───────────────────────────────────-┘
```

## Implementation Design

### Core Interfaces

#### ErrorClassifier (arky-error)

```rust
pub enum ErrorCategory {
    Authentication, RateLimit, QuotaExceeded,
    ContextWindowExceeded, InvalidRequest, Timeout,
    SpawnFailure, StreamCorruption, ToolExecution,
    Network, ApiError, Unknown,
}

pub struct ErrorPattern {
    pub regex: Regex,
    pub error_code: &'static str,
    pub category: ErrorCategory,
    pub is_retryable: bool,
    pub http_status: u16,
}

pub struct ErrorInput<'a> {
    pub stderr: Option<&'a str>,
    pub message: Option<&'a str>,
    pub status_code: Option<u16>,
    pub exit_code: Option<i32>,
    pub error_code: Option<&'a str>,
}

pub struct ClassifiedResult {
    pub error_code: &'static str,
    pub category: ErrorCategory,
    pub is_retryable: bool,
    pub http_status: u16,
    pub matched_pattern: Option<String>,
}

pub struct ErrorClassifier { /* pattern registry */ }

impl ErrorClassifier {
    pub fn new() -> Self;
    pub fn register_patterns(
        &mut self,
        provider_id: &str,
        patterns: Vec<ErrorPattern>,
    );
    pub fn classify(&self, input: &ErrorInput) -> ClassifiedResult;
    pub fn format_for_agent(
        &self,
        result: &ClassifiedResult,
        original_error: &str,
        attempt: u32,
    ) -> String;
}
```

#### NormalizedUsage (arky-usage)

```rust
pub struct NormalizedUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub input_details: InputTokenDetails,
    pub output_details: OutputTokenDetails,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<f64>,
}

pub struct UsageAggregator { /* per-turn, per-session */ }

impl UsageAggregator {
    pub fn new() -> Self;
    pub fn accumulate_chunk(&mut self, chunk_usage: &Usage);
    pub fn accumulate_turn(&mut self, turn_usage: &NormalizedUsage);
    pub fn session_total(&self) -> NormalizedUsage;
    pub fn current_turn(&self) -> Option<&NormalizedUsage>;
}

pub trait ProviderMetadataExtractor: Send + Sync {
    fn extract(&self, raw: &Value) -> ProviderMetadata;
}

pub struct ProviderMetadata {
    pub session_id: Option<String>,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<f64>,
    pub raw_usage: Option<Value>,
    pub warnings: Vec<String>,
}
```

#### AgentEvent Reasoning Variants (arky-protocol)

```rust
// Added to existing AgentEvent enum:
pub enum AgentEvent {
    // ... existing 11 variants ...

    ReasoningStart {
        meta: EventMetadata,
        reasoning_id: String,
    },
    ReasoningDelta {
        meta: EventMetadata,
        reasoning_id: String,
        delta: String,
    },
    ReasoningComplete {
        meta: EventMetadata,
        reasoning_id: String,
        full_text: String,
    },
}
```

#### CodexServerRegistry (arky-codex)

```rust
pub struct CodexServerRegistry { /* Mutex<HashMap<String, RegistrySlot>> */ }

impl CodexServerRegistry {
    pub fn new(default_idle_shutdown: Duration) -> Self;
    pub async fn acquire(
        &self,
        config: &CodexProviderConfig,
    ) -> Result<CodexLease, ProviderError>;
}

pub struct CodexLease {
    server: Arc<CodexAppServer>,
    // Drop decrements refcount
}

pub struct CodexAppServer {
    // Long-lived process, RpcTransport, lazy init
    pub async fn ensure_ready(&self) -> Result<(), ProviderError>;
    pub async fn start_thread(&self, ...) -> ...;
    pub async fn start_turn(&self, ...) -> ...;
}
```

#### Chat Stream Endpoint (arky-server)

```rust
// POST /v1/chat/stream
pub struct ChatStreamRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub system_prompt: Option<String>,
    pub session_key: Option<String>,
    pub resume_session: Option<bool>,
    pub max_steps: Option<u32>,
    pub reasoning_effort: Option<ReasoningEffort>,
}
```

### Data Models

#### ReasoningEffort (arky-protocol)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
    XHigh,
}
```

#### ModelInfo (arky-provider)

```rust
pub struct ModelInfo {
    pub id: String,
    pub display_name: Option<String>,
    pub provider_id: ProviderId,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_reasoning: bool,
    pub cost: Option<ModelCost>,
}
```

#### CapabilityWarning (arky-provider)

```rust
pub struct CapabilityWarning {
    pub capability: String,
    pub message: String,
    pub severity: WarningSeverity,
}

pub fn validate_capabilities(
    request: &ProviderRequest,
    caps: &ProviderCapabilities,
) -> Vec<CapabilityWarning>;
```

### API Endpoints

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| `POST` | `/v1/chat/stream` | Start streaming chat, returns SSE | P0 |
| `GET` | `/v1/models` | List available models (OpenAI-compatible) | P0 |
| `GET` | `/health` | Health check (existing) | -- |
| `GET` | `/ready` | Readiness check (existing) | -- |
| `GET` | `/sessions` | List sessions (existing) | -- |
| `GET` | `/sessions/{id}/events` | SSE event stream (existing, enhanced with seq IDs) | P1 |

## Integration Points

### Claude Code CLI

- **Binary**: `claude` CLI subprocess via `ProcessManager` + `StdioTransport`
- **New CLI flags**: `--agents`, `--permission-mode`, `--output-format json_schema`, `--max-turns`, `--max-thinking-tokens`, reasoning effort, hooks config, MCP server config, plugin paths, sandbox config
- **Error patterns**: 18 regex patterns registered with `ErrorClassifier` during provider init

### Codex App Server

- **Binary**: `codex` CLI subprocess via `CodexAppServer` (long-lived)
- **Protocol**: JSON-RPC 2.0 over stdio (existing `RpcTransport`)
- **New RPC methods**: `models/list` (with pagination), `thread/compact`
- **Registry**: `CodexServerRegistry` manages shared instances with ref-counting
- **Error patterns**: 11 regex patterns registered with `ErrorClassifier`

### MCP Tool Bridge

- Both providers wire `arky-tools` definitions through `arky-mcp` as MCP servers
- Claude Code: passes MCP server configs via `--mcp-server` CLI flag
- Codex: starts HTTP bridge server, passes URL via `rmcp_client` config override

## Impact Analysis

| Affected Component | Type of Impact | Description & Risk Level | Required Action |
|---|---|---|---|
| `arky-protocol::AgentEvent` | Enum expansion (breaking) | Adds 3 reasoning variants. Medium risk. | Update all match arms across crates |
| `arky-error::ClassifiedError` | Non-breaking enhancement | ErrorClassifier is additive. Low risk. | None |
| `arky-provider::ProviderCapabilities` | Struct expansion | Adds image_inputs, extended_thinking, code_execution fields. Low risk. | Defaults preserve backwards compat |
| `arky-server` routes | New endpoints | Adds /v1/chat/stream, /v1/models. Low risk. | None (additive) |
| `arky-session::InMemorySessionStore` | Behavior change | Adds TTL eviction and capacity limits. Medium risk. | Existing tests updated |
| `arky-codex::CodexProvider` | Architecture change | Server registry replaces spawn-per-stream. High risk. | Comprehensive integration tests |
| `Cargo.toml` workspace | New crate | Adds arky-usage. Low risk. | Update workspace members |

## Testing Approach

### Unit Tests

- **arky-error**: ErrorClassifier with mock patterns, format_for_agent output validation, category matching
- **arky-usage**: NormalizedUsage computation, UsageAggregator accumulation, ModelCost calculation, provider metadata extraction per provider family
- **arky-protocol**: AgentEvent reasoning variant serde round-trip, ReasoningEffort enum
- **arky-claude-code**: Error pattern matching against fixture stderr strings, reasoning block parsing from fixture streams, config serialization to CLI args
- **arky-codex**: Event dispatcher for all 40+ notification types, text accumulator with reasoning phases, config override building, model service with mock RPC, server registry lifecycle (acquire/release/idle/reconfigure)
- **arky-server**: Chat stream request validation, auth middleware (valid/invalid/missing token), model listing response format, SSE sequence IDs
- **arky-provider**: Capability validation with various request/capability combinations, model-prefix inference

### Integration Tests

- **End-to-end streaming**: Agent -> Provider -> stream -> events -> SSE (with fixture CLI output)
- **Codex server lifecycle**: Registry acquire -> multiple turns -> idle shutdown -> re-acquire
- **Session persistence**: Create session via /v1/chat/stream -> query via /sessions/{id}/messages
- **Usage accumulation**: Multi-turn conversation -> verify per-turn and session-total usage
- **Error classification**: Inject fixture stderr -> verify classified error code and retryability

## Development Sequencing

### Phase 1: P0 — Production Blockers

Build order within Phase 1 follows dependency graph:

1. **arky-error enhancements** (no deps) — ErrorClassifier, ErrorPattern, ErrorCategory, format_for_agent
2. **arky-protocol enhancements** (no deps) — ReasoningStart/Delta/Complete variants, ReasoningEffort, FinishReason
3. **arky-usage crate** (depends on arky-error, arky-protocol) — NormalizedUsage, UsageAggregator, ModelCost, ProviderMetadataExtractor
4. **arky-claude-code P0** (depends on 1-3) — error patterns, reasoning parsing, config expansion (60 fields)
5. **arky-codex P0** (depends on 1-3) — server registry, event dispatcher (40+ events), text accumulator reasoning, config expansion (40 fields), model service
6. **arky-server P0** (depends on 3) — POST /v1/chat/stream, GET /v1/models, bearer auth middleware

### Phase 2: P1 — Important Completeness

7. **arky-provider enhancements** — capability validation, model-prefix inference, generate_from_stream enhancement
8. **arky-tools enhancements** — tool output truncation
9. **arky-claude-code P1** — tool bridge/MCP wiring, generate override with truncation recovery, hooks integration, message conversion with images, structured output/JSON mode, permission modes, finish reason mapping, warnings, subagent config passthrough
10. **arky-codex P1** — stream pipeline (abort, finalization), duplicate detection, cancellation, tool payload builders, request preparation, hooks integration, subagent config passthrough
11. **arky-core enhancements** — usage aggregation in turn loop, capability validation calls
12. **arky-server P1** — SSE sequence IDs + [DONE] sentinel, runtime client abstraction
13. **arky-session enhancements** — in-memory TTL + capacity, reasoning effort resolution
14. **Cross-cutting P1** — model discovery service, runtime usage aggregation, provider metadata extraction, token consumption from chunks/results

### Phase 3: P2 — Polish

15. **arky-claude-code P2** — plugin support, sandbox config, streaming input/message injection, stream recovery, nested tool preview events, tool input serialization, MCP custom/combined servers, debug/verbose config, env passthrough, settings warnings, image handling
16. **arky-codex P2** — thread compaction, scheduler queue overflow, env sanitization
17. **Cross-cutting P2** — model cost computation, xhigh detection, provider family gateway classification, compound session key lookup, runtime error union, native event utils, configuration validation with rich schemas

### Technical Dependencies

- `regex` crate: needed for ErrorClassifier pattern matching
- `subtle` crate: needed for timing-safe token comparison in auth middleware
- No external service dependencies — all providers communicate with local CLI processes

## Monitoring & Observability

- **Tracing**: All new code uses `tracing` crate (enforced by clippy)
- **Usage metrics**: `arky-usage` emits tracing events for token consumption per turn
- **Error classification**: ErrorClassifier logs classified category and retryability at `warn` level
- **Server auth**: Failed auth attempts logged at `warn` level with client IP
- **Codex registry**: Server lifecycle events (acquire, release, idle shutdown, respawn) at `info` level

## Technical Considerations

### Key Decisions

All 10 decisions are documented as ADRs in `tasks/prd-gaps/adrs/`:

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-001 | Full parity scope | Complete API surface for consumer migration |
| ADR-002 | New arky-usage crate | Cross-cutting concern needs clean dependency graph |
| ADR-003 | Centralized error classifier | formatForAgent and retry logic identical across providers |
| ADR-004 | Subagent config passthrough | CLI handles orchestration; SDK-level orchestration deferred |
| ADR-005 | Codex server registry | Matches proven TS design for process lifecycle |
| ADR-006 | Chat streaming endpoint | HTTP access for external consumers |
| ADR-007 | Typed config schemas | Type safety and validation for all settings |
| ADR-008 | Reasoning event variants | First-class lifecycle semantics for thinking blocks |
| ADR-009 | Generate with override | DRY helper + provider-specific truncation recovery |
| ADR-010 | Priority-based phasing | Critical gaps first, uniform SDK improvement |

### Known Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| TS codebase evolves during implementation | Medium | Snapshot feature set, track delta separately |
| Codex server registry lifecycle bugs | High | Drop-based lease release, comprehensive integration tests, Mutex for all mutations |
| AgentEvent enum growth (11 -> 14) | Low | `#[non_exhaustive]` already present, match arms use wildcard |
| Config struct maintenance burden | Medium | Serde deny_unknown_fields in strict mode, CI check for config drift |
| Phase 1 scope too large for single sprint | Medium | Sub-phases within P0 following dependency order |

### Standards Compliance

- All code follows `CLAUDE.md` coding standards (edition 2024, 90 char width, snake_case)
- Error handling via `thiserror` with `ClassifiedError` implementations
- Async patterns use `tokio` with `CancellationToken` for cooperative cancellation
- Tests use `pretty_assertions::assert_eq` (enforced by clippy)
- No `unwrap()` in library code, no `log` crate, no `todo!()`/`dbg!()`

## Reference Files

### Gap Analysis Documents

- `tasks/prd-gaps/analysis_core_runtime.md` — 18 gaps, ~55-60% coverage
- `tasks/prd-gaps/analysis_claude_code.md` — 18 gaps, ~25-30% coverage
- `tasks/prd-gaps/analysis_codex.md` — 16 gaps, ~35-40% coverage
- `tasks/prd-gaps/analysis_server_session_usage.md` — 11 gaps, ~40-45% coverage

### ADR Documents

- `tasks/prd-gaps/adrs/adr-001-scope-full-parity.md`
- `tasks/prd-gaps/adrs/adr-002-usage-tracking-crate.md`
- `tasks/prd-gaps/adrs/adr-003-centralized-error-classifier.md`
- `tasks/prd-gaps/adrs/adr-004-subagent-config-passthrough.md`
- `tasks/prd-gaps/adrs/adr-005-codex-server-registry.md`
- `tasks/prd-gaps/adrs/adr-006-chat-streaming-endpoint.md`
- `tasks/prd-gaps/adrs/adr-007-typed-config-schemas.md`
- `tasks/prd-gaps/adrs/adr-008-reasoning-event-variants.md`
- `tasks/prd-gaps/adrs/adr-009-generate-with-provider-override.md`
- `tasks/prd-gaps/adrs/adr-010-phasing-by-priority.md`

### TypeScript Source (compozy-code)

- `providers/core/src/` — hooks, error classifier, token consumption, tools bridge
- `providers/runtime/src/` — server, session, usage, capabilities, reasoning, models, adapters
- `providers/claude-code/src/` — classifier, conversion, stream, tools, MCP, generate, services
- `providers/codex/src/` — server, streaming, config, errors, model, bridge, util

### Original Techspec

- `compozy-code/tasks/prd-rust-providers/techspec.md`
- `compozy-code/tasks/prd-rust-providers/adrs/`
