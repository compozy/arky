## markdown

## status: pending

<task_context>
<domain>crates/arky-provider,arky-tools,arky-claude-code,arky-codex,arky-core,arky-server,arky-session</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>critical</complexity>
<dependencies>task1</dependencies>
</task_context>

# Task 2.0: Phase 2 (P1) — Important Completeness

## Overview

Implement all P1 features to complete the provider surface area: provider enhancements (capability validation, model-prefix inference), tool output truncation, Claude Code P1 (tool bridge, generate override, hooks, message conversion, permissions, finish reasons, warnings, subagents), Codex P1 (stream pipeline, dedup, cancellation, tool payloads, hooks), core integration (usage aggregation in turn loop, model discovery, reasoning resolution), and server/session P1 (SSE enhancements, runtime client, session TTL).

<critical>
- **ALWAYS READ** @CLAUDE.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the techspec at `tasks/prd-gaps/techspec.md`
- **YOU CAN ONLY** finish when `make fmt && make lint && make test` all pass at 100%
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- ProviderCapabilities validation against requests, model-prefix auto-inference
- Tool output truncation (array, object, string strategies with UTF-8 safe cutting)
- Claude Code: tool bridge via arky-mcp, generate with truncation recovery, hooks integration, message conversion with image support, structured output/JSON mode, permission modes, finish reason mapping, warnings, subagent config
- Codex: stream pipeline with abort signal and finalization, fingerprint-based dedup, CancellationToken wiring, per-type tool payload builders, request preparation pipeline, hooks integration
- Core: UsageAggregator in turn loop, ModelDiscoveryService trait, reasoning effort resolution, capability validation at agent entry
- Server/Session: SSE sequence IDs and [DONE] sentinel, RuntimeClient, session TTL with capacity eviction
</requirements>

## Subtasks

### 2.1 Provider Enhancements

- [ ] 2.1.1 Implement validate_capabilities(request, capabilities) -> Vec<CapabilityWarning>
- [ ] 2.1.2 Add image_inputs check via messagesHaveImageInputs()
- [ ] 2.1.3 Add extended_thinking check for reasoning effort requests
- [ ] 2.1.4 Implement model-prefix auto-inference map (claude- -> claude-code, gpt- -> codex)
- [ ] 2.1.5 Enhance generate_from_stream with better text + tool call + usage accumulation
- [ ] 2.1.6 Unit tests: capability validation combinations, model-prefix inference

### 2.2 Tool Output Truncation

- [ ] 2.2.1 Define TruncationConfig (max_bytes default 100KB, warn_bytes, enabled)
- [ ] 2.2.2 Implement string truncation with UTF-8 boundary detection and "[truncated]" marker
- [ ] 2.2.3 Implement array truncation with binary search element removal
- [ ] 2.2.4 Implement object truncation (string values first, then key removal)
- [ ] 2.2.5 Implement truncate_tool_output() dispatching by content type
- [ ] 2.2.6 Unit tests: string/array/object truncation, UTF-8 boundaries, disabled passthrough

### 2.3 Claude Code P1

- [ ] 2.3.1 Create tool_bridge.rs: wire arky-tools -> arky-mcp -> MCP server for Claude CLI
- [ ] 2.3.2 Override generate() with truncation recovery (detect incomplete JSON, retry)
- [ ] 2.3.3 Wire arky-hooks into stream pipeline (before_tool_call, after_tool_call)
- [ ] 2.3.4 Create conversion.rs: message conversion with text, image, tool-call parts
- [ ] 2.3.5 Implement image content handling (base64, data URL conversion)
- [ ] 2.3.6 Implement structured output: --output-format json_schema flag + result extraction
- [ ] 2.3.7 Implement permission mode mapping to CLI flags
- [ ] 2.3.8 Implement finish reason mapping (stop_reason -> FinishReason)
- [ ] 2.3.9 Create warnings collection for unsupported params (temperature, topP, etc.)
- [ ] 2.3.10 Add subagent config serialization to --agents JSON
- [ ] 2.3.11 Unit tests: tool bridge, generate with recovery, message conversion, permissions, finish reasons, warnings

### 2.4 Codex P1

- [ ] 2.4.1 Create pipeline.rs: CodexStreamPipeline with abort signal handling
- [ ] 2.4.2 Implement CodexStreamState (closed, lastUsage, turnFailure, sessionId, fingerprints)
- [ ] 2.4.3 Implement stream-start, response-metadata emission, finalization with turn failure detection
- [ ] 2.4.4 Create dedup.rs: fingerprint generation and duplicate detection
- [ ] 2.4.5 Wire CancellationToken from ProviderRequest through stream loop
- [ ] 2.4.6 Create tool_payloads.rs: per-type tool input/result payload builders
- [ ] 2.4.7 Implement canonical tool name mapping (command_execution->shell, file_change->apply_patch)
- [ ] 2.4.8 Implement per-type error detection (exitCode, status, error fields)
- [ ] 2.4.9 Implement request preparation pipeline (settings merge + mandatory + overrides)
- [ ] 2.4.10 Wire arky-hooks into stream pipeline (after_tool_use, after_agent)
- [ ] 2.4.11 Add subagent config passthrough in config overrides
- [ ] 2.4.12 Unit tests: pipeline lifecycle, fingerprint dedup, cancellation, tool payloads, request prep

### 2.5 Core & Cross-Cutting P1

- [ ] 2.5.1 Integrate UsageAggregator into TurnRuntime (accumulate per-turn and per-session)
- [ ] 2.5.2 Expose usage in AgentEvent::TurnEnd and AgentResponse
- [ ] 2.5.3 Define ModelDiscoveryService trait with per-provider discovery
- [ ] 2.5.4 Implement ModelInfo type with all fields
- [ ] 2.5.5 Implement reasoning effort resolution functions per provider
- [ ] 2.5.6 Integrate validate_capabilities() in agent turn entry
- [ ] 2.5.7 Implement incremental token consumption extraction from chunks
- [ ] 2.5.8 Unit tests: usage aggregation, model discovery, reasoning resolution, capability validation

### 2.6 Server & Session P1

- [ ] 2.6.1 Add monotonic sequence counter to SSE event emission
- [ ] 2.6.2 Emit [DONE] sentinel SSE event on stream completion
- [ ] 2.6.3 Emit error payloads as SSE events on failure
- [ ] 2.6.4 Create RuntimeClient (stream_text, create_session, resume_session, dispose)
- [ ] 2.6.5 Add TTL tracking to InMemorySessionStore entries
- [ ] 2.6.6 Implement lazy TTL expiration on access
- [ ] 2.6.7 Add capacity limit with LRU eviction
- [ ] 2.6.8 Unit tests: SSE sequence IDs, [DONE] sentinel, RuntimeClient, session TTL, capacity eviction

## Implementation Details

### Relevant Files

- `crates/arky-provider/src/descriptor.rs` - ProviderCapabilities expansion + validation
- `crates/arky-provider/src/registry.rs` - Model-prefix inference
- `crates/arky-provider/src/traits.rs` - generate_from_stream enhancement
- `crates/arky-provider/src/discovery.rs` - NEW: ModelDiscoveryService
- `crates/arky-provider/src/reasoning.rs` - NEW: Reasoning effort resolution
- `crates/arky-tools/src/truncation.rs` - NEW: truncation module
- `crates/arky-claude-code/src/tool_bridge.rs` - NEW: MCP tool bridge
- `crates/arky-claude-code/src/conversion.rs` - NEW: message conversion
- `crates/arky-claude-code/src/generate.rs` - NEW: generate override
- `crates/arky-codex/src/pipeline.rs` - NEW: stream pipeline
- `crates/arky-codex/src/dedup.rs` - NEW: fingerprint dedup
- `crates/arky-codex/src/tool_payloads.rs` - NEW: tool payload builders
- `crates/arky-core/src/turn.rs` - Usage aggregation, capability validation
- `crates/arky-core/src/agent.rs` - Capability validation entry
- `crates/arky-server/src/routes/events.rs` - SSE enhancements
- `crates/arky-server/src/client.rs` - NEW: RuntimeClient
- `crates/arky-session/src/memory.rs` - TTL + capacity

### TS Reference Files

- `compozy-code/providers/runtime/src/capabilities/capability-validator.ts`
- `compozy-code/providers/runtime/src/services/provider-registry.ts`
- `compozy-code/providers/runtime/src/models/model-discovery-service.ts`
- `compozy-code/providers/runtime/src/reasoning/resolve-reasoning.ts`
- `compozy-code/providers/claude-code/src/tools/bridge.ts`
- `compozy-code/providers/claude-code/src/tools/truncation.ts`
- `compozy-code/providers/claude-code/src/generate/generate.ts`
- `compozy-code/providers/claude-code/src/conversion/messages.ts`
- `compozy-code/providers/claude-code/src/conversion/warnings.ts`
- `compozy-code/providers/claude-code/src/conversion/finish-reason.ts`
- `compozy-code/providers/codex/src/streaming/CodexStreamPipeline.ts`
- `compozy-code/providers/codex/src/streaming/CodexEventDispatcher.ts`
- `compozy-code/providers/codex/src/streaming/tool-payloads.ts`
- `compozy-code/providers/codex/src/model/request-preparation.ts`
- `compozy-code/providers/runtime/src/server/sse-writer.ts`
- `compozy-code/providers/runtime/src/client/runtime-client.ts`
- `compozy-code/providers/runtime/src/session/in-memory-store.ts`
- `compozy-code/providers/runtime/src/usage/consumption.ts`

### Internal Execution Plan

```
2.1 Provider ──┬──> 2.3 Claude Code P1 ──┐
               ├──> 2.4 Codex P1 ────────┤
               └──> 2.2 Truncation       ├──> 2.5 Core/Cross-cutting ──> 2.6 Server/Session
                                          │
```

## Deliverables

- Provider capability validation and model-prefix inference
- Tool output truncation with array/object/string strategies
- Claude Code: tool bridge, generate with recovery, hooks, conversion, permissions, warnings, subagents
- Codex: stream pipeline, dedup, cancellation, tool payloads, request prep, hooks
- Core: usage aggregation in turn loop, model discovery, reasoning resolution
- Server: SSE with sequence IDs + [DONE], RuntimeClient
- Session: TTL + capacity eviction for in-memory store
- `make fmt && make lint && make test` passing

## Tests

### Unit Tests (Required)

- [ ] Provider: validate_capabilities warns on image inputs without capability
- [ ] Provider: model-prefix "claude-3.5-sonnet" infers claude-code provider
- [ ] Truncation: string over max_bytes truncated at UTF-8 boundary
- [ ] Truncation: array elements removed from end to fit budget
- [ ] Truncation: object string values truncated first, then keys removed
- [ ] Truncation: disabled config passes output unchanged
- [ ] Claude Code: tool bridge creates valid MCP server config for CLI
- [ ] Claude Code: generate accumulates text + tool calls + usage
- [ ] Claude Code: generate recovers from truncated JSON stream
- [ ] Claude Code: message conversion handles text and image parts
- [ ] Claude Code: permission mode "acceptEdits" maps to correct flag
- [ ] Claude Code: finish reason "end_turn" maps to Stop
- [ ] Claude Code: temperature param generates unsupported warning
- [ ] Claude Code: subagent config serializes to valid JSON
- [ ] Codex: pipeline emits stream-start on begin
- [ ] Codex: pipeline cancellation token stops stream cleanly
- [ ] Codex: first notification passes dedup, duplicate fingerprint suppressed
- [ ] Codex: command_execution maps to "shell" with correct payload
- [ ] Codex: file_change maps to "apply_patch" with correct payload
- [ ] Codex: error detection from exitCode != 0
- [ ] Codex: mandatory settings enforced in request preparation
- [ ] Core: TurnRuntime accumulates usage from TurnEnd events
- [ ] Core: AgentResponse includes session-total usage
- [ ] Core: ModelDiscoveryService returns models from mock provider
- [ ] Core: Low reasoning effort resolves to correct Claude token budget
- [ ] Core: capability validation warns on incompatible request
- [ ] Server: SSE events have incrementing sequence IDs
- [ ] Server: stream completion emits [DONE]
- [ ] Server: error produces error event with payload
- [ ] Server: RuntimeClient stream_text returns event stream
- [ ] Session: expired entry not returned by load
- [ ] Session: over-capacity evicts oldest entry
- [ ] Session: access refreshes TTL

### Verification Commands

- [ ] `make fmt`
- [ ] `make lint`
- [ ] `make test`

## Success Criteria

- All P1 gaps from analysis documents are resolved
- Provider capability validation prevents incompatible requests at entry
- Both providers have full tool integration, hooks, and subagent support
- Usage tracking flows through the entire turn loop
- SSE follows OpenAI conventions with sequence IDs and [DONE]
- Session store manages memory with TTL and capacity limits
