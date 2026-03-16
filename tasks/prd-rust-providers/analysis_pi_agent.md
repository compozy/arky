# Deep Analysis Report: Pi Agent & AI Packages

## 1. Context Summary

This report provides a comprehensive analysis of the Pi monorepo's `packages/agent` and `packages/ai` packages. Pi (by Mario Zechner, `@mariozechner`) is a full-stack AI agent framework built in TypeScript that provides a unified multi-provider LLM API and an agent runtime with tool calling and state management.

The two packages analyzed:

- **`@mariozechner/pi-ai`** -- Unified multi-provider LLM streaming API
- **`@mariozechner/pi-agent-core`** -- Agent runtime with tool execution, event system, and state management

---

## 2. Agent Architecture (`packages/agent`)

### 2.1 Agent Class (`agent.ts`)

The `Agent` class is the primary public API. It is a self-contained unit that owns:

- **State** (`AgentState`): system prompt, model, thinking level, tools, messages, streaming status, pending tool calls, and error state
- **Event system**: Simple pub/sub with `subscribe(fn)` returning an unsubscribe function
- **Abort control**: Per-prompt `AbortController` with `abort()` and `waitForIdle()` methods
- **Queuing**: Steering and follow-up message queues for mid-run and post-run intervention
- **Stream function**: Configurable `streamFn` (defaults to `streamSimple` from the AI package)

**Key design decisions:**

1. **No transport abstraction** -- The Agent calls `streamSimple` directly or accepts a custom `StreamFn`. There is no intermediate "transport layer" between the agent and the LLM.
2. **Message conversion at the boundary** -- The agent works with `AgentMessage[]` internally and only converts to `Message[]` (LLM-compatible) at the `streamAssistantResponse` call site via `convertToLlm`.
3. **Single-flight guarantee** -- Calling `prompt()` while streaming throws. Concurrent messages must use `steer()` or `followUp()`.
4. **Configurable tool execution** -- Supports both `"sequential"` and `"parallel"` tool execution modes.
5. **Lifecycle hooks** -- `beforeToolCall` and `afterToolCall` allow blocking, modifying, or replacing tool results.

```
AgentOptions {
  initialState?: Partial<AgentState>
  convertToLlm?: (messages: AgentMessage[]) => Message[]
  transformContext?: (messages: AgentMessage[], signal?) => Promise<AgentMessage[]>
  steeringMode?: "all" | "one-at-a-time"
  followUpMode?: "all" | "one-at-a-time"
  streamFn?: StreamFn
  sessionId?: string
  getApiKey?: (provider: string) => Promise<string | undefined>
  onPayload?: SimpleStreamOptions["onPayload"]
  thinkingBudgets?: ThinkingBudgets
  transport?: Transport
  maxRetryDelayMs?: number
  toolExecution?: ToolExecutionMode
  beforeToolCall?: (context, signal?) => Promise<BeforeToolCallResult | undefined>
  afterToolCall?: (context, signal?) => Promise<AfterToolCallResult | undefined>
}
```

### 2.2 Agent Loop (`agent-loop.ts`)

The agent loop is the core execution engine, separated from the Agent class for reusability. It can be used independently via `runAgentLoop()` / `runAgentLoopContinue()` or through the streaming `agentLoop()` / `agentLoopContinue()` wrappers.

**Two-level loop structure:**

```
OUTER LOOP (follow-up messages):
  while true:
    INNER LOOP (tool calls + steering):
      while hasMoreToolCalls || pendingMessages:
        1. Process pending messages (steering/follow-up)
        2. Stream assistant response (LLM call)
        3. If error/aborted -> emit agent_end, return
        4. Execute tool calls (sequential or parallel)
        5. Check for steering messages -> skip remaining tools if found
        6. Emit turn_end

    Check for follow-up messages
    If found -> continue outer loop
    Else -> break

  emit agent_end
```

**Key functions:**

- `streamAssistantResponse()` -- Applies `transformContext`, then `convertToLlm`, builds `Context` for the LLM, calls `streamFn`, and processes the streaming event protocol into `AgentEvent`s
- `executeToolCalls()` -- Dispatches to sequential or parallel execution
- `executeToolCallsSequential()` -- Prepare-execute-finalize one at a time, checking steering between each
- `executeToolCallsParallel()` -- Prepare all sequentially (for beforeToolCall gating), then execute concurrently, finalize in order
- `prepareToolCall()` -- Finds tool, validates args, runs `beforeToolCall` hook. Returns either "immediate" (blocked/error) or "prepared" (ready to execute)
- `executePreparedToolCall()` -- Runs `tool.execute()` with progress callback
- `finalizeExecutedToolCall()` -- Runs `afterToolCall` hook, emits result events

### 2.3 Steering & Follow-up System

This is a distinctive Pi feature:

- **Steering messages** (`steer()`) -- Injected mid-run, after current tool completes. When steering arrives during tool execution, remaining tool calls are **skipped** (marked as "Skipped due to queued user message"). This enables the user to redirect the agent while it's working.
- **Follow-up messages** (`followUp()`) -- Delivered only after the agent would otherwise stop (no more tool calls, no steering). This enables chaining prompts.
- Both support `"all"` (batch) and `"one-at-a-time"` (sequential) modes.

### 2.4 Proxy (`proxy.ts`)

The `streamProxy` function is an alternative `StreamFn` that routes LLM calls through a proxy server. It:

- Sends `POST` to `${proxyUrl}/api/stream` with auth token
- Receives SSE events in a bandwidth-optimized format (no `partial` field)
- Reconstructs the partial `AssistantMessage` client-side from delta events

This demonstrates Pi's approach: instead of building a transport layer, it lets you swap the stream function.

---

## 3. Event System

### 3.1 AgentEvent Types

```typescript
type AgentEvent =
  // Agent lifecycle
  | { type: "agent_start" }
  | { type: "agent_end"; messages: AgentMessage[] }
  // Turn lifecycle
  | { type: "turn_start" }
  | { type: "turn_end"; message: AgentMessage; toolResults: ToolResultMessage[] }
  // Message lifecycle
  | { type: "message_start"; message: AgentMessage }
  | { type: "message_update"; message: AgentMessage; assistantMessageEvent: AssistantMessageEvent }
  | { type: "message_end"; message: AgentMessage }
  // Tool execution lifecycle
  | { type: "tool_execution_start"; toolCallId: string; toolName: string; args: any }
  | {
      type: "tool_execution_update";
      toolCallId: string;
      toolName: string;
      args: any;
      partialResult: any;
    }
  | {
      type: "tool_execution_end";
      toolCallId: string;
      toolName: string;
      result: any;
      isError: boolean;
    };
```

**Event flow for a typical prompt:**

```
agent_start
  turn_start
    message_start (user message)
    message_end (user message)
    message_start (assistant streaming begins)
    message_update* (streaming deltas)
    message_end (assistant message complete)
    tool_execution_start (per tool)
    tool_execution_update* (progress)
    tool_execution_end (per tool)
    message_start (tool result)
    message_end (tool result)
  turn_end
  turn_start (if more tool calls or steering)
    ...
  turn_end
agent_end
```

### 3.2 Subscription Model

Simple synchronous listener pattern:

```typescript
subscribe(fn: (e: AgentEvent) => void): () => void
```

The Agent class processes events in `_processLoopEvent()` which:

1. Updates internal state (streamMessage, pendingToolCalls, error, isStreaming)
2. Emits to all listeners via `emit()`

This is deliberately NOT an async event system -- events are dispatched synchronously to avoid ordering issues.

---

## 4. Tool System

### 4.1 AgentTool Interface

```typescript
interface AgentTool<
  TParameters extends TSchema = TSchema,
  TDetails = any,
> extends Tool<TParameters> {
  label: string; // Human-readable label for UI
  execute: (
    toolCallId: string,
    params: Static<TParameters>,
    signal?: AbortSignal,
    onUpdate?: AgentToolUpdateCallback<TDetails>
  ) => Promise<AgentToolResult<TDetails>>;
}

interface AgentToolResult<T> {
  content: (TextContent | ImageContent)[];
  details: T; // Structured data for UI display
}
```

Key aspects:

- **TypeBox schemas** for parameter validation (via AJV)
- **Typed details** -- Tools return both LLM-consumable `content` and typed `details` for UI rendering
- **Progress callbacks** -- `onUpdate` enables streaming tool execution updates
- **AbortSignal** -- Every tool receives the abort signal for cancellation

### 4.2 Tool Execution Modes

**Sequential:** Prepare -> execute -> finalize, one at a time. Check steering between each tool.

**Parallel:** Prepare all sequentially (important: `beforeToolCall` runs sequentially even in parallel mode, enabling gating). Then execute all prepared tools concurrently. Finalize in original order. Check steering after all complete.

### 4.3 beforeToolCall / afterToolCall Hooks

- **beforeToolCall**: Can block execution with `{ block: true, reason: "..." }`. Used for permission systems, rate limiting, etc.
- **afterToolCall**: Can override `content`, `details`, or `isError` of the result. Used for result sanitization, logging, etc.

---

## 5. Message System

### 5.1 AgentMessage and Custom Messages

```typescript
// Extensible via declaration merging
interface CustomAgentMessages {
  // Empty by default -- apps add entries here
}

type AgentMessage = Message | CustomAgentMessages[keyof CustomAgentMessages];
```

This allows apps to define custom message types (artifacts, notifications, status updates) that flow through the agent's message history but are filtered out by `convertToLlm` before reaching the LLM.

Example from Pi's coding agent:

```typescript
declare module "@mariozechner/agent" {
  interface CustomAgentMessages {
    artifact: ArtifactMessage;
    notification: NotificationMessage;
  }
}
```

### 5.2 Message Flow

```
AgentMessage[] (includes custom types)
    |
    v
transformContext() -- prune, inject context
    |
    v
AgentMessage[] (still includes custom types)
    |
    v
convertToLlm() -- filter to LLM-compatible messages
    |
    v
Message[] (UserMessage | AssistantMessage | ToolResultMessage)
    |
    v
LLM provider call
```

---

## 6. AI Package (`packages/ai`)

### 6.1 Architecture Overview

The AI package provides a unified streaming API across 10+ LLM providers. Its architecture centers on:

1. **API Registry** -- A global `Map<string, RegisteredApiProvider>` that maps API identifiers to provider implementations
2. **Model Registry** -- Generated models with full metadata (pricing, context window, capabilities)
3. **Unified Event Protocol** -- `AssistantMessageEvent` stream that all providers must emit
4. **EventStream** -- Push-based async iterable with completion semantics

### 6.2 API Registry (`api-registry.ts`)

```typescript
interface ApiProvider<TApi, TOptions> {
  api: TApi;
  stream: StreamFunction<TApi, TOptions>; // Provider-specific options
  streamSimple: StreamFunction<TApi, SimpleStreamOptions>; // Unified options
}
```

Functions:

- `registerApiProvider(provider, sourceId?)` -- Register with optional source ID for batch removal
- `getApiProvider(api)` -- Look up by API identifier
- `unregisterApiProviders(sourceId)` -- Remove all providers from a source
- `clearApiProviders()` / `resetApiProviders()` -- Reset to built-in providers

**Design insight:** Every provider exposes two entry points:

1. `stream()` -- Takes provider-specific options (e.g., `AnthropicOptions` with `thinkingEnabled`, `effort`, etc.)
2. `streamSimple()` -- Takes unified `SimpleStreamOptions` and maps to provider-specific options

This dual-entry design lets power users access provider-specific features while providing a simple unified API for common use cases.

### 6.3 Stream Entry Points (`stream.ts`)

```typescript
export function stream(model, context, options?); // Provider-specific options
export function streamSimple(model, context, options?); // Unified options
export async function complete(model, context, options?); // Blocking
export async function completeSimple(model, context, options?); // Blocking
```

`streamSimple` is what the agent loop uses by default.

### 6.4 EventStream (`utils/event-stream.ts`)

A generic push-based async iterable:

```typescript
class EventStream<T, R = T> implements AsyncIterable<T> {
  constructor(
    isComplete: (event: T) => boolean, // Detect terminal events
    extractResult: (event: T) => R // Extract final result
  );
  push(event: T): void; // Producer pushes events
  end(result?: R): void; // Signal completion
  result(): Promise<R>; // Await final result
  [Symbol.asyncIterator](); // Consume as async iterator
}
```

The `AssistantMessageEventStream` specialization knows that `done` and `error` events are terminal.

**Key design:** The EventStream uses a queue + waiting array pattern. If consumers are slower than producers, events queue up. If consumers are faster, they wait on promises. This avoids backpressure complexity while ensuring no events are lost.

### 6.5 AssistantMessageEvent Protocol

All providers must emit events in this protocol:

```
start -> [text_start, text_delta*, text_end]*
      -> [thinking_start, thinking_delta*, thinking_end]*
      -> [toolcall_start, toolcall_delta*, toolcall_end]*
      -> done | error
```

Every delta event carries the full `partial: AssistantMessage` which is mutated in place. This means consumers always have access to the complete message state at any point during streaming.

### 6.6 Model System (`models.ts`)

```typescript
interface Model<TApi extends Api> {
  id: string;
  name: string;
  api: TApi;
  provider: Provider;
  baseUrl: string;
  reasoning: boolean;
  input: ("text" | "image")[];
  cost: { input; output; cacheRead; cacheWrite }; // $/million tokens
  contextWindow: number;
  maxTokens: number;
  headers?: Record<string, string>;
  compat?: OpenAICompletionsCompat | OpenAIResponsesCompat;
}
```

Models are generated at build time (`models.generated.ts`) and loaded into a `Map<string, Map<string, Model>>` (provider -> modelId -> Model). This provides:

- Type-safe model lookup: `getModel("anthropic", "claude-opus-4-6")`
- Cost calculation: `calculateCost(model, usage)`
- Provider enumeration: `getProviders()`, `getModels(provider)`

---

## 7. Provider Implementations

### 7.1 Registration System (`register-builtins.ts`)

All built-in providers are registered in a single file that runs on module load:

```typescript
registerBuiltInApiProviders(); // Called at module load
```

This registers 10 API providers:

- `anthropic-messages` (Anthropic, GitHub Copilot via Anthropic API)
- `openai-completions` (OpenAI Chat, Groq, xAI, OpenRouter, etc.)
- `openai-responses` (OpenAI Responses API)
- `openai-codex-responses` (OpenAI Codex)
- `azure-openai-responses` (Azure OpenAI)
- `google-generative-ai` (Google Gemini)
- `google-gemini-cli` (Google Gemini CLI variant)
- `google-vertex` (Google Vertex AI)
- `mistral-conversations` (Mistral)
- `bedrock-converse-stream` (Amazon Bedrock, lazy-loaded)

**Bedrock is lazy-loaded** to avoid pulling in the AWS SDK for users who don't need it. This is done by dynamically importing the module and forwarding events.

### 7.2 Provider Implementation Pattern

Every provider follows the same pattern (Anthropic as example):

```typescript
// 1. Define provider-specific options
export interface AnthropicOptions extends StreamOptions {
  thinkingEnabled?: boolean;
  thinkingBudgetTokens?: number;
  effort?: AnthropicEffort;
  interleavedThinking?: boolean;
  toolChoice?: "auto" | "any" | "none" | { type: "tool"; name: string };
}

// 2. Export the raw stream function
export const streamAnthropic: StreamFunction<"anthropic-messages", AnthropicOptions> = (
  model,
  context,
  options?
) => {
  const stream = new AssistantMessageEventStream();
  (async () => {
    // Build output AssistantMessage
    // Create provider client
    // Build provider-specific params
    // Call provider API
    // Process provider events -> emit AssistantMessageEvent
    // Handle errors -> emit error event
  })();
  return stream; // Return immediately, events push async
};

// 3. Export the simple stream function
export const streamSimpleAnthropic: StreamFunction<"anthropic-messages", SimpleStreamOptions> = (
  model,
  context,
  options?
) => {
  // Map SimpleStreamOptions -> AnthropicOptions
  // Call streamAnthropic with mapped options
};
```

**Key patterns across all providers:**

1. **Immediate return, async processing** -- The stream function returns an `AssistantMessageEventStream` immediately. The actual API call happens in an async IIFE.
2. **Partial message mutation** -- The `output` `AssistantMessage` is mutated in place as events arrive. Each event carries a reference to this same object.
3. **Error normalization** -- All provider errors are caught and converted to `{ type: "error", reason, error: AssistantMessage }` events.
4. **Usage tracking** -- Every provider extracts usage (input/output/cache tokens) and computes cost via `calculateCost()`.

### 7.3 Cross-Provider Compatibility (`transform-messages.ts`)

The `transformMessages` function handles cross-provider message replay:

- **Thinking blocks**: Keep for same model, convert to text for different model, drop redacted blocks for different model
- **Tool call IDs**: Normalize (e.g., OpenAI 450-char IDs -> Anthropic 64-char max)
- **Orphaned tool calls**: Insert synthetic "No result provided" tool results
- **Errored/aborted messages**: Skip entirely to prevent API errors on replay

### 7.4 Notable Provider Features

**Anthropic (`anthropic.ts`):**

- "Stealth mode" -- Can mimic Claude Code's tool naming and identity headers when using OAuth tokens
- Adaptive thinking support for Opus 4.6 / Sonnet 4.6 (effort levels)
- Budget-based thinking for older models
- Cache control with retention preferences ("none", "short", "long")
- Handles redacted thinking blocks (safety filters)

**OpenAI Responses (`openai-responses.ts`):**

- Reasoning effort levels mapping
- Reasoning summary modes (auto/detailed/concise)
- Service tier selection
- Prompt cache retention for api.openai.com

**Google (`google.ts`):**

- Thinking budget configuration
- Thought signature retention for multi-turn
- Tool choice mapping

### 7.5 Compatibility Layer

The `OpenAICompletionsCompat` interface handles the wide variety of OpenAI-compatible APIs:

```typescript
interface OpenAICompletionsCompat {
  supportsStore?: boolean;
  supportsDeveloperRole?: boolean;
  supportsReasoningEffort?: boolean;
  reasoningEffortMap?: Partial<Record<ThinkingLevel, string>>;
  supportsUsageInStreaming?: boolean;
  maxTokensField?: "max_completion_tokens" | "max_tokens";
  requiresToolResultName?: boolean;
  requiresAssistantAfterToolResult?: boolean;
  requiresThinkingAsText?: boolean;
  thinkingFormat?: "openai" | "zai" | "qwen" | "qwen-chat-template";
  supportsStrictMode?: boolean;
}
```

This is embedded in the `Model` type and auto-detected from `baseUrl`, enabling a single `openai-completions` provider to handle OpenAI, Groq, xAI, OpenRouter, Cerebras, HuggingFace, and many more.

---

## 8. Key Design Decisions

### 8.1 Agent-Centric vs. Provider-Centric

Pi's approach is **agent-centric**: The `Agent` class is the primary abstraction. It owns state, events, tools, and message history. Providers are interchangeable backends accessed through a registry.

This contrasts with Compozy's current approach which is more **runtime-centric**: The runtime/SDK handles orchestration, and providers are more prominent abstractions.

### 8.2 No Intermediate Layers

Pi deliberately avoids intermediate abstractions:

- No "transport layer" between agent and LLM
- No "middleware chain" for request/response processing
- No "adapter pattern" for tool compatibility
- The `transformContext` -> `convertToLlm` pipeline is the ONLY processing between the agent's message history and the LLM call

This keeps the code path short and debuggable.

### 8.3 Push-Based Streaming

The `EventStream` is push-based (producer pushes events, consumers pull via async iterator). This is simpler than pull-based streaming but means the entire response is always processed, even if the consumer stops early (though `abort()` handles this).

### 8.4 Partial Message Mutation

Rather than building new message objects on each delta, Pi mutates a single `AssistantMessage` in place and passes a reference. This is efficient but means:

- Consumers see the latest state, not a snapshot at event time
- The `partial` field on events is always the same object reference
- UI must spread/clone if it needs snapshots for rendering

### 8.5 Declaration Merging for Extensibility

Using TypeScript's declaration merging for `CustomAgentMessages` is elegant:

- Zero runtime cost
- Full type safety
- No registration ceremony
- Custom messages flow through the same message array

### 8.6 Dual Stream Functions

Having both `stream()` (provider-specific) and `streamSimple()` (unified) is a practical compromise:

- Agent users get a simple, provider-agnostic API
- Power users can access provider-specific features
- No "lowest common denominator" problem

---

## 9. Comparison: Pi Agent-Centric vs. Compozy Provider-Centric

| Aspect                    | Pi (Agent-Centric)                                 | Compozy (Provider/Runtime-Centric)             |
| ------------------------- | -------------------------------------------------- | ---------------------------------------------- |
| **Primary abstraction**   | `Agent` class owns everything                      | Runtime orchestrates separate providers        |
| **State management**      | Agent owns messages, tools, model, streaming state | State distributed across runtime and providers |
| **Provider switching**    | Change `model` field, auto-routes via registry     | Provider instances are distinct objects        |
| **Tool execution**        | Agent loop handles parallel/sequential, hooks      | Provider-level tool handling                   |
| **Message history**       | Single `AgentMessage[]` with custom types          | Provider-specific message formats              |
| **Streaming**             | Unified `EventStream` protocol                     | Provider-specific streaming                    |
| **Mid-run intervention**  | Built-in steering/follow-up queues                 | Not a first-class concept                      |
| **Extensibility**         | Declaration merging, hook functions                | Adapter/plugin patterns                        |
| **Cross-provider replay** | `transformMessages` normalizes at call boundary    | N/A (provider-specific)                        |
| **Transport**             | StreamFn swap (direct, proxy, custom)              | Transport layer abstraction                    |

**What Pi does better:**

1. **Simplicity** -- Single class, single message array, single event protocol
2. **Steering** -- First-class support for mid-run user intervention
3. **Cross-provider** -- Seamless model switching with automatic message transformation
4. **Hook system** -- `beforeToolCall`/`afterToolCall` is minimal but powerful
5. **Custom messages** -- Declaration merging is zero-cost and type-safe

**What Compozy does better:**

1. **Type safety** -- Effect-TS provides stronger guarantees at the type level
2. **Error handling** -- Tagged errors are more composable than string errors
3. **Separation of concerns** -- Provider implementations are more isolated
4. **Backend integration** -- Tighter coupling with auth, billing, and infrastructure

---

## 10. Lessons for a Rust SDK

### 10.1 Patterns to Adopt

1. **Unified event protocol** -- Define a Rust enum equivalent of `AssistantMessageEvent` that all providers must produce. This is the most important abstraction.

2. **Agent-as-primary-abstraction** -- The agent should own state, message history, and tool execution. Providers are interchangeable backends.

3. **Registry pattern** -- A global (or scoped) registry mapping API identifiers to provider implementations. In Rust, this could be a `HashMap<String, Box<dyn Provider>>` or use the typestate pattern.

4. **Dual-level API** -- Provider-specific options + unified simple options. In Rust, this maps naturally to traits with associated types.

5. **Push-based streaming** -- Use `tokio::sync::mpsc` or similar channel for event delivery. The producer (provider) pushes events, the consumer (agent loop) processes them.

6. **Steering/follow-up queues** -- These are queues checked between tool executions. In Rust: `tokio::sync::mpsc::Receiver` polled between `tool.execute().await` calls.

7. **beforeToolCall/afterToolCall hooks** -- These map to Rust closures or trait implementations. Essential for permission systems.

8. **Transform at the boundary** -- Keep the internal message format rich (with custom types), transform to provider format only when calling the API.

### 10.2 Rust-Specific Adaptations

1. **Enum for messages instead of declaration merging** -- Rust doesn't have declaration merging. Use a `Message` enum with a `Custom(Box<dyn Any>)` variant, or define the custom message types as generic parameters on the Agent.

2. **Traits for providers** -- Define `Provider` and `SimpleProvider` traits:

   ```rust
   trait Provider {
       type Options: StreamOptions;
       fn stream(&self, model: &Model, context: &Context, options: Self::Options) -> EventStream;
   }
   trait SimpleProvider {
       fn stream_simple(&self, model: &Model, context: &Context, options: &SimpleOptions) -> EventStream;
   }
   ```

3. **Channel-based EventStream** -- Replace Pi's queue+waiting pattern with `tokio::sync::mpsc`:

   ```rust
   struct EventStream {
       receiver: mpsc::Receiver<AgentEvent>,
       result: oneshot::Receiver<AssistantMessage>,
   }
   ```

4. **Error types** -- Use proper Rust error enums instead of string errors. Each provider can have its own error type that implements a common trait.

5. **Tool system** -- Use Rust's type system for tool parameters:
   ```rust
   trait Tool: Send + Sync {
       type Params: DeserializeOwned + JsonSchema;
       type Details: Serialize;
       async fn execute(&self, id: &str, params: Self::Params, signal: CancellationToken) -> ToolResult<Self::Details>;
   }
   ```

### 10.3 Key Takeaways

- **Pi proves that a simple, agent-centric architecture works at scale** -- It supports 20+ providers, complex tool execution, mid-run steering, and cross-provider replay with about 1500 lines of core code (agent + loop + types).
- **The event protocol is the contract** -- Everything else (providers, tools, hooks) is implementation detail. Get the event protocol right and the rest follows.
- **Avoid over-abstraction** -- Pi has no middleware, no adapter layers, no transport abstraction. The stream function IS the provider. This keeps the code path short and debuggable.
- **Cross-provider compatibility requires explicit handling** -- The `transformMessages` function is non-trivial (handling thinking blocks, tool call IDs, orphaned calls, error messages). This complexity is inherent and must be designed for in any multi-provider system.

---

## 11. Relevant Files

### Agent Package

- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/agent/src/agent.ts` -- Agent class (612 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/agent/src/agent-loop.ts` -- Core loop (683 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/agent/src/types.ts` -- All types and interfaces (311 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/agent/src/proxy.ts` -- Proxy stream function (341 lines)

### AI Package -- Core

- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/types.ts` -- All types (337 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/stream.ts` -- Stream entry points (59 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/api-registry.ts` -- Provider registry (98 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/models.ts` -- Model registry (78 lines)

### AI Package -- Providers

- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/providers/register-builtins.ts` -- Registration (187 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/providers/anthropic.ts` -- Anthropic provider (884 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/providers/openai-responses.ts` -- OpenAI Responses provider
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/providers/google.ts` -- Google Gemini provider
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/providers/transform-messages.ts` -- Cross-provider message normalization (173 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/providers/simple-options.ts` -- Unified options mapping (47 lines)

### AI Package -- Utilities

- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/utils/event-stream.ts` -- EventStream class (88 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/utils/validation.ts` -- Tool argument validation via AJV (85 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/utils/overflow.ts` -- Context overflow detection (124 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/utils/json-parse.ts` -- Streaming JSON parser (29 lines)
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/packages/ai/src/env-api-keys.ts` -- Environment API key resolution (134 lines)

### Project Context

- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/README.md` -- Project overview
- `/Users/pedronauck/Dev/compozy/compozy-code/.resources/pi/AGENTS.md` -- Development rules and provider addition guide
