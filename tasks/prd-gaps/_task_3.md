## markdown

## status: pending

<task_context>
<domain>crates/arky-claude-code,arky-codex,arky-provider,arky-usage,arky-config,arky-protocol,arky-session,arky-server</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2</dependencies>
</task_context>

# Task 3.0: Phase 3 (P2) — Polish

## Overview

Complete the final polish layer across the workspace: Claude Code P2 (plugin support, sandbox config, streaming input, stream recovery, nested tool preview events, tool serialization, MCP custom/combined servers, debug/verbose config, env passthrough, settings warnings, image handling), Codex P2 (thread compaction, scheduler queue overflow, environment sanitization), and cross-cutting P2 (model cost computation, xhigh detection, provider family gateway classification, compound session key lookup, runtime error union, native event utils, configuration validation with rich schemas).

<critical>
- **ALWAYS READ** @CLAUDE.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the techspec at `tasks/prd-gaps/techspec.md`
- **YOU CAN ONLY** finish when `make fmt && make lint && make test` all pass at 100%
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Claude Code: plugin paths, sandbox config passthrough, AsyncIterator-style streaming input with message injection, stream recovery on truncation, nested tool preview events, tool input serialization with size checks, MCP custom server (Zod->JSON schema), MCP combined server, debug/verbose/debugFile config, environment variable merge, settings warnings (model ID, prompt length, session ID format), image content handling (base64, data URL, Uint8Array)
- Codex: thread/compact RPC method, scheduler queue overflow handling, environment sanitization (LD_PRELOAD, DYLD_ blocking)
- Cross-cutting: model cost computation from pricing tables, xhigh reasoning detection for capable models, provider family gateway classification, compound session key (provider+model+sessionId), runtime error union type, native event extraction utils, configuration validation with rich error schemas
</requirements>

## Subtasks

### 3.1 Claude Code P2

- [ ] 3.1.1 Implement plugin support: --plugin CLI flag with local file path resolution
- [ ] 3.1.2 Implement sandbox config: --sandbox CLI flag with SandboxConfig struct
- [ ] 3.1.3 Implement streaming input with message injection (async channel-based prompt with injector callback)
- [ ] 3.1.4 Implement stream recovery: detect truncation in-flight, emit partial + retry marker
- [ ] 3.1.5 Implement nested tool preview events (emit preview events for nested tool starts before completion)
- [ ] 3.1.6 Implement tool input serialization with size checks and JSON limits
- [ ] 3.1.7 Create MCP custom server: convert arky-tools definitions to MCP tool schemas (JSON Schema from Rust types)
- [ ] 3.1.8 Create MCP combined server: merge custom tools + external MCP server definitions
- [ ] 3.1.9 Implement debug/verbose configuration (debug flag, debugFile path, verbose flag, stderr callback)
- [ ] 3.1.10 Implement environment variable merge (process env + user env + provider env)
- [ ] 3.1.11 Implement settings warnings (model ID validation, prompt length warning, session ID format check)
- [ ] 3.1.12 Implement image content handling (base64 encoding, data URL parsing, binary image support)
- [ ] 3.1.13 Unit tests: plugin flag, sandbox config, streaming input, stream recovery, nested preview, tool serialization, MCP servers, debug config, env merge, settings warnings, image handling

### 3.2 Codex P2

- [ ] 3.2.1 Implement thread/compact RPC method call with response handling
- [ ] 3.2.2 Implement scheduler queue overflow detection and back-pressure handling
- [ ] 3.2.3 Implement environment sanitization: block LD_PRELOAD, DYLD_*, sensitive env vars before spawning codex process
- [ ] 3.2.4 Add response metadata emission (stream-start, response-metadata events)
- [ ] 3.2.5 Add part ID tracking (UUID per text part for delta reconciliation)
- [ ] 3.2.6 Unit tests: thread compaction, queue overflow, env sanitization, metadata emission, part IDs

### 3.3 Cross-Cutting P2

- [ ] 3.3.1 Implement model cost computation: pricing tables for Claude, GPT, Codex models, compute_estimated_cost() with input/output/cached token rates
- [ ] 3.3.2 Implement xhigh reasoning detection: XHIGH_CAPABLE_MODEL_IDS list, supports_xhigh_reasoning() check
- [ ] 3.3.3 Implement provider family gateway classification: resolve_provider_family() with gateway provider detection (e.g., openrouter, litellm)
- [ ] 3.3.4 Implement compound session key: SessionKey(provider_id, model_id, session_id) for provider+model-scoped session lookup
- [ ] 3.3.5 Define runtime error union type: RuntimeError enum unifying all crate errors (ProviderError, ToolError, SessionError, HookError, ConfigError, ServerError, McpError)
- [ ] 3.3.6 Implement native event extraction utils: extract_text_from_events(), extract_tool_uses(), extract_tool_results(), extract_usage()
- [ ] 3.3.7 Implement configuration validation with rich error schemas: validate config structs with field-level error messages, suggest corrections
- [ ] 3.3.8 Unit tests: model cost for multiple providers, xhigh detection, provider family, session key, error union, event utils, config validation

## Implementation Details

### Relevant Files

- `crates/arky-claude-code/src/provider.rs` - Plugin, sandbox, debug, env, warnings
- `crates/arky-claude-code/src/parser.rs` - Stream recovery, nested preview
- `crates/arky-claude-code/src/conversion.rs` - Image handling, message injection
- `crates/arky-claude-code/src/tool_bridge.rs` - MCP custom/combined servers, tool serialization
- `crates/arky-codex/src/provider.rs` - Thread compaction, env sanitization
- `crates/arky-codex/src/scheduler.rs` - Queue overflow handling
- `crates/arky-codex/src/pipeline.rs` - Metadata emission, part IDs
- `crates/arky-usage/src/cost.rs` - Model cost computation
- `crates/arky-provider/src/family.rs` - NEW: Provider family gateway classification
- `crates/arky-provider/src/reasoning.rs` - xhigh detection
- `crates/arky-session/src/key.rs` - NEW: Compound session key
- `crates/arky-error/src/union.rs` - NEW: RuntimeError union
- `crates/arky-protocol/src/utils.rs` - NEW: Native event extraction utils
- `crates/arky-config/src/validation.rs` - NEW: Rich config validation

### TS Reference Files

- `compozy-code/providers/claude-code/src/conversion/options.ts` - Plugin, sandbox, debug, env
- `compozy-code/providers/claude-code/src/conversion/warnings.ts` - Settings warnings
- `compozy-code/providers/claude-code/src/conversion/messages.ts` - Image handling, streaming input
- `compozy-code/providers/claude-code/src/tools/truncation.ts` - Tool serialization
- `compozy-code/providers/claude-code/src/tools/bridge.ts` - MCP custom/combined
- `compozy-code/providers/claude-code/src/stream/event-normalizer.ts` - Stream recovery, nested preview
- `compozy-code/providers/codex/src/streaming/CodexStreamPipeline.ts` - Thread compaction
- `compozy-code/providers/codex/src/server/CodexAppServer.ts` - Env sanitization
- `compozy-code/providers/codex/src/util/queue.ts` - Queue overflow
- `compozy-code/providers/runtime/src/usage/consumption.ts` - Model cost
- `compozy-code/providers/runtime/src/reasoning/resolve-reasoning.ts` - xhigh detection
- `compozy-code/providers/runtime/src/services/provider-registry.ts` - Provider family

### Internal Execution Plan

```
3.1 Claude Code P2 ──┐
3.2 Codex P2 ────────┼──> Done
3.3 Cross-cutting P2 ┘
```

All three subtask groups are independent and can be executed in parallel.

## Deliverables

- Claude Code: full CLI flag coverage (plugin, sandbox, debug, env), streaming input, stream recovery, nested preview, tool serialization, MCP servers, image handling, settings warnings
- Codex: thread compaction, queue overflow handling, env sanitization, metadata emission
- Cross-cutting: model cost computation, xhigh detection, provider family gateway, compound session key, runtime error union, event utils, config validation
- `make fmt && make lint && make test` passing

## Tests

### Unit Tests (Required)

- [ ] Claude Code: plugin config serializes to --plugin flag
- [ ] Claude Code: sandbox config serializes to --sandbox flag
- [ ] Claude Code: streaming input accepts injected messages via channel
- [ ] Claude Code: stream recovery detects truncation and emits retry marker
- [ ] Claude Code: nested tool start emits preview event
- [ ] Claude Code: tool input serialization respects size limits
- [ ] Claude Code: MCP custom server converts tool definitions to JSON Schema
- [ ] Claude Code: MCP combined server merges custom + external tools
- [ ] Claude Code: debug config maps to --debug and --debug-file flags
- [ ] Claude Code: env merge combines process + user + provider envs correctly
- [ ] Claude Code: settings warning for invalid model ID
- [ ] Claude Code: settings warning for overly long prompt
- [ ] Claude Code: base64 image encoded correctly for Claude CLI
- [ ] Claude Code: data URL image parsed and converted
- [ ] Codex: thread/compact sends correct RPC call and handles response
- [ ] Codex: scheduler overflow triggers back-pressure
- [ ] Codex: env sanitization blocks LD_PRELOAD and DYLD_ vars
- [ ] Codex: stream-start and response-metadata events emitted
- [ ] Codex: text parts receive unique UUID part IDs
- [ ] Cost: compute_estimated_cost for claude-3.5-sonnet input/output tokens
- [ ] Cost: compute_estimated_cost for gpt-4o input/output tokens
- [ ] Cost: unknown model returns None cost
- [ ] Reasoning: xhigh supported for GPT-5.x models
- [ ] Reasoning: xhigh not supported for GPT-4o
- [ ] Family: resolve_provider_family("openrouter/claude-3.5-sonnet") -> Gateway
- [ ] Family: resolve_provider_family("claude-3.5-sonnet") -> ClaudeCode
- [ ] Session key: compound key equality and hashing
- [ ] Error union: ProviderError wraps into RuntimeError correctly
- [ ] Event utils: extract_text_from_events concatenates text deltas
- [ ] Event utils: extract_tool_uses returns all tool call events
- [ ] Config: validation produces field-level error messages
- [ ] Config: validation suggests corrections for typos

### Verification Commands

- [ ] `make fmt`
- [ ] `make lint`
- [ ] `make test`

## Success Criteria

- All P2 gaps from analysis documents are resolved
- Full CLI flag coverage for both providers
- Complete feature parity with compozy-code TypeScript providers
- Model cost computation enables billing visibility
- Configuration validation provides actionable error messages
- Runtime error union enables unified error handling at the facade layer
