# Deep Analysis Report: providers/claude-code Package

## 1. Overview

The `@compozy/provider-claude-code` package is a **Vercel AI SDK v3 (`LanguageModelV3`) provider** that wraps the `@anthropic-ai/claude-agent-sdk` CLI/process model. It spawns a Claude Code subprocess, streams its SDK messages, normalizes them into AI SDK stream parts, and exposes both `doGenerate` (non-streaming) and `doStream` (streaming) methods.

**Key architectural properties:**

- Built entirely on **Effect-TS** for service composition, error handling, and streaming
- Uses `Context.Tag` + `Layer` service pattern for all stateful components
- All errors are **tagged discriminated unions** (`Schema.TaggedError`) with exhaustive `Match` handling
- Stream processing is a **multi-stage functional pipeline** with immutable accumulator state
- Implements the `LanguageModelV3` interface from `@ai-sdk/provider`

**Dependencies:**

- `@ai-sdk/provider` (v3.0.8) -- AI SDK provider types and error classes
- `@ai-sdk/provider-utils` (v4.0.19) -- `generateId`, `Tool` type, `bindTools`
- `@anthropic-ai/claude-agent-sdk` (v0.2.74) -- subprocess query, MCP server creation, SDK types
- `@compozy/provider-core` (workspace) -- shared tools-bridge, MCP server, error classifier
- `effect` (v3.19.19) -- core runtime (Effect, Stream, Layer, Context, Schema, Match, Ref, HashMap, Option, Duration)
- `es-toolkit` (v1.45.1) -- utility functions (clamp, includes, trim, merge, cloneDeep, isArray)

---

## 2. Provider & Language Model

### 2.1 Provider Factory (`services/provider.ts`)

**`createClaudeCode(options?: ClaudeCodeProviderSettings): ClaudeCodeProvider`**

Creates a callable provider that implements `ProviderV3`. The provider is a function object with methods:

- `(modelId, settings?)` -- creates a `ClaudeCodeLanguageModel`
- `.languageModel(modelId, settings?)` -- same
- `.chat(modelId, settings?)` -- same
- `.imageModel()` / `.embeddingModel()` -- throws `NoSuchModelError`

Settings merging: `defaultSettings` from provider-level are spread under per-model settings.

**`ClaudeCodeProviderService`** is an Effect `Context.Tag` service wrapping the provider factory, enabling DI in Effect programs.

### 2.2 Language Model (`services/language-model.ts`)

**`ClaudeCodeLanguageModel implements LanguageModelV3`**

Key fields:

- `specificationVersion: "v3"`
- `provider: "claude-code"`
- `defaultObjectGenerationMode: "json"`
- `supportsStructuredOutputs: true`
- `supportsImageUrls: false`

Constructor:

1. Validates model ID (throws `NoSuchModelError` if empty)
2. Decodes settings via `decodeClaudeCodeSettingsSync`
3. Creates a `ManagedRuntime` with a layer composed of **7 services**: `ClaudeCodeConfig`, `SpawnFailureTracker`, `SessionManager`, `ToolsBridgeRegistry`, `ToolLifecycleService`, `NestedToolTracker`, `TextDeduplicator`

**`doGenerate(options)`**: Runs `generateClaudeCode` inside the managed runtime, catches errors via `toBoundaryError` which classifies and maps to AI SDK error types.

**`doStream(options)`**: Runs `createClaudeCodeStream` to get an Effect `Stream`, converts to `ReadableStream` via `Stream.toReadableStream`, wraps with cleanup logic for abort signal propagation.

Implements `Symbol.dispose` for resource cleanup (disposes managed runtime).

### 2.3 Rust Port Considerations -- Provider Layer

- The provider factory is a simple object composition; maps to a Rust struct with trait implementation
- `ManagedRuntime` maps to a Rust `Arc<Runtime>` with shared services
- `doGenerate`/`doStream` map to async trait methods
- Settings validation (`Schema.decode`) maps to `serde` deserialization with custom validation

---

## 3. Session Management

### 3.1 SessionManager (`services/session.ts`)

An Effect service (`Context.Tag`) managing an `Option<SessionId>` in a `Ref`.

**API:**

- `get: Effect<Option<SessionId>>` -- read current session
- `set(sessionId): Effect<void>` -- store session
- `clear: Effect<void>` -- reset to `None`

Session IDs come from the Claude Code SDK's `system.init` and `result` events. They are persisted across turns to enable session resumption.

### 3.2 SpawnFailureTracker (`services/spawn-failure-tracker.ts`)

Tracks consecutive spawn failures with cooldown. Uses `Ref<SpawnFailureState>` where:

```
SpawnFailureState = {
  consecutiveFailures: number
  cooldownUntilMs: Option<number>
}
```

**API:**

- `canAttempt(nowMs)` -- checks if spawn is allowed; normalizes expired cooldowns
- `recordFailure(nowMs)` -- increments counter; triggers cooldown at threshold
- `recordSuccess` -- resets state
- `reset` -- resets state

Policy is configured via `ClaudeCodeConfig.settings.spawnFailurePolicy` (default: 3 failures, 10s cooldown).

### 3.3 ToolsBridgeRegistry (`services/tools-bridge-registry.ts`)

Manages a `HashMap<string, ToolsBridgeInstance>` in a `Ref`. Each bridge instance wraps custom tools as an MCP server that Claude Code can call.

**API:**

- `register(bridge)` -- generates UUID, stores bridge, returns ID
- `get(id)` -- lookup by ID
- `remove(id)` -- removes and calls `bridge.close()`
- `getAll` -- returns full registry

### 3.4 Rust Port Considerations -- Services

- `Ref` maps to `Arc<Mutex<T>>` or `Arc<RwLock<T>>` in Rust
- `Option` maps to Rust `Option<T>`
- `HashMap` maps to `std::collections::HashMap` (or `dashmap` for concurrent access)
- `Duration` maps to `std::time::Duration`
- `Context.Tag` + `Layer` pattern maps to trait objects + dependency injection (e.g., via constructor injection or a `ServiceRegistry` struct)

---

## 4. Streaming Pipeline

The streaming pipeline is the most complex subsystem, with 6 processing stages.

### 4.1 Data Flow

```
Claude Code SDK (AsyncIterable<SDKMessage>)
  |
  v
[1] Event Normalizer (SDKMessage -> NormalizedEvent[])
  |
  v
[2] Tool Lifecycle Service (state machine validation)
  |
  v
[3] Nested Tool Tracker (parent-child tool relationship management)
  |
  v
[4] Nested Preview Events (emit preliminary ToolResult for UI)
  |
  v
[5] Text Deduplicator (remove duplicate text from assistant fallback)
  |
  v
[6] Session Persistence (save session ID from metadata/finish events)
  |
  v
[7] Stream Parts Emitter (NormalizedEvent -> LanguageModelV3StreamPart[])
  |
  v
ReadableStream<LanguageModelV3StreamPart>
```

### 4.2 Normalized Events (`stream/normalized-events.ts`)

13 event types forming a tagged union (`_tag` discriminant):

| Tag                 | Source                  | Purpose                     |
| ------------------- | ----------------------- | --------------------------- |
| `TextDelta`         | stream_event, assistant | Incremental text output     |
| `ReasoningStart`    | stream_event, assistant | Extended thinking begins    |
| `ReasoningDelta`    | stream_event, assistant | Reasoning text chunk        |
| `ReasoningComplete` | stream_event, assistant | Reasoning block ends        |
| `ToolUseStart`      | stream_event, assistant | Tool invocation begins      |
| `ToolUseInputDelta` | stream_event, assistant | Streaming tool input JSON   |
| `ToolUseComplete`   | stream_event, assistant | Tool input finalized        |
| `ToolResult`        | user                    | Tool execution result       |
| `ToolError`         | user                    | Tool execution error        |
| `ToolProgress`      | tool_progress           | Long-running tool status    |
| `MetadataEvent`     | system, result          | Session ID, model metadata  |
| `FinishEvent`       | result                  | Usage, finish reason, costs |
| `ErrorEvent`        | any                     | Error passthrough           |

### 4.3 Event Normalizer (`stream/event-normalizer.ts`)

**Core function:** `normalizeSdkMessage(state: StreamAccumState, message: SDKMessage) -> [StreamAccumState, NormalizedEvent[]]`

This is a **pure stateful fold** over SDK messages. The `StreamAccumState` (defined in `stream-state.ts`) tracks:

- `textBuffer` -- accumulated text
- `activeToolMap` -- HashMap of active tool calls with name/input/parent
- `toolBlocksByIndex` / `reasoningBlocksByIndex` -- maps content block indices to IDs
- `toolInputByCallId` -- accumulated streaming JSON input per tool
- `messageIndex` -- monotonic counter
- `usage`, `finishReason` -- final aggregates
- `hasSeenStreamEvents` -- flag to skip assistant-level duplicates
- `hasStreamedJson` -- for JSON mode handling
- `currentTextPartId` -- tracks open text-start/text-end pairs
- `nextSyntheticId` -- counter for generating IDs when SDK doesn't provide them

**Message type dispatch** (using `Match`):

- `system` -> extract `session_id` from `init` subtype
- `stream_event` -> handle `content_block_start` (tool_use, thinking), `content_block_delta` (text_delta, input_json_delta, thinking_delta), `content_block_stop`
- `assistant` -> extract text blocks, reasoning blocks, tool_use blocks (with dedup against stream events)
- `user` -> extract tool_result and tool_error blocks
- `result` -> extract usage, finish reason, structured output, session ID
- `tool_progress` -> emit tool progress events

**Key design:** Both streaming (`stream_event`) and non-streaming (`assistant`) paths are handled. When `hasSeenStreamEvents` is true, assistant-level text/reasoning blocks are skipped to avoid duplication.

**Class wrapper:** `ClaudeCodeEventNormalizer` provides mutable stateful API. Also exposed as an Effect `Stream.mapAccum` combinator (`normalizeEvents`).

### 4.4 Stream State (`stream/stream-state.ts`)

Immutable accumulator type. All state transitions produce new objects. Uses Effect `HashMap` for O(log n) immutable map operations.

### 4.5 Tool Lifecycle Service (`stream/tool-lifecycle.ts`)

A **finite state machine** per tool call ID:

```
Idle -> Started -> InputReceiving -> Executing -> Completed
                    |                               |
                    +-> Executing                    +-> Started (new tool)
```

States:

- `Idle` -- no tool in progress
- `Started` -- `ToolUseStart` received
- `InputReceiving` -- `ToolUseInputDelta` received (accumulates input)
- `Executing` -- `ToolUseComplete` received (final input locked)
- `Completed` -- `ToolResult` / `ToolError` received

Invalid transitions produce `ClaudeStreamCorruptedError`.

### 4.6 Nested Tool Tracker (`stream/nested-tool-tracker.ts`)

Manages parent-child tool relationships. When a tool has a `parentToolCallId`:

- Tracks it as a nested call under the parent
- On parent `ToolResult`, merges nested call summaries into the result payload
- Emits preliminary `ToolResult` events for UI progress updates

Uses `HashMap<parentId, NestedToolStore>` where `NestedToolStore` = `{ order: string[], calls: HashMap<nestedId, NestedToolInfo> }`.

### 4.7 Text Deduplicator (`stream/text-deduplicator.ts`)

Prevents duplicate text when both `stream_event` and `assistant` messages contain the same text. Tracks `streamedTextLength` and slices assistant text to only emit the delta beyond what was already streamed.

### 4.8 Stream Parts (`stream/stream-parts.ts`)

**Pure mapping** from `NormalizedEvent` to `LanguageModelV3StreamPart[]`. Each event type maps to one or more stream parts:

- `TextDelta` -> `text-start` + `text-delta`
- `ToolUseStart` -> optionally `text-end` + `tool-input-start`
- `ToolUseComplete` -> `tool-input-end` + `tool-call`
- `ToolResult` -> `tool-result` (with `dynamic: true`)
- `FinishEvent` -> optionally `text-end` + `finish`

All tool-related parts include `providerExecuted: true` and `dynamic: true` flags.

### 4.9 Main Stream (`stream/stream.ts`)

`createClaudeCodeStream` is the top-level Effect function that:

1. Checks `SpawnFailureTracker.canAttempt`
2. Validates settings (canUseTool vs permissionPromptToolName)
3. Gets session ID from `SessionManager`
4. Converts messages and builds query options
5. Creates SDK source stream from `query()` async iterable
6. Pipes through: normalizer -> tool lifecycle -> nested tracker -> preview events -> deduplicator -> session persist -> stream parts
7. Wraps with `stream-start` header and fallback `finish` event
8. Handles `ClaudeStreamCorruptedError` via recovery stream
9. Ensures cleanup (spawn tracker update, service resets)

### 4.10 Rust Port Considerations -- Streaming

- The fold-based normalizer maps well to Rust: `fn normalize(state: &mut State, msg: SdkMessage) -> Vec<NormalizedEvent>`
- `HashMap` immutable operations should use a mutable `HashMap` in Rust (performance) since the state machine is single-threaded per stream
- The 6-stage pipeline maps to a chain of `Stream::map` / `Stream::flat_map` in `tokio-stream` or `futures::Stream`
- Tagged unions map to Rust `enum` with `#[derive(Debug, Clone)]`
- `Match.exhaustive` maps to Rust `match` with exhaustiveness checking (compiler-enforced)
- `Stream.mapAccum` maps to `futures::stream::StreamExt::scan`
- The `ReadableStream` bridge would be replaced by `Pin<Box<dyn Stream<Item = StreamPart>>>`

---

## 5. Tools System

### 5.1 Tools Bridge (`tools/bridge.ts`)

Creates an MCP server from AI SDK tool definitions. Delegates to `@compozy/provider-core`:

- `createToolExecutor(config)` -- wraps tools with execution logic
- `createMcpServerFromTools(...)` -- creates SDK MCP server instance
- `bindTools(...)` -- binds tool entries to execution function

Registry: module-level `Map<string, BridgeRegistryEntry>` for resolving bridge servers by UUID.

`TOOLS_BRIDGE_KEY = "__compozyToolsBridgeClaudeCode"` -- metadata key embedded in tool `providerOptions`.

### 5.2 Tool Extraction (`tools/extraction.ts`)

Pure functions to extract tool-related blocks from SDK message content arrays:

- `extractToolUses(content)` -- finds `type: "tool_use"` blocks, returns `ClaudeToolUse[]`
- `extractToolResults(content)` -- finds `type: "tool_result"` blocks, normalizes structured content
- `extractToolErrors(content)` -- finds `type: "tool_error"` blocks

### 5.3 Tool Serialization (`tools/serialization.ts`)

`serializeToolInput(input, limits?)` -- serializes tool input to JSON string with size validation:

- Strings pass through with size check
- Objects are `JSON.stringify`'d
- Checks against `maxSize` (fail with `ToolInputSizeExceededError`) and `warnSize` (log warning)
- Default limits: 100KB max, 50KB warn

### 5.4 Tool Output Truncation (`tools/truncation.ts`)

`truncateToolOutput(output, limits?)` -- intelligent truncation of large tool outputs:

- Arrays: binary search removal of elements from end
- Objects: truncate large string values first, then remove keys
- Strings: byte-level truncation with UTF-8 boundary awareness
- Hard fallback: summary JSON with preview
- Default limits: 100KB max, 50KB warn, truncation disabled by default

### 5.5 Rust Port Considerations -- Tools

- Tool bridge pattern maps to trait objects in Rust
- JSON serialization maps to `serde_json`
- Truncation logic is algorithmic and maps directly
- The module-level registry (`Map`) needs to become `Arc<Mutex<HashMap>>` for thread safety

---

## 6. MCP Integration

### 6.1 Custom MCP Server (`mcp/custom-server.ts`)

`createCustomMcpServer(config)` -- wraps user-defined tools (with Zod schemas) into Claude SDK MCP server:

- Converts `ZodRecord` schemas to `ZodObject({}).passthrough()` for SDK compatibility
- Maps each tool definition to SDK `tool()` call
- Returns `McpSdkServerConfigWithInstance`

### 6.2 Combined MCP Server (`mcp/combined-server.ts`)

`createCombinedMcpServer(config)` -- convenience wrapper that creates a single MCP server from custom tools. Currently throws if `sdkServers` are provided (they should be registered directly in Claude settings).

### 6.3 Rust Port Considerations -- MCP

- MCP server creation is SDK-specific; in Rust, this would use a Rust MCP SDK
- Zod schema conversion maps to JSON Schema generation from Rust types (via `schemars` or similar)

---

## 7. Classifier System

### 7.1 Patterns (`classifier/patterns.ts`)

Detection patterns organized by error category:

| Category        | Status Codes | Error Codes                    | String/Regex Patterns                            |
| --------------- | ------------ | ------------------------------ | ------------------------------------------------ |
| Auth            | 401          | --                             | "not logged in", "authentication required", etc. |
| Authorization   | 403          | --                             | "forbidden", "permission denied", etc.           |
| Rate Limit      | 429          | --                             | "rate limit", "too many requests", etc.          |
| Timeout         | --           | TIMEOUT, ETIMEDOUT             | "timeout", "timed out", etc.                     |
| Network         | --           | ECONNRESET, ECONNREFUSED, etc. | "network", "socket hang up", etc.                |
| Spawn           | --           | ENOENT, EACCES, SPAWN          | "spawn", "failed to start", etc.                 |
| Stream          | --           | CLAUDE_CODE_NO_RESULT          | "stream corrupted", etc.                         |
| JSON Parse      | --           | --                             | "json", "unexpected token", etc.                 |
| Session         | --           | --                             | "session not found", etc.                        |
| Invalid Request | --           | --                             | "invalid request", "bad request", etc.           |
| Tool Execution  | --           | --                             | "tool execution failed", etc.                    |

### 7.2 Classifier (`classifier/classifier.ts`)

`classifyErrorSync(error, stderr?) -> ClaudeCodeError`

1. If error is already a known `ClaudeCodeError` (checked via `Schema.is(ClaudeCodeErrorSchema)`), return as-is
2. Extract error details: message, code, exitCode, statusCode, stderr, timeoutMs, retryAfterMs, sessionId, toolName
3. Build combined lowercase string for pattern matching
4. Classify by priority order (auth > authorization > rate limit > session > invalid request > JSON > stream > timeout > tool execution > spawn > network > process exited > unknown)
5. Build typed error from classified tag

### 7.3 Mapper (`classifier/mapper.ts`)

`mapToAiSdkErrorSync(error, options?) -> APICallError | LoadAPIKeyError`

Maps each `ClaudeCodeError` variant to either:

- `LoadAPIKeyError` (for authentication errors)
- `APICallError` with metadata in `data` field (all other errors)

Metadata (`ClaudeCodeAiSdkErrorMetadata`) includes: errorType, retryable, code, exitCode, timeoutMs, retryAfterMs, sessionId, stderr snippet, promptExcerpt (debug only).

Enriches error messages with stderr snippets (max 320 chars, last 8 lines).

### 7.4 Rust Port Considerations -- Classifier

- Pattern matching maps directly to Rust regex + string contains
- Error classification maps to Rust `match` on enum variants
- The `Schema.is` guard maps to `matches!()` or enum discriminant checks
- `APICallError` / `LoadAPIKeyError` equivalents need to be defined in the Rust AI SDK abstraction

---

## 8. Conversion Layer

### 8.1 Messages (`conversion/messages.ts`)

`convertToClaudeCodeMessages(prompt: LanguageModelV3Prompt) -> ClaudeCodeMessageConversion`

Converts AI SDK prompt format to Claude Code SDK format:

- System messages -> extracted as `systemPrompt`
- User messages -> `"Human: {text}"` format, with image extraction (base64 only, no URLs)
- Assistant messages -> `"Assistant: {text}"`, with `[Tool calls made]` annotation
- Tool messages -> `"Tool Result ({toolName}): {result}"` format

Returns:

- `messagesPrompt` -- concatenated conversation string
- `streamingContentParts` -- structured content parts for streaming input (supports images)
- `hasImageParts` -- flag for streaming mode resolution
- `systemPrompt` -- extracted system prompt
- `warnings` -- conversion warnings (unsupported image URLs, etc.)

### 8.2 Options (`conversion/options.ts`)

`buildClaudeQueryOptions(params) -> Options`

Maps `ClaudeCodeSettings` + `LanguageModelV3CallOptions` to Claude SDK `Options`:

- Core: model, abortController, resume/sessionId, maxTurns, maxBudgetUsd, cwd
- Execution: executable (bun/deno/node), executableArgs
- Permissions: permissionMode, canUseTool, allowedTools, disallowedTools
- System prompt: supports string, preset with append, deprecated fields
- MCP servers: stdio, sse, http, sdk types
- Hooks: maps hook event handlers with timeout conversion
- Agents: maps agent configurations
- Tools bridge: collects MCP servers from tool `providerOptions` metadata
- Output format: JSON schema for structured output
- Reasoning: maps `reasoningEffort` to token budgets (low=16K, medium=32K, high=64K)

### 8.3 Finish Reason (`conversion/finish-reason.ts`)

`mapFinishReason(subtype?, stopReason?) -> LanguageModelV3FinishReason`

Maps Claude SDK finish signals to AI SDK finish reasons:

- `end_turn` / `success` -> `stop`
- `max_tokens` / `error_max_turns` / `error_max_budget_usd` -> `length`
- `tool_use` -> `tool-calls`
- `error_during_execution` -> `error`
- Others -> `other`

### 8.4 Warnings (`conversion/warnings.ts`)

`buildClaudeWarnings(params) -> SharedV3Warning[]`

Generates warnings for:

- Unsupported parameters (temperature, topP, topK, presencePenalty, frequencyPenalty, stopSequences, seed)
- JSON response format without schema
- Very long prompts (>100K chars)
- Model validation warnings
- Settings validation warnings

### 8.5 Rust Port Considerations -- Conversion

- Message conversion is string manipulation + pattern matching; straightforward in Rust
- Image handling (base64 encoding/decoding) maps to `base64` crate
- Options building is struct construction with optional fields
- Finish reason mapping is simple enum conversion

---

## 9. Error Handling

### 9.1 Error Types (`errors.ts`)

18 error types, all `Schema.TaggedError`:

**Retryable (8):**

- `ClaudeTimeoutError` -- timeoutMs
- `ClaudeRateLimitedError` -- retryAfterMs
- `ClaudeNetworkError` -- code
- `ClaudeProcessSpawnError` -- code, exitCode
- `ClaudeSpawnCooldownError` -- consecutiveFailures, retryAfterMs, cooldownMs
- `ClaudeProcessExitedError` -- exitCode, stderr
- `ClaudeStreamCorruptedError` -- reason
- `ClaudeTruncationRecoveryError` -- partialText

**Non-retryable (10):**

- `ClaudeAuthenticationError` -- code, exitCode
- `ClaudeAuthorizationError` -- code, exitCode
- `ClaudeSessionNotFoundError` -- sessionId
- `ClaudeInvalidRequestError` -- details
- `ClaudeJsonParseError` -- source, cause
- `ToolInputSizeExceededError` -- maxBytes, actualBytes
- `ImageConversionError` -- reason
- `SettingsValidationError` -- details (string[])
- `ClaudeToolExecutionError` -- toolName, toolCallId
- `ClaudeUnknownError` -- cause

### 9.2 Error Flow

```
Unknown Error
  -> classifyErrorSync() [classifier]
    -> ClaudeCodeError (tagged union)
      -> mapToAiSdkErrorSync() [mapper]
        -> APICallError | LoadAPIKeyError (AI SDK boundary)
```

`isClaudeRetryableError` uses exhaustive `Match` to classify all 18 variants.

### 9.3 Public API Error Utilities (`index.ts`)

- `classifyClaudeCodeError(error, stderr?)` -- returns `ClaudeCodeClassifiedError` with `type`, `retryable`, `code`, `exitCode`, `message`
- `isAuthenticationError(error)` -- checks `LoadAPIKeyError` or metadata
- `isTimeoutError(error)` -- checks metadata
- `getErrorMetadata(error)` -- extracts metadata from `APICallError`
- `createClaudeSpawnFailureTracker(settings?)` -- plain JS tracker (non-Effect)

### 9.4 Rust Port Considerations -- Errors

- `Schema.TaggedError` maps to Rust `enum` with `#[derive(thiserror::Error)]`
- Exhaustive `Match` maps to Rust `match` (compiler-enforced)
- Retryable classification maps to a method on the error enum
- The `APICallError` / `LoadAPIKeyError` boundary types need Rust equivalents

---

## 10. Generate (Non-Streaming) Path

### `generateClaudeCode` (`generate/generate.ts`)

Similar to `createClaudeCodeStream` but accumulates all events into a `GenerateAccum`:

1. Check spawn failure tracker
2. Validate settings
3. Convert messages, build options, build warnings
4. Resolve streaming input mode (for image support)
5. Create async iterable prompt with optional message injection
6. Run SDK `query()` and pipe through: normalize -> session persist -> accumulate
7. Handle truncation errors gracefully (return partial text with `length` finish reason)
8. Handle spawn failures (record and enrich error metadata)
9. Return `LanguageModelV3GenerateResult` with content, usage, warnings, provider metadata

**Structured output:** If `structuredOutput` exists in the result event, uses it as final text instead of accumulated streaming text.

---

## 11. Configuration & Validation

### Settings Schema (`schemas.ts`)

`ClaudeCodeSettingsSchema` validates ~50 settings fields using Effect `Schema`:

- Model: pathToClaudeCodeExecutable, maxTurns (1-100), maxThinkingTokens, reasoningEffort
- Session: sessionId, resume, continue, persistSession, forkSession
- Permissions: permissionMode (6 modes), allowDangerouslySkipPermissions, canUseTool, allowedTools, disallowedTools
- MCP: mcpServers (stdio/sse/http/sdk), hooks (event -> matcher -> handler[])
- Output: toolOutputLimits, maxToolResultSize, streamingInput (auto/always/off)
- Agents: nested agent configs with description, tools, prompt, model
- Debug: verbose, debug, debugFile, logger
- Environment: cwd (validated as existing directory), env, executable, executableArgs
- Advanced: spawnFailurePolicy, sdkOptions (escape hatch), extraArgs, plugins, sandbox

### Config Service (`config.ts`)

`ClaudeCodeConfig` is a `Context.Tag` holding validated settings. Provides:

- `ClaudeCodeConfig.make(rawSettings)` -- validates and wraps
- `ClaudeCodeConfig.layer(rawSettings)` -- Layer for DI
- `ClaudeCodeConfig.fromSettings(validated)` -- Layer from pre-validated settings

---

## 12. Utility Layer (`utils.ts`)

- `MessageInjector` -- queue-based mechanism for injecting user messages mid-stream
- `toAsyncIterablePrompt(...)` -- creates async iterable yielding initial prompt then waiting for injections or stream end
- `isClaudeCodeTruncationError(error, bufferedText)` -- detects truncated JSON streams (SyntaxError + min 512 chars)
- `isAbortError(error)` -- detects AbortError by name or code

---

## 13. Comprehensive Rust Port Mapping

### Effect-TS to Rust Pattern Mapping

| Effect-TS                                              | Rust Equivalent                             |
| ------------------------------------------------------ | ------------------------------------------- |
| `Effect<A, E, R>`                                      | `Result<A, E>` + async + DI                 |
| `Effect.gen(function* () { ... })`                     | `async fn() -> Result<T, E>`                |
| `yield*`                                               | `?` operator / `.await`                     |
| `Effect.fn("name")`                                    | Tracing spans via `#[tracing::instrument]`  |
| `Stream<A, E, R>`                                      | `Pin<Box<dyn Stream<Item = Result<A, E>>>>` |
| `Stream.mapAccum`                                      | `futures::stream::StreamExt::scan`          |
| `Stream.flatMap`                                       | `futures::stream::StreamExt::flat_map`      |
| `Context.Tag` + `Layer`                                | Trait + struct injection / `Arc<dyn Trait>` |
| `Ref<A>`                                               | `Arc<Mutex<A>>` or `Arc<RwLock<A>>`         |
| `HashMap` (immutable)                                  | `std::collections::HashMap` (mutable)       |
| `Option`                                               | `std::option::Option`                       |
| `Match.value().pipe(Match.tag(...), Match.exhaustive)` | `match value { Variant::A(..) => ..., }`    |
| `Schema.TaggedError`                                   | `#[derive(thiserror::Error)] enum`          |
| `Schema.decode`                                        | `serde::Deserialize` + validation           |
| `Duration`                                             | `std::time::Duration`                       |
| `ManagedRuntime`                                       | Shared `Arc<ServiceContainer>`              |

### Architecture Mapping

| TypeScript Component      | Rust Structure                                                                   |
| ------------------------- | -------------------------------------------------------------------------------- |
| `ClaudeCodeLanguageModel` | `struct ClaudeCodeModel` implementing `LanguageModel` trait                      |
| `ClaudeCodeProvider`      | `struct ClaudeCodeProvider` implementing `Provider` trait                        |
| `StreamAccumState`        | `struct StreamState` (mutable, single-ownership per stream)                      |
| `NormalizedEvent` enum    | `enum NormalizedEvent` with 13 variants                                          |
| `ClaudeCodeError` union   | `enum ClaudeCodeError` with 18 variants                                          |
| `ToolLifecycleService`    | `struct ToolLifecycle` with `HashMap<String, ToolPhase>`                         |
| `NestedToolTracker`       | `struct NestedTracker` with `HashMap<String, NestedStore>`                       |
| `TextDeduplicator`        | `struct TextDedup` with `(usize, String)` state                                  |
| `SpawnFailureTracker`     | `struct SpawnTracker` with `Arc<Mutex<SpawnState>>`                              |
| `SessionManager`          | `struct SessionMgr` with `Arc<RwLock<Option<String>>>`                           |
| Event normalizer          | `fn normalize(state: &mut StreamState, msg: SdkMessage) -> Vec<NormalizedEvent>` |
| Stream pipeline           | `stream.scan().flat_map().then().then()...` chain                                |
| Error classifier          | `fn classify(error: &dyn Error, stderr: Option<&str>) -> ClaudeCodeError`        |
| Settings validation       | `#[derive(Deserialize, Validate)] struct Settings`                               |

### Key Challenges for Rust Port

1. **Streaming pipeline composition** -- Effect's `Stream.pipe` chains need equivalent `futures::Stream` combinators
2. **Service injection** -- Effect's `Layer` system is very ergonomic; Rust needs manual DI or a framework
3. **JSON handling** -- The normalizer does extensive `asRecord`/`asString` duck-typing; Rust needs `serde_json::Value` pattern matching
4. **Process spawning** -- The SDK wraps subprocess management; Rust can use `tokio::process::Command`
5. **MCP server** -- Needs a Rust MCP SDK implementation
6. **AbortController** -- Maps to `tokio_util::sync::CancellationToken`

---

## 14. Relevant Files

All source files in the package:

**Core:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/index.ts` (public API surface)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/types.ts` (re-exported types)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/schemas.ts` (settings validation, ~50 fields)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/config.ts` (Effect config service)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/errors.ts` (18 tagged error types)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/utils.ts` (message injector, prompt iterable, error detection)

**Services:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/services/language-model.ts` (LanguageModelV3 implementation)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/services/provider.ts` (provider factory + Effect service)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/services/session.ts` (session ID management)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/services/spawn-failure-tracker.ts` (spawn cooldown logic)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/services/tools-bridge-registry.ts` (tools bridge DI)

**Streaming Pipeline:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/stream.ts` (main stream orchestrator, ~472 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/event-normalizer.ts` (SDK message -> normalized events, ~1147 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/normalized-events.ts` (13 event type definitions)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/stream-state.ts` (accumulator state type)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/stream-parts.ts` (event -> AI SDK stream part mapping)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/tool-lifecycle.ts` (tool FSM)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/nested-tool-tracker.ts` (parent-child tool management)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/stream/text-deduplicator.ts` (stream/assistant dedup)

**Tools:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/tools/bridge.ts` (AI SDK tools -> MCP bridge)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/tools/extraction.ts` (extract tool blocks from content)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/tools/serialization.ts` (tool input serialization + size check)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/tools/truncation.ts` (intelligent output truncation)

**MCP:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/mcp/custom-server.ts` (Zod-schema tools -> SDK MCP server)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/mcp/combined-server.ts` (convenience wrapper)

**Classifier:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/classifier/classifier.ts` (error classification logic)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/classifier/mapper.ts` (ClaudeCodeError -> AI SDK error mapping)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/classifier/patterns.ts` (detection patterns by category)

**Conversion:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/conversion/messages.ts` (AI SDK prompt -> Claude Code format)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/conversion/options.ts` (settings -> SDK Options, ~705 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/conversion/finish-reason.ts` (finish reason mapping)
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/conversion/warnings.ts` (warning generation)

**Generate:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/generate/generate.ts` (non-streaming generation, ~445 lines)

**Shared dependency:**

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/index.ts` (provider-core shared abstractions)
