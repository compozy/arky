# Deep Analysis Report: providers/opencode Package

## 1. Overview

The `@compozy/provider-opencode` package is an AI SDK-compatible provider that communicates with an **OpenCode server** (a local/remote coding AI agent). It implements the `LanguageModelV3` specification from `@ai-sdk/provider`, translating AI SDK calls into OpenCode server API requests and SSE event streams.

### Architecture Summary

The package follows a **layered Effect-TS service architecture**:

```
OpencodeProvider (ProviderV3)
  -> OpencodeLanguageModel (LanguageModelV3)
       -> ManagedRuntime<OpencodeLanguageModelRuntimeContext>
            -> RequestService (prepare requests, create sessions)
            -> GenerateService (non-streaming generate)
            -> StreamingService (SSE streaming pipeline)
            -> SessionManagerService (per-session semaphore locks)
```

All services are composed via Effect `Layer` and wired together in `OpencodeProviderLayer()`. The provider spawns or connects to an OpenCode CLI server process, manages SDK clients through a pool, and converts SSE events into `LanguageModelV3StreamPart` chunks.

### Key External Dependencies

| Dependency                           | Purpose                                              |
| ------------------------------------ | ---------------------------------------------------- |
| `@ai-sdk/provider` (^3.0.8)          | AI SDK provider interfaces (LanguageModelV3, errors) |
| `@compozy/provider-core` (workspace) | Shared tools bridge, MCP server, tool executor       |
| `@opencode-ai/sdk` (1.2.24)          | OpenCode server SDK client                           |
| `effect` (^3.19.19)                  | Effect-TS runtime (services, layers, streams, refs)  |
| `es-toolkit` (^1.45.1)               | Utility functions (array, string, object, math)      |

### Key Internal Dependencies

- `jsonc-parser` - JSONC config file parsing
- `node:child_process` - Server process spawning
- `node:fs/promises` - Config file I/O
- `node:buffer` - Base64 encoding for file parts

---

## 2. Adapters

### 2.1 AI SDK Adapter (`adapters/ai-sdk-adapter.ts`)

**Purpose**: Translates between AI SDK V3 protocol types and OpenCode server types.

**Key Functions**:

| Function                        | Signature                                                            | Purpose                                                               |
| ------------------------------- | -------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `toOpencodePrompt`              | `(prompt: LanguageModelV3Prompt) => OpencodePromptTranslation`       | Converts AI SDK prompt messages to OpenCode parts (text, file, agent) |
| `buildPromptBody`               | `(input: BuildPromptBodyInput) => PromptBody`                        | Assembles the full prompt body for the OpenCode server                |
| `buildGenerateResult`           | `(input: BuildGenerateResultInput) => LanguageModelV3GenerateResult` | Converts OpenCode response data to AI SDK generate result             |
| `toContents`                    | `(part: Part) => ReadonlyArray<LanguageModelV3Content>`              | Converts OpenCode parts to AI SDK content parts                       |
| `toUsage`                       | `(tokens: TokenUsage) => LanguageModelV3Usage`                       | Maps token usage with cache breakdown                                 |
| `mapFinishReason`               | `(finishReason: string) => LanguageModelV3FinishReason`              | Maps finish reason strings to typed union                             |
| `mapSdkErrorToOpencodeError`    | `(error: unknown, context: SdkErrorContext) => OpencodeError`        | Maps SDK errors to typed OpencodeError hierarchy                      |
| `extractPromptResponseEnvelope` | `(response: unknown) => { data, error }`                             | Unwraps response envelope safely                                      |

**Key Types**:

- `OpencodePromptTranslation` - `{ system: Option<string>, parts: PromptPartInput[], title: Option<string> }`
- `PromptBody` - Full request body for OpenCode server prompt endpoint
- `TokenUsage` - `{ input?, output?, reasoning?, cacheRead?, cacheWrite? }`
- `SdkErrorContext` - `{ provider: string, sessionId: Option<string> }`

**Patterns**:

- Uses `Effect/Option` extensively for nullable value handling (not null/undefined)
- Agent mention extraction via regex `@name` patterns from text
- File normalization handles `Uint8Array`, `URL`, remote URLs, and base64 data URLs
- Context overflow detection via 11 regex patterns matching common error messages

### 2.2 Error Adapter (`adapters/error-adapter.ts`)

**Purpose**: Converts internal `OpencodeError` tagged union to `@ai-sdk/provider.APICallError` instances.

**Pattern**: Uses `Effect/Match` with `Match.exhaustive` to map all 18 error tags to appropriate HTTP status codes and error metadata.

**Error-to-HTTP mapping**:

| OpencodeError Tag         | HTTP Status | AI SDK errorType          |
| ------------------------- | ----------- | ------------------------- |
| `AuthError`               | 401         | `authentication_error`    |
| `BusyError`               | 429         | `session_busy`            |
| `RateLimitError`          | 429         | `rate_limit`              |
| `ContextOverflowError`    | 413         | `context_length_exceeded` |
| `ConnectionError`         | 503         | (retryable)               |
| `ServerStartError`        | 503         | (retryable)               |
| `ServerNotRunningError`   | 503         | (retryable)               |
| `ServerTimeoutError`      | 504         | `server_timeout`          |
| `ConfigFileNotFoundError` | 400         | `config_file_not_found`   |
| `ConfigParseError`        | 400         | `config_parse_error`      |
| `ConfigValidationError`   | 400         | `config_validation_error` |
| `StreamError`             | 502         | `stream_error`            |
| `StreamTimeoutError`      | 504         | `stream_timeout`          |
| `ReconnectExhaustedError` | 503         | `reconnect_exhausted`     |
| `DoomLoopError`           | 429         | `doom_loop_detected`      |
| `HookTimeoutError`        | 408         | `hook_timeout`            |
| `HookBlockedError`        | 400         | `hook_blocked`            |
| `PromptBlockedError`      | 400         | `prompt_blocked`          |

---

## 3. Error Hierarchy

All errors use `Data.TaggedError` from Effect-TS, enabling exhaustive pattern matching. The union type is:

```typescript
type OpencodeError = ProviderError | ClientError | StreamingError | ConfigError | HookError;
```

### 3.1 Provider Errors (`errors/provider-errors.ts`)

| Error                  | Fields                                  | Purpose                               |
| ---------------------- | --------------------------------------- | ------------------------------------- |
| `AuthError`            | `message, provider`                     | Authentication/authorization failures |
| `BusyError`            | `sessionId`                             | Session already in use                |
| `RateLimitError`       | `message, retryAfterMs: Option<number>` | Rate limiting                         |
| `ContextOverflowError` | `message, maxTokens: Option<number>`    | Context window exceeded               |

### 3.2 Client Errors (`errors/client-errors.ts`)

| Error                   | Fields                    | Purpose                      |
| ----------------------- | ------------------------- | ---------------------------- |
| `ConnectionError`       | `message, cause?: Defect` | Network/connection failures  |
| `ServerStartError`      | `message, cause: Defect`  | Server process spawn failure |
| `ServerNotRunningError` | `message`                 | Server not available         |
| `ServerTimeoutError`    | `timeoutMs, message`      | Server startup timeout       |

### 3.3 Streaming Errors (`errors/streaming-errors.ts`)

| Error                     | Fields                                            | Purpose                      |
| ------------------------- | ------------------------------------------------- | ---------------------------- | ----------------- | ------------------ |
| `StreamError`             | `message, stage: "subscribe"                      | "consume"                    | "convert", cause` | SSE stream failure |
| `StreamTimeoutError`      | `sessionId, timeoutMs`                            | Stream stall timeout         |
| `ReconnectExhaustedError` | `attempts, lastError: Option<StreamErrorSummary>` | Max reconnects exceeded      |
| `DoomLoopError`           | `toolName, repeatCount, sessionId`                | Repeated tool call detection |

### 3.4 Config Errors (`errors/config-errors.ts`)

| Error                     | Fields                           | Purpose                   |
| ------------------------- | -------------------------------- | ------------------------- |
| `ConfigFileNotFoundError` | `source, path`                   | Config file missing       |
| `ConfigParseError`        | `source, message, cause: Defect` | JSONC parse failure       |
| `ConfigValidationError`   | `source, issues: string[]`       | Schema validation failure |

### 3.5 Hook Errors (`errors/hook-errors.ts`)

| Error                | Fields                      | Purpose                |
| -------------------- | --------------------------- | ---------------------- |
| `HookTimeoutError`   | `event, handler, timeoutMs` | Hook execution timeout |
| `HookBlockedError`   | `event, reason`             | Hook blocked execution |
| `PromptBlockedError` | `reason`                    | Prompt blocked by hook |

### 3.6 Hook Runner Errors (`errors/hook-runner-errors.ts`)

| Error                      | Fields                     | Purpose                      |
| -------------------------- | -------------------------- | ---------------------------- |
| `HookRunnerExecutionError` | `handler, message, cause?` | Internal hook runner failure |

### Rust Port Consideration

All 18 error types map directly to Rust enums. The `Data.TaggedError` pattern translates to:

```rust
#[derive(Debug, thiserror::Error)]
pub enum OpencodeError {
    #[error("Auth error: {message}")]
    Auth { message: String, provider: String },
    #[error("Session busy: {session_id}")]
    Busy { session_id: String },
    // ... etc
}
```

The `Option<T>` fields map to Rust's `Option<T>`. The `Defect` type (arbitrary serializable error) maps to `Box<dyn std::error::Error + Send + Sync>` or `anyhow::Error`.

---

## 4. Model Layer

### 4.1 OpencodeLanguageModel (`model/opencode-language-model.ts`)

**Purpose**: Implements `LanguageModelV3` interface with `doGenerate` (non-streaming) and `doStream` (streaming) methods.

**Key Properties**:

- `specificationVersion: "v3"`
- `provider: "opencode"`
- `supportedUrls: { "*/*": [/^https?:\/\//] }`
- Per-model `Effect.Semaphore(1)` for session selection serialization
- Tracks `currentSessionId: Option<string>` across calls

**doGenerate Flow**:

1. Check `isDisposed()`
2. Run Effect in `ManagedRuntime`:
   - `RequestService.prepare()` (within semaphore) -> gets client, session, body
   - `SessionManagerService.withLock()` -> `GenerateService.generate()`
3. Convert `Exit` to throw via `toAISDKError()`

**doStream Flow**:

1. Check `isDisposed()`
2. Run Effect in `ManagedRuntime`:
   - `RequestService.prepare()` (within semaphore)
   - `SessionManagerService.acquire()` -> get release handle
   - `RequestService.startPromptAsync()` -> fire prompt
   - `StreamingService.stream()` -> get `ReadableStream`
3. Wrap stream with `withStreamFinalizer()` to release session lock on close/error

**Rust Port Consideration**: The `ManagedRuntime` pattern maps to a Rust struct with `Arc<Mutex<...>>` or `tokio::sync::Semaphore`. The `ReadableStream` with finalizer pattern maps to Rust's `Stream` trait with `Drop` or explicit cleanup via `tokio::sync::oneshot`.

### 4.2 OpencodeProvider (`model/opencode-provider.ts`)

**Purpose**: Factory that creates the `ManagedRuntime` with all layers and exposes the provider API.

**Key Type**:

```typescript
type OpencodeProvider = ProviderV3 & {
  (modelId, settings?): OpencodeLanguageModel;
  languageModel(modelId, settings?): OpencodeLanguageModel;
  chat(modelId, settings?): OpencodeLanguageModel;
  getClientManager(): OpencodeClientManagerAdapter;
  dispose(): Promise<void>;
};
```

**Layer Composition** (`OpencodeProviderLayer`):

```
ValidationService.layer
ConfigSources (Remote + File + Directory + Env + Managed + Programmatic)
  -> OpencodeConfigService.layer
ServerProcessConfig
  -> ServerProcessService.layer
RestartPolicyConfig -> RestartPolicyService.layer
HealthMonitorConfig -> HealthMonitorService.layer
  -> ClientPoolService.layer
    -> ClientManagerService.layer
OpencodeHookService.layerFor()
SubagentManagerConfig -> SubagentManagerService.layer
  -> StreamingService.layer
PromptBuilderService.layer
  -> RequestService.layer
GenerateService.layer
SessionManagerService.layer
```

**Rust Port Consideration**: This layer composition maps to a Rust builder pattern or dependency injection. The `ManagedRuntime` maps to a struct holding `Arc<dyn Service>` references. The `dispose()` method maps to `Drop` trait implementation or explicit `shutdown()`.

---

## 5. Client Management

### 5.1 ServerProcessService (`services/client/server-process-service.ts`)

**Purpose**: Manages the OpenCode server process lifecycle -- spawn, monitor, stop.

**Key Types**:

- `ServerHandle` - `{ url: string, managed: boolean, pid: Option<number> }`
- `OpencodeSpawnAdapter` - Abstraction over `child_process.spawn`
- `OpencodeSpawnedProcess` - Process handle with output/close/error event listeners
- `ServerProcessConfig` - Configuration tag (hostname, port, baseUrl, autoStart, timeout, executable path)

**State Machine** (via `Ref<ServerProcessState>`):

- `StartDecision`: `Existing | AwaitStart | StartNow` (deduplication pattern)
- Handles external baseUrl (no spawn) vs managed spawn
- Parses server URL from stdout via regex: `opencode server listening on <url>`
- Timeout via `Effect.timeoutFail`
- Cleanup via `Effect.acquireRelease` + `Scope`

**Rust Port Consideration**: Maps to `tokio::process::Command` with stdout parsing. The deferred/state pattern maps to `tokio::sync::watch` or `tokio::sync::Mutex<State>` with `tokio::sync::Notify`.

### 5.2 ClientPoolService (`services/client/client-pool-service.ts`)

**Purpose**: Keyed pool of OpenCode SDK clients, one per config key (directory).

**Pattern**: Thread-safe caching using `Ref<PoolState>` where:

- `PoolState.clients: HashMap<string, OpencodeClient>` - cached clients
- `PoolState.inFlight: HashMap<string, Deferred<OpencodeClient, ConnectionError>>` - pending creations

**Decision pattern**: `Cached | AwaitInFlight | CreateNow` (same deduplication as ServerProcess)

**Rust Port Consideration**: Maps to `DashMap<String, Arc<OpencodeClient>>` or `tokio::sync::RwLock<HashMap<String, ...>>` with inflight tracking via `Arc<tokio::sync::Notify>`.

### 5.3 HealthMonitorService (`services/client/health-monitor-service.ts`)

**Purpose**: Periodic health polling of the OpenCode server.

**Key Types**:

- `HealthState`: `Healthy | Degraded | Unhealthy` (tagged enum)
- `HealthSnapshot`: state + consecutiveFailures + timestamps + lastError

**Behavior**:

- Polls endpoints `/global/health`, `/health`, `/config` in fallback order
- Runs on `Schedule.fixed(interval)` as a daemon fiber
- State changes are published via `SubscriptionRef<HealthSnapshot>`
- Transitions: Healthy (0 failures) -> Degraded (1..threshold) -> Unhealthy (>=threshold)

**Rust Port Consideration**: Maps to a `tokio::spawn` background task with `tokio::sync::watch::Sender<HealthSnapshot>`. The `SubscriptionRef` maps to `watch::Receiver`.

### 5.4 RestartPolicyService (`services/client/restart-policy-service.ts`)

**Purpose**: Circuit breaker + exponential backoff for server restarts.

**Key Types**:

- `CircuitState`: `Closed | Open { openedAtMs } | HalfOpen` (tagged enum)
- Config: `maxAttempts, baseDelay, maxDelay, cooldown`

**Behavior**:

- `shouldRestart(attempt)` - checks circuit state and attempt count
- `nextDelay(attempt)` - computes jittered exponential backoff
- `recordRestartSuccess()` -> Closed
- `recordRestartFailure()` -> Open

**Rust Port Consideration**: Standard circuit breaker pattern. Maps directly to a Rust struct with `AtomicU32` for state and `tokio::time::sleep` for delays.

### 5.5 ClientManagerService (`services/client/client-manager-service.ts`)

**Purpose**: Facade that orchestrates ServerProcess, HealthMonitor, ClientPool, and RestartPolicy.

**Methods**:

- `start()` -> `ServerHandle`
- `getClient(configKey)` -> `OpencodeClient`
- `getHealthSnapshot()` -> `HealthSnapshot`
- `subscribeHealth()` -> `Stream<HealthSnapshot>`
- `initialize(configKey)` -> `ClientInitialization` (server + client + health)
- `restartIfNeeded(attempt, configKey)` -> `boolean`
- `stop()` -> void

---

## 6. Configuration System

### 6.1 Config Schema (`schemas/config-schema.ts`)

Defines the complete OpenCode configuration using Effect Schema. Key sections:

| Config Section         | Purpose                                                      |
| ---------------------- | ------------------------------------------------------------ |
| `model`, `small_model` | Default model selection                                      |
| `provider`             | Per-provider config (whitelist, blacklist, models, API keys) |
| `agent`                | Agent definitions (model, tools, prompt, permissions, mode)  |
| `mcp`                  | MCP server configs (local/remote)                            |
| `permission`           | Per-tool permission rules (ask/allow/deny)                   |
| `server`               | Server port, hostname, CORS                                  |
| `skills`               | Skill paths and URLs                                         |
| `compaction`           | Auto-compaction settings                                     |
| `experimental`         | Feature flags                                                |
| `hooks`                | via experimental.hook                                        |

### 6.2 Multi-Source Config Resolution

Config sources are loaded concurrently with priority-based merging:

| Source                     | Priority    | File                                               |
| -------------------------- | ----------- | -------------------------------------------------- |
| `RemoteConfigSource`       | (remote)    | Enterprise/remote config                           |
| `FileConfigSource`         | (file)      | `opencode.jsonc` / `opencode.json` in CWD + global |
| `DirectoryConfigSource`    | (directory) | Config in `.opencode/` directory                   |
| `EnvConfigSource`          | (env)       | `OPENCODE_CONFIG` env var                          |
| `ManagedConfigSource`      | (managed)   | Dynamically managed config                         |
| `ProgrammaticConfigSource` | 600         | Options passed to `createOpencode()`               |

**Config source interface**:

```typescript
type ConfigSourceService = {
  load: () => Effect<Option<ConfigSourceResult>, ConfigLoadError>;
};
```

**Config merging** (`mergeConfigObjects`):

- Deep merges objects
- Arrays are replaced (not concatenated), except `plugin` and `instructions` which are unioned
- Higher priority sources override lower ones

**Config variable substitution**:

- `{env:VAR_NAME}` -> environment variable
- `{file:./path}` -> file contents (with path traversal protection)
- `~` expansion for home directory

### 6.3 Config Mapper (`services/config/config-mapper.ts`)

Converts raw `OpencodeConfig` to `OpencodeModelSettings`:

- Resolves model IDs: `"provider/model"` format or shortcut names
- Extracts agent config (tools, reasoning settings)
- Provider model reasoning options override
- Agent-level reasoning options override

### 6.4 OpencodeConfigService (`services/config/config-service.ts`)

**Methods**:

- `resolve(options?)` -> `OpencodeResolvedConfig` (full validated config + settings)
- `resolveRaw()` -> `OpencodeConfig` (raw merged config)

**Rust Port Consideration**: The multi-source config system maps to a Rust trait `ConfigSource` with implementations for file, env, etc. JSONC parsing via `serde_jsonc` or similar. Schema validation via custom validators or `serde` with custom deserialize implementations. Variable substitution via string replacement.

---

## 7. Hooks System

### 7.1 Hook Service (`services/hooks/hook-service.ts`)

**Purpose**: Lifecycle hook system for pre/post tool use, session start/end, stop, and prompt submission.

**Hook Events**:
| Event | Context Type | Decision Type |
|---|---|---|
| `PreToolUse` | `ToolUseContext` | `ToolUseDecision` (Allow/Block/Transform) |
| `PostToolUse` | `ToolUseContext` | void |
| `SessionStart` | `SessionContext` | void |
| `SessionEnd` | `SessionContext` | void |
| `Stop` | `SessionContext` | `StopDecision` (Allow/Block) |
| `UserPromptSubmit` | `PromptContext` | `PromptDecision` (Allow/Block/Transform) |

**Decision Types** (all `Data.TaggedEnum`):

- `ToolUseDecision`: `Allow | Block { reason } | Transform { input }`
- `StopDecision`: `Allow | Block { reason }`
- `PromptDecision`: `Allow | Block { reason } | Transform { prompt }`

**Hook Matching**:

- Matchers compiled to `RegExp` with caching
- Suspicious regex patterns rejected (catastrophic backtracking prevention)
- Max matcher length: 256 characters

**Hook Timeout Resolution** (precedence):

1. Handler-level `timeoutMs` / `timeout`
2. Matcher-level `timeoutMs` / `timeout`
3. Default timeout (10 seconds)

### 7.2 Hook Runners (`services/hooks/hook-runners.ts`)

Three runner types:

| Runner            | Input                    | Mechanism                                          |
| ----------------- | ------------------------ | -------------------------------------------------- |
| `runCallbackHook` | JS function / descriptor | Direct function call                               |
| `runShellHook`    | Command string / object  | `child_process.spawn` with stdin JSON              |
| `runPromptHook`   | Prompt template          | Template rendering with `{{ path }}` interpolation |

**Shell Hook Protocol**:

- Stdin: `{ event: string, input: HookInput }` as JSON
- Stdout: JSON object parsed as `HookJSONOutput`, or plain text as systemMessage
- Max output: 1MB
- Non-zero exit = error

**Rust Port Consideration**: Callback hooks are JS-specific and won't port. Shell hooks map to `tokio::process::Command`. Prompt hooks map to template string rendering. The hook matching system is straightforward regex-based filtering.

---

## 8. Streaming Architecture

### 8.1 Overview

The streaming pipeline is assembled in `StreamingService.buildStreamingPipeline()`:

```
Event Source (SSE)
  -> Resilient reconnection
  -> Session filter (parent + child sessions)
  -> Event converter (Event -> LanguageModelV3StreamPart)
  -> Hook executor (PreToolUse / PostToolUse)
  -> Doom loop detector
  -> Stream state machine
  -> Session lifecycle hooks (SessionStart/SessionEnd)
  -> Finish detector (takeUntil "finish")
  -> Safety limits (max events, max duration)
  -> ReadableStream output
```

### 8.2 Event Source (`services/streaming/event-source.ts`)

**Purpose**: Creates Effect `Stream<Event>` from various sources.

**Source types** (`StreamSource` tagged enum):

- `Subscribe` - Custom subscribe function returning async iterable
- `AsyncIterable` - Direct async iterable
- `SdkClient` - OpenCode SDK client's `event.subscribe()` method

**SDK Event Source**: Passes `directory` and `Last-Event-ID` headers for reconnection.

### 8.3 Resilient Event Source (in `streaming-service.ts`)

**Reconnection behavior**:

- Exponential backoff with jitter: `Schedule.exponential(baseDelay).pipe(Schedule.jittered)`
- Configurable max attempts (default: 3)
- Stall timeout per event (default: 60s)
- Tracks `lastEventId` for SSE resumption
- State transitions: `Connecting -> Receiving -> Reconnecting -> ...`
- On exhaustion: `ReconnectExhaustedError`

### 8.4 Event Converter (`services/streaming/event-converter.ts`)

**Purpose**: Stateful conversion of OpenCode SSE events to `LanguageModelV3StreamPart` chunks.

**State** (`EventConverterState`):

- `textParts: Map<id, text>` - accumulated text per part ID
- `reasoningParts: Map<id, text>` - accumulated reasoning
- `messageRoles: Map<messageId, role>` - track user vs assistant
- `toolEmitStates: Map<callId, ToolEmitState>` - track tool lifecycle
- `toolCallStates: HashMap<callId, ToolCallState>` - typed tool states
- `usage: LanguageModelV3Usage` - accumulated token usage
- `finishReason` - latest finish reason
- `providerMetadata` - accumulated metadata
- `compactedCount` - compaction counter

**Event type handling**:
| Event Type | Output Parts |
|---|---|
| `message.updated` | Updates message role map |
| `message.part.updated` (text) | `text-start`, `text-delta`, `text-end` |
| `message.part.updated` (reasoning) | `reasoning-start`, `reasoning-delta`, `reasoning-end` |
| `message.part.updated` (tool) | `tool-input-start`, `tool-input-delta`, `tool-input-end`, `tool-call`, `tool-result` |
| `message.part.updated` (step-finish) | Updates usage/finishReason/metadata |
| `message.part.updated` (file/patch) | `source` |
| `session.status` (idle) | `finish` |
| `session.idle` | `finish` |
| `session.compacted` | Updates compacted count |
| `session.error` | `error` + `finish` |

**Tool part conversion** is the most complex: tracks input started/closed/call emitted/result emitted per callId, emitting incremental deltas.

### 8.5 Doom Loop Detector (`services/streaming/doom-loop-detector.ts`)

**Purpose**: Detects repeated identical tool calls (same tool + same input).

**Algorithm**:

- Tracks tool call signatures: `"toolName:serializedInput"`
- Uses `Sink.foldWeighted` for weighted counting
- Configurable threshold (default: 4, min: 2)
- Configurable weight per tool name
- Emits `DoomLoopError` when weighted count >= threshold

### 8.6 Hook Executor (`services/streaming/hook-executor.ts`)

**Purpose**: Applies PreToolUse/PostToolUse hooks to the stream.

**Behavior**:

- For `tool-call` parts: runs `PreToolUse` hooks
  - `Allow`: pass through
  - `Block`: suppress tool-call and its result
  - `Transform`: modify input
- For `tool-result` parts: runs `PostToolUse` hooks (fire-and-forget)
- Tracks blocked tool calls via `Ref<HashMap<callId, reason>>`
- Hook timeouts are warnings (don't block the stream)

### 8.7 Session Filter (`services/streaming/session-filter.ts`)

**Purpose**: Filters events to only those belonging to the current session or known child sessions.

**Functions**:

- `getEventSessionId(event)` - extracts sessionID from event properties (checks properties.sessionID, properties.part.sessionID, properties.info.sessionID)
- `filterEventsBySession(stream, options)` - filters stream by allowed session IDs

### 8.8 Stream State Machine (`services/streaming/stream-state.ts`)

**Purpose**: Formal state machine tracking the overall stream lifecycle.

**States** (`StreamState` tagged enum):

- `Connecting { attempt }`
- `Receiving { sessionId, lastEventId, toolCalls, usage }`
- `Reconnecting { attempt, lastEventId, reason }`
- `Completing { finishReason, usage }`
- `Errored { error }`

**Transitions**: Validated state transitions (e.g., can't go from `Errored` to `Receiving`). Invalid transitions return `Either.left(InvalidStreamStateTransitionError)`.

**Rust Port Consideration**: The streaming pipeline maps to Rust's `tokio_stream::Stream` or `futures::Stream`. The state machine is a standard Rust enum. The event converter maintains mutable state -- in Rust this would be a `fold`/`scan` over the stream with a state struct. SSE reconnection maps to a retry loop around `reqwest::Client` or `eventsource-client`.

### 8.9 Subagent Manager (`services/streaming/subagent-manager.ts`)

**Purpose**: Tracks parent-child session relationships for subagent (subtask) streams.

**State**:

- `Queue<PendingSubagentSession>` - pending child sessions awaiting parent tool call match
- `Ref<HashMap<childSessionId, parentToolCallId>>` - resolved mappings

**Methods**:

- `registerChildSession(childSessionId, parentToolCallId)`
- `resolveParentToolCall(childSessionId) -> Option<parentToolCallId>`
- `enqueuePendingSession(childSessionId)`
- `matchNextPendingSession(parentToolCallId) -> Option<PendingSubagentSession>`

---

## 9. Tools System

### 9.1 Tool Executor (`services/tools/tool-executor.ts`)

**Purpose**: Effect-TS wrapper around `@compozy/provider-core`'s tool executor.

**Error types**:

- `ToolExecutorClosedError` - executor already closed
- `ToolNotFoundError` - tool not registered
- `ToolExecutionFailedError` - execution failure

**Interface** (`OpencodeToolExecutor`):

- `tools()` -> `BoundTools` (with metadata for OpenCode registration)
- `callTool(toolName, args, context)` -> `MinimalCallToolResult`
- `execute(params)` -> `MinimalCallToolResult`
- `listTools()` -> `ToolsList`
- `close()`

### 9.2 Tools Bridge Service (`services/tools/tools-bridge-service.ts`)

**Purpose**: Full Effect-TS service that manages tool executor + MCP HTTP server lifecycle.

**Architecture**:

- Creates tool executor adapter
- Starts MCP HTTP server on `127.0.0.1` with `/mcp` path
- Exposes `tools()`, `callTool()`, `listTools()` through service interface
- Uses `Effect.acquireRelease` for lifecycle management

**Key constant**: `TOOLS_BRIDGE_KEY = "__compozyToolsBridgeOpenCode"` - metadata key for bridge identification

### 9.3 ToolsBridge (`tools-bridge.ts`)

**Purpose**: Simplified non-Effect imperative API for tools bridge (used externally).

**Interface** (`ToolsBridgeInstance`):

- `name: string`
- `tools()` -> `Promise<BoundTools>`
- `callTool(toolName, args, context)` -> `Promise<MinimalCallToolResult>`
- `listTools()` -> `ToolsList`
- `close()` -> void

---

## 10. Session Management

### SessionManagerService (`services/session-manager-service.ts`)

**Purpose**: Per-session mutex to prevent concurrent operations on the same session.

**Implementation**:

- `HashMap<string, Effect.Semaphore>` - one semaphore per session ID
- `HashSet<string>` - tracks which sessions are currently locked
- `acquire(sessionId)` -> returns `SessionRelease` effect
- `withLock(sessionId, effect)` -> runs effect under session lock
- `isLocked(sessionId)` -> boolean check

**Rust Port Consideration**: Maps to `Arc<DashMap<String, tokio::sync::Mutex<()>>>` or `Arc<Mutex<HashMap<String, Arc<Semaphore>>>>`.

---

## 11. Supporting Services

### ValidationService (`services/validation-service.ts`)

Validates config, model settings, and provider settings using Effect Schema decoders.

### PromptBuilderService (`services/prompt-builder-service.ts`)

Thin wrapper around `toOpencodePrompt()` as an Effect service.

### GenerateService (`services/generate-service.ts`)

Non-streaming generate: calls `client.session.prompt()`, unwraps envelope, builds result.

### RequestService (`services/request-service.ts`)

Prepares requests:

1. Optionally loads config via `OpencodeConfigService`
2. Merges settings (config + defaults + call settings + provider options)
3. Validates merged settings
4. Builds prompt via `PromptBuilderService`
5. Gets client from `ClientManagerService`
6. Creates/reuses session
7. Returns `PreparedRequest` with everything needed for generate/stream

---

## 12. Schemas

### Model Schema (`schemas/model-schema.ts`)

- Branded types: `ProviderId`, `ModelId`, `SessionId`, `ProviderQualifiedModel`
- `ReasoningEffort`: `"low" | "medium" | "high" | "xhigh"`
- `ReasoningSummary`: `"auto" | "none" | "concise" | "detailed"`
- `TextVerbosity`: `"low" | "medium" | "high"`
- `OPENCODE_MODEL_SHORTCUTS`: 11 model shortcut names mapped to `provider/model-id`

### Settings Schema (`schemas/settings-schema.ts`)

- `OpencodeModelSettings`: Per-call settings (directory, sessionId, agent, tools, hooks, model, reasoning, etc.)
- `OpencodeProviderSettings`: Provider-level settings (hostname, port, baseUrl, autoStart, serverTimeout, etc.)
- Hook schemas: `HookHandler` (command/prompt/callback), `HookMatcher`, `Hooks`, `HookOptions`

### Event Schema (`schemas/event-schema.ts`)

- 12 part types: text, subtask, reasoning, file, tool, step-start, step-finish, snapshot, patch, agent, retry, compaction
- 7 event types: message.updated, message.part.updated, session.status, session.idle, session.compacted, session.created, session.error
- Tool states: pending, running, completed, error
- Session status: idle, retry, busy

---

## 13. Compatibility Layer (`compat.ts`)

Provides backward-compatible utility functions:

- Error factory functions: `createAPICallError()`, `createAuthenticationError()`, etc.
- Error detection: `isAuthenticationError()`, `isTimeoutError()`, `isBusyError()`
- `OpencodeBusyError` class (non-Effect error for external consumers)
- `validateModelId()`, `validateSettings()`, `validateProviderSettings()`
- `isValidSessionId()` - regex validation
- `OPENCODE_BUILTIN_TOOLS` - 16 built-in tool names
- `clearOpencodeConfigCache()` - no-op (config resolved through services)

---

## 14. Rust Port Considerations

### Architecture Mapping

| TypeScript Pattern              | Rust Equivalent                             |
| ------------------------------- | ------------------------------------------- |
| `Context.Tag` service           | Trait + `Arc<dyn Trait>`                    |
| `Layer.effect` / `Layer.scoped` | Builder pattern / DI container              |
| `ManagedRuntime`                | Struct with `Arc<...>` + `Drop`             |
| `Effect.gen`                    | `async fn` with `?` operator                |
| `Effect.fn("name")`             | Instrumented with `tracing::instrument`     |
| `Ref<T>`                        | `Arc<Mutex<T>>` or `Arc<RwLock<T>>`         |
| `HashMap` (immutable)           | Standard `HashMap` under `RwLock`           |
| `Deferred<A, E>`                | `tokio::sync::oneshot::Sender/Receiver`     |
| `SubscriptionRef<T>`            | `tokio::sync::watch::Sender<T>`             |
| `Queue.bounded`                 | `tokio::sync::mpsc::channel`                |
| `Stream.Stream<A, E>`           | `futures::Stream<Item = Result<A, E>>`      |
| `Schedule`                      | Manual retry loop with `tokio::time::sleep` |
| `Data.TaggedEnum`               | Rust `enum`                                 |
| `Data.TaggedError`              | `#[derive(thiserror::Error)]` enum variant  |
| `Option.Option<T>`              | `Option<T>` (native Rust)                   |
| `Either<L, R>`                  | `Result<R, L>`                              |
| `Match.exhaustive`              | Rust `match` (exhaustive by default)        |
| `Schema.Struct/Union`           | `serde::Deserialize` + validation           |

### Key Challenges for Rust Port

1. **SSE Streaming**: Need `reqwest` + `eventsource-client` or `reqwest-eventsource` crate. The stateful event conversion needs a scan/fold pattern over the stream.

2. **Process Management**: `tokio::process::Command` with stdout parsing. The spawn adapter pattern translates well.

3. **Config System**: Multi-source config loading with JSONC support. Need `serde_json` + JSONC preprocessing. Variable substitution is string manipulation.

4. **Hooks - Callback Type**: JS callback hooks cannot be ported. Shell hooks (command execution) and prompt hooks (template rendering) port directly. The hook system should support only command and prompt types in Rust.

5. **AI SDK Protocol**: The `LanguageModelV3StreamPart` type system is extensive (text-start, text-delta, text-end, tool-call, tool-result, reasoning-\*, source, finish, error). This needs a comprehensive Rust enum.

6. **MCP Server**: The tools bridge uses `@compozy/provider-core`'s MCP server. In Rust, use the `mcp-sdk` crate or equivalent.

7. **Error Mapping**: The 18-error tagged union maps cleanly to a Rust enum. The bidirectional mapping (OpencodeError <-> APICallError) needs a `From`/`Into` implementation.

---

## 15. Relevant Files

| File                                                                                                          | Purpose                                            |
| ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/index.ts`                                  | Package entry point                                |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/types.ts`                                  | Public type exports                                |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/compat.ts`                                 | Backward compatibility layer                       |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/tools-bridge.ts`                           | Imperative tools bridge API                        |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/opencode-client-manager.ts`                | High-level client manager                          |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/adapters/ai-sdk-adapter.ts`                | Prompt/response translation                        |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/adapters/error-adapter.ts`                 | Error mapping (OpencodeError -> APICallError)      |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/errors/`                                   | All error type definitions                         |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/model/opencode-language-model.ts`          | LanguageModelV3 implementation                     |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/model/opencode-provider.ts`                | Provider factory + layer composition               |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/schemas/config-schema.ts`                  | Full OpenCode config schema                        |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/schemas/event-schema.ts`                   | SSE event types + part types                       |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/schemas/model-schema.ts`                   | Model IDs, branded types, shortcuts                |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/schemas/settings-schema.ts`                | Model + provider settings schemas                  |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/client/server-process-service.ts` | Server process lifecycle                           |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/client/client-pool-service.ts`    | Client connection pool                             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/client/health-monitor-service.ts` | Health check polling                               |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/client/restart-policy-service.ts` | Circuit breaker + backoff                          |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/client/client-manager-service.ts` | Client management facade                           |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/config/config-service.ts`         | Multi-source config resolver                       |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/config/config-mapper.ts`          | Config -> settings mapping                         |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/config/config-source.ts`          | Config source utilities (JSONC, validation, merge) |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/hooks/hook-service.ts`            | Hook lifecycle service                             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/hooks/hook-runners.ts`            | Hook execution (callback/shell/prompt)             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/streaming-service.ts`   | Main streaming pipeline + service                  |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/event-converter.ts`     | Event -> StreamPart stateful converter             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/doom-loop-detector.ts`  | Repeated tool call detector                        |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/event-source.ts`        | SSE event source adapters                          |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/finish-detector.ts`     | Stream termination on "finish"                     |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/hook-executor.ts`       | Pre/Post tool use hooks in stream                  |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/session-filter.ts`      | Session-based event filtering                      |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/stream-state.ts`        | Stream state machine                               |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/streaming/subagent-manager.ts`    | Child session tracking                             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/generate-service.ts`              | Non-streaming generate                             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/request-service.ts`               | Request preparation                                |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/session-manager-service.ts`       | Per-session mutex                                  |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/validation-service.ts`            | Schema validation service                          |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/prompt-builder-service.ts`        | Prompt translation service                         |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/tools/tool-executor.ts`           | Effect-wrapped tool executor                       |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/opencode/src/services/tools/tools-bridge-service.ts`    | Tools bridge Effect service                        |

---

## 16. Metrics Summary

| Metric                         | Count                  |
| ------------------------------ | ---------------------- |
| Source files (non-test)        | ~40                    |
| Test files                     | ~35                    |
| Effect services (Context.Tag)  | 16                     |
| Effect layers                  | 16                     |
| Error types                    | 18 + 3 (tool executor) |
| Schema types                   | ~25                    |
| Tagged enums (Data.TaggedEnum) | 8                      |
| SSE event types                | 7 + generic            |
| Part types                     | 12                     |
| Hook events                    | 6                      |
| Config sources                 | 6                      |
| Model shortcuts                | 11                     |
| Built-in tools                 | 16                     |
