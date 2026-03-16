## markdown

## status: completed

<task_context>
<domain>crates/arky-error,arky-protocol,arky-usage,arky-claude-code,arky-codex,arky-server</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>critical</complexity>
<dependencies>none</dependencies>
</task_context>

# Task 1.0: Phase 1 (P0) — Production Blockers

## Overview

Implement all P0 (production-blocking) features across the arky workspace: centralized error classification with pattern registry, reasoning event variants and protocol types, a new arky-usage crate for token tracking and cost calculation, Claude Code and Codex provider P0 enhancements (error patterns, reasoning parsing, config expansion, server registry, event dispatcher, model service), and server P0 endpoints (chat streaming, model listing, auth middleware).

<critical>
- **ALWAYS READ** @CLAUDE.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the techspec at `tasks/prd-gaps/techspec.md`
- **YOU CAN ONLY** finish when `make fmt && make lint && make test` all pass at 100%
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- ErrorClassifier with regex-based pattern registry, ErrorPattern, ErrorCategory, format_for_agent()
- AgentEvent reasoning variants (ReasoningStart, ReasoningDelta, ReasoningComplete)
- ReasoningEffort enum, FinishReason enum, ProviderCapabilities expansion
- New arky-usage crate: NormalizedUsage, UsageAggregator, ModelCost, ProviderMetadataExtractor trait
- Claude Code: 18 error patterns, reasoning block parsing, config schema (~60 fields)
- Codex: CodexServerRegistry with ref-counting and idle shutdown, event dispatcher (40+ events), text accumulator reasoning, config schema (~40 fields), model service via RPC
- Server: POST /v1/chat/stream, GET /v1/models, bearer token auth middleware, SSE-based streaming
</requirements>

## Subtasks

### 1.1 ErrorClassifier with Pattern Registry

- [x] 1.1.1 Define ErrorCategory enum (Authentication, Authorization, RateLimit, Timeout, NetworkError, ParseError, StreamCorruption, ToolExecution, SpawnFailure, ConfigInvalid, ModelNotFound, ContextLength, ContentFilter, ServerOverloaded, InternalError, Unknown)
- [x] 1.1.2 Define ErrorPattern struct (name, regex, category, is_retryable, retry_after_hint)
- [x] 1.1.3 Implement ErrorClassifier with pattern registration and classify() method
- [x] 1.1.4 Implement format_for_agent(error, attempt) producing structured retry messages
- [x] 1.1.5 Unit tests: pattern matching, classification, format_for_agent output

### 1.2 AgentEvent Reasoning Variants and Protocol Types

- [x] 1.2.1 Add ReasoningStart, ReasoningDelta, ReasoningComplete to AgentEvent enum
- [x] 1.2.2 Define ReasoningEffort enum (Low, Medium, High, XHigh)
- [x] 1.2.3 Define FinishReason enum (Stop, Length, ToolUse, ContentFilter, Error, Unknown)
- [x] 1.2.4 Expand ProviderCapabilities with image_inputs, extended_thinking, code_execution
- [x] 1.2.5 Unit tests: serde round-trip for new variants and enums

### 1.3 arky-usage Crate

- [x] 1.3.1 Create crate with NormalizedUsage struct (input, output, cached, reasoning breakdowns)
- [x] 1.3.2 Implement UsageAggregator (per-turn accumulation, session totals, merge)
- [x] 1.3.3 Implement ModelCost struct and compute_estimated_cost() per model family
- [x] 1.3.4 Define ProviderMetadataExtractor trait (session_id, cost_usd, duration_ms, raw_usage, warnings)
- [x] 1.3.5 Unit tests: usage accumulation, cost computation, metadata extraction

### 1.4 Claude Code Provider P0

- [x] 1.4.1 Register 18 error patterns (authentication, rate_limit, overloaded, network, timeout, json_parse, stream_corrupt, tool_execution, spawn_failure, context_length, content_filter, model_not_found, authorization, config_invalid, server_error, process_crash, binary_missing, permission_denied)
- [x] 1.4.2 Parse thinking/reasoning blocks in stream parser (content_block_start type:"thinking", content_block_delta type:"thinking_delta")
- [x] 1.4.3 Emit ReasoningStart/ReasoningDelta/ReasoningComplete AgentEvents
- [x] 1.4.4 Expand ClaudeCodeProviderConfig to ~60 fields with serde validation
- [x] 1.4.5 Serialize config to CLI args (--max-turns, --max-thinking-tokens, --permission-mode, --agents, --hooks, --mcp-server, --plugin, --sandbox, etc.)
- [x] 1.4.6 Unit tests: error pattern matching against fixture stderr, reasoning parsing from fixture streams, config serialization to CLI args

### 1.5 Codex Provider P0

- [x] 1.5.1 Implement CodexServerRegistry (acquire/release with ref-counting, idle timeout, reconfigure)
- [x] 1.5.2 Implement CodexAppServer (long-lived process, initialize handshake, graceful shutdown)
- [x] 1.5.3 Implement event dispatcher for 40+ notification types (16 dispatch categories)
- [x] 1.5.4 Implement text accumulator reasoning (reasoning_start/delta/complete with UUIDs)
- [x] 1.5.5 Expand CodexProviderConfig to ~40 fields with serde validation
- [x] 1.5.6 Implement CodexModelService via models/list RPC method
- [x] 1.5.7 Wire registry into provider (replace spawn-per-stream with shared server)
- [x] 1.5.8 Unit tests: registry lifecycle (acquire/release/idle/reconfigure), event dispatcher for all types, text accumulator reasoning, config override building, model service with mock RPC

### 1.6 Server P0

- [x] 1.6.1 Implement POST /v1/chat/stream with ChatStreamRequest validation
- [x] 1.6.2 Implement SSE response streaming from Agent -> Provider -> events
- [x] 1.6.3 Implement GET /v1/models with OpenAI-compatible ModelList response
- [x] 1.6.4 Implement bearer token auth middleware with timing-safe comparison (subtle crate)
- [x] 1.6.5 Unit tests: chat stream request validation, auth middleware (valid/invalid/missing), model listing format

## Implementation Details

### Relevant Files

- `crates/arky-error/src/classifier.rs` - NEW: ErrorClassifier
- `crates/arky-protocol/src/event.rs` - AgentEvent expansion
- `crates/arky-protocol/src/request.rs` - ReasoningEffort, FinishReason
- `crates/arky-usage/` - NEW CRATE
- `crates/arky-claude-code/src/classifier.rs` - NEW: error patterns
- `crates/arky-claude-code/src/parser.rs` - reasoning block parsing
- `crates/arky-claude-code/src/config.rs` - NEW: full config schema
- `crates/arky-codex/src/registry.rs` - NEW: CodexServerRegistry
- `crates/arky-codex/src/app_server.rs` - NEW: CodexAppServer
- `crates/arky-codex/src/dispatcher.rs` - NEW: event dispatcher
- `crates/arky-codex/src/config.rs` - NEW: full config schema
- `crates/arky-codex/src/model_service.rs` - NEW: model listing
- `crates/arky-server/src/routes/chat.rs` - NEW: chat stream
- `crates/arky-server/src/routes/models.rs` - NEW: model listing
- `crates/arky-server/src/middleware.rs` - NEW: bearer auth

### TS Reference Files

- `compozy-code/providers/core/src/error-classifier.ts`
- `compozy-code/providers/core/src/token-consumption.ts`
- `compozy-code/providers/claude-code/src/classifier/`
- `compozy-code/providers/claude-code/src/schemas.ts`
- `compozy-code/providers/claude-code/src/stream/event-normalizer.ts`
- `compozy-code/providers/codex/src/server/CodexRegistry.ts`
- `compozy-code/providers/codex/src/server/CodexAppServer.ts`
- `compozy-code/providers/codex/src/streaming/CodexEventDispatcher.ts`
- `compozy-code/providers/codex/src/config/schemas.ts`
- `compozy-code/providers/codex/src/server/CodexModelService.ts`
- `compozy-code/providers/runtime/src/server/app.ts`
- `compozy-code/providers/runtime/src/server/auth.ts`
- `compozy-code/providers/runtime/src/server/routes-models.ts`

### Internal Execution Plan

```
1.1 ErrorClassifier ─────┐
                          ├─> 1.3 arky-usage ─┬─> 1.4 Claude Code P0 ─┐
1.2 Protocol Types ───────┘                   │                        ├─> 1.6 Server P0
                                              └─> 1.5 Codex P0 ───────┘
```

## Deliverables

- ErrorClassifier with pattern registry and format_for_agent
- AgentEvent reasoning variants, ReasoningEffort, FinishReason enums
- arky-usage crate with NormalizedUsage, UsageAggregator, ModelCost
- Claude Code: error patterns, reasoning parsing, config schema
- Codex: server registry, event dispatcher, config schema, model service
- Server: /v1/chat/stream, /v1/models, bearer auth
- `make fmt && make lint && make test` passing

## Tests

### Unit Tests (Required)

- [x] ErrorClassifier: pattern matching produces correct ErrorCategory
- [x] ErrorClassifier: format_for_agent with attempt number and field suggestions
- [x] ErrorClassifier: unmatched error classifies as Unknown
- [x] AgentEvent: serde round-trip for ReasoningStart/Delta/Complete
- [x] ReasoningEffort: serde variants (low/medium/high/xhigh)
- [x] FinishReason: all variants serialize correctly
- [x] Usage: NormalizedUsage accumulation across turns
- [x] Usage: UsageAggregator merge produces correct session totals
- [x] Usage: ModelCost computation for claude-sonnet, gpt-4o
- [x] Claude Code: 18 error patterns match against fixture stderr strings
- [x] Claude Code: reasoning block parsing from thinking content blocks
- [x] Claude Code: config serialization produces correct CLI args
- [x] Codex: registry acquire returns same server for same config
- [x] Codex: registry release decrements refcount, idle timeout triggers shutdown
- [x] Codex: event dispatcher handles all 40+ notification types
- [x] Codex: text accumulator tracks reasoning lifecycle
- [x] Codex: model service parses models/list response
- [x] Server: POST /v1/chat/stream validates request schema
- [x] Server: bearer auth rejects invalid/missing tokens
- [x] Server: GET /v1/models returns OpenAI-compatible format

### Integration Tests (Required)

- [x] End-to-end: Agent -> Provider -> stream -> events -> SSE (fixture CLI output)
- [x] Codex: registry acquire -> multiple turns -> idle shutdown -> re-acquire
- [x] Error classification: inject fixture stderr -> verify classified code and retryability

### Verification Commands

- [x] `make fmt`
- [x] `make lint`
- [x] `make test`

## Success Criteria

- All P0 gaps from analysis documents are resolved
- Error classification provides structured retry guidance
- Usage tracking enables per-turn and per-session cost visibility
- Both providers have full config schemas and enhanced streaming
- Server exposes OpenAI-compatible chat and model endpoints
