# Deep Analysis: `providers/codex` Package

## 1. Overview

The `providers/codex` package implements an AI SDK Provider V3 (`LanguageModelV3`) that wraps the OpenAI Codex CLI as a subprocess. It communicates via newline-delimited JSON-RPC over stdin/stdout, translating between the AI SDK streaming protocol and the Codex CLI's notification-based event model.

### Architecture at a Glance

```
AI SDK Consumer
    |
    v
CodexCliProvider (compat.ts) / codexProvider (CodexProvider.ts)
    |
    v
CodexLanguageModel (doStream / doGenerate)
    |
    v
CodexAppServer (orchestrator)
    |
    +---> CodexProcessManager (spawn/lifecycle)
    +---> CodexRpcTransport (JSON-RPC over stdio)
    +---> CodexThreadManager (thread/turn lifecycle)
    +---> CodexNotificationRouter (event routing)
    +---> CodexScheduler (concurrency control)
    +---> CodexModelService (model listing + caching)
    +---> CodexRuntimeConfig (live reconfiguration)
    |
    v
CodexStreamPipeline
    |
    +---> CodexEventDispatcher (event -> stream parts)
    +---> CodexTextAccumulator (text delta reconciliation)
    +---> CodexToolTracker (tool call state machine)
    |
    v
LanguageModelV3StreamPart (output)
```

### Key Characteristics

- **Effect-TS throughout**: All services use `Context.Tag`, `Layer`, `Effect.gen`, `Ref`, `Deferred`, `Queue`, `Stream`, `SynchronizedRef`, `Semaphore`, `ManagedRuntime`
- **Two API surfaces**: Effect-native (`codexProvider`) and Promise-based compat (`CodexCliProvider`)
- **Process-per-server model**: Each `CodexAppServer` instance spawns one Codex CLI process
- **Registry pattern**: Both Effect-based (`CodexRegistry`) and compat (`CodexAppServerRegistryImpl`) registries pool/share server instances with ref-counting and idle shutdown
- **Tool bridge**: An MCP-based HTTP server exposes AI SDK tools to the Codex CLI subprocess

### Public API Surface (`src/index.ts`)

Exports from: `config/`, `errors/`, `streaming/`, `server/`, `model/`, `util/`, `compat`, `hooks`, `bridge/`

---

## 2. Bridge System

### Files

| File                             | Lines | Purpose                                 |
| -------------------------------- | ----- | --------------------------------------- |
| `src/bridge/CodexToolsBridge.ts` | ~80   | Creates MCP server from AI SDK tools    |
| `src/bridge/CodexBridge.ts`      | ~30   | HTTP server wrapper for the tool bridge |

### CodexToolsBridge (`CodexToolsBridge.ts`)

Creates an MCP (Model Context Protocol) server from AI SDK `LanguageModelV3FunctionTool[]`. Uses `@compozy/provider-core` utilities (`createMcpServerFromTools`, `mcpToolFromAiSdkTool`).

**Key export:**

```typescript
CodexToolsBridge(tools: LanguageModelV3FunctionTool[]): ToolsBridgeInstance
```

Returns a `ToolsBridgeInstance` with an MCP server that can be mounted on an HTTP transport.

### CodexBridge (`CodexBridge.ts`)

Simple wrapper that starts an HTTP server hosting the MCP tool bridge.

**Key export:**

```typescript
startCodexToolBridge(tools): Promise<CodexBridgeServer>
// CodexBridgeServer = { name: string; url: string; close: () => Promise<void> }
```

### Rust Port Notes (Bridge)

- The MCP server creation depends on `@compozy/provider-core` and `@modelcontextprotocol/sdk`
- In Rust, this would require either an MCP Rust SDK or a custom JSON-RPC HTTP server
- The tool schema mapping (AI SDK tool -> MCP tool) is straightforward JSON schema transformation
- HTTP server could use `axum` or `hyper`

---

## 3. Configuration

### Files

| File                                 | Lines | Purpose                                  |
| ------------------------------------ | ----- | ---------------------------------------- |
| `src/config/schemas.ts`              | ~567  | Effect Schema definitions for all config |
| `src/config/CodexConfig.ts`          | ~80   | Composite config service                 |
| `src/config/CodexProcessConfig.ts`   | ~55   | Process-related config                   |
| `src/config/CodexStreamingConfig.ts` | ~50   | Streaming/reasoning config               |
| `src/config/CodexSchedulerConfig.ts` | ~45   | Concurrency/timeout config               |
| `src/config/CodexMcpConfig.ts`       | ~40   | MCP server config                        |

### Schema Architecture (`schemas.ts`)

All schemas use `Effect/Schema` with a custom `strictObjectSchema()` helper that rejects unknown keys.

**Top-level settings schema**: `CodexCliSettingsSchema` (~40+ fields)

Key types defined:

- `ApprovalMode`: `"suggest" | "auto-edit" | "full-auto"`
- `SandboxMode`: `"off" | "light" | "full"`
- `ReasoningEffort`: `"low" | "medium" | "high"`
- `ShellEnvironmentPolicy`: `"inherit" | "os-default" | "none"`
- `McpServerConfig`: discriminated union of `{ transport: "stdio", ... }` and `{ transport: "http", ... }`
- `CodexFeatureFlags`: extensible record of boolean flags

**Per-request options**: `CodexCliProviderOptionsSchema` -- settings overrides that can be applied per-request via `providerOptions.codex-cli`

**Internal config schemas** (used by the 4 config services):

- `CodexProcessConfigInputSchema`: codexPath, cwd, env, allowNpx, sanitizeEnvironment, startupTimeoutMs
- `CodexStreamingConfigInputSchema`: reasoningEffort, reasoningSummary, modelVerbosity, compactionTokenLimit, modelContextWindow, compactPrompt
- `CodexSchedulerConfigInputSchema`: maxInFlightRequests, maxQueuedRequests, requestTimeoutMs, modelCacheTtlMs, idleShutdownMs
- `CodexMcpConfigInputSchema`: mcpServers, rmcpClient

### Config Services

All four config services follow the same pattern: `Context.Tag` + static `make`/`makeEffect`/`layer` methods.

**CodexConfig** (`CodexConfig.ts`) -- Composite service:

```typescript
class CodexConfig extends Context.Tag("@compozy/codex/Config")<CodexConfig, {
  readonly process: CodexProcessConfig.Type;
  readonly streaming: CodexStreamingConfig.Type;
  readonly scheduler: CodexSchedulerConfig.Type;
  readonly mcp: CodexMcpConfig.Type;
}>
```

Static methods:

- `make(input)` -- Synchronous construction
- `makeEffect(input)` -- Effect-based construction with Schema validation
- `layer(input)` -- Layer from raw input
- `layers(input)` -- Individual layers for each sub-config (useful for partial overrides)

**Defaults:**
| Config | Field | Default |
|--------|-------|---------|
| Process | allowNpx | `true` |
| Process | sanitizeEnvironment | `true` |
| Process | startupTimeoutMs | `60000` |
| Scheduler | maxInFlightRequests | `8` |
| Scheduler | maxQueuedRequests | `64` |
| Scheduler | requestTimeoutMs | `300000` |
| Scheduler | modelCacheTtlMs | `300000` |
| Scheduler | idleShutdownMs | `60000` |

### Rust Port Notes (Config)

- Effect Schema -> `serde` with custom deserialization + validation
- `Option.Option<T>` -> `Option<T>` natively
- Config services -> structs passed by reference or via dependency injection
- The strict-object-schema rejection of unknown keys maps to `#[serde(deny_unknown_fields)]`
- Schema validation errors map to custom error enums

---

## 4. Error Hierarchy

### Files

| File                           | Lines | Purpose                                                         |
| ------------------------------ | ----- | --------------------------------------------------------------- |
| `src/errors/auth.ts`           | ~15   | `CodexAuthError`                                                |
| `src/errors/config.ts`         | ~30   | `CodexConfigError`, `CodexValidationError`                      |
| `src/errors/rpc.ts`            | ~25   | `CodexRpcError`, `CodexRpcTimeoutError`                         |
| `src/errors/scheduler.ts`      | ~20   | `CodexOverflowError`, `CodexTimeoutError`                       |
| `src/errors/spawn.ts`          | ~15   | `CodexSpawnError`                                               |
| `src/errors/stream.ts`         | ~25   | `CodexStreamError`, `CodexTurnFailedError`, `CodexAbortedError` |
| `src/errors/disposed.ts`       | ~15   | `CodexDisposedError`                                            |
| `src/errors/classification.ts` | ~120  | Error classification + exhaustive matching                      |

### Error Types

All errors use `Data.TaggedError` from Effect-TS, providing:

- Discriminated union via `_tag`
- Structural equality
- Stack trace capture

| Error                  | Tag                      | Key Fields                              | Source                    |
| ---------------------- | ------------------------ | --------------------------------------- | ------------------------- |
| `CodexSpawnError`      | `"CodexSpawnError"`      | message, cause                          | Process spawn failures    |
| `CodexRpcError`        | `"CodexRpcError"`        | message, code: `Option<number>`, method | JSON-RPC response errors  |
| `CodexRpcTimeoutError` | `"CodexRpcTimeoutError"` | message, method, timeoutMs              | RPC call timeout          |
| `CodexOverflowError`   | `"CodexOverflowError"`   | message, queueSize                      | Scheduler queue full      |
| `CodexTimeoutError`    | `"CodexTimeoutError"`    | message, label, timeoutMs               | Generic operation timeout |
| `CodexStreamError`     | `"CodexStreamError"`     | message, classification, isRetryable    | Stream processing errors  |
| `CodexTurnFailedError` | `"CodexTurnFailedError"` | message                                 | Turn completion failure   |
| `CodexAbortedError`    | `"CodexAbortedError"`    | reason                                  | User abort signal         |
| `CodexAuthError`       | `"CodexAuthError"`       | message                                 | Authentication failures   |
| `CodexConfigError`     | `"CodexConfigError"`     | message, field: `Option<string>`        | Configuration validation  |
| `CodexValidationError` | `"CodexValidationError"` | message, field: `Option<string>`        | Input validation          |
| `CodexDisposedError`   | `"CodexDisposedError"`   | message                                 | Use-after-dispose         |

### Error Classification (`classification.ts`)

**`classifyCodexError(error)`** -- Exhaustive `Match` on all error `_tag` values. Returns `{ classification, isRetryable }`.

**`classifyCodexMessage(message: string)`** -- Regex-based classification of error message text:

| Classification            | Pattern                                                   | Retryable |
| ------------------------- | --------------------------------------------------------- | --------- |
| `context_window_exceeded` | /context.*window.*exceeded/i, /maximum.*context.*length/i | false     |
| `quota_exceeded`          | /quota.*exceeded/i, /insufficient.*quota/i                | false     |
| `rate_limited`            | /rate.*limit/i, /too.*many.\*requests/i                   | true      |
| `invalid_api_key`         | /invalid.*api.*key/i, /authentication.\*failed/i          | false     |
| `model_not_found`         | /model.*not.*found/i                                      | false     |
| `content_filter`          | /content.*filter/i, /content.*policy/i                    | false     |
| `server_error`            | /server.*error/i, /internal.*error/i                      | true      |
| `timeout`                 | /timeout/i                                                | true      |
| `unknown`                 | (fallback)                                                | false     |

### Rust Port Notes (Errors)

- `Data.TaggedError` -> Rust `enum` with `#[derive(thiserror::Error)]`
- Exhaustive `Match` -> Rust `match` (already exhaustive by default)
- `Option<number>` -> `Option<i32>`
- Classification regex -> `regex` crate or string matching
- The error hierarchy maps cleanly to Rust's enum-based error handling

---

## 5. Model Layer

### Files

| File                               | Lines | Purpose                                              |
| ---------------------------------- | ----- | ---------------------------------------------------- |
| `src/model/CodexLanguageModel.ts`  | ~687  | Core `LanguageModelV3` implementation                |
| `src/model/CodexProvider.ts`       | ~120  | Provider factory + layer composition                 |
| `src/model/request-preparation.ts` | ~300  | Request preparation, tool mapping, option extraction |

### CodexLanguageModel (`CodexLanguageModel.ts`)

Implements the `LanguageModelV3` interface from `@ai-sdk/provider`.

**Key methods:**

**`doStream(options)`** -- Main streaming method:

1. Extracts provider options via `extractCodexProviderOptions`
2. Prepares request via `prepareCodexRequest` (validates, maps messages/tools, builds config overrides)
3. Gets `CodexAppServer` and `CodexStreamPipeline` from Effect context
4. Registers model ID, reconfigures runtime if needed
5. Ensures server is ready
6. Starts or resumes thread, starts turn
7. Transforms notification stream through pipeline
8. Returns `{ stream: ReadableStream, request: {}, warnings: [] }`

**`doGenerate(options)`** -- Non-streaming:

- Delegates to `doStream`, collects all stream parts
- Assembles `LanguageModelV3GenerateResult` from collected parts (text, tool calls, usage, finish reason)

**Error mapping:**

```
isCodexError(e) -> classifyCodexError(e) -> APICallError({
  message, statusCode: classification-based, isRetryable
})
```

**Runtime type:**

```typescript
type CodexLanguageModelRuntime = CodexAppServer | CodexStreamPipeline;
```

### CodexProvider (`CodexProvider.ts`)

**Layer composition:**

```typescript
CodexStreamPipelineLive = CodexStreamPipeline.layer.pipe(
  Layer.provide(CodexEventDispatcher.layer),
  Layer.provide(CodexTextAccumulator.layer),
  Layer.provide(CodexToolTracker.layer)
);
```

**`makeCodexModelStackLayer(appServerLayer)`** -- Merges app server layer with stream pipeline layer.

**`createCodexProvider(settings?)`** -- Factory function:

1. Creates config from settings
2. Builds server layer + model stack
3. Creates `ManagedRuntime`
4. Returns `ProviderV3` with `languageModel(modelId)` method

**`codexProvider`** -- Lazy singleton (creates provider on first access).

### Request Preparation (`request-preparation.ts`)

**`prepareCodexRequest(options, providerOptions)`:**

1. Validates provider options with Schema
2. Merges settings (base settings + per-request overrides via `configMerge`)
3. Maps messages to prompt text via `mapMessages`
4. Maps tools to Codex definitions via `mapToolsToCodexDefinitions`
5. Builds config overrides (model, reasoning, MCP servers, tool bridge)
6. Returns `{ prompt, tools, configOverrides, mergedSettings }`

**`mapToolsToCodexDefinitions(tools)`:**

- Maps `LanguageModelV3FunctionTool[]` to `CodexMappedTool[]`
- Each tool: `{ name, description, inputSchema: sanitizeJsonSchema(parameters) }`

**`extractCodexProviderOptions(providerOptions)`:**

- Extracts from `providerOptions["codex-cli"]`
- Validates with `Schema.decodeUnknown(CodexCliProviderOptionsSchema)`

### Rust Port Notes (Model)

- The `LanguageModelV3` interface is specific to the AI SDK -- Rust would need its own trait
- `doStream` returning `ReadableStream` -> Rust `Stream<Item = Result<StreamPart, Error>>`
- `Effect.gen` workflow -> Rust `async fn` with `?` operator
- `ManagedRuntime` -> `Arc<Runtime>` with shared state
- Provider factory pattern -> builder pattern or `impl Provider`
- Tool mapping is pure data transformation, straightforward in Rust

---

## 6. Server Architecture

### Files

| File                                    | Lines | Purpose                          |
| --------------------------------------- | ----- | -------------------------------- |
| `src/server/CodexAppServer.ts`          | ~345  | Central orchestrator             |
| `src/server/CodexProcessManager.ts`     | ~287  | Process spawn/lifecycle          |
| `src/server/CodexRpcTransport.ts`       | ~806  | JSON-RPC over stdio              |
| `src/server/CodexScheduler.ts`          | ~100  | Concurrency control              |
| `src/server/CodexNotificationRouter.ts` | ~150  | Event routing by thread          |
| `src/server/CodexThreadManager.ts`      | ~200  | Thread/turn lifecycle            |
| `src/server/CodexApprovalHandler.ts`    | ~60   | Auto-approval of server requests |
| `src/server/CodexModelService.ts`       | ~120  | Model listing + caching          |
| `src/server/CodexRuntimeConfig.ts`      | ~150  | Live reconfiguration             |
| `src/server/CodexRegistry.ts`           | ~200  | Instance pooling                 |
| `src/server/CodexServerLayer.ts`        | ~120  | Layer composition                |
| `src/server/types.ts`                   | ~92   | Shared types                     |

### CodexAppServer (`CodexAppServer.ts`)

Central orchestrator, `Context.Tag` service:

```typescript
class CodexAppServer extends Context.Tag("@compozy/codex/AppServer")<CodexAppServer, {
  readonly ensureReady: Effect.Effect<void, CodexSpawnError | CodexTimeoutError>;
  readonly startThread: (params?) => Effect.Effect<{ threadId: string }>;
  readonly resumeThread: (threadId, params?) => Effect.Effect<void>;
  readonly startTurn: (threadId, params) => Effect.Effect<Stream<Notification, RpcError>>;
  readonly compactThread: (threadId, params?) => Effect.Effect<void>;
  readonly listModels: (params?) => Effect.Effect<ModelListResult>;
  readonly listAllModels: () => Effect.Effect<ModelDescriptor[]>;
  readonly registerModelId: (modelId) => Effect.Effect<void>;
  readonly reconfigure: (settings) => Effect.Effect<ReconfigureResult>;
}>
```

**Initialization flow:**

1. `createEnsureReady` -- Uses `Deferred` for one-shot initialization
2. Spawns two background workers (forked fibers):
   - `startNotificationsWorker` -- Reads notifications from RPC transport, routes via `NotificationRouter`
   - `startServerRequestsWorker` -- Reads server requests, delegates to `ApprovalHandler`
3. Waits for `initialize` RPC handshake to complete

**Queue-based stdin writing:**

- `makeQueuedProcessWriter` -- Creates `Queue<Uint8Array>` with a consumer fiber that writes to process stdin
- Ensures ordered, non-concurrent writes

**Layer bridge:**

- `CodexRpcTransportFromProcessManager` -- Adapts `CodexProcessManager` (stdin/stdout/stderr streams) to `CodexRpcTransport` dependencies

### CodexProcessManager (`CodexProcessManager.ts`)

Manages Codex CLI subprocess lifecycle.

**Binary resolution order:**

1. Explicit `codexPath` from config
2. Installed `@openai/codex` package (resolved via `import.meta.resolve`)
3. `npx @openai/codex` fallback (if `allowNpx` is true)

**Spawn configuration:**

- Uses `@effect/platform` `Command` API
- Pipes: stdin (writable), stdout (readable), stderr (readable)
- Working directory from config
- Environment: base env + config overrides, with optional sanitization

**Environment sanitization:**

- Removes: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_*`, `NODE_OPTIONS`, `ELECTRON_RUN_AS_NODE`

**Shutdown sequence:**

1. `SIGTERM` sent to process
2. 500ms grace period
3. `SIGKILL` if still running
4. Layer uses `acquireRelease` for automatic cleanup

### CodexRpcTransport (`CodexRpcTransport.ts`)

JSON-RPC 2.0 transport over newline-delimited stdio.

**Message types:**

```typescript
type CodexJsonRpcNotification = { method: string; params?: Record<string, unknown> };
type CodexJsonRpcServerRequest = {
  id: string | number;
  method: string;
  params?: Record<string, unknown>;
};
```

**Message routing (from stdout):**

- Has `id` + matches pending request -> resolve `Deferred` for that request
- Has `method` + no `id` -> notification -> push to notification queue
- Has `method` + has `id` (server request) -> push to server request queue
- Parse error -> log and skip

**Key operations:**

- `sendRequest(method, params, timeout?)` -- Sends JSON-RPC request, creates `Deferred`, waits for response
- `initialize()` -- Sends `initialize` request + `initialized` notification (handshake)
- `sendNotification(method, params)` -- Fire-and-forget notification

**State management:**

- `HashMap<string, Deferred>` for pending request correlation
- `Queue` for notifications
- `Queue` for server requests
- Trace logging via `COMPOZY_CODEX_TRACE_RPC` env var

### CodexScheduler (`CodexScheduler.ts`)

Semaphore-based concurrency control.

```typescript
class CodexScheduler extends Context.Tag("@compozy/codex/Scheduler")<CodexScheduler, {
  readonly schedule: <A, E, R>(task: Effect.Effect<A, E, R>) => Effect.Effect<A, E | CodexOverflowError | CodexTimeoutError, R>;
}>
```

**Behavior:**

1. Try immediate `Semaphore.withPermit` (no blocking)
2. If semaphore full, check queue size against `maxQueuedRequests` -> fail with `CodexOverflowError` if exceeded
3. Queue and wait for semaphore permit
4. Apply per-task timeout from config
5. Timeout triggers `Effect.interrupt` with `CodexTimeoutError`

### CodexNotificationRouter (`CodexNotificationRouter.ts`)

Routes notifications to the correct thread's queue.

```typescript
class CodexNotificationRouter extends Context.Tag("@compozy/codex/NotificationRouter")<CodexNotificationRouter, {
  readonly register: (threadId, scopeId?, queue?) => Effect.Effect<Queue>;
  readonly unregister: (threadId) => Effect.Effect<void>;
  readonly route: (notification) => Effect.Effect<void>;
  readonly broadcastError: (error) => Effect.Effect<void>;
}>
```

**Routing logic:**

1. Extract `threadId` from notification params -> route to registered queue
2. Fall back to `scopeId` -> route to thread registered for that scope
3. If method starts with `account/` -> fan out to all registered threads
4. Unroutable notifications are logged and dropped

### CodexThreadManager (`CodexThreadManager.ts`)

Thread and turn lifecycle management.

**Operations:**

- `startThread(params?)` -- `thread/start` RPC -> returns `{ threadId }`
- `resumeThread(threadId, params?)` -- `thread/resume` RPC
- `startTurn(threadId, params)` -- Registers thread with NotificationRouter, sends `turn/start` RPC, returns notification stream
- `compactThread(threadId, params?)` -- `thread/compact/start` RPC

**Turn completion detection:**

- Stream terminates on: `turn/completed`, `turn/failed`, or error notifications
- Uses `Stream.takeUntil` with predicate checking method names

### CodexApprovalHandler (`CodexApprovalHandler.ts`)

Handles server-initiated approval requests.

**Supported methods:**

- `item/commandExecution/requestApproval`
- `item/fileChange/requestApproval`
- `item/command_execution/requestApproval` (snake_case variant)
- `item/file_change/requestApproval` (snake_case variant)

**Behavior:**

- If `autoApproveServerRequests` is enabled -> respond with `{ outcome: "approved", decision: "accept", acceptSettings: { forSession: true } }`
- Otherwise -> respond with rejection

### CodexModelService (`CodexModelService.ts`)

Model listing with caching.

```typescript
class CodexModelService extends Context.Tag("@compozy/codex/ModelService")<CodexModelService, {
  readonly listModels: (params?) => Effect.Effect<ModelListResult>;
  readonly listAllModels: () => Effect.Effect<ModelDescriptor[]>;
}>
```

**Features:**

- Paginated `model/list` RPC with cursor support
- TTL-based caching via `Effect.cachedWithTTL` (configured by `modelCacheTtlMs`)
- `listAllModels` iterates all pages
- Fallback model descriptors for configured model IDs (if RPC fails or returns empty)

### CodexRuntimeConfig (`CodexRuntimeConfig.ts`)

Live reconfiguration support.

```typescript
class CodexRuntimeConfig extends Context.Tag("@compozy/codex/RuntimeConfig")<CodexRuntimeConfig, {
  readonly get: Effect.Effect<CodexRuntimeOptions>;
  readonly reconfigure: (settings) => Effect.Effect<ReconfigureResult>;
  readonly registerModelId: (modelId) => Effect.Effect<void>;
}>
```

**Key behavior:**

- Uses `SynchronizedRef` for atomic config updates
- **Critical field detection**: Compares old vs new config for fields that require process restart (codexPath, cwd, env, sanitizeEnvironment)
- `onCriticalChange` hook -> triggers process respawn
- `registerModelId` tracks which model IDs are in use (for config override purposes)

### CodexRegistry (`CodexRegistry.ts`)

Instance pooling for `CodexAppServer`.

```typescript
class CodexRegistry extends Context.Tag("@compozy/codex/Registry")<CodexRegistry, {
  readonly acquire: (key) => Effect.Effect<CodexAppServer>;
  readonly release: (key) => Effect.Effect<void>;
}>
```

**Features:**

- Key-based deduplication (same key = same server instance)
- Reference counting (multiple acquires share one instance)
- Idle shutdown timer (configurable via `idleShutdownMs`)
- Mutex-protected slot management for thread safety

### CodexServerLayer (`CodexServerLayer.ts`)

Layer composition helpers.

**Main layers:**

```typescript
CodexAppServerLive = CodexAppServer.layer.pipe(
  Layer.provide(CodexRpcTransportFromProcessManager),
  Layer.provide(CodexProcessManager.layer),
  Layer.provide(CodexThreadManager.layer),
  Layer.provide(CodexNotificationRouter.layer),
  Layer.provide(CodexScheduler.layer),
  Layer.provide(CodexModelService.layer),
  Layer.provide(CodexRuntimeConfig.layer),
  Layer.provide(CodexApprovalHandler.layer)
);

CodexServerLive = Layer.merge(CodexAppServerLive, CodexRegistry.layer);
```

**Helper functions:**

- `makeCodexServerLayer(config)` -- Full server from config
- `makeCodexAppServerLayer(config)` -- App server only (no registry)
- `makeCodexRpcTransportLayer(config)` -- Transport only
- `makeCodexThreadManagerLayer(config)` -- Thread manager only
- `makeCodexModelServiceLayer(config)` -- Model service only

### Shared Types (`types.ts`)

```typescript
TurnStartParams = { scopeId?, prompt?, input?, model?, configOverrides?, outputSchema? }
TurnStartResult = { turn?: { id?, status? } }
CompactParams = { scopeId?, [key: string]: unknown }
ThreadStartResult = { thread?: { id? }, threadId?, thread_id?, id? }
ThreadOpenParams = { model?, configOverrides? }
ModelListParams = { cursor?, limit?, includeHidden? }
ModelDescriptor = { id, name?, displayName?, model?, created? }
ModelListResult = { models, nextCursor? }
ReconfigureResult = { changed, respawned }
CodexRuntimeOptions = { codexPath?, allowNpx, sharedAppServerKey?, cwd?, env, sanitizeEnvironment, autoApproveServerRequests, requestTimeoutMs, startupTimeoutMs, maxInFlightRequests, maxQueuedRequests, modelCacheTtlMs, idleShutdownMs, compactionTokenLimit?, modelContextWindow?, compactPrompt? }
ApprovalMethod = "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" | ... (snake_case variants)
ApprovalResponse = { outcome: "approved", decision: "accept", acceptSettings: { forSession: boolean } }
```

### Rust Port Notes (Server)

- **Process management**: `std::process::Command` or `tokio::process::Command`
- **JSON-RPC**: `serde_json` for parsing, custom framing for newline-delimited messages
- **Concurrency**: `tokio::sync::Semaphore` for scheduler, `tokio::sync::Mutex` for registry
- **Queues**: `tokio::sync::mpsc` channels
- **Deferred**: `tokio::sync::oneshot` channels
- **Ref/SynchronizedRef**: `Arc<RwLock<T>>` or `Arc<Mutex<T>>`
- **Stream**: `tokio_stream` or `futures::Stream`
- **Layer composition**: Struct-based dependency injection or `tower::Service` patterns
- **HashMap for pending requests**: `dashmap::DashMap` or `HashMap` behind `Mutex`
- The `acquireRelease` pattern maps to Rust's `Drop` trait or `tokio::select!` with cleanup

---

## 7. Streaming Pipeline

### Files

| File                                    | Lines | Purpose                        |
| --------------------------------------- | ----- | ------------------------------ |
| `src/streaming/CodexEventDispatcher.ts` | ~871  | Core event dispatch            |
| `src/streaming/CodexStreamPipeline.ts`  | ~279  | Stream transformation          |
| `src/streaming/CodexStreamState.ts`     | ~41   | Immutable stream state         |
| `src/streaming/CodexTextAccumulator.ts` | ~534  | Text delta reconciliation      |
| `src/streaming/CodexToolTracker.ts`     | ~250  | Tool call state machine        |
| `src/streaming/event-parser.ts`         | ~589  | Event parsing utilities        |
| `src/streaming/event-normalizer.ts`     | ~50   | Event type normalization       |
| `src/streaming/json-utils.ts`           | ~80   | JSON utilities                 |
| `src/streaming/tool-payloads.ts`        | ~358  | Tool-specific payload builders |

### CodexEventDispatcher (`CodexEventDispatcher.ts`)

Maps `CodexJsonRpcNotification` -> `CodexNormalizedEvent` -> `LanguageModelV3StreamPart[]`.

**Service interface:**

```typescript
class CodexEventDispatcher extends Context.Tag("@compozy/codex/EventDispatcher")<CodexEventDispatcher, {
  readonly dispatch: (
    state: CodexStreamState,
    event: CodexNormalizedEvent
  ) => Effect.Effect<readonly [CodexStreamState, ReadonlyArray<LanguageModelV3StreamPart>]>;
}>
```

**Normalization** (`normalizeCodexNotificationEvent`):

- Extracts `type` from notification method (after last `.` or `/`)
- Normalizes to snake_case
- Preserves original method and params

**16 dispatch event types:**

| Event Type           | Handler                               | Output Stream Parts                    |
| -------------------- | ------------------------------------- | -------------------------------------- |
| `message_start`      | Dedup by fingerprint, start text part | `text-start`                           |
| `message_delta`      | Extract delta text                    | `text-delta`                           |
| `message_complete`   | Dedup, finalize text                  | `text-end`                             |
| `message_snapshot`   | Reconcile snapshot with accumulated   | `text-delta` (if diverged)             |
| `reasoning_start`    | Start reasoning part                  | `reasoning-start`                      |
| `reasoning_delta`    | Append reasoning delta                | `reasoning-delta`                      |
| `reasoning_complete` | Finalize reasoning                    | `reasoning-end`                        |
| `reasoning_snapshot` | Sync reasoning snapshot               | `reasoning-delta` (catchup)            |
| `tool_call_start`    | Register tool, emit input start       | `tool-input-start`, `tool-input-delta` |
| `tool_call_delta`    | Append tool input                     | `tool-input-delta`                     |
| `tool_call_complete` | Finalize tool input                   | `tool-input-end`, `tool-call`          |
| `tool_result`        | Emit tool result                      | `tool-result`                          |
| `usage`              | Update usage stats                    | (state update only)                    |
| `session_id`         | Track session ID                      | (state update only)                    |
| `turn_complete`      | Mark turn done                        | (state update only)                    |
| `turn_failed`        | Record failure message                | (state update + deferred error)        |

**Deduplication:**

- `lastMessageStartFingerprint` / `lastMessageCompleteFingerprint` -- prevents duplicate processing of same event
- Fingerprint computed from event content hash

### CodexStreamPipeline (`CodexStreamPipeline.ts`)

Top-level stream transformation.

```typescript
class CodexStreamPipeline extends Context.Tag("@compozy/codex/StreamPipeline")<CodexStreamPipeline, {
  readonly transform: (notifications, params) => Stream<LanguageModelV3StreamPart, CodexStreamError | CodexAbortedError>;
  readonly transformQueue: (queue, params) => Stream<...>;
}>
```

**Transform pipeline:**

```
notifications
  |> Stream.map(normalizeCodexNotificationEvent)
  |> Stream.tap(traceStream)  // if COMPOZY_CODEX_TRACE_STREAM=1
  |> Stream.mapAccumEffect(initialState, dispatcher.dispatch)
  |> Stream.flatMap(parts => Stream.fromIterable(parts))
  |> prepend(streamStartParts)  // stream-start + response-metadata
  |> concat(finalization)       // text finalize + tool errors + finish
  |> Stream.catchTag("CodexRpcError", mapRpcError)
  |> Stream.catchTag("CodexTurnFailedError", mapTurnFailure)
  |> withAbortSignal(signal)    // Stream.interruptWhen
```

**Finalization:**

1. Get latest state from `Ref`
2. Call `textAccumulator.finalize` -- emits remaining text-end/reasoning-end
3. Call `toolTracker.emitOpenToolErrors` -- error results for uncompleted tools
4. If `turnFailureMessage` exists -> emit final parts then fail with `CodexTurnFailedError`
5. Otherwise -> emit `finish` part with usage stats

**Start metadata:** Includes `sessionId` if available, wrapped as `providerMetadata.codex-cli`.

### CodexStreamState (`CodexStreamState.ts`)

Immutable state tracked through the stream pipeline:

```typescript
interface CodexStreamState {
  readonly closed: boolean;
  readonly lastUsage: Option<LanguageModelV3Usage>;
  readonly turnFailureMessage: Option<string>;
  readonly currentSessionId: Option<string>;
  readonly responseMetadataSent: boolean;
  readonly lastMessageStartFingerprint: Option<string>;
  readonly lastMessageCompleteFingerprint: Option<string>;
  readonly lastMessageStartContentFingerprint: Option<string>;
  readonly lastMessageCompleteContentFingerprint: Option<string>;
}
```

**Default usage:**

```typescript
{ inputTokens: { total: undefined, ... }, outputTokens: { total: undefined, ... } }
```

### CodexTextAccumulator (`CodexTextAccumulator.ts`)

Manages text and reasoning part lifecycle with delta reconciliation.

**State:**

```typescript
interface AccumulatorState {
  readonly activeTextPartId: Option<string>;
  readonly streamedAssistantText: string;
  readonly accumulatedText: string;
  readonly activeReasoningPartId: Option<string>;
  readonly streamedReasoningText: string;
  readonly activeMessagePhase: Option<MessagePhase>;
}
```

**Service interface:**

```typescript
class CodexTextAccumulator extends Context.Tag("@compozy/codex/TextAccumulator")<...> {
  getState, reset,
  messageStart, messageDelta, messageSnapshot, messageComplete,
  reasoningStart, reasoningDelta, reasoningSnapshot, reasoningComplete,
  finalize
}
```

**Key reconciliation logic (`reconcileAssistantSnapshot`):**

1. Empty snapshot -> reset accumulated text
2. Snapshot equals streamed text -> update accumulated only
3. No streamed text yet -> emit as delta
4. Snapshot extends streamed text -> emit continuation delta
5. No active text part -> emit as new delta
6. Divergence -> close current text part, open new one, emit full snapshot as delta

**Finalization (`finalizeAssistantText`):**

- If active text part and target extends streamed -> emit continuation + text-end
- If diverged -> close old, open+close new with full text
- If no active part but text exists -> open+delta+close ephemeral part

**Message phase tracking:**

- `MessagePhase` type (e.g., "commentary", "final_answer")
- Attached as `providerMetadata.codex-cli.messagePhase`
- Phase persists across deltas until explicitly changed

### CodexToolTracker (`CodexToolTracker.ts`)

Tracks tool call lifecycle using HashMap state.

```typescript
class CodexToolTracker extends Context.Tag("@compozy/codex/ToolTracker")<CodexToolTracker, {
  readonly reset: Effect.Effect<void>;
  readonly toolCallStart: (callId, toolName, initialArgs?) => Effect.Effect<StreamPart[]>;
  readonly toolCallDelta: (callId, argsDelta) => Effect.Effect<StreamPart[]>;
  readonly toolCallComplete: (callId, finalArgs) => Effect.Effect<StreamPart[]>;
  readonly toolResult: (callId, result) => Effect.Effect<StreamPart[]>;
  readonly toolResultFromItem: (item) => Effect.Effect<StreamPart[]>;
  readonly emitOpenToolErrors: Effect.Effect<StreamPart[]>;
}>
```

**State per tool call:**

- `toolCallId`, `toolName`, `accumulatedArgs` (string), `completed` (boolean)

**Stream part emission:**
| Operation | Emitted Parts |
|-----------|--------------|
| `toolCallStart` | `tool-input-start` + `tool-input-delta` (if initial args) |
| `toolCallDelta` | `tool-input-delta` |
| `toolCallComplete` | `tool-input-end` + `tool-call` (with parsed args) |
| `toolResult` | `tool-result` |
| `emitOpenToolErrors` | `tool-result` with error for each uncompleted tool |

### Event Parser (`event-parser.ts`)

Extraction functions for Codex notification payloads:

| Function                          | Returns                        | Source Path                                          |
| --------------------------------- | ------------------------------ | ---------------------------------------------------- |
| `getCallIdFromEvent(event)`       | `Option<string>`               | `params.call_id` or `params.item.call_id`            |
| `getThreadIdFromEvent(event)`     | `Option<string>`               | `params.threadId` or `params.thread_id`              |
| `getSessionIdFromEvent(event)`    | `Option<string>`               | `params.sessionId` or `params.session_id`            |
| `getItemType(event)`              | `Option<string>`               | `params.item.type`                                   |
| `getAssistantMessageText(event)`  | `Option<string>`               | `params.item.content[].text` (role=assistant)        |
| `getAssistantMessageDelta(event)` | `Option<string>`               | `params.delta.content[].text` or `params.delta.text` |
| `getReasoningText(event)`         | `Option<string>`               | `params.item.reasoning_content[].text`               |
| `getReasoningDelta(event)`        | `Option<string>`               | `params.delta.reasoning_content[].text`              |
| `getMessagePhase(event)`          | `Option<MessagePhase>`         | `params.item.phase` or `params.phase`                |
| `extractUsage(event)`             | `Option<LanguageModelV3Usage>` | `params.usage` with token mapping                    |

**`ExperimentalJsonEvent` type:**

```typescript
type ExperimentalJsonEvent = {
  type?: string;
  method?: string;
  params?: Record<string, unknown>;
};
```

**`createExperimentalJsonEventParser()`:**

- Line-based streaming parser for newline-delimited JSON
- Returns `{ feed(chunk): ExperimentalJsonEvent[], flush(): ExperimentalJsonEvent[] }`

### Event Normalizer (`event-normalizer.ts`)

Normalizes Codex notification method names:

- camelCase -> snake_case (e.g., `messageStart` -> `message_start`)
- Slash -> dot (e.g., `response/message.start` -> `response.message.start`)
- Extracts final segment as event type

### JSON Utilities (`json-utils.ts`)

**`toJsonValue(value, seen?)`:**

- Recursive conversion of arbitrary values to JSON-safe form
- Cycle detection via `WeakSet`
- Handles: primitives, arrays, objects, undefined -> null

**`buildCodexProviderMetadata(payload?)`:**

```typescript
// Wraps under "codex-cli" namespace
{ "codex-cli": { id: "codex-cli", ...payload } }
```

### Tool Payloads (`tool-payloads.ts`)

Maps Codex item types to tool names and builds payloads.

**Item type -> tool name mapping:**

| Item Type            | Tool Name            |
| -------------------- | -------------------- |
| `command_execution`  | `shell`              |
| `file_change`        | `apply_patch`        |
| `mcp_tool_call`      | (dynamic, from item) |
| `web_search`         | `web_search`         |
| `image_view`         | `image_view`         |
| `context_compaction` | `context_compaction` |
| `collab_tool_call`   | `collab_tool_call`   |
| `todo_list`          | `update_plan`        |

**Key functions:**

- `buildToolInputPayload(item)` -- Extracts input/arguments from item based on type
- `buildToolResultPayload(item)` -- Extracts output/result from item based on type
- `buildToolResultPart(toolCallId, toolName, result)` -- Builds `tool-result` stream part
- `buildToolInvocationParts(item)` -- Builds complete tool-call + tool-result for inline items

### Rust Port Notes (Streaming)

- **Stream accumulation**: The `Ref.modify` pattern maps to `Mutex<State>` with lock-modify-unlock
- **Text reconciliation**: Pure functions, direct translation to Rust
- **HashMap state**: `std::collections::HashMap` behind `Mutex` or use `dashmap`
- **Event dispatching**: Pattern matching on normalized event type string
- **Tool tracking**: State machine per tool call, straightforward in Rust
- **Fingerprint deduplication**: String hashing, trivial in Rust
- **Stream pipeline composition**: `futures::Stream` combinators or manual async state machine
- **The `LanguageModelV3StreamPart` types**: Need equivalent Rust enum with serde support

---

## 8. Utilities

### Files

| File                           | Lines | Purpose                            |
| ------------------------------ | ----- | ---------------------------------- |
| `src/util/args.ts`             | ~120  | CLI argument building              |
| `src/util/config-merge.ts`     | ~100  | Settings deep merge                |
| `src/util/message-mapper.ts`   | ~80   | AI SDK messages -> prompt text     |
| `src/util/runtime-layer.ts`    | ~20   | Layer error widening               |
| `src/util/settings-mapping.ts` | ~80   | Settings -> config/options mapping |
| `src/util/validation.ts`       | ~60   | Settings validation                |

### CLI Arguments (`args.ts`)

**`MANDATORY_CODEX_CONFIG_OVERRIDES`:**

```typescript
{
  approval_policy: "never",
  sandbox_mode: "danger-full-access",
  disable_response_storage: true,
  notify: { type: "jsonrpc" }
}
```

These overrides ensure:

- CLI never prompts for approval (managed by our approval handler)
- Full filesystem access (sandboxing managed externally)
- No response storage on Codex side
- JSON-RPC notification mode (vs terminal UI)

**`buildCodexConfigOverrides(settings, modelId?, toolBridgeUrl?)`:**
Assembles config overrides from settings, including:

- Model ID
- Reasoning effort/summary
- MCP server configs
- Tool bridge URL (if tools are bridged)
- Feature flags
- Shell environment policy

**`sanitizeJsonSchema(schema)`:**
Strips non-essential JSON Schema fields: `$schema`, `$ref`, `$defs`, `title`, `description`, `examples`, `default`, `$comment`.

### Config Merge (`config-merge.ts`)

**`mergeCodexSettings(base, overrides)`:**
Deep merges `CodexCliSettings` with `CodexCliProviderOptions`:

- Scalar fields: override wins
- `mcpServers`: merged by server name (override wins per-server)
- `featureFlags`: merged (override wins per-flag)
- `shellEnvironmentPolicy`: override wins
- `env`: merged (override wins per-key)

### Message Mapper (`message-mapper.ts`)

**`mapMessages(messages: LanguageModelV3Message[])`:**
Maps AI SDK message array to a single prompt string:

```
Human: <user message text>
[image: <base64 or url>]
Assistant: <assistant message text>
Tool Result (call_id): <result text>
```

Handles message types: `user`, `assistant`, `tool`, `system` (prepended as system instruction).

### Runtime Layer (`runtime-layer.ts`)

**`widenLayerError<R, E>(layer)`:**
Widens a `Layer<R, E, never>` error type to `Layer<R, E | Cause.UnknownException, never>`.
Needed for `ManagedRuntime.make()` which requires compatible error types.

### Settings Mapping (`settings-mapping.ts`)

**`mapSettingsToConfigInput(settings)`:**
Maps `CodexCliSettings` -> `CodexConfigInput` (for the 4 config services).

**`mapSettingsToRuntimeOptions(settings)`:**
Maps `CodexCliSettings` -> `CodexRuntimeOptions` (for `CodexRuntimeConfig`).

**Helper**: `conditionalProp(key, value)` -- Only includes property if value is defined.

### Validation (`validation.ts`)

**`validateCodexSettings(settings)`:**

- Schema validation via `Schema.decodeUnknown(CodexCliSettingsSchema)`
- Dollar-prefix key stripping (removes `$schema`, `$ref`, etc.)
- Model ID format validation

### Rust Port Notes (Utilities)

- CLI argument building: string manipulation, trivial in Rust
- Config merge: struct merge with `Option` field handling
- Message mapping: string formatting, direct translation
- Settings mapping: struct-to-struct transformation
- Validation: `serde` deserialization + custom validation functions
- JSON schema sanitization: `serde_json::Value` manipulation

---

## 9. Compatibility Layer (`compat.ts`)

### File

| File            | Lines | Purpose                                     |
| --------------- | ----- | ------------------------------------------- |
| `src/compat.ts` | ~1027 | Promise-based API wrapping Effect internals |

### Purpose

Provides backward-compatible Promise-based APIs for consumers that cannot use Effect-TS directly.

### Key Types

**`CodexAppServerManager`** -- Promise-based manager interface:

```typescript
interface CodexAppServerManager {
  ensureReady(): Promise<void>;
  startThread(params?): Promise<{ threadId: string }>;
  startTurn(threadId, params): Promise<ReadableStream<Notification>>;
  listModels(params?): Promise<ModelListResult>;
  listAllModels(): Promise<ModelDescriptor[]>;
  reconfigureRuntime(settings): Promise<ReconfigureResult>;
  dispose(): Promise<void>;
}
```

**`CodexAppServerRegistry`** -- Instance pooling:

```typescript
interface CodexAppServerRegistry {
  acquire(key, settings?): Promise<CodexAppServerManager>;
  release(key): Promise<void>;
  dispose(): Promise<void>;
}
```

**`CodexAppServerRegistryImpl`** -- Implementation:

- `Map<string, { manager, refCount, idleTimer }>` for instance tracking
- Ref-counting: multiple `acquire` calls with same key share one manager
- Idle shutdown: timer starts on last `release`, cancelled on next `acquire`
- Reconfiguration support: `reconfigure(key, settings)` updates running instance

**`CodexCompatLanguageModel`** -- `LanguageModelV3` wrapper:

- Wraps the Effect-based `CodexLanguageModel`
- Bridges Effect streams to `ReadableStream` via `ManagedRuntime.runPromise`

**`CodexCliProvider`** -- `ProviderV3` factory:

```typescript
interface CodexCliProvider {
  languageModel(modelId): LanguageModelV3;
  chat(modelId): LanguageModelV3; // alias
  listModels(): Promise<ModelDescriptor[]>;
  dispose(): Promise<void>;
}
```

**Factory functions:**

- `createCodexAppServerManager(settings?)` -- Creates `RuntimeBackedManager` with `ManagedRuntime`
- `createCodexCli(settings?)` -- Creates `CodexCliProvider` (builds layers, runtime, model instances)
- `codexCli` -- Lazy singleton (creates on first property access)

### Rust Port Notes (Compat)

- This entire file exists to bridge Effect-TS to Promise-based APIs
- In Rust, the native async API would be the primary interface
- No need for a separate compat layer -- Rust's `async/await` is the standard
- The registry pattern (ref-counting, idle shutdown) would be implemented directly
- `ManagedRuntime` -> `Arc<Runtime>` with `Drop`-based cleanup

---

## 10. Hooks System

### File

| File           | Lines | Purpose                        |
| -------------- | ----- | ------------------------------ |
| `src/hooks.ts` | ~40   | EventEmitter-based hook system |

### CodexHooks

Simple `EventEmitter` subclass:

```typescript
class CodexHooks extends EventEmitter implements CodexHookEmitter {
  emitAfterToolUse(payload: CodexAfterToolUsePayload): void;
  emitAfterAgent(payload: CodexAfterAgentPayload): void;
}
```

**Payloads:**

- `CodexAfterToolUsePayload`: tool name, call ID, input, output, duration
- `CodexAfterAgentPayload`: agent name, session ID, result summary

### Rust Port Notes (Hooks)

- EventEmitter -> callback registry or `tokio::sync::broadcast` channel
- Or trait-based observer pattern

---

## 11. Rust Port Considerations

### What Maps Cleanly

| TypeScript Concept         | Rust Equivalent                           |
| -------------------------- | ----------------------------------------- |
| `Data.TaggedError`         | `enum` with `#[derive(thiserror::Error)]` |
| `Effect.gen`               | `async fn` with `?`                       |
| `Context.Tag` service      | Trait + struct impl                       |
| `Layer` composition        | Struct-based DI or `tower::Layer`         |
| `Ref<T>`                   | `Arc<Mutex<T>>` or `Arc<RwLock<T>>`       |
| `Deferred<T>`              | `tokio::sync::oneshot`                    |
| `Queue<T>`                 | `tokio::sync::mpsc`                       |
| `Stream<T>`                | `futures::Stream` or `tokio_stream`       |
| `Semaphore`                | `tokio::sync::Semaphore`                  |
| `HashMap`                  | `std::collections::HashMap` or `dashmap`  |
| `Option.Option<T>`         | `Option<T>`                               |
| `Schema` validation        | `serde` + custom validation               |
| Pattern matching (`Match`) | Native `match`                            |
| `acquireRelease`           | `Drop` trait or RAII guards               |

### What Needs Rethinking

1. **ManagedRuntime / Layer system**: Effect's DI is deeply integrated. Rust would need a custom DI approach (likely struct composition with generics, or `Arc<dyn Trait>` for dynamic dispatch).

2. **Stream combinators**: Effect's `Stream.mapAccumEffect`, `Stream.flatMap`, etc. would need custom implementations or use `futures::StreamExt` with manual state threading.

3. **Fiber forking**: `Effect.fork` for background workers maps to `tokio::spawn`, but the structured concurrency guarantees differ. Need explicit `JoinHandle` management and `CancellationToken` for cooperative shutdown.

4. **SynchronizedRef**: `Ref.modify` with atomic read-modify-write semantics. In Rust, use `Mutex` with careful lock scoping to avoid deadlocks.

5. **Error channel composition**: Effect's typed error channels (`Effect<A, E1 | E2, R>`) have no direct Rust equivalent. Need explicit error enum composition or `anyhow`/`eyre` for dynamic errors.

6. **AI SDK types**: `LanguageModelV3StreamPart`, `LanguageModelV3Message`, etc. would need Rust struct/enum equivalents with serde support.

7. **Tool bridge (MCP over HTTP)**: Needs Rust MCP SDK or custom JSON-RPC server implementation.

### Recommended Rust Architecture

```
codex-provider (crate)
├── bridge/         -- MCP tool bridge (axum HTTP server)
├── config/         -- serde-based config structs
├── error/          -- thiserror enum hierarchy
├── model/          -- Provider trait + LanguageModel impl
├── server/
│   ├── process.rs      -- tokio::process::Command
│   ├── transport.rs    -- JSON-RPC framing over stdio
│   ├── scheduler.rs    -- tokio::sync::Semaphore
│   ├── router.rs       -- mpsc-based notification routing
│   ├── thread.rs       -- thread/turn RPC operations
│   ├── approval.rs     -- auto-approval handler
│   ├── models.rs       -- model listing + TTL cache
│   ├── runtime_config.rs -- live reconfiguration
│   └── registry.rs     -- instance pooling
├── streaming/
│   ├── dispatcher.rs   -- event dispatch (match on type)
│   ├── pipeline.rs     -- Stream transformation
│   ├── state.rs        -- immutable state struct
│   ├── accumulator.rs  -- text delta reconciliation
│   ├── tool_tracker.rs -- tool call state machine
│   ├── parser.rs       -- event extraction functions
│   └── payloads.rs     -- tool payload builders
└── util/
    ├── args.rs         -- CLI argument building
    ├── merge.rs        -- config merge
    ├── messages.rs     -- message mapping
    └── validation.rs   -- settings validation
```

### Complexity Estimates

| Module              | Complexity | Notes                                  |
| ------------------- | ---------- | -------------------------------------- |
| Config              | Low        | Direct serde mapping                   |
| Errors              | Low        | thiserror enums                        |
| Utilities           | Low        | Pure functions                         |
| Process Manager     | Medium     | tokio::process with lifecycle          |
| RPC Transport       | High       | Async framing, correlation, routing    |
| Scheduler           | Low        | tokio::Semaphore wrapper               |
| Notification Router | Medium     | mpsc channel management                |
| Thread Manager      | Medium     | RPC operations + stream construction   |
| Event Dispatcher    | High       | 16 event types, complex state          |
| Text Accumulator    | High       | Reconciliation logic is intricate      |
| Tool Tracker        | Medium     | State machine per tool                 |
| Stream Pipeline     | High       | Composing all pieces                   |
| Registry            | Medium     | Ref-counting + idle shutdown           |
| Bridge              | Medium     | MCP server implementation              |
| Model/Provider      | Medium     | Trait implementation + stream bridging |

### Critical Path for Port

1. **Error types** (foundation for everything)
2. **Config structs** (needed by all services)
3. **RPC Transport** (core communication layer)
4. **Process Manager** (depends on config)
5. **Scheduler + Router** (concurrency infrastructure)
6. **Thread Manager** (depends on transport, router)
7. **Event Parser + Normalizer** (streaming foundation)
8. **Text Accumulator + Tool Tracker** (streaming state)
9. **Event Dispatcher** (depends on accumulator, tracker)
10. **Stream Pipeline** (depends on dispatcher)
11. **App Server** (orchestrates everything)
12. **Model/Provider** (public API)
13. **Registry** (instance management)
14. **Bridge** (tool integration)

---

## 12. File Index

### Root Files

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/index.ts` -- Barrel exports
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/hooks.ts` -- Hook system
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/compat.ts` -- Promise-based compat layer

### Bridge

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/bridge/CodexToolsBridge.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/bridge/CodexBridge.ts`

### Config

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/config/schemas.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/config/CodexConfig.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/config/CodexProcessConfig.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/config/CodexStreamingConfig.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/config/CodexSchedulerConfig.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/config/CodexMcpConfig.ts`

### Errors

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/auth.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/config.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/rpc.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/scheduler.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/spawn.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/stream.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/disposed.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/errors/classification.ts`

### Model

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/model/CodexLanguageModel.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/model/CodexProvider.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/model/request-preparation.ts`

### Server

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexAppServer.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexProcessManager.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexRpcTransport.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexScheduler.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexNotificationRouter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexThreadManager.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexApprovalHandler.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexModelService.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexRuntimeConfig.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexRegistry.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/CodexServerLayer.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/server/types.ts`

### Streaming

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/CodexEventDispatcher.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/CodexStreamPipeline.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/CodexStreamState.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/CodexTextAccumulator.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/CodexToolTracker.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/event-parser.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/event-normalizer.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/json-utils.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/streaming/tool-payloads.ts`

### Utilities

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/util/args.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/util/config-merge.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/util/message-mapper.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/util/runtime-layer.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/util/settings-mapping.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/codex/src/util/validation.ts`
