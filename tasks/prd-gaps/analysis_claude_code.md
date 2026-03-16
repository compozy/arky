# Gap Analysis: Claude Code Provider

## Summary

The TypeScript `compozy-code` Claude Code provider is a mature, feature-rich implementation built on the `@anthropic-ai/claude-agent-sdk` and Effect.ts, exposing a full `LanguageModelV3` interface from `@ai-sdk/provider`. It encompasses approximately 35 source files across 7 subdirectories covering stream normalization, error classification, message conversion, tool bridging, MCP integration, session management, structured output, and a non-streaming `generate` endpoint. The Rust `arky-claude-code` crate is a substantially more compact implementation at 8 source files. It covers stream parsing, text deduplication, nested tool tracking, tool lifecycle FSM, spawn-failure cooldown, and session management -- all wired into a `Provider` trait implementation that drives a Claude CLI subprocess via `arky-provider`'s `ProcessManager` + `StdioTransport`.

The Rust crate has solid parity on the core stream-parsing pipeline: it handles `system`, `stream_event` (content_block_start/delta/stop), `assistant`, `user`, `result`, `tool_progress`, and `rate_limit_event` message types. Its `TextDeduplicator`, `ToolLifecycleTracker`, `NestedToolTracker`, `SpawnFailureTracker`, and `SessionManager` map directly to the TS equivalents. However, there are significant gaps in several dimensions: no error classification system, no message/prompt conversion layer, no generate (non-streaming) endpoint, no tool bridge / MCP integration, no configuration schema validation, no structured output / JSON mode support, no reasoning/thinking block handling, and missing many configuration options that the TS provider supports (hooks, agents, permissions, plugins, sandboxing, etc.).

The gaps range from P0 (error classification, which is required for robust error handling and retries) to P2 (MCP combined-server creation, tool output truncation). The overall functional coverage of the Rust crate is estimated at 25-30% of the TS provider's surface area, though the 25-30% that exists is architecturally well-designed and will serve as a strong foundation.

## Feature Comparison Matrix

| Feature | compozy-code (TS) | arky (Rust) | Status | Priority |
|---------|-------------------|-------------|--------|----------|
| Stream parsing (stream_event) | Full: content_block_start/delta/stop for text, tool_use, thinking | Partial: text_delta + input_json_delta + tool_use; no thinking/reasoning blocks | Partial | P0 |
| Stream parsing (assistant snapshot) | Full: text, thinking, tool_use blocks with dedup-aware logic | Partial: text + tool_use blocks; no thinking blocks | Partial | P0 |
| Stream parsing (user/tool_result) | Full: tool_result + tool_error + tool_approval_response | Partial: tool_result only; no tool_error or tool_approval_response | Partial | P1 |
| Stream parsing (result) | Full: usage, finish reason, structured_output, cost, duration | Partial: usage + finish reason + is_error; no structured_output, cost, duration | Partial | P1 |
| Stream parsing (rate_limit_event) | Not present as distinct type | Full: ClaudeRateLimitEvent | Complete+ | -- |
| Error classification system | Full: 18 error types, pattern-based classifier, regex + status codes + error codes | None | Missing | P0 |
| Error-to-SDK mapping | Full: maps classified errors to APICallError/LoadAPIKeyError with metadata | None | Missing | P0 |
| Finish reason mapping | Full: maps stop_reason + subtype to LanguageModelV3FinishReason | Basic: raw string passthrough only | Partial | P1 |
| Message/prompt conversion | Full: LanguageModelV3Prompt -> messagesPrompt + image handling | Basic: renders messages to text prompt | Partial | P1 |
| Image content handling | Full: base64, data URLs, Uint8Array, object images | None | Missing | P2 |
| Structured output / JSON mode | Full: outputFormat, structured_output extraction | None | Missing | P1 |
| Reasoning/thinking blocks | Full: ReasoningStart/Delta/Complete events | None | Missing | P1 |
| Text deduplication | Full: stream_event vs assistant dedup with state tracking | Full: equivalent logic | Complete | -- |
| Tool lifecycle FSM | Full: Idle -> Started -> InputReceiving -> Executing -> Completed | Full: equivalent states and transitions | Complete | -- |
| Nested tool tracking | Full: parent tracking, result merging, preview events | Full: equivalent core logic; no preview events | Partial | P1 |
| Spawn failure cooldown | Full: policy-based circuit breaker with Duration support | Full: equivalent with tokio::time::Instant | Complete | -- |
| Session management | Full: get/set/clear with Effect Ref | Full: equivalent with tokio Mutex | Complete | -- |
| Generate (non-streaming) endpoint | Full: doGenerate with accumulation, truncation recovery | None | Missing | P1 |
| Tool bridge (MCP passthrough) | Full: createToolsBridge, ToolsBridgeRegistry, MCP server creation | None (arky-tools/arky-mcp exist but not wired) | Missing | P1 |
| Tool extraction helpers | Full: extractToolUses, extractToolResults, extractToolErrors | None | Missing | P2 |
| Tool input serialization | Full: size checks, JSON serialization with limits | None | Missing | P2 |
| Tool output truncation | Full: array/object/string truncation with byte limits | None | Missing | P2 |
| MCP custom server | Full: createCustomMcpServer with Zod schema conversion | None | Missing | P2 |
| MCP combined server | Full: createCombinedMcpServer merging custom tools + SDK servers | None | Missing | P2 |
| Configuration schema | Full: 60+ settings with Effect Schema validation | Basic: ClaudeCodeProviderConfig with 9 fields | Partial | P1 |
| Hooks system | Full: 17 hook events with matcher/callback/timeout support | None | Missing | P1 |
| Agents/subagents | Full: named agent configs with model/tools/prompt | None | Missing | P1 |
| Permission modes | Full: default/acceptEdits/bypassPermissions/plan/delegate/dontAsk | None | Missing | P1 |
| Plugin support | Full: local plugins with path | None | Missing | P2 |
| Sandbox support | Full: sandbox configuration passthrough | None | Missing | P2 |
| Streaming input / message injection | Full: AsyncIterable prompt with message injector | None | Missing | P2 |
| Settings validation warnings | Full: model ID, prompt length, session ID format warnings | None | Missing | P2 |
| Debug/verbose configuration | Full: debug, debugFile, verbose, stderr callback | Basic: verbose flag only | Partial | P2 |
| Environment variable passthrough | Full: env merge with process.env | Basic: env BTreeMap passthrough | Partial | P2 |
| Tool ID codec integration | Not present (tools resolved by name) | Present: ToolIdCodec for canonical name resolution | Complete+ | -- |
| Binary version validation | Not present (SDK handles it) | Present: ensure_binary_validated | Complete+ | -- |
| Process management | SDK handles internally | Full: ProcessManager + StdioTransport | Complete+ | -- |
| Event metadata emission | Not present (SDK handles stream parts) | Full: EventMetadata with sequence, timestamps, IDs | Complete+ | -- |
| Provider descriptor / capabilities | Not present (SDK interface) | Full: ProviderDescriptor with capability flags | Complete+ | -- |

## Detailed Gap Analysis

### 1. Error Classification System
- **TS Location**: `classifier/classifier.ts`, `classifier/patterns.ts`, `classifier/mapper.ts`, `errors.ts`
- **Rust Status**: Completely missing. The Rust crate uses `ProviderError` from `arky-provider` with generic variants (ProtocolViolation, ProcessCrashed, etc.) but has no pattern-based classification of stderr/error messages.
- **Complexity**: High
- **Description**: The TS implementation has a sophisticated error classifier that examines error messages, status codes, exit codes, and stderr content against pattern libraries to classify errors into 18 distinct types (authentication, authorization, rate limit, timeout, spawn failure, network, JSON parse, stream corruption, tool execution, etc.). Each classified error carries structured metadata and is tagged as retryable or non-retryable. The Rust crate has `classify_terminal_error` in `provider.rs` which only checks `error_code == "authentication_failed"` -- a minimal fraction of the classification logic. A full port would need: (a) the pattern sets from `patterns.ts`, (b) the extraction logic from `classifier.ts`, (c) error type definitions mirroring `errors.ts`, and (d) the retryability classification.
- **Dependencies**: Blocks retry logic, error reporting, and any consumer that needs structured error types.

### 2. Error-to-SDK Mapping Layer
- **TS Location**: `classifier/mapper.ts`
- **Rust Status**: Missing
- **Complexity**: Medium
- **Description**: The TS mapper converts classified `ClaudeCodeError` instances into `APICallError` or `LoadAPIKeyError` from `@ai-sdk/provider`, enriching them with metadata (stderr snippets, prompt excerpts, spawn failure backoff info). In Rust, the equivalent would be mapping classified errors into the `arky-error` `ClassifiedError` contract. This depends on the error classification system being implemented first.
- **Dependencies**: Depends on Error Classification System (#1).

### 3. Reasoning/Thinking Block Handling
- **TS Location**: `stream/event-normalizer.ts` (handleStreamContentBlockStart for "thinking", handleAssistantReasoningBlock), `stream/normalized-events.ts` (ReasoningStart/Delta/Complete), `stream/stream-parts.ts` (reasoning-start/delta/end parts)
- **Rust Status**: Missing entirely. The parser in `parser.rs` only handles `tool_use` content blocks in `content_block_start` and only `text_delta` / `input_json_delta` in `content_block_delta`. Thinking/reasoning blocks are silently dropped.
- **Complexity**: Medium
- **Description**: When Claude uses extended thinking, it emits `content_block_start` with `type: "thinking"` and `content_block_delta` with `type: "thinking_delta"`. The TS normalizer converts these to `ReasoningStart`, `ReasoningDelta`, and `ReasoningComplete` events, which are then mapped to `reasoning-start`, `reasoning-delta`, and `reasoning-end` stream parts. The Rust parser needs to: (a) detect thinking blocks in `parse_stream_content_block_start`, (b) handle `thinking_delta` in `parse_stream_content_block_delta`, (c) emit corresponding events, and (d) the provider's `StreamRuntime` needs to handle these events as `AgentEvent` variants.
- **Dependencies**: Requires new `AgentEvent` variants or custom event emission in arky-protocol.

### 4. Structured Output / JSON Mode
- **TS Location**: `stream/event-normalizer.ts` (handleStreamContentBlockDelta for jsonMode, handleResultEvent for structured_output), `conversion/options.ts` (outputFormat), `generate/generate.ts` (resolveFinalText)
- **Rust Status**: Missing
- **Complexity**: Medium
- **Description**: When `responseFormat.type === "json"` with a schema, the TS provider: (a) suppresses text_delta events and instead emits input_json_delta as text, (b) passes `outputFormat: { type: "json_schema", schema }` to Claude, (c) extracts `structured_output` from result events, (d) uses it as the final text in generate mode. The Rust crate has no concept of JSON mode and does not pass `--output-format json_schema` to the CLI.
- **Dependencies**: Requires CLI argument construction changes and parser mode flag.

### 5. Generate (Non-Streaming) Endpoint
- **TS Location**: `generate/generate.ts`
- **Rust Status**: Missing. The `Provider` trait in `arky-provider` may not yet have a `generate` method; only `stream` is implemented.
- **Complexity**: High
- **Description**: The TS `generateClaudeCode` function runs a full stream internally but accumulates results into a `LanguageModelV3GenerateResult` instead of yielding stream parts. It handles: spawn cooldown checks, message conversion, event normalization, text/tool-call accumulation, truncation recovery (when stream ends with incomplete JSON), session persistence, and structured output extraction. A Rust equivalent would need to consume the existing stream and accumulate into a result type.
- **Dependencies**: Depends on the Provider trait having a generate method.

### 6. Configuration Schema Expansion
- **TS Location**: `schemas.ts` (ClaudeCodeSettingsSchema with 60+ fields), `config.ts`, `conversion/options.ts` (buildClaudeQueryOptions)
- **Rust Status**: `ClaudeCodeProviderConfig` has 9 fields: binary, cwd, extra_args, env, version_args, verbose, max_frame_len, spawn_failure_policy. Missing ~50 configuration options.
- **Complexity**: High
- **Description**: The TS settings schema includes: pathToClaudeCodeExecutable, customSystemPrompt, appendSystemPrompt, systemPrompt (string or preset), maxTurns, maxThinkingTokens, reasoningEffort, cwd, executable, executableArgs, permissionMode, allowDangerouslySkipPermissions, permissionPromptToolName, continue, resume, sessionId, resumeSessionAt, allowedTools, disallowedTools, tools (array or preset), settingSources, betas, streamingInput, canUseTool, hooks, hookOptions, mcpServers, verbose, logger, env, additionalDirectories, enableFileCheckpointing, maxBudgetUsd, plugins, sandbox, persistSession, agents, includePartialMessages, fallbackModel, forkSession, maxToolResultSize, toolOutputLimits, stderr, onStreamStart, onQueryCreated, strictMcpConfig, extraArgs, debug, debugFile, sdkOptions, spawnClaudeCodeProcess, spawnFailurePolicy. The Rust config needs to grow significantly to support these as CLI arguments.
- **Dependencies**: Each config option needs corresponding CLI argument mapping in `build_process_config`.

### 7. Hooks System
- **TS Location**: `schemas.ts` (HooksSchema, HookHandlerSchema, HookMatcherSchema), `conversion/options.ts` (toHooks, toHookCallback, toHookMatcher), `types.ts` (HookCallback, HookCallbackMatcher, HookEvent)
- **Rust Status**: Missing. The arky-hooks crate exists but is not wired into the Claude Code provider.
- **Complexity**: High
- **Description**: The TS hooks system supports 17 event types (PreToolUse, PostToolUse, PostToolUseFailure, Notification, UserPromptSubmit, SessionStart, SessionEnd, Stop, SubagentStart, SubagentStop, PreCompact, PermissionRequest, Setup, TeammateIdle, TaskCompleted, ConfigChange, WorktreeCreate, WorktreeRemove) with matcher patterns, timeout support, and callback/command/prompt handler types. Each hook event maps to a Claude SDK callback. In Rust, this would need to be expressed as CLI arguments or a config file.
- **Dependencies**: arky-hooks crate integration.

### 8. Agents/Subagent Configuration
- **TS Location**: `schemas.ts` (agents field), `conversion/options.ts` (toAgents)
- **Rust Status**: Missing
- **Complexity**: Medium
- **Description**: The TS provider supports named agent configurations with description, prompt, model, tools, disallowedTools, mcpServers, and criticalSystemReminder_EXPERIMENTAL. These are passed to the Claude SDK for subagent spawning. In the Rust implementation, Claude's `--agents` flag or equivalent config would need to be constructed.
- **Dependencies**: Requires understanding of Claude CLI's agent flags.

### 9. Permission Modes
- **TS Location**: `schemas.ts` (permissionMode), `conversion/options.ts` (toPermissionMode)
- **Rust Status**: Missing
- **Complexity**: Low
- **Description**: The TS provider maps permission modes (default, acceptEdits, bypassPermissions, plan, delegate, dontAsk) to Claude SDK options. In the Rust CLI, this maps to `--permission-mode` or `--dangerously-skip-permissions` flags.
- **Dependencies**: CLI argument mapping.

### 10. Tool Bridge / MCP Integration
- **TS Location**: `tools/bridge.ts`, `services/tools-bridge-registry.ts`, `mcp/custom-server.ts`, `mcp/combined-server.ts`, `conversion/options.ts` (collectToolBridgeServers)
- **Rust Status**: Missing from claude-code crate. arky-mcp and arky-tools crates exist separately.
- **Complexity**: High
- **Description**: The TS ToolsBridge creates an in-process MCP server that wraps user-defined tools, registers it with Claude via the `mcpServers` option, so Claude can call user tools through MCP. The ToolsBridgeRegistry manages multiple bridge instances. In Rust, the equivalent would wire arky-tools definitions through arky-mcp as MCP servers passed to the Claude CLI via `--mcp-server` config.
- **Dependencies**: arky-mcp, arky-tools crate APIs.

### 11. Tool Output Truncation
- **TS Location**: `tools/truncation.ts`
- **Rust Status**: Missing
- **Complexity**: Medium
- **Description**: Sophisticated truncation system that handles arrays (binary search removal), objects (string field truncation, then key removal), and plain strings with byte-level UTF-8 safe truncation. Includes configurable limits (maxSize, warnSize, enableTruncation) with structured truncation notices.
- **Dependencies**: None, standalone utility.

### 12. Tool Input Serialization
- **TS Location**: `tools/serialization.ts`
- **Rust Status**: Missing
- **Complexity**: Low
- **Description**: Serializes tool input to JSON with size checking against configurable limits (maxSize, warnSize). Rejects inputs exceeding maxSize with ToolInputSizeExceededError.
- **Dependencies**: Error type definitions.

### 13. Nested Tool Preview Events
- **TS Location**: `stream/stream.ts` (emitNestedPreviewEvents, NestedPreviewStateRefs)
- **Rust Status**: Missing. The Rust `NestedToolTracker` handles start/result merging but not preview emission.
- **Complexity**: Medium
- **Description**: The TS stream emits preliminary `ToolResult` events with `{ preliminary: true }` when nested tool calls are tracked, allowing consumers to see real-time nested tool progress before the parent tool completes. This requires tracking root tool names, computing preview signatures, and deduplicating identical previews.
- **Dependencies**: Requires changes to stream event emission in provider.rs.

### 14. Tool Error Handling in User Messages
- **TS Location**: `stream/event-normalizer.ts` (handleUserToolErrorBlock)
- **Rust Status**: Missing. The Rust parser only handles `tool_result` blocks in user messages, not `tool_error` blocks.
- **Complexity**: Low
- **Description**: The TS normalizer handles both `tool_result` and `tool_error` block types within user messages. `tool_error` blocks are converted to `ToolError` normalized events. The Rust parser skips non-`tool_result` blocks.
- **Dependencies**: New event type or reuse of ToolResult with is_error flag.

### 15. Message Conversion / Image Support
- **TS Location**: `conversion/messages.ts`
- **Rust Status**: Basic text prompt rendering exists in `render_prompt`. No image handling.
- **Complexity**: High
- **Description**: The TS converter handles LanguageModelV3Prompt (system, user, assistant, tool roles) with rich content part handling: text parts, image parts (base64, data URLs, Uint8Array, object images with multiple mime type fields), file parts, tool-call parts, tool-result parts, and tool-approval-response parts. It produces both a flat `messagesPrompt` string and `streamingContentParts` array for the streaming input path. The Rust `render_prompt` does basic text concatenation with no image or structured content support.
- **Dependencies**: arky-protocol ContentBlock types.

### 16. Warnings System
- **TS Location**: `conversion/warnings.ts`
- **Rust Status**: Missing
- **Complexity**: Low
- **Description**: Builds warnings for unsupported parameters (temperature, topP, topK, etc.), JSON response format without schema, model validation warnings, prompt length warnings. These are attached to stream-start and generate results.
- **Dependencies**: Warning types in arky-protocol or equivalent.

### 17. Streaming Input / Message Injection
- **TS Location**: `utils.ts` (toAsyncIterablePrompt, createMessageInjector)
- **Rust Status**: Missing
- **Complexity**: Medium
- **Description**: Creates an async iterable prompt that yields an initial message then waits for injected messages via a `MessageInjector` API. Allows real-time user input during an active session. The injector supports queue-based delivery with close semantics.
- **Dependencies**: Would require async channel integration with the subprocess stdin.

### 18. Stream Recovery on Truncation
- **TS Location**: `stream/stream.ts` (recoveryStream), `utils.ts` (isClaudeCodeTruncationError)
- **Rust Status**: Missing
- **Complexity**: Medium
- **Description**: When a `ClaudeStreamCorruptedError` occurs during streaming, the TS provider emits a recovery stream with truncated text, an error event, and a finish event with `unified: "error"`. In generate mode, truncation errors trigger partial result recovery with a `length` finish reason. The Rust crate does not have any truncation detection or recovery logic.
- **Dependencies**: Error classification.

## Files Reference

### TypeScript (compozy-code) - All source files examined:

**Root**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/index.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/schemas.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/config.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/errors.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/types.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/utils.ts`

**Classifier**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/classifier/classifier.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/classifier/mapper.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/classifier/patterns.ts`

**Conversion**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/conversion/finish-reason.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/conversion/messages.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/conversion/options.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/conversion/warnings.ts`

**Stream**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/stream.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/event-normalizer.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/normalized-events.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/stream-parts.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/stream-state.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/text-deduplicator.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/tool-lifecycle.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/stream/nested-tool-tracker.ts`

**Tools**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/tools/bridge.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/tools/extraction.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/tools/serialization.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/tools/truncation.ts`

**MCP**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/mcp/custom-server.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/mcp/combined-server.ts`

**Services**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/services/language-model.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/services/provider.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/services/session.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/services/spawn-failure-tracker.ts`
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/services/tools-bridge-registry.ts`

**Generate**:
- `/Users/pedronauck/dev/compozy/compozy-code/providers/claude-code/src/generate/generate.ts`

### Rust (arky-claude-code) - All source files examined:

- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/lib.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/parser.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/provider.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/session.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/cooldown.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/dedup.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/nested.rs`
- `/Users/pedronauck/Dev/compozy/arky/crates/arky-claude-code/src/tool_fsm.rs`
