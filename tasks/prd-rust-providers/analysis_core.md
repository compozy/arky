# Deep Analysis Report: `providers/core` Package

## 1. Overview

`@compozy/provider-core` is the foundational shared library for all Compozy AI provider implementations. It sits at the bottom of the provider dependency graph -- every concrete provider (`claude-code`, `codex`, `opencode`) and the `runtime` abstraction layer depend on it. The package contains **zero provider-specific logic**; instead, it provides the cross-cutting infrastructure that all providers need:

- **Tool execution and bridging** -- Adapting AI SDK tools into MCP-compatible tool calls with full lifecycle hook support.
- **Error classification and retry** -- Determining which tool errors are retryable and formatting actionable feedback for the AI agent.
- **Hook system** -- A provider-agnostic lifecycle hook framework (pre/post tool use, session, prompt, stop events).
- **MCP server creation** -- Converting AI SDK tool definitions into MCP SDK server instances.
- **Token consumption normalization** -- Resolving heterogeneous provider metadata into a unified token consumption format.

**Package identity:** `@compozy/provider-core` (v0.0.1, private, ESM)

**Key external dependencies:**
| Dependency | Role |
|---|---|
| `@ai-sdk/provider-utils` | Tool type definition, `executeTool`, `asSchema`, `getErrorMessage` |
| `@anthropic-ai/claude-agent-sdk` | `McpSdkServerConfigWithInstance`, `createSdkMcpServer`, `tool` (MCP tool factory) |
| `@modelcontextprotocol/sdk` | `StreamableHTTPServerTransport` for HTTP-based MCP servers |
| `zod` (v4.3.6) | Schema validation, JSON Schema to Zod conversion, error classification |
| `es-toolkit` | `merge`, `compact`, `cloneDeep`, `has`, `isPlainObject`, `isString`, `isEmpty` utilities |
| `@compozy/types` | Shared workspace types (not heavily used in core itself) |

---

## 2. Module-by-Module Analysis

### 2.1 `tool-provider.ts` -- Configuration Types

**Purpose:** Defines the configuration interfaces for tool providers. This is a pure type module with no runtime code.

**Key types:**

```typescript
interface RetryConfig {
  maxRetries?: number; // Default: 2
  nonRetryableErrorPatterns?: RegExp[]; // Blacklist patterns
  enableAutoRetry?: boolean; // Default: true
}

interface ToolProviderSettings {
  tools?: Record<string, boolean>; // Enable/disable specific tools
  toolTimeout?: number; // Execution timeout in ms
  retryConfig?: RetryConfig;
}

interface ToolProviderCapabilities {
  supportsCustomTools: boolean; // MCP bridge support
  builtinTools: string[]; // Provider-native tools
  supportsDynamicSchemas: boolean; // Runtime schema modification
}
```

**Rust port notes:** These are straightforward config structs. `RetryConfig` uses `RegExp[]` for patterns -- in Rust, use `Vec<regex::Regex>`.

---

### 2.2 `error-classifier.ts` -- Error Classification System

**Purpose:** Classifies tool execution errors to determine retryability and extracts structured error context for agent self-correction.

**Key exports:**

| Export                 | Kind      | Description                                                               |
| ---------------------- | --------- | ------------------------------------------------------------------------- | ---------------- | --------------- | --------- | --------- | ----------- | ------------ | ---------- |
| `ErrorClassifier`      | Class     | Main error classification engine                                          |
| `DEFAULT_RETRY_CONFIG` | Const     | `{ maxRetries: 2, nonRetryableErrorPatterns: [], enableAutoRetry: true }` |
| `ErrorContext`         | Interface | `{ message, details?, suggestions? }`                                     |
| `ErrorType`            | Type      | Union: `"validation"                                                      | "authentication" | "authorization" | "timeout" | "network" | "not_found" | "rate_limit" | "unknown"` |

**`ErrorClassifier` methods:**

1. **`isRetryable(error: unknown): boolean`** -- Blacklist approach. All errors default to retryable unless:
   - `enableAutoRetry` is false
   - Error has `isRetryable: false` property
   - Error is `LoadAPIKeyError` or `APICallError` with 401
   - Error message matches permanent patterns (`not found`, `permission denied`, `forbidden`, `unauthorized`)
   - Error message matches custom `nonRetryableErrorPatterns`
   - **Exception:** `ZodError` is ALWAYS retryable (checked first to avoid false positive pattern matches like "Invalid UUID" hitting `/invalid.*id/i`)

2. **`extractContext(error: unknown): ErrorContext`** -- Produces structured context:
   - For `ZodError`: Extracts field-level errors with paths, codes, expected/received types, and enum options
   - For generic errors: Extracts name, message, stack

3. **`formatForAgent(error, attemptNumber): string`** -- Combines context + retryability into a human-readable message for the AI agent, including numbered suggestions.

4. **`createEnhancedMessage(error): string`** -- Shorthand for `formatForAgent(error, 1)`.

**Pattern:** The classifier uses duck-typing for cross-realm error detection (checks `error.name`, `error.constructor.name`, `error.issues` array) rather than `instanceof` alone. This is critical because errors can cross module boundaries.

**Rust port notes:** This maps well to a Rust `ErrorClassifier` struct. The Zod-specific handling would need adaptation -- likely pattern-match on a Rust enum of error types. The duck-typing for ZodError detection translates to enum variants or trait-based classification.

---

### 2.3 `errorContext.ts` / `error-context.ts` -- Error Context Attachment

**Purpose:** `error-context.ts` is a re-export barrel. `errorContext.ts` contains the actual `attachErrorContext` function.

**Key function:**

```typescript
function attachErrorContext<T extends Error>(mappedError: T, sourceError: unknown): T;
```

Copies diagnostic context (stack, cause, data) from a source error onto a mapped provider error. Preserves the mapped error's type while enriching it with the original error's debugging information.

**Logic:**

1. If `sourceError` is not an `Error`, return `mappedError` unchanged.
2. Copy `stack` from source to mapped.
3. Copy `cause` (avoiding circular reference where `cause === sourceError`).
4. Build a `data` record with `cause` and `stack` if not already present.

**Rust port notes:** Rust errors don't have mutable stack/cause like JS. Instead, use `#[source]` attribute on error enums and the `Error::source()` chain. The "data" bag pattern would become structured error metadata fields.

---

### 2.4 `hooks.ts` -- Lifecycle Hook System

**Purpose:** A comprehensive, provider-agnostic hook system for intercepting and modifying tool execution, session events, and prompt submission. This is the largest module in the package (~813 lines).

**Hook Events (6 total):**
| Event | When | Can Block? | Can Modify? |
|---|---|---|---|
| `PreToolUse` | Before tool execution | Yes (deny/ask) | Yes (updatedInput) |
| `PostToolUse` | After tool execution | No | Yes (updatedMCPToolOutput, additionalContext) |
| `UserPromptSubmit` | When user submits prompt | Yes | Yes (updatedPrompt, systemMessages) |
| `SessionStart` | Session begins | No | Yes (env, systemMessages) |
| `SessionEnd` | Session ends | No | No (informational) |
| `Stop` | Agent wants to stop | Yes (block) | No |

**Hook Handler Types (4 variants):**
| Type | Description |
|---|---|
| `HookCallback` (function) | Direct async function `(input, toolUseId, {signal}) => Promise<HookJSONOutput \| void>` |
| `HookCallbackDescriptor` | Object wrapper: `{ type: "callback", hook: HookCallback, timeout?, timeoutMs? }` |
| `HookCommand` | Shell command: `{ type: "command", command: string, cwd?, env?, timeout? }` |
| `HookPrompt` | LLM prompt: `{ type: "prompt", prompt: string, model?, metadata?, timeout? }` |

**Key data structures:**

```typescript
type ProviderHooks = Partial<Record<HookEvent, HookCallbackMatcher[]>>;

interface HookCallbackMatcher {
  matcher?: string; // Regex pattern to match against tool name
  hooks: HookHandler[]; // Handlers to run when matched
  timeout?: number; // Seconds (Claude SDK compat)
  timeoutMs?: number; // Milliseconds (preferred)
}

type HookJSONOutput = AsyncHookJSONOutput | SyncHookJSONOutput;

type SyncHookJSONOutput = {
  continue?: boolean;
  suppressOutput?: boolean;
  stopReason?: string;
  decision?: "approve" | "block";
  systemMessage?: string;
  reason?: string;
  hookSpecificOutput?: HookSpecificOutput;
};
```

**Core execution function:**

```typescript
async function executeHookEvent(params: {
  hooks?: ProviderHooks;
  event: HookEvent;
  input: HookInput;
  matchTarget: string;
  options?: HookExecutionOptions;
}): Promise<HookExecutionResult>;
```

**Execution flow:**

1. Filter matchers by regex match against `matchTarget` (tool name for tool hooks)
2. Flatten all matching hooks into a task list with preserved insertion order
3. Execute all hooks in parallel via `Promise.allSettled`
4. Sort results back to original order
5. Collect outputs and errors separately

**Command hook execution (`runCommandHook`):**

- Spawns a child process with the command
- Pipes JSON input `{ input, event }` via stdin
- Captures stdout/stderr with 1MB output limit
- Handles stdin write errors (EPIPE, stream destroyed) gracefully
- Parses stdout as JSON, falls back to `{ continue: true, systemMessage: stdout }`

**Timeout handling (`createAbortSignal`):**

- Creates an AbortController with a timeout timer
- Supports parent signal propagation for cascading cancellation
- Cleanup function clears timer and removes parent listener

**Output extractors (5 functions):**
| Function | Returns | Key Behavior |
|---|---|---|
| `extractPreToolUseOutcome` | `{ blocked, reason?, updatedInput? }` | Merges updatedInput from multiple hooks |
| `extractPostToolUseUpdate` | `{ updatedOutput?, additionalContext[] }` | Collects context from system messages + hook-specific |
| `extractPromptUpdate` | `{ blocked, reason?, updatedPrompt?, systemMessages[] }` | Last updatedPrompt wins |
| `extractSessionStartUpdate` | `{ env?, systemMessages[] }` | Merges env from multiple hooks |
| `extractStopDecision` | `{ blocked, reason? }` | Any block wins |

**`normalizeHooks` function:**
Converts `ProviderHooks` (mixed handler types) into `HookCallbackMap` (pure callbacks). Wraps command/prompt hooks in async callbacks that catch errors and fail-open (`{ continue: true }`).

**Rust port notes:** This is the most complex module to port. Key challenges:

- The `spawn` child process model maps to `tokio::process::Command`
- `Promise.allSettled` maps to `futures::join_all` with per-task error catching
- The hook output union type needs a Rust enum with careful deserialization
- The `normalizeHooks` pattern (wrapping diverse handlers into uniform callbacks) maps well to trait objects or `Box<dyn Fn>` closures
- AbortSignal/AbortController maps to `tokio::sync::CancellationToken`

---

### 2.5 `tools-bridge.ts` -- Tool Execution Bridge

**Purpose:** The central nervous system of tool execution. Bridges AI SDK tool definitions into MCP-compatible tool calls with full hook lifecycle, error classification, output truncation, and bound-tool patterns.

**Key exports:**

| Export                  | Kind      | Description                                                        |
| ----------------------- | --------- | ------------------------------------------------------------------ |
| `createToolExecutor`    | Function  | Creates a `ToolExecutor` instance from config                      |
| `bindTools`             | Function  | Creates bound tools with execute handlers                          |
| `listToolsFromEntries`  | Function  | Lists tool metadata (name, description, schema)                    |
| `MinimalCallToolResult` | Interface | MCP-compatible tool result                                         |
| `ToolExecutionContext`  | Interface | Per-call context (toolCallId, abortSignal, session info, hook env) |
| `ToolsBridgeConfig`     | Interface | Configuration for tool bridge                                      |
| `ToolExecutor`          | Type      | Full executor interface                                            |
| `BoundTools`            | Type      | Tools with guaranteed `execute` handlers                           |

**`MinimalCallToolResult` (MCP-compatible):**

```typescript
interface MinimalCallToolResult {
  content: Array<{ type: "text"; text: string }>;
  structuredContent?: Record<string, unknown>;
  isError?: boolean; // Top-level error flag (MCP spec)
  _meta?: Record<string, unknown>;
}
```

**`ToolExecutionContext`:**

```typescript
interface ToolExecutionContext {
  toolCallId: string;
  abortSignal?: AbortSignal;
  messages?: ToolCallOptions["messages"];
  experimentalContext?: ToolCallOptions["experimental_context"];
  sessionId?: string;
  messageId?: string;
  agent?: string;
  cwd?: string;
  transcriptPath?: string;
  permissionMode?: string;
  hookEnv?: Record<string, string>;
}
```

**`ToolsBridgeConfig`:**

```typescript
interface ToolsBridgeConfig<TTools> {
  tools: TTools;
  serverName?: string;
  defaultTimeout?: number;
  retryConfig?: RetryConfig;
  onError?: (error: ToolExecutionError) => void;
  hooks?: ProviderHooks;
  hookOptions?: HookExecutionOptions;
  baseDirectory?: string;
  outputLimits?: Partial<ToolOutputLimits>;
  logger?: { warn; info };
}
```

**`ToolExecutor` interface:**

```typescript
type ToolExecutor<TTools> = {
  tools: () => Promise<BoundTools<TTools>>;
  callTool: (toolName, args, context?) => Promise<MinimalCallToolResult>;
  execute: (params) => Promise<MinimalCallToolResult>;
  listTools: () => ToolsList;
  close: () => void;
  getToolEntries: () => ToolEntry[];
  isClosed: () => boolean;
};
```

**Execution flow in `createToolExecutor.execute()`:**

1. Resolve tool call ID from context or extra metadata (supports multiple naming conventions: `toolCallId`, `tool_use_id`, `toolUseID`, `toolUseId`, `_meta.toolCallId`)
2. Build hook base input from context
3. **Pre-tool hook phase:** Run `executeHookEvent("PreToolUse")` -> check for errors/blocking
4. Apply `updatedInput` from hooks if present
5. **Tool execution:** Call `resolveToolOutput` which iterates over `executeTool` generator chunks
6. **Post-tool hook phase:** Run `executeHookEvent("PostToolUse")` -> apply output updates
7. **Output truncation:** Apply `ToolOutputLimits` to prevent JSON parsing failures
8. Return `MinimalCallToolResult` with content, structuredContent, and metadata

**Output truncation (`truncateToolOutput`):**

- Default limits: 100KB max, 50KB warn, truncation disabled by default
- Smart truncation for arrays (removes elements from end, adds `_truncated` marker)
- Smart truncation for objects (adds `_truncated` metadata field)
- UTF-8 safe byte-level truncation as fallback
- Truncation metadata in `_meta.truncated`

**`bindTools` function:**
Creates tools with injected `execute` handlers that route through `callTool`. Preserves `inputSchema` separately from the merge to maintain Symbol properties (AI SDK's `asSchema` checks for `Symbol.for("vercel.ai.schema")`).

**`listToolsFromEntries` function:**
Enumerates tools with metadata, deep-cloning JSON schemas to prevent mutation between calls.

**Rust port notes:**

- The generic `Tool<any, any>` type needs a Rust trait or enum representation
- `executeTool` generator iteration maps to Rust async streams
- The Symbol-based schema detection is JS-specific; Rust would use a schema enum
- Output truncation logic maps directly to Rust with `String::len()` for bytes
- The `close()` / `isClosed()` lifecycle maps to `Drop` trait or explicit close

---

### 2.6 `mcp-server.ts` -- MCP Server from Tools

**Purpose:** Converts AI SDK tool definitions into an MCP SDK server instance with Zod schema conversion.

**Key function:**

```typescript
function createMcpServerFromTools({
  name: string,
  entries: Array<[string, Tool]>,
  execute: ExecuteTool,
}): McpSdkServerConfigWithInstance
```

**Internal helpers:**

1. **`jsonSchemaTypeToZod`** -- Recursive converter from JSON Schema types to Zod types. Handles: `string` (with uuid/email/url formats), `number`, `integer`, `boolean`, `array` (with item schemas), `object` (with nested properties and required tracking), and `unknown` fallback.

2. **`getInputSchema`** -- Extracts the input schema from a tool definition, handling three cases:
   - Raw `ZodObject` instance
   - AI SDK wrapped schema (detected via `Symbol.for("vercel.ai.schema")`)
   - Fallback: `z.object({}).passthrough()`

3. Uses `@anthropic-ai/claude-agent-sdk`'s `createSdkMcpServer` and `tool` to build the final MCP server.

**Rust port notes:** The JSON Schema to Zod conversion is entirely JS-specific. In Rust, the MCP server would accept JSON Schema directly (the MCP protocol uses JSON Schema natively). The `createSdkMcpServer` dependency would be replaced by a Rust MCP SDK implementation. The `Symbol.for("vercel.ai.schema")` detection is irrelevant in Rust.

---

### 2.7 `mcp-http-server.ts` -- HTTP Transport for MCP

**Purpose:** Wraps an MCP SDK server instance in an HTTP server using `StreamableHTTPServerTransport`.

**Key function:**

```typescript
async function createMcpHttpServer({
  mcpServer: McpSdkServerConfigWithInstance,
  host?: string,       // Default: "127.0.0.1"
  port?: number,       // Default: 0 (random)
  path?: string,       // Default: "/mcp"
  sessionIdGenerator?: () => string,
}): Promise<McpHttpServerInfo>
```

**Returns:**

```typescript
type McpHttpServerInfo = {
  url: string; // e.g., "http://127.0.0.1:54321/mcp"
  server: Server; // Node.js HTTP server
  close: () => Promise<void>;
};
```

**Close behavior:**

1. Close the MCP transport
2. End all tracked sockets gracefully
3. Set a 250ms force-destroy timeout (unref'd to not block process exit)
4. Close the HTTP server
5. Clear the timeout and destroy remaining sockets

**Rust port notes:** This maps to an `axum` or `hyper` HTTP server wrapping an MCP transport. Socket tracking for graceful shutdown is a common Rust pattern with `tokio::net::TcpListener`. The `unref` timeout pattern maps to `tokio::select!` with a timeout branch.

---

### 2.8 `token-consumption.ts` -- Token Usage Normalization

**Purpose:** Normalizes token usage metadata across different AI providers into a unified format. Handles provider aliasing (e.g., `zai`, `bedrock`, `vertex` all map to `claude-code` family).

**Provider registry:**

```typescript
const CLAUDE_BASED_PROVIDER_IDS = [
  "claude-code",
  "zai",
  "openrouter",
  "vercel",
  "moonshot",
  "minimax",
  "bedrock",
  "vertex",
  "ollama",
] as const;

const RUNTIME_PROVIDER_IDS = [...CLAUDE_BASED_PROVIDER_IDS, "codex", "opencode"] as const;
```

**Provider families:** `"claude-code" | "codex" | "opencode" | "unknown"`

**Key types:**

```typescript
type NormalizedTokenBreakdown = {
  inputTotal?: number;
  inputNoCache?: number;
  inputCacheRead?: number;
  inputCacheWrite?: number;
  outputTotal?: number;
  outputText?: number;
  outputReasoning?: number;
  total?: number;
};

type NormalizedTokenConsumption = {
  providerId: string;
  providerFamily: ProviderFamilyId;
  metadataKeyUsed?: string;
  tokens: NormalizedTokenBreakdown;
  costUsd?: number;
  durationMs?: number;
  rawUsage?: Record<string, unknown>;
};
```

**Metadata key fallback system:** Each provider has an ordered list of keys to try when resolving metadata from the AI SDK's `providerMetadata` bag. E.g., `zai` tries `["zai", "claude-code"]`, `codex` tries `["codex", "codex-cli"]`.

**Key functions:**
| Function | Description |
|---|---|
| `isRuntimeProviderId(id)` | Type guard for known provider IDs |
| `resolveProviderFamily(id)` | Maps any provider ID to its family |
| `getProviderMetadataKeys(id)` | Returns fallback key chain for a provider |
| `resolveProviderMetadata(metadata, id)` | Resolves provider-specific data from metadata bag |
| `computeTokenTotal(input, output)` | Safe addition with undefined handling |

**Rust port notes:** This module maps cleanly to Rust. Provider IDs become an enum, the fallback key system becomes a `match` or `HashMap`, and `NormalizedTokenBreakdown` / `NormalizedTokenConsumption` become plain structs with `Option<>` fields.

---

## 3. Cross-Cutting Patterns

### 3.1 Error Handling Philosophy

The package uses a **fail-open** approach for hooks (errors are logged but don't block execution) and a **blacklist** approach for retryability (everything is retryable unless explicitly excluded). This is deliberate -- in an AI agent context, it's better to let the agent retry and self-correct than to permanently fail.

### 3.2 Duck-Typing for Cross-Realm Errors

Multiple places check `error.name`, `error.constructor.name`, and structural properties rather than relying solely on `instanceof`. This handles errors that cross module boundaries where `instanceof` would fail.

### 3.3 Deep Merge with Symbol Preservation

The `bindTools` function carefully separates `inputSchema` before merging to preserve Symbol properties that `es-toolkit`'s `merge` would strip. This is a JS-specific concern that won't exist in Rust.

### 3.4 Timeout/Cancellation Architecture

The hook system implements its own timeout + parent signal propagation via `createAbortSignal`. This is a pattern that maps well to Rust's `tokio::select!` with `CancellationToken`.

### 3.5 Parallel Execution with Order Preservation

Hooks execute in parallel (`Promise.allSettled`) but results are re-sorted by original insertion index. This ensures deterministic output ordering while maximizing throughput.

---

## 4. Dependency Graph (Intra-package)

```
index.ts
  +-- error-classifier.ts  <-- tool-provider.ts (RetryConfig type)
  +-- error-context.ts      <-- errorContext.ts (re-export)
  +-- hooks.ts              (standalone, no internal deps)
  +-- mcp-http-server.ts    (standalone, external deps only)
  +-- mcp-server.ts         <-- tools-bridge.ts (MinimalCallToolResult, ToolExecutionContext)
  +-- token-consumption.ts  (standalone)
  +-- tool-provider.ts      (standalone types)
  +-- tools-bridge.ts       <-- error-classifier.ts, hooks.ts, tool-provider.ts
```

**Central hub:** `tools-bridge.ts` is the primary integration point, depending on `error-classifier.ts`, `hooks.ts`, and `tool-provider.ts`.

---

## 5. Consumer Usage Patterns

### 5.1 providers/claude-code

- Uses `createToolExecutor`, `createMcpServerFromTools`, `bindTools`, `listToolsFromEntries` to create a `ToolsBridge` wrapper
- Adds a registry pattern for tracking bridge instances by UUID
- Re-exports core types

### 5.2 providers/opencode

- Uses hooks system extensively (`HookCallbackDescriptor`, `HookInput`, `ProviderHooks`, `executeHookEvent`, extractors)
- Has its own `hook-runners.ts` and `hook-service.ts` that build on the core hook system
- Uses `ToolsBridgeConfig` and `createToolExecutor` for tool integration

### 5.3 providers/codex

- Uses `ToolsBridgeConfig` type and `computeTokenTotal`
- Has its own `CodexToolsBridge` that wraps core functionality

### 5.4 providers/runtime

- Uses `resolveProviderMetadata`, `computeTokenTotal`, `NormalizedTokenConsumption` for token tracking
- Uses `ProviderHooks`, `HookExecutionOptions` for hook configuration passthrough

---

## 6. Rust Port Considerations

### 6.1 Maps Well to Rust

| Component                       | Rust Approach                                                |
| ------------------------------- | ------------------------------------------------------------ |
| `ErrorClassifier`               | Struct with `is_retryable(&self, error: &dyn Error) -> bool` |
| `RetryConfig`                   | Struct with `Vec<Regex>` for patterns                        |
| `NormalizedTokenConsumption`    | Struct with `Option<>` fields                                |
| Provider ID system              | Enum with `impl From<&str>`                                  |
| `ToolOutputLimits` / truncation | Direct port with `String::len()`                             |
| Hook output extractors          | Functions taking `&[HookOutput]`                             |
| `ToolExecutor` lifecycle        | Struct with `Arc<Mutex<>>` for closed state                  |

### 6.2 Needs Adaptation

| Component                          | Challenge                               | Rust Approach                                                        |
| ---------------------------------- | --------------------------------------- | -------------------------------------------------------------------- |
| Hooks system                       | `child_process.spawn` + stdin/stdout    | `tokio::process::Command` with async stdin write                     |
| `AbortSignal`                      | No direct equivalent                    | `tokio_util::sync::CancellationToken`                                |
| `Promise.allSettled`               | Parallel execution with error isolation | `futures::join_all` wrapping each future in `catch_unwind` or Result |
| `ZodError` detection               | Duck-typing across realms               | Rust enum `ToolError::Validation { fields }`                         |
| `Symbol.for("vercel.ai.schema")`   | JS-only schema wrapping                 | Not needed; use JSON Schema directly                                 |
| AI SDK `Tool` type                 | Generic with `execute` closure          | Trait `ToolHandler` with `async fn execute`                          |
| `merge()` with Symbol preservation | JS prototype chain concerns             | Not needed in Rust                                                   |
| `HookHandler` union type           | 4 handler variants                      | `enum HookHandler { Callback(..), Command(..), Prompt(..) }`         |

### 6.3 Key Design Decisions for Rust

1. **Tool interface:** Define a `ToolHandler` trait with `async fn execute(&self, args: Value) -> Result<Value>` rather than the JS generic `Tool<INPUT, OUTPUT>`.
2. **Hook system:** Use `enum` for handler types and `trait` for the callback interface. The `normalizeHooks` pattern becomes unnecessary if all handlers implement the same trait.
3. **Error classification:** Create a `ToolError` enum with variants that carry structured data, replacing the duck-typing approach.
4. **MCP integration:** Use the Rust MCP SDK directly rather than the JS bridge pattern.
5. **Output truncation:** Port directly -- the byte-level UTF-8 safe truncation is well-suited for Rust's `String`/`&str`.

---

## 7. Relevant Files

| File                                                                                          | Purpose                                                     |
| --------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/index.ts`                      | Package exports (83 lines)                                  |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/tool-provider.ts`              | RetryConfig, ToolProviderSettings, ToolProviderCapabilities |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/error-classifier.ts`           | ErrorClassifier class (262 lines)                           |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/errorContext.ts`               | attachErrorContext function (42 lines)                      |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/error-context.ts`              | Re-export barrel                                            |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/hooks.ts`                      | Full hook system (813 lines)                                |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/tools-bridge.ts`               | Tool execution bridge (737 lines)                           |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/mcp-server.ts`                 | MCP server creation (152 lines)                             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/mcp-http-server.ts`            | HTTP transport (80 lines)                                   |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/src/token-consumption.ts`          | Token normalization (103 lines)                             |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/core/package.json`                      | Package configuration                                       |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/claude-code/src/tools/bridge.ts`        | Consumer: claude-code bridge wrapper                        |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/types/runtime-options.ts`   | Consumer: runtime options using core types                  |
| `/Users/pedronauck/Dev/compozy/compozy-code/providers/runtime/src/usage/token-consumption.ts` | Consumer: token consumption resolution                      |
