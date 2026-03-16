# Deep Analysis Report: providers/runtime Package

## 1. Overview -- Runtime Package Architecture

The `@compozy/provider-runtime` package is the **orchestration layer** that sits between consumers (the Electron app, HTTP clients) and the underlying AI provider SDKs (`@compozy/provider-claude-code`, `@compozy/provider-opencode`, `@compozy/provider-codex`). It uses **Effect-TS** for dependency injection, error management, and concurrency, combined with the **Vercel AI SDK v6** (`ai` package) for the streaming abstraction.

### Package Identity

- **Name**: `@compozy/provider-runtime`
- **Dependencies**: `ai` (v6), `effect`, `es-toolkit`, `zod`, `@compozy/provider-core`, `@compozy/provider-claude-code`, `@compozy/provider-opencode`, `@compozy/provider-codex`
- **Exports**: Main entry (`./src/index.ts`) plus sub-path exports: `./reasoning`, `./protocol`, `./errors`, `./server`, `./models`, `./codec`, `./types/runtime-options`

### Architectural Layers (top-down)

```
HTTP Server (server/app.ts)       RuntimeClient (client/)
         |                              |
         v                              v
    Runtime Service (runtime.ts)
         |
    +----+----+
    |         |
SessionStore  ToolRegistry (tools/)
    |         |
    v         v
  ProviderAdapter (adapters/)
         |
    +----+----+----+
    |    |    |    |
 claude opencode codex [derived adapters]
```

---

## 2. Runtime Core

### Files

- `/providers/runtime/src/runtime.ts` -- Main runtime service, ~808 lines
- `/providers/runtime/src/runtime-async.ts` -- Promise-based wrappers, ~35 lines
- `/providers/runtime/src/index.ts` -- Re-exports

### Core Types

```typescript
interface Runtime {
  readonly providerId: ProviderId;
  readonly capabilities: RuntimeCapabilities;
  readonly streamText: (
    options: RuntimeStreamTextOptions
  ) => Effect<RuntimeStreamResult, RuntimeHostError>;
  readonly registerTools: (tools, serverName, options?) => Effect<void, ToolRegistrationError>;
  readonly getSessionId: () => Effect<Option<SessionId>>;
  readonly resumeSession: (sessionId: SessionId) => Effect<void>;
  readonly clearSession: () => Effect<void>;
}
```

### Key Functions (all `Effect.fn`-traced)

| Function                                       | Purpose                                                                                                                                          |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `makeRuntime(deps)`                            | Factory; creates a `Runtime` from `RuntimeDependencies` (adapter + sessionStore + toolRegistry). Scoped (adds finalizer to clear tool registry). |
| `createRuntime(source, options?)`              | High-level factory; accepts either a `ProviderAdapter` directly or a `ProviderRegistryServiceShape`. Composes all layers via `makeRuntimeLayer`. |
| `validateStreamRequest`                        | Pre-flight check: validates provider capabilities against request features (images, reasoning, tools, agents, session resume).                   |
| `resumeSessionIfRequested`                     | Looks up existing session by `sessionKey` or `taskId` and calls `adapter.resumeSession`.                                                         |
| `registerCallScopedTools`                      | Registers per-call tool registrations, tracks server names for later cleanup, rolls back on failure.                                             |
| `buildAdapterOptions`                          | Maps `RuntimeStreamTextOptions` -> `AdapterStreamTextOptions`, wrapping `onStepFinish` to emit usage updates.                                    |
| `emitUsageUpdate` / `emitUsageUpdateFromChunk` | Deduplicates and emits usage callback with serialized comparison. Uses `Ref` to track last payload.                                              |
| `createUsageTracker`                           | Forks a daemon fiber that resolves final `totalUsage` from `StreamTextResult` and emits it.                                                      |
| `schedulePostStreamCleanup`                    | Forks a daemon that waits for `result.response` then unregisters call-scoped tool servers.                                                       |

### RuntimeService (Effect Context.Tag)

```typescript
class RuntimeConfig extends Context.Tag("@compozy/runtime/RuntimeConfig")<RuntimeConfig, { adapter: ProviderAdapter }>()
class RuntimeService extends Context.Tag("@compozy/runtime/Runtime")<RuntimeService, Runtime>()
```

`RuntimeService.layer` is scoped and requires `RuntimeConfig`, `SessionStore`, and `ToolRegistry` in context.

### Async Wrappers (runtime-async.ts)

`runtimeStreamTextAsync` and `runtimeRegisterToolsAsync` are thin `Effect.runPromise` wrappers that unwrap `FiberFailure` for non-Effect consumers.

### Stream Lifecycle (per call)

1. Create `UsageUpdateState` (Ref for dedup)
2. `validateStreamRequest` -- capability checks
3. `resumeSessionIfRequested` -- session lookup
4. `registerCallScopedTools` -- register per-call tools
5. `buildAdapterOptions` -- wrap callbacks
6. `adapter.streamText(adapterOptions, toolRegistry)` -- actual streaming
7. `createUsageTracker` -- fork daemon for final usage
8. `schedulePostStreamCleanup` -- fork daemon for tool cleanup
9. `syncSessionFromResult` -- fork daemon to persist session metadata
10. Return `RuntimeStreamResult`

---

## 3. Adapter System

### Base Interface (`adapters/adapter.ts`)

```typescript
interface ProviderAdapter {
  readonly providerId: ProviderId;
  readonly capabilities: RuntimeCapabilities;
  readonly toolIdCodec: ToolIdCodec;
  readonly streamText: (
    options,
    tools: ToolRegistryService
  ) => Effect<AdapterStreamTextResult, ProviderAdapterError>;
  readonly getSessionId: () => Effect<Option<SessionId>>;
  readonly resumeSession: (sessionId: SessionId) => Effect<void>;
  readonly clearSession: () => Effect<void>;
}
```

Every adapter is an `Effect.fn`-based factory that returns `Effect<ProviderAdapter, ProviderAdapterError, Scope.Scope>`.

### Adapter Types (`adapters/types.ts`)

```typescript
interface AdapterConfig {
  apiKey: string;
  selectedModel: string;
}
interface ClaudeCodeAdapterDefinition {
  providerId: ProviderId;
  displayName: string;
  buildEnvVars: (config: AdapterConfig) => Record<string, string>;
}
```

### Three Adapter Families

#### A. Claude Code Adapter (`adapters/claude-code/`)

**Primary adapter** (`claude-code/index.ts`):

- Uses `@compozy/provider-claude-code` `createClaudeCode()` to get a Vercel AI provider
- Manages session via `createSessionManager()` (Ref-based)
- Resolves model, merges settings (agents, hooks, MCP servers, reasoning, extensions)
- Calls `streamText()` from `ai` package
- Returns adapter with `createClaudeCodeToolIdCodec(providerId)`
- Provider IDs that use this: `"claude-code"` (direct), plus all derived

**Derived adapter pattern** (`claude-code/derived-adapter.ts`):

- `createDerivedClaudeCodeAdapter<TOptions>(config)` -- generic factory that wraps the base Claude Code adapter
- Config provides: `providerId`, `providerLabel`, `defaultModelId`, `requiresSelectedModel`, `resolveSelectedModel`, `buildEnvVars`
- The derived adapter overrides `streamText` to inject provider-specific environment variables and model selection

**Derived adapters** (all use `createDerivedClaudeCodeAdapter`):

| Adapter    | File                    | Provider ID    | Base URL / Env Strategy                                                  |
| ---------- | ----------------------- | -------------- | ------------------------------------------------------------------------ |
| Z.ai       | `zai-adapter.ts`        | `"zai"`        | `ANTHROPIC_BASE_URL=https://api.z.ai/api/anthropic`, requires API key    |
| OpenRouter | `openrouter-adapter.ts` | `"openrouter"` | `ANTHROPIC_BASE_URL=https://openrouter.ai/api`, requires API key         |
| Vercel     | `vercel-adapter.ts`     | `"vercel"`     | `ANTHROPIC_BASE_URL=https://ai-gateway.vercel.sh`, requires API key      |
| Moonshot   | `moonshot-adapter.ts`   | `"moonshot"`   | `ANTHROPIC_BASE_URL=https://api.moonshot.ai/anthropic`, requires API key |
| MiniMax    | `minimax-adapter.ts`    | `"minimax"`    | `ANTHROPIC_BASE_URL=https://api.minimax.io/anthropic`, requires API key  |
| Bedrock    | `bedrock-adapter.ts`    | `"bedrock"`    | `CLAUDE_CODE_USE_BEDROCK=1`, uses AWS region                             |
| Vertex     | `vertex-adapter.ts`     | `"vertex"`     | `CLAUDE_CODE_USE_VERTEX=1`, uses GCP project ID                          |
| Ollama     | `ollama-adapter.ts`     | `"ollama"`     | `ANTHROPIC_BASE_URL=http://localhost:11434`, no API key needed           |

All derived adapters set `ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL` to force the selected model across all tiers.

#### B. OpenCode Adapter (`adapters/opencode/`)

**`opencode/adapter.ts`** (~373 lines):

- Uses `@compozy/provider-opencode` `createOpencode()` provider
- Manages its own server lifecycle via `ServerLifecycle` interface
- Tracks tool signatures via `Ref` to detect tool changes between calls
- Supports subagent configuration at adapter creation time (injected into `serverConfig.agent`)
- Supports additional repositories, hooks, hook options, reasoning effort
- Uses `DEFAULT_OPENCODE_MAX_STEPS = 8`
- Adds a finalizer to shutdown the managed server on scope close
- Returns adapter with `createOpenCodeToolIdCodec()`

**`opencode/server-lifecycle.ts`**:

- `ServerLifecycle` interface: `ensureReady()`, `getInfo()`, `shutdown()`, `healthCheck()`, `invalidateExternalInstance()`
- `createOpenCodeServerLifecycle(settings)` -- wraps `OpencodeClientManager`
- Health check tries `/global/health`, `/health`, `/config` endpoints
- `invalidateExternalInstance` forces config/tool refresh for external servers

#### C. Codex Adapter (`adapters/codex-adapter.ts`)

**`codex-adapter.ts`** (~315 lines):

- Uses `@compozy/provider-codex` `createCodexCli()` provider
- Serializes runtime startup via `runtimeStartupQueue` (Promise chain)
- Separates settings into model-scoped (per-request) vs runtime-scoped (need `reconfigureRuntime`)
- Collects MCP servers from tool bridge payloads via `collectMcpServersFromTools`
- Has model compatibility overrides (e.g., disabling reasoning summary for spark variants)
- Runtime overrides limited to `cwd` and `env` at call scope
- Returns adapter with `createCodexToolIdCodec()`

### Shared Utilities (`adapters/shared/`)

| File                      | Purpose                                                                                                                                               |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `build-stream-options.ts` | Builds the options object for `ai.streamText()`. Maps `maxSteps` -> `stepCountIs(maxSteps)`.                                                          |
| `error-mapping.ts`        | `mapToolRegistrationError(providerId)` and `mapAdapterStreamError(providerId)` -- curried error mappers.                                              |
| `merge-settings.ts`       | `mergeSettings(base, overrides)` -- deep merge using `es-toolkit/object.mergeWith`, arrays are replaced not concatenated.                             |
| `resolve-extension.ts`    | `resolveExtension<TSettings>(extensions, providerId)` -- extracts provider-specific extension from `RuntimeProviderExtensions`.                       |
| `resolve-model-id.ts`     | `resolveModelId({ fallbackModelId, overrideModelId, providerId, providerLabel })` -- returns `Effect<string, ModelResolutionError>`.                  |
| `session-manager.ts`      | `createSessionManager()` -- returns `AdapterSessionManager` with `getSessionId`, `resumeSession`, `clearSession`, backed by `Ref<Option<SessionId>>`. |

---

## 4. Capabilities System

### Files

- `/providers/runtime/src/capabilities/capability-validator.ts`
- `/providers/runtime/src/types/capabilities.ts`

### RuntimeCapabilities Schema

```typescript
class RuntimeCapabilities extends Schema.Class({
  sessionResume: Schema.Boolean,
  extendedThinking: Schema.Boolean,
  customSystemPrompt: Schema.Boolean,
  imageInputs: Schema.Boolean,
  mcpServers: Schema.Boolean,
  customTools: Schema.Boolean,
  lifecycleHooks: Schema.Boolean,
  requiresServerManagement: Schema.Boolean,
  toolSupport: Schema.Boolean,
  agentSupport: Schema.Boolean,
  approvalStopPolicy: ApprovalStopPolicy,
})
```

### Pre-defined Capability Sets

| Constant                 | imageInputs | agentSupport | lifecycleHooks | requiresServerManagement |
| ------------------------ | ----------- | ------------ | -------------- | ------------------------ |
| `ClaudeCodeCapabilities` | true        | true         | true           | false                    |
| `OpenCodeCapabilities`   | false       | true         | true           | true                     |
| `CodexCapabilities`      | false       | false        | false          | false                    |

### Capability Validator

`validateCapabilities(options)` checks:

- `imageInputs` -- fails with `CapabilityUnsupportedError` if provider doesn't support
- `extendedThinking` -- reasoning options check
- `toolSupport` -- tool registrations check
- `agentSupport` -- agent delegation check
- `sessionResume` -- session resume check

Helper functions:

- `messagesHaveImageInputs(messages)` -- scans message content for `type: "image"`, `type: "input_image"`, or `type: "file"` with `image/*` media type
- `hasAgentOptions(options)` -- checks if agents or subagents arrays are non-empty
- `hasToolRegistrations(registrations)` -- checks if any registration has tools

---

## 5. Client Layer

### Files

- `/providers/runtime/src/client/runtime-client.ts`

### RuntimeClientService Interface

```typescript
type RuntimeClientService = {
  streamText: (options) => Effect<RuntimeStreamResult, RuntimeHostError>;
  registerTools: (tools, serverName, options?) => Effect<void, RuntimeHostError>;
  createSession: (sessionId?) => Effect<SessionId, RuntimeHostError>;
  resumeSession: (sessionId) => Effect<void, RuntimeHostError>;
  resumeOrCreateSession: (sessionId?) => Effect<SessionId, RuntimeHostError>;
  dispose: () => Effect<void, RuntimeHostError>;
};
```

### makeRuntimeClient (Effect-based)

- Creates a `disposedRef` (Ref<boolean>) and a `disposeLock` (semaphore)
- `ensureNotDisposed()` guard on every operation
- `dispose()` clears session, runs `onDispose` callback, sets disposed flag
- Adds `addFinalizer` for auto-cleanup

### RuntimeClientAsync (Promise-based wrapper)

- Static `create(options)` factory
- All methods map to `runEffect` which wraps `Effect.runPromise` and unwraps `FiberFailure`
- Supports `Symbol.asyncDispose` for `using` syntax
- Manages its own `Scope.CloseableScope`
- `dispose()` is idempotent with in-flight tracking

### RuntimeClient (alias)

`RuntimeClient = RuntimeClientAsync` -- the default export alias.

---

## 6. Error System

### Error Type ID

```typescript
const RuntimeErrorTypeId = "~@compozy/runtime/RuntimeError";
```

All runtime errors carry this symbol property for runtime type checking via `isRuntimeError(u)`.

### Error Hierarchy

All errors use `Schema.TaggedError` from Effect-TS:

| Error Class                  | Module          | Key Fields                                                                                                  | Notes                      |
| ---------------------------- | --------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------- | ------------------ |
| `AdapterStreamError`         | adapter-errors  | `message, providerId, cause?`                                                                               | Stream-level failures      |
| `ModelResolutionError`       | adapter-errors  | `message, providerId, requestedModel?, inputModel?, resolvedSelectedModel?, fallbackSelectedModel?, cause?` | Model lookup failures      |
| `CapabilityUnsupportedError` | provider-errors | `message, providerId, capability: ProviderCapabilityFlag`                                                   | Capability validation      |
| `ProviderResolutionError`    | provider-errors | `message, providerId?`                                                                                      | Provider registry lookup   |
| `ProviderExecutionError`     | provider-errors | `message, cause?`                                                                                           | General provider errors    |
| `UnsupportedProviderError`   | provider-errors | `message, providerId, supportedProviders: NonEmptyArray<string>`                                            | Unknown provider ID        |
| `ToolRegistrationError`      | tool-errors     | `message, providerId?, serverName?, toolName?, cause?`                                                      | Tool registration failures |
| `ToolBridgeError`            | tool-errors     | `message, providerId?, serverName?, cause?`                                                                 | MCP bridge failures        |
| `InvalidToolNameError`       | tool-errors     | `toolName, reason?, message`                                                                                | Tool name parsing          |
| `AuthorizationError`         | server-errors   | `status: 401                                                                                                | 403, code, message`        | HTTP auth failures |
| `RequestValidationError`     | server-errors   | `message, cause?`                                                                                           | HTTP request parsing       |
| `ServerLifecycleError`       | server-errors   | `message, providerId?, cause?`                                                                              | Server management          |
| `SessionError`               | session-errors  | `message, providerId, sessionId?, taskId?, cause?`                                                          | Session store operations   |
| `UnknownProviderError`       | codec           | `providerId, supportedProviders, message?`                                                                  | Tool codec lookup          |

### Error Unions

```typescript
const RuntimeHostError = Schema.Union(
  AdapterStreamError,
  ModelResolutionError,
  ProviderResolutionError,
  ProviderExecutionError,
  ServerLifecycleError,
  CapabilityUnsupportedError,
  UnsupportedProviderError,
  ToolRegistrationError,
  InvalidToolNameError,
  ToolBridgeError,
  SessionError
);

const UserInputError = Schema.Union(CapabilityUnsupportedError, InvalidToolNameError);
```

### Error Details and Formatting

**`ProviderErrorDetails`** -- structured metadata extracted from errors:

- `message, tag, providerId, exitCode, statusCode, code, classification, stderr, responseBody, eventType, threadId, turnId, causeChain, isRetryable`

**`extractDetails(error)`** -- deep-crawls the error cause chain (max 8 levels), collects records, resolves best message, determines retryability from status codes (429, 503, 529) or classification strings.

**`format(error)`** / **`formatDetails(details)`** -- produces human-readable multi-section text.

### FiberFailure Unwrapping

`unwrapFiberFailure(error)` -- extracts the failed value from Effect's `FiberFailure` wrapper using `Runtime.isFiberFailure` and `Cause.failureOption`.

---

## 7. Models

### Files

- `/providers/runtime/src/models/model-cost.ts`
- `/providers/runtime/src/models/model-discovery-service.ts`

### Model Cost

```typescript
const ModelCost = Schema.Struct({
  inputPerMillion: Schema.optional(NonNegativeFiniteRate),
  outputPerMillion: Schema.optional(NonNegativeFiniteRate),
});

computeEstimatedCost(inputTokens, outputTokens, cost): number | undefined
```

Simple linear cost computation: `inputTokens * (inputPerMillion / 1_000_000) + outputTokens * (outputPerMillion / 1_000_000)`.

### Model Discovery Service

```typescript
interface ModelDiscoveryServiceShape {
  listModels: () => Effect<readonly ModelInfo[]>;
}

class ModelDiscoveryService extends Context.Tag("@compozy/runtime/ModelDiscoveryService")
```

- Accepts multiple `ModelSource` instances, each with a `priority` and `listModels()` effect
- Sources are fetched concurrently (`concurrency: "unbounded"`)
- Results are merged by priority (higher priority wins for overlapping model IDs)
- Model merging: left-biased field selection with cost merging

`ModelInfo` schema: `id, providerId, displayName?, contextWindow?, maxOutputTokens?, supportsTools?, supportsReasoning?, description?, cost?`

---

## 8. Protocol

### Files

- `/providers/runtime/src/protocol/branded.ts`
- `/providers/runtime/src/protocol/provider-family.ts`

### Provider IDs (Branded Types)

```typescript
const PROVIDER_IDS = [
  "claude-code",
  "zai",
  "openrouter",
  "vercel",
  "moonshot",
  "minimax",
  "bedrock",
  "vertex",
  "ollama",
  "codex",
  "opencode",
] as const;
const ProviderId = Schema.Literal(...PROVIDER_IDS);
const NON_CLAUDE_PROVIDER_IDS = ["codex", "opencode"] as const;
const CLAUDE_FAMILY_PROVIDER_IDS = [
  /* all except codex, opencode */
];
const SessionId = Schema.String.pipe(Schema.brand("SessionId"));
```

### Provider Family Resolution

```typescript
type ProviderFamilyId = "claude-code" | "codex" | "opencode" | "unknown";
type GatewayProviderId = Exclude<ClaudeFamilyProviderId, "claude-code">; // zai, openrouter, vercel, ...

resolveProviderFamily(providerId: string): ProviderFamilyId
resolveProviderFamilyFromProviderId(providerId: ProviderId): Exclude<ProviderFamilyId, "unknown">
isClaudeFamilyProviderId(providerId): boolean
isGatewayProviderId(providerId): boolean
```

Maps any specific provider ID to one of three families: `"claude-code"`, `"codex"`, or `"opencode"`. Gateway providers (zai, openrouter, vercel, moonshot, minimax, bedrock, vertex, ollama) all map to `"claude-code"` family.

---

## 9. Reasoning System

### Files

- `/providers/runtime/src/reasoning/resolve-reasoning.ts`
- `/providers/runtime/src/reasoning/xhigh-detection.ts`

### Reasoning Effort

```typescript
const ReasoningEffort = Schema.Literal("low", "medium", "high", "xhigh");
```

### Token Budgets (Claude family)

```typescript
const CLAUDE_REASONING_TOKEN_BUDGET = {
  low: 15999,
  medium: 31999,
  high: 63999,
};
```

### Key Functions

| Function                                                               | Purpose                                                                                                                                    |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `resolveReasoningForProvider({ family, effort, modelId })`             | Maps reasoning effort to provider-specific values. Claude caps at "high" (xhigh -> high). Codex/OpenCode support xhigh if model allows it. |
| `mapMaxThinkingTokensToReasoningEffort({ family, maxThinkingTokens })` | Converts raw token count to effort level based on provider thresholds.                                                                     |
| `resolveClaudeMaxThinkingTokens(options)`                              | Resolves max thinking tokens for Claude from multiple sources (direct, reasoning effort, extensions).                                      |
| `supportsXHighReasoning(modelId)`                                      | Checks if model supports xhigh. Uses exact ID set and regex series patterns.                                                               |
| `normalizeReasoningModelId(modelId)`                                   | Strips `"openai/"` prefix, lowercases and trims.                                                                                           |

### xhigh-Capable Models

- Exact IDs: `gpt-5.1-codex`, `gpt-5.1-codex-mini`, `gpt-5.1-codex-max`
- Series patterns: `gpt-5.2*`, `gpt-5.3*`

---

## 10. Server

### Files

- `/providers/runtime/src/server/app.ts` -- HTTP server factory
- `/providers/runtime/src/server/auth.ts` -- Auth service
- `/providers/runtime/src/server/http-utils.ts` -- JSON response helper
- `/providers/runtime/src/server/routes-models.ts` -- Models endpoint
- `/providers/runtime/src/server/sse-writer.ts` -- SSE streaming

### HTTP Server (`createRuntimeServer`)

**Routes**:

- `GET /health` -- `{ status: "ok" }`
- `GET /v1/models` -- Lists models from ModelDiscoveryService, supports `?provider_id` filter
- `POST /v1/chat/stream` -- Main streaming endpoint, SSE response

**Architecture**:

- Uses `ManagedRuntime` from Effect for the server layer
- Composes `AuthService` + `ModelDiscoveryService` into the app layer
- `withErrorResponses` maps all `RuntimeServerRouteError` types to appropriate HTTP status codes

**StreamRequestBody Schema**:

- `messages` (non-empty array), `model?`, `system?`, `maxSteps?`, `workingDirectory?`, `taskId?`, `sessionKey?`, `resumeSession?`, `reasoningEffort?`, `maxThinkingTokens?`

### Auth Service

```typescript
class AuthService extends Context.Tag("@compozy/runtime/AuthService")<AuthService, AuthServiceShape>()
```

- `AuthService.layer(apiKey)` -- validates bearer token with timing-safe comparison
- `AuthService.noAuthLayer` -- no-op (always authorized)
- `extractBearerToken(header)` -- parses `Authorization: Bearer <token>`
- `timingSafeCompare(left, right)` -- constant-time string comparison using `node:crypto.timingSafeEqual` with padding

### SSE Writer

- `formatSseEvent({ id?, event?, data })` -- formats as `id: ...\nevent: ...\ndata: ...\n\n`
- `formatSseDone()` -- `data: [DONE]\n\n`
- `RuntimeSseWriter` class -- stateful writer that prevents writing after DONE
- `streamRuntimeSse(chunks, options?)` -- converts Effect `Stream<unknown>` to `ReadableStream<Uint8Array>`, assigns sequence IDs, handles errors with an `error` SSE event

### Models Route

- OpenAI-compatible response format (`/v1/models` endpoint)
- `toOpenAiModel(model)` -- maps `ModelInfo` to `OpenAiModelResponse` with nested `compozy` extension field
- Supports `provider_id` query parameter for filtering

---

## 11. Services

### Files

- `/providers/runtime/src/services/layers.ts` -- Layer composition
- `/providers/runtime/src/services/provider-registry.ts` -- Provider registry

### Layer Composition (`layers.ts`)

**`defaultToolRegistryConfig`**: Pre-configured tool bridge factories and aggregation settings:

- Claude-family providers: `ClaudeCodeToolsBridge`, canonical names, aggregated bridge with `compozy` server name and `__` separator
- Codex: `CodexToolsBridge`, same aggregation settings
- OpenCode: `OpenCodeToolsBridge`, supports `baseDirectory` parameter

**`composeRuntimeLayers(options)`**: Composes `RuntimeConfig` + `SessionStore` + `ToolRegistry` + `RuntimeService` into a single layer.

**`makeRuntimeLayer(options)`**: Convenience wrapper around `composeRuntimeLayers`.

**`createRuntimeDependencies(options)`**: Creates plain `SessionStoreService` + `ToolRegistryService` without layers (for non-Effect consumers).

### Provider Registry (`provider-registry.ts`)

```typescript
class ProviderRegistryService extends Context.Tag("@compozy/runtime/ProviderRegistryService")
```

**Methods**:

- `registerProvider(providerId, adapter)` -- validates adapter.providerId matches
- `resolveProvider(options)` -- resolution strategy:
  1. Explicit `providerId` -> direct lookup
  2. Infer from `model` string prefix using `modelPrefixMap`
  3. Single registered provider -> use it
  4. Ambiguous -> fail
- `listProviders()` -- returns registered provider IDs

**Default model prefix map**:

```typescript
{ "claude-": "claude-code", "gpt-": "codex", "o1-": "codex", "o3-": "codex", "codex-": "codex" }
```

---

## 12. Session Management

### Files

- `/providers/runtime/src/session/session-store.ts` -- Interface + Context.Tag
- `/providers/runtime/src/session/in-memory-store.ts` -- In-memory implementation
- `/providers/runtime/src/session/sqlite-session-store.ts` -- SQLite-backed implementation

### SessionStoreService Interface

```typescript
interface SessionStoreService {
  getByTaskId: (taskId, providerId) => Effect<Option<SessionMetadata>>;
  getByKey: (sessionKey, providerId) => Effect<Option<SessionMetadata>>;
  get: (sessionId) => Effect<Option<SessionMetadata>>;
  set: (metadata) => Effect<void, SessionError>;
  delete: (sessionId) => Effect<void, SessionError>;
  touch: (sessionId) => Effect<void>;
}
```

**SessionMetadata**: `sessionId, providerId, taskId?, sessionKey?, createdAt, lastAccessedAt, workingDirectory?`

### InMemorySessionStore

- Backed by two Maps: `bySessionId` and `byKey` (forward index)
- Configurable `maxEntries` (default: 500), `idleTtlMs` (default: 10 minutes)
- Eviction: idle-based TTL + capacity-based (oldest first)
- Keys: composite `"${providerId}:${sessionKey}"` for isolation

### SqliteSessionStore

- Adapter over `SqliteSessionRepository<E>` interface (just `getSessionId`, `setSessionId`, `deleteSession`)
- Persisted format: `PersistedSessionRecordV1` (JSON with version field, Schema-validated)
- Uses reverse index: `"__compozy:session:${sessionId}"` -> full metadata JSON
- Forward keys: `"${providerId}:${sessionKey}"` -> session ID string
- Legacy support: `claude-code` provider can look up by bare `taskId`
- Handles stale key cleanup on set operations

---

## 13. Tools System

### Files

- `/providers/runtime/src/tools/codec.ts` -- Tool ID encoding/decoding
- `/providers/runtime/src/tools/registry.ts` -- Tool registry service (~1200+ lines)
- `/providers/runtime/src/tools/bridge.ts` -- Bridge type definitions
- `/providers/runtime/src/tools/compozy-tools.ts` -- Tools class (builder pattern)
- `/providers/runtime/src/tools/types.ts` -- Shared types

### Tool ID Codec (`codec.ts`)

**Canonical format**: `mcp/<serverName>/<toolName>` -- provider-agnostic identity.

**Provider-specific formats**:
| Provider Family | Pattern | Example |
|----------------|---------|---------|
| Claude Code | `mcp__compozy__<server>__<tool>` | `mcp__compozy__myserver__mytool` |
| OpenCode | `compozy_<server>__<tool>` | `compozy_myserver__mytool` |
| Codex | `codex__compozy__<server>__<tool>` | `codex__compozy__myserver__mytool` |

**Key functions**:

- `buildCanonicalToolId(serverName, toolName)` -> `CanonicalToolId`
- `createClaudeCodeToolIdCodec(providerId?)` -> `ToolIdCodec`
- `createOpenCodeToolIdCodec()` -> `ToolIdCodec`
- `createCodexToolIdCodec()` -> `ToolIdCodec`
- `formatProviderToolName(options)` -- canonical -> provider-specific
- `parseProviderToolName(options)` -- provider-specific -> parsed (with fallback to legacy and UI-normalized formats)
- `detectProviderIdFromToolName(providerToolName)` -- prefix-based detection
- `normalizeToolName(options)` -- full normalization with round-trip

### Tool Registry (`registry.ts`)

```typescript
class ToolRegistry extends Context.Tag("@compozy/runtime/ToolRegistry")<ToolRegistry, ToolRegistryService>()
```

**ToolRegistryService Interface** (key methods):

- `registerAiSdkTools(tools, serverName, options?)` -- registers AI SDK tools, creates bridge instances
- `getToolsForProvider(providerId, options?)` -- retrieves tools formatted for specific provider
- `getMcpServersForProvider(providerId)` -- retrieves MCP server configs for provider
- `unregisterServer(serverName)` -- removes all tools for a server
- `clear()` -- removes all tools, closes all bridges

**Internal architecture**:

- Stores tools in a `Ref<Map<CanonicalToolId, InternalToolEntry>>`
- Stores bridge instances in a `Ref<Map<string, ManagedBridgeEntry>>`
- Bridge lifecycle: created lazily per server name, closed on unregister/clear
- Supports tool registration modes: `"merge"` (default) and `"replace"` (atomic replace per server)
- Aggregated bridge mode: multiple servers collapsed into one MCP bridge with encoded tool names

**Configuration**:

```typescript
type ToolRegistryConfig = {
  toolIdCodecs?: ProviderConfigMap<ToolIdCodec>;
  toolBridgeFactories?: ProviderConfigMap<ToolBridgeFactory>;
  applyProviderCodec?: ProviderConfigMap<boolean>;
  bridgeUsesCanonicalNames?: ProviderConfigMap<boolean>;
  useAggregatedBridge?: ProviderConfigMap<boolean>;
  bridgeAggregationConfig?: ProviderConfigMap<BridgeAggregationSettings>;
};
```

### Tool Bridge Types (`bridge.ts`)

```typescript
type ToolBridgeInstance = {
  name: string;
  tools: () => Promise<Record<string, RegisteredTool>>;
  close: () => void;
  getMcpServer?: () => unknown;
};

type ToolBridgeFactory = (options: {
  serverName: string;
  tools: Record<string, RegisteredTool>;
  baseDirectory?: string;
}) => ToolBridgeInstance;
```

### Tools Class (`compozy-tools.ts`)

Builder pattern for tool definitions:

- `register(tool)` / `registerMany(tools)` -- adds tool definitions
- `createTools(toolDefinitions, config)` -- creates MCP server via injected factory
- `getNeedsApprovalToolNames()` -- returns set of tool names that need approval
- `checkNeedsApproval(toolName, input, options?)` -- evaluates approval predicate (supports boolean and function)

---

## 14. Usage Tracking

### Files

- `/providers/runtime/src/usage/types.ts` -- Schema types
- `/providers/runtime/src/usage/native-event-utils.ts` -- Low-level extractors
- `/providers/runtime/src/usage/metadata-extractor.ts` -- Provider metadata extraction
- `/providers/runtime/src/usage/consumption.ts` -- Main consumption resolution
- `/providers/runtime/src/usage/token-consumption.ts` -- NormalizedTokenConsumption bridge

### Usage Types

```typescript
class RuntimeUsage extends Schema.Class({
  inputTokens?: NonNegativeInt,
  outputTokens?: NonNegativeInt,
  totalTokens?: NonNegativeInt,
  inputDetails?: RuntimeInputTokenDetails, // { cacheRead?, cacheWrite?, noCache? }
  outputDetails?: RuntimeOutputTokenDetails, // { text?, reasoning? }
  costUsd?: NonNegativeNumber,
  durationMs?: NonNegativeNumber,
})
```

### RuntimeConsumption

```typescript
class RuntimeConsumption extends Schema.Class({
  providerId: string,
  providerFamily: ProviderFamilyId,
  metadataKeyUsed?: string,
  usage: RuntimeUsage,
  rawUsage?: Record<string, unknown>,
})
```

**Static factories**:

- `resolve({ providerId, usage?, providerMetadata? })` -- normalizes from multiple data shapes
- `fromStreamResult({ result, providerId })` -- awaits `result.totalUsage` + `result.providerMetadata` concurrently
- `fromChunk({ chunk, providerId })` -- extracts from stream chunk events

### Data Shape Resolution

Supports two input shapes:

1. **V3 shape**: `usage.inputTokens.total`, `usage.outputTokens.total`, `usage.raw`, etc.
2. **LanguageModel shape**: `usage.inputTokens`, `usage.outputTokens`, `usage.inputTokenDetails.cacheReadTokens`, etc.

Metering (costUsd, durationMs) resolved from multiple fallback locations in provider metadata and raw usage.

### Token Consumption Bridge

`resolveTokenConsumptionFromResult`, `resolveTokenConsumptionFromUsage`, `extractTokenConsumptionFromChunk` -- all convert `RuntimeConsumption` to `NormalizedTokenConsumption` (from `@compozy/provider-core`) for callback emission.

---

## 15. Rust Port Considerations

### Key Architectural Decisions for Rust

1. **Effect-TS to Rust Mapping**:
   - `Effect<A, E, R>` -> Rust `Result<A, E>` with dependency injection via trait objects or generics
   - `Context.Tag` services -> Rust traits with `Arc<dyn Trait>` for runtime polymorphism
   - `Layer` composition -> Builder pattern or `Provider<T>` containers
   - `Ref<T>` -> `Arc<RwLock<T>>` or `tokio::sync::watch`
   - `Schema.TaggedError` -> Rust enums with `#[derive(thiserror::Error)]`
   - `Effect.fn` tracing -> `tracing::instrument`
   - `Effect.forkDaemon` -> `tokio::spawn` for background tasks
   - `Scope` / finalizers -> `Drop` trait or `tokio::sync::oneshot` shutdown signals

2. **Adapter Pattern**:
   - `ProviderAdapter` trait with async methods
   - Derived Claude adapters can share a base struct with env-var configuration
   - OpenCode adapter needs lifecycle management (server process)
   - Codex adapter needs serialized runtime startup (mutex)

3. **Tool System Complexity**:
   - The tool codec (canonical ID <-> provider-specific name) is a critical piece
   - Bridge factory pattern needs MCP server management
   - Aggregated bridge mode collapses multiple servers into one
   - The `ToolRegistry` is the most complex module (~1200+ lines) -- consider splitting

4. **Error Hierarchy**:
   - 12 distinct error types -- map to a Rust enum with `thiserror`
   - `RuntimeHostError` union -> single enum with all variants
   - `ProviderErrorDetails` extraction logic is complex (recursive cause chain crawling)

5. **Session Store**:
   - `SessionStoreService` trait with two implementations
   - In-memory: LRU-like with TTL eviction
   - SQLite: reverse-index pattern for bidirectional lookup
   - The interface is clean and maps well to Rust traits

6. **Streaming**:
   - SSE writer is straightforward
   - The main complexity is in the stream lifecycle (tool registration, session sync, usage tracking)
   - Daemon fibers for background work -> `tokio::spawn`
   - Stream chunks are generic `unknown` -- need typed events in Rust

7. **Server**:
   - Simple HTTP server with 3 routes
   - Uses `ManagedRuntime` for Effect layer lifecycle
   - In Rust: `axum` or `actix-web` with dependency injection

8. **Usage Tracking**:
   - Multiple data shape support (V3, LanguageModel)
   - Deduplication via serialized payload comparison
   - Cost computation is trivial
   - Metering fallback chain is provider-specific

9. **Module Sizes for Prioritization**:
   - **Large/Complex**: `tools/registry.ts` (~1200 lines), `tools/codec.ts` (~470 lines), `runtime.ts` (~808 lines), `adapters/opencode/adapter.ts` (~373 lines), `adapters/codex-adapter.ts` (~315 lines)
   - **Medium**: `server/app.ts` (~300 lines), `session/sqlite-session-store.ts` (~390 lines), `usage/consumption.ts` (~250 lines), `client/runtime-client.ts` (~265 lines)
   - **Small/Straightforward**: All remaining modules <200 lines each

10. **Critical Invariants**:
    - Tool names must be provider-specific (different formats per family)
    - Session ID is a branded string (type safety)
    - Capabilities must be validated before streaming
    - Call-scoped tools must be cleaned up after stream completion (even on error)
    - Usage updates must be deduplicated (serialized comparison)
    - FiberFailure unwrapping is required at the Effect/Promise boundary

---

## Relevant Files (Absolute Paths)

### Core

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/runtime.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/runtime-async.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/index.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/package.json`

### Adapters

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/types.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/claude-code/index.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/claude-code/derived-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/opencode/adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/opencode/server-lifecycle.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/codex-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/vercel-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/openrouter-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/zai-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/bedrock-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/vertex-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/moonshot-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/minimax-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/ollama-adapter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/shared/build-stream-options.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/shared/error-mapping.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/shared/merge-settings.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/shared/resolve-extension.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/shared/resolve-model-id.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/adapters/shared/session-manager.ts`

### Capabilities

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/capabilities/capability-validator.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/types/capabilities.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/types/runtime-options.ts`

### Client

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/client/runtime-client.ts`

### Errors

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/adapter-errors.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/provider-errors.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/tool-errors.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/server-errors.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/session-errors.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/error-details.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/error-formatter.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/fiber-failure.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/runtime-error-type-id.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/errors/unions.ts`

### Models

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/models/model-cost.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/models/model-discovery-service.ts`

### Protocol

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/protocol/branded.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/protocol/provider-family.ts`

### Reasoning

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/reasoning/resolve-reasoning.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/reasoning/xhigh-detection.ts`

### Server

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/server/app.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/server/auth.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/server/http-utils.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/server/routes-models.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/server/sse-writer.ts`

### Services

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/services/layers.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/services/provider-registry.ts`

### Session

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/session/session-store.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/session/in-memory-store.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/session/sqlite-session-store.ts`

### Tools

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/tools/codec.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/tools/registry.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/tools/bridge.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/tools/compozy-tools.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/tools/types.ts`

### Usage

- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/types.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/native-event-utils.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/metadata-extractor.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/consumption.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/token-consumption.ts`
- `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/index.ts`
