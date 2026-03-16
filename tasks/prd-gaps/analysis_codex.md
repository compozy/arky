# Gap Analysis: Codex Provider (TypeScript -> Rust)

## Summary

This document is a systematic gap analysis comparing the TypeScript Codex
provider (`compozy-code/providers/codex/src/`) against the Rust
implementation (`arky/crates/arky-codex/src/`). The analysis covers 12
dimensions and identifies 34 discrete gaps ranging from critical streaming
pipeline deficiencies to missing convenience features.

**Overall assessment:** The Rust implementation has a solid structural
foundation -- the RPC transport, notification router, thread manager, approval
handler, and scheduler are well-designed and pass their unit tests. However,
the streaming pipeline, configuration system, error classification, model
service, and compatibility layer are either absent or significantly
under-implemented compared to the TypeScript reference. The most impactful
gaps are in streaming event dispatch (P0), config validation/override building
(P0), and error classification (P1).

**Coverage estimate:** The Rust crate implements approximately 35-40% of the
TypeScript provider's surface area by feature count, but covers roughly 60% of
the critical runtime path (spawn -> RPC -> thread -> turn -> notification ->
basic event mapping).

---

## Feature Comparison Matrix

| Feature Area | TS Status | Rust Status | Gap Severity | Notes |
|---|---|---|---|---|
| **1. Server / Process Management** | | | | |
| Binary resolution (explicit, require.resolve, npx) | Full | Partial | P1 | Rust has explicit+npx; missing `require.resolve` equivalent |
| Environment sanitization (LD_/DYLD_ blocking) | Full | Missing | P2 | TS `sanitizeSpawnEnv` has no Rust equivalent |
| Startup timeout | Full | Missing | P1 | TS `startupTimeoutMs` config; Rust has no startup timeout |
| Process restart / reconnect policy | Full (via compat layer) | Missing | P2 | TS registry handles restart; Rust spawns fresh each stream |
| Graceful SIGTERM / SIGKILL shutdown | Full | Full | -- | Both implement graceful shutdown |
| stderr collection on crash | Full | Full | -- | Both collect stderr for error messages |
| **2. Streaming Pipeline** | | | | |
| Stream pipeline orchestration | Full (CodexStreamPipeline) | Partial (StreamRuntime) | P0 | Rust has basic loop; TS has abort signal, finalization, turn failure detection |
| Event dispatcher (40+ event types) | Full (CodexEventDispatcher) | Partial (normalize_notification) | P0 | Rust handles ~10 event types; TS handles 40+ with 16 dispatch categories |
| Reasoning events (start/delta/complete) | Full | Missing | P0 | No reasoning text lifecycle in Rust |
| Usage/token tracking | Full | Missing | P1 | TS extracts input/output/cached/reasoning tokens |
| Session ID extraction from stream | Full | Missing | P1 | TS updates current session ID from events |
| Duplicate detection (fingerprinting) | Full | Missing | P1 | TS has fingerprint-based dedup for notifications |
| Stream state management | Full (CodexStreamState) | Minimal | P1 | TS tracks closed, lastUsage, turnFailure, sessionId, fingerprints |
| Context compaction events | Full | Missing | P2 | TS handles context_compaction tool type |
| Web search events | Full | Missing | P2 | TS handles web_search tool type |
| Image view events | Full | Missing | P2 | TS handles image_view tool type |
| Todo/plan events | Full | Missing | P2 | TS handles todo_list tool type |
| Abort signal / cancellation | Full | Missing | P1 | TS wires AbortSignal through stream pipeline |
| Response metadata emission | Full | Missing | P2 | TS emits stream-start and response-metadata parts |
| **3. Text Accumulation** | | | | |
| Basic delta/snapshot assembly | Full | Full | -- | Both implement push_delta + apply_snapshot |
| Message phase tracking (commentary/final_answer) | Full | Missing | P1 | TS tracks which phase assistant text belongs to |
| Reasoning text lifecycle | Full | Missing | P0 | TS has reasoning_start/delta/complete with UUIDs |
| Part ID tracking (UUID per text part) | Full | Missing | P2 | TS assigns unique IDs to text parts |
| Snapshot reconciliation with deltas | Full | Basic | P1 | TS has sophisticated reconciliation; Rust replaces entirely |
| **4. Tool Tracking** | | | | |
| Basic start/update/complete lifecycle | Full | Full | -- | Both track tool lifecycle |
| fail_open_tools on stream end | Full | Full | -- | Both close orphaned tools as errors |
| Parent ID / nested tool tracking | Full | Full | -- | Both support parentId |
| Tool name codec (canonical <-> provider) | Full | Full | -- | Both use ToolIdCodec |
| Tool input payload building (per item type) | Full (buildToolInputPayload) | Basic | P1 | TS has per-type payload builders; Rust uses raw item data |
| Tool result payload building (per item type) | Full (buildToolResultPayload) | Basic | P1 | TS has per-type result builders with metadata; Rust uses raw |
| Tool error detection (per item type) | Full (detectToolError) | Basic | P1 | TS checks exitCode, status, error per type; Rust checks status only |
| Tool invocation parts (start/delta/end/call) | Full (buildToolInvocationParts) | Missing | P2 | TS emits AI SDK V3 stream parts; Rust emits AgentEvent |
| Codex tool name prefix handling | Full (CODEX_PROVIDER_TOOL_PREFIX) | Missing | P2 | TS adds `codex__compozy__` prefix for compozy MCP tools |
| **5. Bridge / RPC Transport** | | | | |
| JSON-RPC framing (newline-delimited) | Full | Full | -- | Both implement NDJSON framing |
| Request correlation (ID matching) | Full | Full | -- | Both correlate requests by ID |
| Notification channel | Full | Full | -- | Both route notifications via channels |
| Server-request channel | Full | Full | -- | Both handle server-initiated requests |
| Fatal error propagation (watch channel) | Full | Full | -- | Both propagate fatal errors |
| Chunked line buffering | Full | Full | -- | Both handle partial lines |
| Initialize / initialized handshake | Full | Full | -- | Both implement the handshake |
| Request timeout | Full | Full | -- | Both support per-request timeout |
| Tracing spans per request | Full | Partial | P2 | TS has detailed tracing; Rust has basic tracing |
| **6. Configuration System** | | | | |
| Unified config composition (Process+Stream+Scheduler+MCP) | Full (CodexConfig) | Minimal | P0 | Rust has CodexProviderConfig; no separation or composition |
| Effect Schema validation (~40 fields) | Full (CodexCliSettingsSchema) | Missing | P0 | No schema validation in Rust |
| Config override building | Full (buildCodexConfigOverrides) | Partial | P1 | Rust has build_config_overrides but misses many TS fields |
| Mandatory settings enforcement | Full (enforceMandatoryCodexSettings) | Partial | P1 | Rust sets approval_policy; TS enforces 12+ mandatory fields |
| Settings merging (base + provider options) | Full (mergeCodexSettings) | Missing | P1 | TS has deep merge for all config categories |
| Settings-to-config mapping | Full (mapSettingsToConfigInput) | Missing | P1 | TS maps user settings to internal config shape |
| Per-request provider options | Full (CodexCliProviderOptions) | Partial | P1 | Rust uses `settings.extra` BTreeMap; TS has typed schema |
| Feature flag normalization | Full (normalizeFeatureFlagKey) | Missing | P2 | TS normalizes camelCase to snake_case for feature flags |
| Shell environment policy | Full (applyShellEnvironmentPolicy) | Missing | P2 | TS has full shell env policy config |
| Compaction settings | Full (applyCompactionSettings) | Partial | P2 | Rust passes through extras; TS has explicit config |
| MCP server config building | Full (applyMcpSettings) | Partial | P1 | Rust flattens mcp_servers from extras; TS has structured config |
| System prompt resolution | Full (resolveSystemPrompt) | Partial | P1 | Rust extracts system messages; TS merges systemPrompt + appendSystemPrompt |
| JSON schema sanitization | Full (sanitizeJsonSchema) | Missing | P2 | TS strips $schema, $ref, format, etc. from tool schemas |
| **7. Error Handling** | | | | |
| Error taxonomy (11+ error types) | Full | Minimal | P1 | TS has Auth, Config, Rpc, Scheduler, Spawn, Stream, etc. |
| Error classification (regex-based) | Full (classification.ts) | Missing | P1 | TS classifies errors as rate_limited, quota_exceeded, etc. |
| Retryability determination | Full | Partial | P1 | TS checks isRetryable per classification; Rust has trait method |
| APICallError mapping | Full | N/A | -- | TS maps to AI SDK errors; Rust uses ProviderError directly |
| Auth error detection | Full (isAuthError) | Missing | P2 | TS detects auth failures from error messages |
| Stderr summarization | Full (summarizeStderr) | Partial | P2 | TS has structured stderr analysis |
| Turn failure propagation | Full | Full | -- | Both propagate turn/failed as errors |
| Process crash detection | Full | Full | -- | Both detect process exit + collect stderr |
| **8. Model Selection / Service** | | | | |
| Model listing with pagination | Full (CodexModelService) | Missing | P0 | TS has models/list RPC with cursor; Rust has none |
| Model cache with TTL | Full | Missing | P1 | TS caches models for modelCacheTtlMs |
| Fallback model injection | Full | Missing | P2 | TS injects codex-mini-latest when listing fails |
| Configured model tracking | Full | Missing | P2 | TS tracks which model was configured vs discovered |
| Model deduplication in listing | Full | Missing | P2 | TS deduplicates model list entries |
| **9. Approval System** | | | | |
| Known method validation | Full | Full | -- | Both check known approval methods |
| AutoApprove / AutoDeny modes | Full | Full | -- | Both support automatic decisions |
| Manual mode with timeout | Full | Full | -- | Both support manual resolution with timeout |
| GrantPermissions decision | Partial | Full | -- | Rust has richer GrantPermissions with scope |
| Wire format (codex response shape) | Full | Full | -- | Both produce correct JSON-RPC responses |
| Runtime config-based approval | Full | Missing | P2 | TS checks runtimeConfig for approval policy |
| **10. Hooks / Lifecycle** | | | | |
| afterToolUse event | Full | Missing | P1 | TS has EventEmitter with emitAfterToolUse |
| afterAgent event | Full | Missing | P1 | TS has EventEmitter with emitAfterAgent |
| Hook integration in stream pipeline | Full | Missing | P1 | TS calls hooks during stream processing |
| **11. Compatibility Layer** | | | | |
| CodexCompatLanguageModel (AI SDK wrapper) | Full (compat.ts) | N/A | -- | Different target API; Rust uses Provider trait |
| App server registry (slot/refcount) | Full (CodexAppServerRegistryImpl) | Missing | P1 | TS has registry with acquire/release/idle shutdown |
| Managed runtime with idle shutdown | Full | Missing | P1 | TS has idleShutdownMs with fiber-based timers |
| Registry key generation from config | Full | Missing | P2 | TS normalizes config into unique registry keys |
| Drop-in provider factory (createCodexCli) | Full | Partial | P2 | Rust has CodexProvider::new(); TS has full factory |
| Runtime reconfiguration | Full (CodexRuntimeConfig) | Missing | P1 | TS detects critical field changes and triggers restart |
| Provider options extraction from request | Full (request-preparation.ts) | Partial | P1 | TS has typed extraction; Rust uses settings.extra map |
| Prompt/message mapping to CLI format | Full (mapMessagesToPrompt) | Full | -- | Both render messages to text prompt |
| Tool definition mapping | Full (request-preparation.ts) | Full | -- | Both map tool definitions to config overrides |
| Warning collection during request prep | Full | Missing | P2 | TS collects warnings (e.g., images ignored) |
| **12. Thread Management** | | | | |
| thread/start (new thread) | Full | Full | -- | Both implement thread/start |
| thread/resume (with ID validation) | Full | Full | -- | Both validate resumed thread ID matches |
| turn/start (with notification stream) | Full | Full | -- | Both start turns with notification routing |
| Thread compaction | Full | Missing | P2 | TS has compactThread; Rust has none |
| Drop-based cleanup (TurnNotificationStream) | N/A | Full | -- | Rust has Drop impl for cleanup; TS uses Effect scope |
| **13. Notification Routing** | | | | |
| Thread-scoped dispatch | Full | Full | -- | Both route by threadId |
| Scope-scoped fanout | Full | Full | -- | Both fanout by scopeId |
| Global fanout (account/) | Full | Full | -- | Both fanout for account/ prefix |
| Thread ID extraction (multi-field) | Full | Full | -- | Both check threadId, conversationId, etc. |
| Error fanout to all threads | Full | Full | -- | Both propagate errors to all |
| Stale routing detection | Full | Full | -- | Both detect missing scope/thread |
| **14. Scheduling** | | | | |
| Semaphore-based concurrency control | Full | Full | -- | Both use semaphore |
| Acquire timeout | Full | Full | -- | Both support timeout |
| Queue overflow protection | Full (maxQueuedRequests) | Missing | P2 | TS tracks queue depth; Rust allows unbounded waiting |
| Per-task timeout | Full (requestTimeoutMs) | Partial | P2 | TS has per-request timeout in scheduler; Rust has RPC timeout |

---

## Detailed Gap Analysis

### GAP-CDX-001: Streaming Event Dispatcher

- **TS Location:** `streaming/CodexEventDispatcher.ts` (~870 lines), `streaming/event-parser.ts` (~588 lines), `streaming/event-normalizer.ts`
- **Rust Status:** Partially implemented in `provider.rs` `normalize_notification()` (~150 lines)
- **Complexity:** XL (estimated 800-1000 lines of Rust)
- **Priority:** P0

**Description:**
The TypeScript event dispatcher handles 40+ notification method types mapped to
16 dispatch categories: message_start, message_delta, message_complete,
reasoning_start, reasoning_delta, reasoning_complete, tool_call_start,
tool_call_delta, tool_call_complete, tool_result, usage, session_id,
turn_complete, turn_failed, status, and context_compaction.

The Rust `normalize_notification()` function handles approximately 10 event
types and maps them to 9 `NormalizedNotification` variants. Missing dispatch
categories:

1. **Reasoning events** (reasoning_start/delta/complete) -- critical for models
   that emit reasoning tokens
2. **Usage events** -- token consumption tracking (input, output, cached,
   reasoning tokens with details)
3. **Session ID events** -- updating the active session ID from the stream
4. **Status events** -- server status notifications
5. **Context compaction events** -- compaction summary and status
6. **Web search events** -- web search tool type handling as first-class events
7. **Image view events** -- image viewing tool type
8. **Todo/plan events** -- plan management tool type

The TS event normalizer also performs sophisticated camelCase-to-snake_case
conversion with segment-based normalization, while Rust does a simpler
lowercase + replace approach.

**Dependencies:** GAP-CDX-003 (TextAccumulator reasoning support), GAP-CDX-005
(usage/token model in arky-protocol)

---

### GAP-CDX-002: Stream Pipeline Architecture

- **TS Location:** `streaming/CodexStreamPipeline.ts`, `streaming/CodexStreamState.ts`
- **Rust Status:** Partially implemented as inline logic in `provider.rs` `build_stream()` and `StreamRuntime`
- **Complexity:** L (estimated 300-400 lines of Rust)
- **Priority:** P0

**Description:**
The TypeScript stream pipeline provides:

1. **Abort signal handling** -- wires an AbortSignal from the consumer through
   the stream, enabling cooperative cancellation
2. **Stream-start emission** -- emits a `stream-start` part at the beginning
3. **Response metadata emission** -- emits `response-metadata` with model info
4. **Finalization with turn failure detection** -- detects if the stream ended
   due to a turn failure and maps the error appropriately
5. **State machine** -- `CodexStreamState` tracks closed, lastUsage,
   turnFailureMessage, currentSessionId, and fingerprints set for dedup

The Rust `StreamRuntime` covers basic message/tool/turn lifecycle but lacks:
- Cancellation token integration (no `CancellationToken` wired through)
- Response metadata emission
- Stream state with usage tracking
- Fingerprint-based duplicate detection (TS generates fingerprints from event
  content to skip duplicate notifications)

**Dependencies:** GAP-CDX-001 (event dispatcher completeness), arky-protocol
(CancellationToken support in ProviderRequest)

---

### GAP-CDX-003: Text Accumulator -- Reasoning and Phase Tracking

- **TS Location:** `streaming/CodexTextAccumulator.ts` (~533 lines)
- **Rust Status:** `accumulator.rs` (~70 lines for TextAccumulator)
- **Complexity:** L (estimated 250-350 lines of Rust)
- **Priority:** P0

**Description:**
The TypeScript `CodexTextAccumulator` provides:

1. **Text part lifecycle** (start/delta/end) with UUID-based part tracking
2. **Snapshot reconciliation** -- when a full-text snapshot arrives, it
   reconciles with accumulated deltas rather than blindly replacing
3. **Message phase tracking** -- tracks whether text belongs to "commentary"
   (thinking/planning) or "final_answer" phase
4. **Reasoning part lifecycle** -- separate tracking for reasoning tokens with
   their own start/delta/complete lifecycle and UUIDs

The Rust `TextAccumulator` implements only basic push_delta and apply_snapshot
(which replaces the entire content). It has no concept of:
- Reasoning text assembly
- Message phases
- Part-level UUID tracking
- Smart snapshot reconciliation that preserves delta-accumulated progress

**Dependencies:** arky-protocol (reasoning content block type or metadata),
GAP-CDX-001 (reasoning event dispatch)

---

### GAP-CDX-004: Configuration System

- **TS Location:** `config/` directory (5 files), `config/schemas.ts` (~567 lines), `util/args.ts`, `util/config-merge.ts`, `util/settings-mapping.ts`, `util/validation.ts`
- **Rust Status:** `CodexProviderConfig` struct (~50 lines), partial override building in `build_config_overrides()` (~80 lines)
- **Complexity:** XL (estimated 600-800 lines of Rust)
- **Priority:** P0

**Description:**
The TypeScript configuration system consists of:

1. **CodexCliSettings schema** -- Effect Schema validation with ~40+ fields
   covering approval modes, sandbox modes, feature flags, MCP server configs,
   reasoning settings, compaction settings, shell environment policies,
   exec policy, and more
2. **Config composition** -- `CodexConfig` composes `ProcessConfig`,
   `StreamingConfig`, `SchedulerConfig`, and `McpConfig`
3. **Mandatory settings enforcement** -- `enforceMandatoryCodexSettings` forces
   12+ fields (approvalMode=never, sandboxMode=danger-full-access,
   fullAuto=false, dangerouslyBypassApprovalsAndSandbox=true, etc.)
4. **Config override building** -- `buildCodexConfigOverrides` applies generic
   overrides, reasoning settings, shell env policy, feature flags, MCP
   settings, compaction settings, exec policy, web search, and system prompt
5. **Settings merging** -- deep merge of base settings with per-request
   provider options, including MCP server merge, feature flag merge, and shell
   env policy merge
6. **Validation** -- Schema-based validation with structured error reporting,
   cross-field validation warnings

The Rust `CodexProviderConfig` has ~15 fields covering the basics (binary,
allow_npx, cwd, env, timeouts, approval_mode). The `build_config_overrides()`
method handles reasoning, developer_instructions, tools, tool_choice,
config_overrides passthrough, and mcp_servers, but everything flows through the
untyped `settings.extra` BTreeMap rather than a validated schema.

Missing in Rust:
- Typed settings struct with validation
- Mandatory settings enforcement
- Deep settings merge (base + per-request)
- Feature flag normalization (camelCase -> snake_case)
- Shell environment policy config
- Structured streaming config (reasoning, compaction)
- Structured MCP config (mcpServers with stdio/HTTP transport types)
- JSON schema sanitization for tool definitions

**Dependencies:** arky-config crate (validation infrastructure)

---

### GAP-CDX-005: Error Classification

- **TS Location:** `errors/` directory (8 files), `errors/classification.ts`, `errors/utilities.ts`
- **Rust Status:** Uses generic `ProviderError` from arky-provider
- **Complexity:** M (estimated 200-300 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript error system defines 11 error types:

1. `CodexAuthError` -- API key / authentication failures
2. `CodexConfigError` -- configuration parsing failures
3. `CodexValidationError` -- settings validation failures
4. `CodexDisposedError` -- using a disposed provider
5. `CodexRpcError` -- JSON-RPC errors with code and method
6. `CodexRpcTimeoutError` -- RPC request timeout
7. `CodexOverflowError` -- scheduler queue overflow
8. `CodexTimeoutError` -- scheduler task timeout
9. `CodexSpawnError` -- process spawn failure
10. `CodexStreamError` -- stream-level error with classification
11. `CodexTurnFailedError` -- turn failure from server
12. `CodexAbortedError` -- explicit abort

The classification system (`classification.ts`) uses regex patterns to classify
error messages into categories: `context_window_exceeded`, `quota_exceeded`,
`rate_limited`, `invalid_request`, `api_error`, `unknown`. Each category
determines retryability.

The Rust implementation uses the shared `ProviderError` enum which has
variants like `BinaryNotFound`, `ProcessCrashed`, `StreamInterrupted`,
`ProtocolViolation`, `AuthFailed`, `RateLimited`, but lacks:
- Regex-based message classification
- Error enrichment from stderr
- Codex-specific error variants (RpcError with code/method, OverflowError,
  DisposedError)
- The `ClassifiedError` trait implementation for Codex-specific errors

**Dependencies:** arky-error (ClassifiedError trait)

---

### GAP-CDX-006: Model Service

- **TS Location:** `server/CodexModelService.ts`
- **Rust Status:** Not implemented
- **Complexity:** M (estimated 150-250 lines of Rust)
- **Priority:** P0

**Description:**
The TypeScript `CodexModelService` provides:

1. **Model listing via RPC** -- sends `models/list` with pagination cursor
2. **Model caching** -- caches results for `modelCacheTtlMs` (default 300000ms)
3. **Fallback model injection** -- injects `codex-mini-latest` when listing
   fails or returns empty
4. **Configured model tracking** -- tracks which models were configured by the
   user vs discovered from the server
5. **Model deduplication** -- removes duplicate entries in the listing

The Rust implementation has no model service. The `Provider` trait does not
expose a `list_models()` method (the techspec does not mention it as a
required method), but the Codex app server supports it and it is used
by the TS compat layer for model selection.

**Dependencies:** May need to extend arky-provider with a model listing method
or implement as a Codex-specific API

---

### GAP-CDX-007: Compatibility Layer / Registry

- **TS Location:** `compat.ts` (~1027 lines), `server/CodexRegistry.ts`, `server/CodexRuntimeConfig.ts`
- **Rust Status:** Not implemented
- **Complexity:** XL (estimated 500-700 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript compatibility layer provides:

1. **CodexCompatLanguageModel** -- wraps the Codex provider to implement the
   AI SDK `LanguageModelV3` interface (this specific wrapper is not needed in
   Rust since the target API is the `Provider` trait)
2. **App server registry** -- `CodexAppServerRegistryImpl` manages multiple
   app server instances with slot management, reference counting, and idle
   shutdown timers
3. **Manager** -- `createCodexAppServerManager` creates an Effect
   ManagedRuntime that handles lifecycle
4. **Runtime reconfiguration** -- `CodexRuntimeConfig` detects when critical
   fields change (codexPath, cwd, allowNpx, env, sanitizeEnvironment) and
   triggers a full restart
5. **Provider factory** -- `createCodexCli()` / `codexCli` drop-in factory
6. **Registry key generation** -- normalizes config options into a unique key
   for registry lookup

While the specific AI SDK wrapper is not needed, the Rust implementation lacks:
- Any form of server registry for reusing app server instances across requests
- Idle shutdown behavior (keep process alive for `idleShutdownMs`)
- Runtime reconfiguration with critical change detection
- Reference counting for shared server instances

Currently, Rust spawns a **fresh app server process for every `stream()` call**,
which is significantly less efficient than the TS approach of keeping a long-
lived server and routing threads through it.

**Dependencies:** This is an architectural decision about whether Rust should
have a long-lived server pattern. The techspec mentions `ProcessManager` for
lifecycle but does not explicitly require a registry.

---

### GAP-CDX-008: Hooks System Integration

- **TS Location:** `hooks.ts`
- **Rust Status:** Not implemented
- **Complexity:** S (estimated 50-100 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript `CodexHooks` provides an `EventEmitter` with two lifecycle
events:
1. `emitAfterToolUse(event)` -- called after a tool completes
2. `emitAfterAgent(event)` -- called after the agent turn completes

The Rust implementation has no hook integration points in the streaming
pipeline. The `arky-hooks` crate defines the `Hooks` trait with richer
lifecycle events (`before_tool_call`, `after_tool_call`, `session_start`,
`session_end`, `on_stop`, `user_prompt_submit`), but the Codex provider does
not invoke any hooks.

**Dependencies:** arky-hooks crate, HookContext in ProviderRequest

---

### GAP-CDX-009: MCP Tool Bridge

- **TS Location:** `bridge/CodexBridge.ts`, `bridge/CodexToolsBridge.ts`
- **Rust Status:** Not implemented
- **Complexity:** L (estimated 200-350 lines of Rust)
- **Priority:** P2

**Description:**
The TypeScript tool bridge creates an HTTP server (`startCodexToolBridge`) that
exposes SDK-registered tools to the Codex app server via MCP. This allows
Codex to call tools that are registered in the SDK's tool registry.

The bridge:
1. Creates an HTTP endpoint for MCP tool calls
2. Maps SDK tool descriptors to MCP-compatible schemas
3. Executes tool calls through the SDK tool registry
4. Returns results in the format Codex expects

The Rust implementation relies on the `arky-mcp` crate for MCP integration,
but the Codex provider does not set up an MCP tool bridge to expose local
tools. The `rmcp_client` config override exists in `build_config_overrides()`
but the actual bridge server is not implemented.

**Dependencies:** arky-mcp crate, arky-tools registry

---

### GAP-CDX-010: Tool Payload Builders

- **TS Location:** `streaming/tool-payloads.ts` (~358 lines)
- **Rust Status:** Basic extraction in `normalize_notification()` functions
- **Complexity:** M (estimated 200-300 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript `tool-payloads.ts` contains specialized payload builders for
each tool item type:

1. **getToolName** -- maps item types to canonical tool names (command_execution
   -> "shell", file_change -> "apply_patch", mcp_tool_call -> prefixed name,
   web_search -> "web_search", image_view -> "view_image",
   context_compaction -> "context_compaction", collab_tool_call -> tool name,
   todo_list -> "update_plan")
2. **buildToolInputPayload** -- extracts structured input per tool type
   (command/status/cwd for shell, changes/path for file_change, arguments for
   MCP, query for web_search, etc.)
3. **buildToolResultPayload** -- extracts structured result per tool type with
   metadata (itemType, itemId, status, server, exitCode, aggregatedOutput,
   etc.)
4. **buildToolInvocationParts** -- emits the AI SDK tool lifecycle stream parts
   (tool-input-start, tool-input-delta, tool-input-end, tool-call)
5. **buildToolResultPart** -- emits tool-result with provider metadata and error
   detection
6. **detectToolError** -- per-type error detection (exitCode != 0 for commands,
   error field present for other types)
7. **Codex tool prefix** -- adds `codex__compozy__` prefix for tools from the
   compozy MCP server

The Rust implementation has `canonical_tool_name()` which maps item types to
names (similar to `getToolName`), `tool_input()` which extracts raw arguments,
and basic error detection from `status` field. But it lacks:
- Structured per-type input payload building
- Structured per-type result payload building with metadata
- The compozy tool prefix convention

**Dependencies:** arky-protocol (tool result metadata fields)

---

### GAP-CDX-011: Request Preparation

- **TS Location:** `model/request-preparation.ts`
- **Rust Status:** Partially in `build_config_overrides()` and `render_prompt()`
- **Complexity:** M (estimated 150-200 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript request preparation module provides:

1. **Prompt mapping** -- `mapMessagesToPrompt` converts AI SDK messages to text,
   handling system/user/assistant/tool roles with structured content parts
   (text, image warnings, tool outputs)
2. **Tool definition mapping** -- converts AI SDK tool definitions to config
   overrides with JSON schema sanitization
3. **Provider options extraction** -- extracts typed CodexCliProviderOptions
   from the AI SDK request
4. **Config overrides building** -- applies settings merging, mandatory
   enforcement, and override building
5. **Warning collection** -- collects warnings for unsupported features
   (e.g., image inputs, unsupported tool types)

The Rust `render_prompt()` handles message rendering adequately. The
`build_config_overrides()` handles a subset of overrides. Missing:
- Typed provider options extraction (uses untyped extra map instead)
- Warning collection
- Full config merge pipeline (mandatory enforcement + settings merge + override
  building)

**Dependencies:** GAP-CDX-004 (configuration system)

---

### GAP-CDX-012: Server Process Reuse

- **TS Location:** `server/CodexAppServer.ts`, `server/CodexRegistry.ts`
- **Rust Status:** Not implemented (spawns fresh process per stream)
- **Complexity:** L (estimated 300-400 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript `CodexAppServer` maintains a long-lived subprocess with:

1. **ensureReady** -- initializes the server once, then reuses across requests
2. **Worker loops** -- notification and approval workers run for the server
   lifetime, not per-request
3. **Thread delegation** -- multiple threads/turns share one server process
4. **Registry integration** -- server instances are managed by the registry
   with reference counting and idle shutdown

The Rust `build_stream()` spawns a new process, creates a new RpcTransport,
initializes, runs one turn, then shuts down the process. This means:
- Every request pays startup latency (process spawn + initialize handshake)
- No session continuity across requests (each request is a fresh server)
- Resource waste from repeated process lifecycle

This is the **single most impactful architectural difference** between the
implementations. The TS design amortizes process startup across many turns.

**Dependencies:** GAP-CDX-007 (compatibility layer / registry concept)

---

### GAP-CDX-013: Duplicate Detection

- **TS Location:** `streaming/CodexEventDispatcher.ts` (fingerprint logic)
- **Rust Status:** Not implemented
- **Complexity:** S (estimated 50-100 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript event dispatcher maintains a fingerprint set to detect and skip
duplicate notifications. This is necessary because the Codex app server can
sometimes emit the same notification more than once (e.g., during reconnection
or retry scenarios).

The Rust implementation processes all notifications without dedup, which could
lead to duplicate events being emitted to consumers.

**Dependencies:** None

---

### GAP-CDX-014: Cancellation / Abort Support

- **TS Location:** `streaming/CodexStreamPipeline.ts` (AbortSignal handling)
- **Rust Status:** Not implemented
- **Complexity:** M (estimated 100-150 lines of Rust)
- **Priority:** P1

**Description:**
The TypeScript stream pipeline accepts an `AbortSignal` and:

1. Checks the signal before starting the stream
2. Wires the signal through the event processing loop
3. Terminates the stream cleanly when abort is signaled
4. Emits appropriate cleanup events on cancellation

The Rust `build_stream()` has no cancellation support. The `ProviderRequest`
does not carry a `CancellationToken` (though the techspec mentions
`CancellationToken` from `tokio-util` for cooperative cancellation). Adding
this requires:
- Adding `CancellationToken` to `ProviderRequest` or `TurnContext`
- Checking the token in the stream loop
- Wiring the token through to the process shutdown

**Dependencies:** arky-provider (CancellationToken in ProviderRequest)

---

### GAP-CDX-015: Thread Compaction

- **TS Location:** `server/CodexThreadManager.ts` (compactThread method)
- **Rust Status:** Not implemented
- **Complexity:** S (estimated 30-50 lines of Rust)
- **Priority:** P2

**Description:**
The TypeScript thread manager has a `compactThread` method that sends a
`thread/compact` RPC to reduce the context window of a long-running thread.
This is important for long conversations that approach the model's context
limit.

The Rust `ThreadManager` only has `start_thread`, `resume_thread`, and
`start_turn`. Adding compaction would be straightforward.

**Dependencies:** None (just an additional RPC method)

---

### GAP-CDX-016: Scheduler Queue Overflow Protection

- **TS Location:** `server/CodexScheduler.ts`
- **Rust Status:** Not implemented
- **Complexity:** S (estimated 30-50 lines of Rust)
- **Priority:** P2

**Description:**
The TypeScript scheduler has `maxQueuedRequests` (default 64) which limits how
many requests can wait for a scheduler slot. When the queue overflows, it
returns a `CodexOverflowError`. The Rust scheduler allows unbounded waiting
with only a timeout.

**Dependencies:** None

---

---

## Files Reference

### TypeScript Source Files (compozy-code/providers/codex/src/)

| File | Lines (approx) | Domain |
|---|---|---|
| `index.ts` | 80 | Barrel exports |
| `compat.ts` | 1027 | Compatibility layer, registry, factory |
| `hooks.ts` | 30 | Lifecycle hooks (afterToolUse, afterAgent) |
| `bridge/CodexBridge.ts` | 50 | HTTP bridge entry point |
| `bridge/CodexToolsBridge.ts` | 150 | MCP tool bridge server |
| `config/CodexConfig.ts` | 80 | Config composition |
| `config/CodexMcpConfig.ts` | 40 | MCP config |
| `config/CodexProcessConfig.ts` | 60 | Process config |
| `config/CodexSchedulerConfig.ts` | 40 | Scheduler config |
| `config/CodexStreamingConfig.ts` | 40 | Streaming config |
| `config/schemas.ts` | 567 | Effect Schema validation |
| `errors/index.ts` | 20 | Error union export |
| `errors/auth.ts` | 20 | Auth error |
| `errors/classification.ts` | 80 | Error classification |
| `errors/config.ts` | 40 | Config errors |
| `errors/rpc.ts` | 40 | RPC errors |
| `errors/scheduler.ts` | 30 | Scheduler errors |
| `errors/spawn.ts` | 20 | Spawn error |
| `errors/stream.ts` | 60 | Stream errors |
| `errors/utilities.ts` | 80 | Error helpers |
| `model/CodexProvider.ts` | 100 | Provider factory |
| `model/CodexLanguageModel.ts` | 687 | LanguageModelV3 implementation |
| `model/request-preparation.ts` | 300 | Request preparation |
| `server/CodexAppServer.ts` | 200 | Server orchestration |
| `server/CodexRpcTransport.ts` | 805 | JSON-RPC transport |
| `server/CodexProcessManager.ts` | 200 | Process lifecycle |
| `server/CodexThreadManager.ts` | 250 | Thread/turn management |
| `server/CodexApprovalHandler.ts` | 150 | Approval handling |
| `server/CodexModelService.ts` | 150 | Model listing/caching |
| `server/CodexNotificationRouter.ts` | 300 | Notification routing |
| `server/CodexRuntimeConfig.ts` | 100 | Runtime reconfiguration |
| `server/CodexScheduler.ts` | 100 | Request scheduling |
| `server/CodexServerLayer.ts` | 80 | Layer composition |
| `server/CodexRegistry.ts` | 200 | Server registry |
| `server/types.ts` | 100 | Shared types |
| `streaming/CodexStreamPipeline.ts` | 200 | Stream orchestration |
| `streaming/CodexEventDispatcher.ts` | 870 | Event dispatch |
| `streaming/CodexTextAccumulator.ts` | 533 | Text assembly |
| `streaming/CodexToolTracker.ts` | 200 | Tool tracking |
| `streaming/CodexStreamState.ts` | 50 | Stream state |
| `streaming/event-normalizer.ts` | 27 | Event type normalization |
| `streaming/event-parser.ts` | 588 | Event parsing |
| `streaming/json-utils.ts` | 80 | JSON utilities |
| `streaming/tool-payloads.ts` | 358 | Tool payload builders |
| `streaming/types.ts` | 20 | Streaming types |
| `util/args.ts` | 350 | CLI args, settings enforcement |
| `util/config-merge.ts` | 303 | Config merging |
| `util/message-mapper.ts` | 125 | Message to prompt mapping |
| `util/settings-mapping.ts` | 87 | Settings to config mapping |
| `util/validation.ts` | 99 | Settings validation |
| `util/runtime-layer.ts` | 9 | Effect layer helper |

### Rust Source Files (arky/crates/arky-codex/src/)

| File | Lines (approx) | Domain |
|---|---|---|
| `lib.rs` | 60 | Module declarations, re-exports |
| `provider.rs` | 1600 | Provider impl, stream runtime, event normalization |
| `rpc.rs` | 1105 | JSON-RPC transport |
| `notification.rs` | 463 | Notification routing |
| `thread.rs` | 446 | Thread/turn management |
| `approval.rs` | 441 | Approval handling |
| `accumulator.rs` | 296 | Text accumulator, tool tracker |
| `scheduler.rs` | 150 | Request scheduling |

### Key Technical References

| Document | Path |
|---|---|
| Tech Spec | `arky/tasks/prd-rust-providers/techspec.md` |
| ADR-003 (CLI wrappers) | `arky/tasks/prd-rust-providers/adrs/adr-003-cli-wrapper-providers.md` |
| ADR-004 (Event model) | `arky/tasks/prd-rust-providers/adrs/adr-004-event-model.md` |
| ADR-006 (Error handling) | `arky/tasks/prd-rust-providers/adrs/adr-006-error-handling.md` |

---

## Implementation Priority Recommendations

### Phase 1: Critical Path (P0 gaps)

1. **GAP-CDX-012** -- Server process reuse (architectural foundation)
2. **GAP-CDX-001** -- Streaming event dispatcher (feature completeness)
3. **GAP-CDX-003** -- Text accumulator reasoning support
4. **GAP-CDX-004** -- Configuration system
5. **GAP-CDX-006** -- Model service

### Phase 2: Important Completeness (P1 gaps)

6. **GAP-CDX-005** -- Error classification
7. **GAP-CDX-002** -- Stream pipeline architecture
8. **GAP-CDX-010** -- Tool payload builders
9. **GAP-CDX-011** -- Request preparation
10. **GAP-CDX-013** -- Duplicate detection
11. **GAP-CDX-014** -- Cancellation/abort support
12. **GAP-CDX-007** -- Registry / idle shutdown (depends on CDX-012)
13. **GAP-CDX-008** -- Hooks integration

### Phase 3: Polish (P2 gaps)

14. **GAP-CDX-009** -- MCP tool bridge
15. **GAP-CDX-015** -- Thread compaction
16. **GAP-CDX-016** -- Scheduler queue overflow

### Estimated Total Implementation Effort

- **P0 gaps:** ~2500-3000 lines of Rust
- **P1 gaps:** ~1500-2000 lines of Rust
- **P2 gaps:** ~500-800 lines of Rust
- **Total:** ~4500-5800 lines of Rust (current codebase is ~4561 lines)
