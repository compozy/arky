# Gap Analysis: Provider Core & Runtime

## Summary

The TypeScript `compozy-code` Provider Core (`providers/core/src/`) and Runtime (`providers/runtime/src/`) form a comprehensive AI agent framework built on Effect.ts, Zod validation, and the Vercel AI SDK (`@ai-sdk/provider-utils`, `ai`). Together they span approximately 60+ source files delivering: a lifecycle hook system with 6 event types and shell/callback/prompt hook execution; an error classifier with structured self-correction context; token consumption normalization across provider families; a full tool bridge with pre/post hook integration and output truncation; model discovery with multi-source priority merging; reasoning effort resolution with xhigh detection; a capability validation layer; a provider registry with model-prefix inference; session stores (in-memory with TTL + SQLite with reverse indexing); an SSE-based streaming server with bearer auth; a runtime client wrapper; and extensive branded types via Effect Schema.

The Rust `arky` workspace (primarily `arky-provider`, `arky-core`, `arky-protocol`, `arky-hooks`, `arky-session`, `arky-tools`, `arky-server`, `arky-error`, `arky-mcp`) provides a structurally sound and architecturally mature foundation. The `Provider` trait, `ProviderRegistry`, `Agent` orchestration loop (with actor model, command queue, turn runtime, steering, and follow-up), `Hooks` trait (6 lifecycle events), `SessionStore` (in-memory + SQLite), `ToolRegistry`, `ClassifiedError` trait, `McpClient`/`McpServer`/`McpToolBridge`, and the Axum-based HTTP server are all present and well-designed. In several areas -- particularly agent orchestration (steering, follow-up, abort), process management (`ProcessManager`, `ManagedProcess`, `RestartPolicy`), stdio transport, replay persistence, and event metadata -- the Rust implementation exceeds TS functionality.

However, significant gaps exist in: token consumption / usage tracking (entirely absent), model discovery (absent), model cost computation (absent), reasoning effort resolution and xhigh detection (absent), capability validation (partial -- fewer flags and no validation function), provider model-prefix inference (absent), tool output truncation (absent), error classifier formatting for agent self-correction (absent -- `ClassifiedError` trait exists but no `formatForAgent` equivalent), subagent orchestration configuration (absent), runtime client wrapper (absent), and bearer token auth service (absent). The overall functional surface coverage of the TS core+runtime by the Rust workspace is estimated at 55-60%, with the Rust side being architecturally stronger in areas it does cover but missing several cross-cutting concerns that the TS side provides as shared infrastructure.

## Feature Comparison Matrix

| Feature | compozy-code (TS) | arky (Rust) | Status | Priority |
|---------|-------------------|-------------|--------|----------|
| Provider trait / adapter interface | `ProviderAdapter` with Effect-based streamText, getSessionId, resumeSession, clearSession | `Provider` trait with descriptor(), stream(), generate() | Complete | -- |
| Provider registry | `ProviderRegistryService` with model-prefix auto-inference (claude- -> claude-code, gpt- -> codex) | `ProviderRegistry` with RwLock<BTreeMap>, register/get/list/remove/clear | Partial | P1 |
| Provider capabilities | `RuntimeCapabilities` with 10+ flags: imageInputs, extendedThinking, toolSupport, agentSupport, sessionResume, streaming, reasoning, followUp, steering, codeExecution | `ProviderCapabilities` with 7 flags: streaming, generate, tool_calls, mcp_passthrough, session_resume, steering, follow_up | Partial | P1 |
| Capability validation | `validateCapabilities()` checking imageInputs, extendedThinking, toolSupport, agentSupport, sessionResume; `messagesHaveImageInputs()`, `hasAgentOptions()`, `hasToolRegistrations()` | None -- capabilities are declared but never validated against requests | Missing | P1 |
| Hooks lifecycle events | 6 events: PreToolUse, PostToolUse, SessionStart, SessionEnd, Stop, UserPromptSubmit via `executeHookEvent()` with command/callback/prompt hook types | 6 events: before_tool_call, after_tool_call, session_start, session_end, on_stop, user_prompt_submit via `Hooks` trait | Complete | -- |
| Hook chain / composition | Function-based composition with matchers, timeouts, AbortSignal, concurrent execution | `HookChain` with ordered execution, `FailureMode` (FailOpen/FailClosed), timeout, `CancellationToken` | Complete | -- |
| Shell command hooks | Command hooks with shell execution, stdout parsing, timeout | `ShellCommandHook` with `ToolMatcher`, shell execution | Complete | -- |
| Hook result types | `PreToolUseOutcome` (Allow/Block/Error), `PostToolUseUpdate`, `PromptUpdate`, `SessionStartUpdate`, `StopDecision` | `Verdict` (Allow/Block/Error), `ToolResultOverride`, `PromptUpdate`, `SessionStartUpdate`, `StopDecision` | Complete | -- |
| Error classification trait | `ErrorClassifier` class with `isRetryable()`, `extractContext()`, `formatForAgent()` | `ClassifiedError` trait with error_code, is_retryable, retry_after, http_status, correction_context | Partial | P0 |
| Error self-correction formatting | `formatForAgent()` producing structured retry messages with attempt number and field-level suggestions for Zod errors | None -- `correction_context` exists as optional field but no formatting logic | Missing | P0 |
| Error context attachment | `attachErrorContext()` copying stack/cause/data from source to mapped error | Rust errors use `thiserror` with `#[source]` for chain propagation | Partial | P2 |
| Token consumption normalization | `NormalizedTokenConsumption`, `NormalizedTokenBreakdown`, `resolveProviderMetadata()`, `computeTokenTotal()` with per-provider metadata key fallbacks | `Usage` struct in arky-protocol with input_tokens, output_tokens, total_tokens; `InputTokenDetails`, `OutputTokenDetails` (types exist but no normalization logic) | Partial | P0 |
| Token consumption from chunks | `extractTokenConsumptionFromChunk()` incrementally extracting usage from stream chunks | None -- usage only captured from final `AgentEvent::TurnComplete` | Missing | P1 |
| Token consumption from result | `resolveTokenConsumptionFromResult()`, `resolveTokenConsumptionFromUsage()` | None -- no aggregation helpers | Missing | P1 |
| Provider metadata extraction | `extractProviderMetadata()` with sessionId, costUsd, durationMs, rawUsage, warnings | None -- metadata not extracted from provider responses | Missing | P1 |
| Model discovery service | `ModelDiscoveryService` with multi-source model listing, priority-based merging, `ModelInfo` schema | None | Missing | P1 |
| Model cost tracking | `ModelCost` schema, `computeEstimatedCost()` per model family | None | Missing | P2 |
| Reasoning effort resolution | `ReasoningEffort` (low/medium/high/xhigh), `resolveReasoningForProvider()`, `resolveClaudeMaxThinkingTokens()`, `CLAUDE_REASONING_TOKEN_BUDGET` | None -- no reasoning effort concept in Rust types | Missing | P1 |
| xhigh reasoning detection | `XHIGH_CAPABLE_MODEL_IDS`, `supportsXHighReasoning()` for GPT-5.x models | None | Missing | P2 |
| Tool bridge with hook integration | `createToolExecutor()` with full pre/post hook integration, `bindTools()`, `listToolsFromEntries()` | `TurnRuntime` in arky-core handles tool execution with hook integration inline in the turn loop | Complete | -- |
| Tool output truncation | 100KB default truncation with array/object/string truncation strategies | None -- tool results passed through without size limits | Missing | P1 |
| Tool registry | ~600+ line ToolRegistry with per-provider bridge factories, aggregated bridges, canonical names | `ToolRegistry` in arky-tools with register/get/list/execute; `ToolIdCodec` for provider-specific name encoding | Partial | P1 |
| Tool ID codec | Provider-specific encoding: mcp__compozy__server__tool, codex__compozy__, compozy_ | `StaticToolIdCodec`, `create_claude_code_tool_id_codec()`, `create_codex_tool_id_codec()`, `create_opencode_tool_id_codec()` | Complete | -- |
| MCP client | MCP SDK client with Effect integration | `McpClient` with stdio and HTTP transports, OAuth auth, connection state management | Complete | -- |
| MCP server | `createMcpServerFromTools()` converting AI SDK tools to MCP tools | `McpServer` with `McpToolBridge` for bidirectional tool bridging, HTTP and stdio server handles | Complete | -- |
| MCP HTTP server | `createMcpHttpServer()` using StreamableHTTPServerTransport | `McpHttpServerHandle` with HTTP transport | Complete | -- |
| Provider family resolution | `ProviderFamilyId`, `resolveProviderFamily()`, gateway provider classification | `ProviderFamily` enum (ClaudeCode, Codex, OpenCode, Custom) | Partial | P2 |
| Provider model-prefix inference | Auto-inference map: claude- -> claude-code, gpt- -> codex, etc. | None -- provider must be explicitly specified | Missing | P1 |
| Session store trait | `SessionStoreService` interface with Effect services | `SessionStore` async trait with get/save/list/delete/messages | Complete | -- |
| In-memory session store | `createInMemorySessionStore()` with TTL and capacity limits | `InMemorySessionStore` with basic HashMap storage | Partial | P2 |
| SQLite session store | `createSqliteSessionStore()` with reverse index, legacy migration | `SqliteSessionStore` with event persistence and replay | Complete | -- |
| Agent orchestration loop | `Runtime` with `makeRuntime()`, stream validation, session sync, usage tracking | `Agent` with actor model, command queue, `TurnRuntime`, steering, follow-up, abort, new_session, resume | Complete+ | -- |
| Agent builder | Layer composition via `composeRuntimeLayers()` with `defaultToolRegistryConfig` | `AgentBuilder` with provider, tools, temporary_tools, hooks, session_store, config, model, system_prompt | Complete | -- |
| Steering (mid-turn injection) | Not present as first-class API | `Agent::steer()` for mid-turn message injection | Complete+ | -- |
| Follow-up turns | Not present as first-class API | `Agent::follow_up()` for continuing after tool results | Complete+ | -- |
| Abort / cancellation | Effect interruption model | `Agent::abort()` with `CancellationToken` propagation | Complete | -- |
| Event subscription / fanout | Effect Stream-based event emission | `EventSubscription` with broadcast channel, `AgentEventStream` | Complete | -- |
| Replay / session restore | Session replay via store | `ReplayWriter` with batched event persistence, `TurnCheckpoint`, `ReplayCursor` | Complete | -- |
| HTTP server routes | /health, /v1/models, /v1/chat/stream | /health, /ready, /providers/health, /providers/{id}/health, /sessions, /sessions/{id}, /sessions/{id}/messages, /sessions/{id}/events (SSE), /sessions/{id}/replay | Complete+ | -- |
| SSE event streaming | `RuntimeSseWriter`, `streamRuntimeSse()` converting Effect Stream to ReadableStream | SSE via `/sessions/{id}/events` route with Axum SSE support | Complete | -- |
| Bearer token auth | `AuthService` with bearer token extraction, timing-safe comparison | None -- no auth middleware on the HTTP server | Missing | P1 |
| Runtime client wrapper | `RuntimeClientService`, `RuntimeClientAsync` with dispose, session management | None -- consumers interact with `Agent` directly | Missing | P2 |
| Subagent orchestration | `RuntimeAgentConfig` with named subagents, model/tools/prompt per agent | None -- single agent model only | Missing | P2 |
| Branded types / IDs | `ProviderId` as Schema.Literal of 11 providers, `SessionId` as branded string | `ProviderId(String)`, `SessionId(Uuid)`, `TurnId(Uuid)` as newtypes | Complete | -- |
| Protocol types (messages, events) | AI SDK types + Effect Schema definitions | `Message`, `ContentBlock`, `Role`, `AgentEvent` (11 variants), `EventMetadata`, `StreamDelta` | Complete | -- |
| Process management | SDK handles internally for Claude Code | `ProcessManager`, `ManagedProcess`, `RestartPolicy`, kill-on-drop | Complete+ | -- |
| Stdio transport | SDK handles internally | `StdioTransport` with buffered read/write, cancellation | Complete+ | -- |
| Runtime usage tracking | `RuntimeConsumption` class resolving usage from V3 and LanguageModel shapes | None -- no runtime-level usage aggregation | Missing | P1 |
| Runtime error union types | `RuntimeHostError` union of 11 error types | Per-crate error enums implementing `ClassifiedError` | Partial | P1 |
| Configuration validation | Zod schemas for all config surfaces | `arky-config` crate exists but scope unclear from lib.rs | Partial | P2 |

## Detailed Gap Analysis

### 1. Error Self-Correction Formatting

- **TS Location**: `providers/core/src/error-classifier.ts` -- `formatForAgent()` method
- **Rust Status**: The `ClassifiedError` trait in `arky-error` provides `correction_context() -> Option<String>` but there is no formatting logic that produces structured retry messages with attempt numbers, field-level suggestions for validation errors, or agent-consumable error descriptions.
- **Complexity**: Medium
- **Description**: The TS `ErrorClassifier.formatForAgent()` takes an error and attempt number, producing a structured message that guides the model to self-correct. For Zod validation errors, it extracts field-level suggestions showing which fields failed and what was expected. This is critical for robust retry loops where the agent needs actionable feedback to fix its tool call inputs. The Rust side needs: (a) a `format_for_agent(error: &dyn ClassifiedError, attempt: u32) -> String` utility, (b) field-level extraction for serde validation errors, and (c) integration into the turn retry path in `arky-core`.
- **Dependencies**: Enhances the existing `ClassifiedError` trait without changing it; integrates into `arky-core/src/turn.rs` retry logic.

### 2. Token Consumption / Usage Tracking System

- **TS Location**: `providers/core/src/token-consumption.ts`, `providers/runtime/src/usage/consumption.ts`, `providers/runtime/src/usage/token-consumption.ts`, `providers/runtime/src/usage/types.ts`, `providers/runtime/src/usage/metadata-extractor.ts`
- **Rust Status**: The `Usage` struct exists in `arky-protocol/src/request.rs` with `input_tokens`, `output_tokens`, `total_tokens`, and optional `InputTokenDetails`/`OutputTokenDetails`. However, there is no normalization logic, no per-provider metadata key fallbacks, no incremental extraction from stream chunks, no aggregation helpers, and no runtime-level consumption tracking.
- **Complexity**: High
- **Description**: The TS implementation spans 5 files and provides: (a) `NormalizedTokenConsumption` and `NormalizedTokenBreakdown` types with cache read/write/reasoning breakdowns, (b) `resolveProviderMetadata()` with provider-specific metadata key fallbacks (e.g., `anthropic.cacheCreationInputTokens` vs `google.cachedContentTokenCount`), (c) `computeTokenTotal()` for aggregation, (d) `extractTokenConsumptionFromChunk()` for incremental stream extraction, (e) `resolveTokenConsumptionFromResult()` / `resolveTokenConsumptionFromUsage()` for final accumulation, (f) `RuntimeConsumption` class resolving from V3 and LanguageModel usage shapes, (g) `extractProviderMetadata()` extracting sessionId, costUsd, durationMs, rawUsage, and warnings. The Rust side needs a new module (likely in `arky-protocol` or a new `arky-usage` crate) that normalizes usage across providers and integrates into the turn runtime for per-turn and per-session tracking.
- **Dependencies**: Requires `arky-protocol` Usage types (already exist), integration into `arky-core/src/turn.rs` and potentially `arky-provider` stream processing.

### 3. Model Discovery Service

- **TS Location**: `providers/runtime/src/models/model-discovery-service.ts`
- **Rust Status**: Completely missing. No equivalent concept exists in the Rust workspace.
- **Complexity**: High
- **Description**: The TS `ModelDiscoveryService` provides multi-source model listing with priority-based merging. It queries multiple sources (provider APIs, local config, cached registries) and merges results using priority rules. The `ModelInfo` schema carries model ID, display name, family, capabilities, context window, max output tokens, and pricing. This powers model selection UIs and runtime validation. The Rust side would need: (a) a `ModelInfo` type in `arky-protocol`, (b) a `ModelDiscoveryService` trait or struct in `arky-provider`, (c) per-provider discovery implementations, and (d) caching/merging logic.
- **Dependencies**: New types in `arky-protocol`, new trait/service in `arky-provider`.

### 4. Reasoning Effort Resolution

- **TS Location**: `providers/runtime/src/reasoning/resolve-reasoning.ts`, `providers/runtime/src/reasoning/xhigh-detection.ts`
- **Rust Status**: Completely missing. No `ReasoningEffort` type, no resolution logic, no thinking token budget constants.
- **Complexity**: Medium
- **Description**: The TS implementation defines `ReasoningEffort` as an enum (low/medium/high/xhigh) and provides `resolveReasoningForProvider()` which maps effort levels to provider-specific parameters (e.g., `resolveClaudeMaxThinkingTokens()` maps effort to token budgets with `CLAUDE_REASONING_TOKEN_BUDGET`). The xhigh detection module maintains `XHIGH_CAPABLE_MODEL_IDS` for GPT-5.x models and `supportsXHighReasoning()`. The Rust side needs: (a) a `ReasoningEffort` enum in `arky-protocol`, (b) resolution functions in `arky-provider` or a new `arky-reasoning` module, (c) integration into `ProviderRequest` or `ProviderSettings`.
- **Dependencies**: New types in `arky-protocol`, integration into `arky-provider/src/request.rs`.

### 5. Capability Validation

- **TS Location**: `providers/runtime/src/capabilities/capability-validator.ts`
- **Rust Status**: `ProviderCapabilities` exists in `arky-provider/src/descriptor.rs` with 7 boolean flags, but there is no validation function that checks whether a request is compatible with the provider's declared capabilities.
- **Complexity**: Medium
- **Description**: The TS `validateCapabilities()` inspects the incoming request and checks: (a) whether messages contain image inputs and the provider supports them, (b) whether extended thinking is requested and supported, (c) whether tools are registered and the provider supports tool calls, (d) whether agent/subagent options are present and supported, (e) whether session resume is requested and supported. It returns a list of validation errors/warnings. The Rust side needs: (a) additional capability flags (image_inputs, extended_thinking, code_execution at minimum), (b) a `validate_capabilities(request: &ProviderRequest, caps: &ProviderCapabilities) -> Vec<CapabilityWarning>` function, (c) integration into the agent turn entry point.
- **Dependencies**: Extends `ProviderCapabilities` in `arky-provider`, adds validation in `arky-core`.

### 6. Provider Model-Prefix Inference

- **TS Location**: `providers/runtime/src/services/provider-registry.ts`
- **Rust Status**: Missing. The `ProviderRegistry` in `arky-provider/src/registry.rs` requires explicit provider specification; it has no model-prefix-to-provider mapping.
- **Complexity**: Low
- **Description**: The TS `ProviderRegistryService` maintains a model prefix map (e.g., `claude-` maps to `claude-code`, `gpt-` maps to `codex`, `o1-`/`o3-`/`o4-` map to `codex`) enabling automatic provider inference from model IDs. This is a convenience feature that simplifies the API surface. The Rust side needs a configurable prefix map in `ProviderRegistry` with a `resolve_provider_for_model(model: &str) -> Option<ProviderId>` method.
- **Dependencies**: Self-contained within `arky-provider/src/registry.rs`.

### 7. Tool Output Truncation

- **TS Location**: `providers/core/src/tools-bridge.ts` -- `truncateToolOutput()` with 100KB default
- **Rust Status**: Missing. Tool results in the Rust turn loop are passed through without size limits.
- **Complexity**: Low
- **Description**: The TS tool bridge applies a configurable output truncation (default 100KB) with strategies for arrays (truncate elements), objects (truncate values), and strings (byte-level cut with marker). This prevents oversized tool outputs from consuming excessive context window tokens. The Rust side needs a `truncate_tool_output(result: &ToolResult, max_bytes: usize) -> ToolResult` utility, likely in `arky-tools`, integrated into `arky-core/src/turn.rs` after tool execution.
- **Dependencies**: New utility in `arky-tools`, integration into `arky-core`.

### 8. Bearer Token Auth Service

- **TS Location**: `providers/runtime/src/server/auth.ts`
- **Rust Status**: Missing. The Axum server in `arky-server` has no authentication middleware.
- **Complexity**: Low
- **Description**: The TS `AuthService` extracts bearer tokens from the Authorization header and performs timing-safe comparison against a configured secret. This is critical for production deployments where the runtime server must not be openly accessible. The Rust side needs an Axum middleware layer in `arky-server/src/middleware.rs` (which already exists for CORS) that performs constant-time token comparison using `subtle::ConstantTimeEq` or equivalent.
- **Dependencies**: Self-contained within `arky-server`, requires `subtle` crate or manual constant-time comparison.

### 9. Runtime Usage Aggregation

- **TS Location**: `providers/runtime/src/usage/consumption.ts`, `providers/runtime/src/usage/metadata-extractor.ts`
- **Rust Status**: Missing. No runtime-level usage aggregation exists; `Usage` is populated in individual events but not tracked across turns or sessions.
- **Complexity**: Medium
- **Description**: The TS `RuntimeConsumption` class resolves usage from both V3 and LanguageModel shapes, merging metadata into a unified consumption record per turn and per session. The `extractProviderMetadata()` function pulls out sessionId, costUsd, durationMs, rawUsage, and warnings from provider-specific metadata bags. The Rust side needs: (a) a `UsageTracker` or `ConsumptionAggregator` that accumulates per-turn and per-session usage, (b) metadata extraction integrated into stream processing, (c) exposure via `AgentEvent::TurnComplete` and session-level queries.
- **Dependencies**: Builds on token consumption types (#2), integrates into `arky-core/src/turn.rs`.

### 10. Model Cost Computation

- **TS Location**: `providers/runtime/src/models/model-cost.ts`
- **Rust Status**: Missing.
- **Complexity**: Low
- **Description**: The TS `ModelCost` schema defines per-model pricing (input cost per million tokens, output cost per million tokens) and `computeEstimatedCost()` calculates estimated dollar cost from token counts. This is useful for billing, observability, and budget enforcement. The Rust side needs a `ModelCost` struct and cost computation function, likely in `arky-protocol` or alongside model discovery.
- **Dependencies**: Depends on token consumption tracking (#2) for inputs; can be implemented standalone for static pricing.

### 11. Runtime Error Union / Unified Error Type

- **TS Location**: `providers/runtime/src/errors/unions.ts`
- **Rust Status**: Partial. Each Rust crate defines its own error enum (`ProviderError`, `CoreError`, `HookError`, `McpError`, `ServerError`) all implementing `ClassifiedError`. However, there is no unified runtime-level error union that consumers can match against.
- **Complexity**: Medium
- **Description**: The TS `RuntimeHostError` is a union of 11 error types providing a single type that runtime consumers can match against. In Rust, the individual error types are well-structured via `thiserror` and `ClassifiedError`, but there is no `RuntimeError` enum aggregating them. This is less critical in Rust because `Box<dyn ClassifiedError>` or `anyhow::Error` can serve as unification, but a dedicated enum would improve pattern matching ergonomics.
- **Dependencies**: Depends on all per-crate error types being stable.

### 12. Subagent Orchestration Configuration

- **TS Location**: `providers/runtime/src/types/runtime-options.ts` -- `RuntimeAgentConfig`
- **Rust Status**: Missing. The `Agent` in `arky-core` operates as a single agent; there is no subagent configuration or multi-agent composition.
- **Complexity**: High
- **Description**: The TS `RuntimeAgentConfig` defines named subagents with per-agent model, tools, and system prompt configuration. The runtime can delegate to subagents during a conversation. The Rust `Agent` already has a strong foundation (actor model, command queue, steering) that could support multi-agent composition, but the configuration types and dispatch logic are absent.
- **Dependencies**: New types in `arky-protocol` or `arky-core`, significant changes to agent orchestration loop.

### 13. In-Memory Session Store TTL and Capacity

- **TS Location**: `providers/runtime/src/session/in-memory-store.ts`
- **Rust Status**: Partial. `InMemorySessionStore` exists in `arky-session` but uses a basic HashMap without TTL-based expiration or capacity limits.
- **Complexity**: Low
- **Description**: The TS in-memory store supports configurable TTL (time-to-live) per session and a maximum capacity with eviction. This prevents unbounded memory growth in long-running processes. The Rust side needs: (a) TTL tracking per entry with lazy or background expiration, (b) capacity limit with LRU or FIFO eviction.
- **Dependencies**: Self-contained within `arky-session`.

### 14. Provider Family Resolution and Gateway Classification

- **TS Location**: `providers/runtime/src/protocol/provider-family.ts`
- **Rust Status**: Partial. `ProviderFamily` exists as an enum with `ClaudeCode`, `Codex`, `OpenCode`, `Custom(String)` variants. However, the TS version has richer gateway provider classification (distinguishing between direct providers and gateway/proxy providers like OpenRouter, Together, etc.) and a `resolveProviderFamily()` function.
- **Complexity**: Low
- **Description**: The TS provider family system classifies providers into direct and gateway categories, which affects how token consumption metadata is extracted (different providers expose usage in different metadata keys). The Rust side needs: (a) gateway vs. direct classification, (b) a `resolve_provider_family()` function, (c) integration with token consumption normalization.
- **Dependencies**: Ties into token consumption (#2) and provider registry (#6).

### 15. Runtime Client Wrapper

- **TS Location**: `providers/runtime/src/client/runtime-client.ts`
- **Rust Status**: Missing. Consumers interact with `Agent` directly.
- **Complexity**: Low
- **Description**: The TS `RuntimeClientAsync` wraps the Runtime with lifecycle management (dispose/cleanup), session convenience methods, and a simplified API surface. In Rust, the `Agent` already provides a clean API, but a higher-level client wrapper could add connection pooling, automatic reconnection for remote agents, and resource cleanup via `Drop`.
- **Dependencies**: Builds on `arky-core` Agent API.

### 16. SSE Writer for Runtime Streams

- **TS Location**: `providers/runtime/src/server/sse-writer.ts`
- **Rust Status**: Partial. The Axum server in `arky-server` has SSE support via the `/sessions/{id}/events` route, but there is no standalone `SseWriter` abstraction that converts arbitrary event streams to SSE format outside the server context.
- **Complexity**: Low
- **Description**: The TS `RuntimeSseWriter` and `streamRuntimeSse()` convert an Effect Stream to a `ReadableStream<Uint8Array>` with proper SSE formatting (event types, data serialization, keep-alive). The Rust server handles this inline via Axum's SSE support, which is functionally equivalent for the server use case. A standalone writer would only be needed if SSE is used outside the Axum server (e.g., in a different transport).
- **Dependencies**: Self-contained; may not be needed if Axum's SSE covers all use cases.

### 17. Configuration Validation with Rich Schema

- **TS Location**: Various Zod schemas across core and runtime
- **Rust Status**: Partial. `arky-config` crate exists but its scope relative to the comprehensive Zod-validated configuration in TS is unclear. Individual crate configs use plain Rust structs with serde.
- **Complexity**: Medium
- **Description**: The TS codebase uses Zod schemas extensively for validating configuration at load time, providing rich error messages when configuration is invalid. The Rust crates use serde deserialization which provides basic structural validation but lacks the custom validation rules, default values with documentation, and user-friendly error messages that Zod provides.
- **Dependencies**: Cross-cutting; touches `arky-config` and all crate configuration types.

### 18. Tool Registry Per-Provider Bridge Factories

- **TS Location**: `providers/runtime/src/tools/registry.ts`, `providers/runtime/src/tools/bridge.ts`, `providers/runtime/src/services/layers.ts`
- **Rust Status**: Partial. `ToolRegistry` in `arky-tools` provides basic registration and lookup. `ToolIdCodec` handles per-provider name encoding. However, the TS concept of per-provider bridge factories (where each provider gets a custom tool bridge that adapts the canonical tool interface to the provider's expected format) is not present.
- **Complexity**: Medium
- **Description**: The TS `ToolRegistry` (~600+ lines) maintains per-provider `ToolBridgeFactory` instances that produce `ToolBridgeInstance` objects. Each bridge factory knows how to adapt the canonical tool definition and results to a specific provider's format (e.g., Claude expects different tool input schemas than Codex). The `defaultToolRegistryConfig` in layers.ts wires up factory functions for each known provider. The Rust `ToolIdCodec` handles naming but not the full bridge pattern.
- **Dependencies**: Extends `arky-tools`, integrates with `arky-provider`.

## Files Reference

### TypeScript (compozy-code) - Core
- `providers/core/src/hooks.ts` -- Lifecycle hook system (813 lines)
- `providers/core/src/error-classifier.ts` -- Error classifier with formatForAgent
- `providers/core/src/errorContext.ts` -- Error context attachment
- `providers/core/src/token-consumption.ts` -- Token normalization and provider metadata
- `providers/core/src/tools-bridge.ts` -- Tool executor with hook integration and truncation
- `providers/core/src/tool-provider.ts` -- RetryConfig, ToolProviderSettings, ToolProviderCapabilities
- `providers/core/src/mcp-server.ts` -- MCP server from tools
- `providers/core/src/mcp-http-server.ts` -- MCP HTTP server

### TypeScript (compozy-code) - Runtime
- `providers/runtime/src/adapters/adapter.ts` -- ProviderAdapter interface
- `providers/runtime/src/adapters/types.ts` -- AdapterConfig, ClaudeCodeAdapterDefinition
- `providers/runtime/src/capabilities/capability-validator.ts` -- Capability validation
- `providers/runtime/src/client/runtime-client.ts` -- RuntimeClientAsync wrapper
- `providers/runtime/src/models/model-discovery-service.ts` -- Model discovery
- `providers/runtime/src/models/model-cost.ts` -- Model cost computation
- `providers/runtime/src/protocol/branded.ts` -- Branded types (ProviderId, SessionId)
- `providers/runtime/src/protocol/provider-family.ts` -- Provider family resolution
- `providers/runtime/src/reasoning/resolve-reasoning.ts` -- Reasoning effort resolution
- `providers/runtime/src/reasoning/xhigh-detection.ts` -- xhigh model detection
- `providers/runtime/src/services/layers.ts` -- Layer composition, tool bridge factories
- `providers/runtime/src/services/provider-registry.ts` -- Provider registry with model inference
- `providers/runtime/src/server/sse-writer.ts` -- SSE event writer
- `providers/runtime/src/server/auth.ts` -- Bearer token auth
- `providers/runtime/src/server/app.ts` -- Runtime server routes
- `providers/runtime/src/usage/consumption.ts` -- RuntimeConsumption class
- `providers/runtime/src/usage/token-consumption.ts` -- Token consumption resolvers
- `providers/runtime/src/usage/types.ts` -- Runtime usage types
- `providers/runtime/src/usage/metadata-extractor.ts` -- Provider metadata extraction
- `providers/runtime/src/runtime.ts` -- Core Runtime interface and creation
- `providers/runtime/src/session/session-store.ts` -- SessionStoreService interface
- `providers/runtime/src/session/in-memory-store.ts` -- In-memory store with TTL
- `providers/runtime/src/session/sqlite-session-store.ts` -- SQLite store
- `providers/runtime/src/tools/codec.ts` -- Tool ID encoding per provider
- `providers/runtime/src/tools/registry.ts` -- Tool registry (~600+ lines)
- `providers/runtime/src/tools/bridge.ts` -- ToolBridgeFactory, ToolBridgeInstance
- `providers/runtime/src/tools/types.ts` -- Tool registration types
- `providers/runtime/src/types/capabilities.ts` -- RuntimeCapabilities with presets
- `providers/runtime/src/types/runtime-options.ts` -- RuntimeAgentConfig, subagents
- `providers/runtime/src/errors/unions.ts` -- RuntimeHostError union

### Rust (arky)
- `crates/arky-provider/src/traits.rs` -- Provider trait, ProviderEventStream
- `crates/arky-provider/src/registry.rs` -- ProviderRegistry
- `crates/arky-provider/src/descriptor.rs` -- ProviderDescriptor, ProviderCapabilities, ProviderFamily
- `crates/arky-provider/src/error.rs` -- ProviderError (ClassifiedError)
- `crates/arky-provider/src/process.rs` -- ProcessManager, ManagedProcess, RestartPolicy
- `crates/arky-provider/src/transport.rs` -- StdioTransport
- `crates/arky-provider/src/request.rs` -- Re-exports from arky-protocol
- `crates/arky-provider/src/replay.rs` -- ReplayWriter
- `crates/arky-protocol/src/event.rs` -- AgentEvent (11 variants), EventMetadata, StreamDelta
- `crates/arky-protocol/src/message.rs` -- Message, ContentBlock, Role
- `crates/arky-protocol/src/request.rs` -- ProviderRequest, Usage, InputTokenDetails, OutputTokenDetails, GenerateResponse
- `crates/arky-protocol/src/session.rs` -- PersistedEvent, TurnCheckpoint, ReplayCursor
- `crates/arky-protocol/src/id.rs` -- SessionId, ProviderId, TurnId
- `crates/arky-protocol/src/tool.rs` -- ToolCall, ToolContent, ToolResult
- `crates/arky-error/src/lib.rs` -- ClassifiedError trait
- `crates/arky-hooks/src/lib.rs` -- Hooks trait, FailureMode, HookChain, ShellCommandHook
- `crates/arky-hooks/src/chain.rs` -- HookChain composition
- `crates/arky-hooks/src/context.rs` -- Hook event contexts
- `crates/arky-hooks/src/result.rs` -- Verdict, ToolResultOverride, PromptUpdate, StopDecision
- `crates/arky-hooks/src/shell.rs` -- ShellCommandHook, ToolMatcher
- `crates/arky-core/src/agent.rs` -- Agent with actor model, command queue
- `crates/arky-core/src/builder.rs` -- AgentBuilder
- `crates/arky-core/src/turn.rs` -- TurnRuntime with hook integration, tool execution, replay
- `crates/arky-core/src/subscription.rs` -- EventSubscription
- `crates/arky-tools/src/lib.rs` -- ToolRegistry, ToolIdCodec, StaticToolIdCodec
- `crates/arky-session/src/lib.rs` -- SessionStore trait, InMemorySessionStore, SqliteSessionStore
- `crates/arky-mcp/src/lib.rs` -- McpClient, McpServer, McpToolBridge, auth, naming
- `crates/arky-server/src/lib.rs` -- Axum router, ServerHandle, serve()
- `crates/arky-server/src/middleware.rs` -- CORS layer
- `crates/arky-config/src/lib.rs` -- Configuration loading
