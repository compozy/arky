# ADR-010: Implementation Phasing by Priority (P0 -> P1 -> P2)

## Status

Accepted

## Date

2026-03-16

## Context

The full parity scope (ADR-001) encompasses ~15-20k lines across 4 domains. We need a phasing strategy that delivers maximum value incrementally while maintaining a buildable, testable codebase at each phase boundary.

Three strategies were considered: bottom-up by layer, vertical slices by provider, and cross-cutting by priority.

## Decision

Phase implementation **by priority level (P0 -> P1 -> P2), cross-cutting across all domains**. Each phase delivers the most critical gaps across all crates simultaneously, ensuring the entire SDK improves uniformly rather than one provider being complete while others remain broken.

### Phase 1: P0 — Production Blockers

All gaps that prevent production use of the SDK.

**New crate:**
- `arky-usage` — token consumption, normalization, cost calculation, provider metadata extraction

**arky-error:**
- Centralized `ErrorClassifier` with pattern registry
- `format_for_agent()` for self-correction messages
- `ErrorCategory` enum, `ErrorPattern` struct, `ErrorInput`/`ClassifiedResult` types

**arky-protocol:**
- `AgentEvent::ReasoningStart/Delta/Complete` variants
- `ReasoningEffort` enum (low/medium/high/xhigh)
- Enhanced `Usage` type integration with `arky-usage`

**arky-claude-code:**
- Error classification patterns (18 error types)
- Reasoning/thinking block parsing
- Config schema expansion (all ~60 fields typed)
- Stream parsing: tool_error, tool_approval_response, structured_output

**arky-codex:**
- Streaming event dispatcher (40+ events)
- Text accumulator reasoning support
- Config schema expansion (all ~40 fields typed)
- Model service (models/list RPC with caching)
- Server registry with ref-counting and idle shutdown

**arky-server:**
- `POST /v1/chat/stream` endpoint with SSE
- Bearer token auth middleware
- `GET /v1/models` endpoint

### Phase 2: P1 — Important Completeness

Features needed for complete SDK functionality.

- Generate endpoint (generic helper + Claude Code override with truncation recovery)
- Tool bridge / MCP wiring (Claude Code + Codex)
- Hooks system integration in both providers
- Capability validation (`validate_capabilities()`)
- Provider model-prefix inference in registry
- Tool output truncation
- Codex stream pipeline (abort signal, finalization, state machine)
- Codex duplicate detection (fingerprinting)
- Codex cancellation/abort support
- Codex tool payload builders (per-type input/result)
- Codex request preparation pipeline
- Runtime usage aggregation (per-turn, per-session)
- Token consumption from chunks and results
- Provider metadata extraction
- Model discovery service
- Reasoning effort resolution per provider
- SSE writer enhancements (sequence IDs, `[DONE]` sentinel)
- Runtime client abstraction
- Subagent config passthrough (Claude Code + Codex)
- Permission modes (Claude Code)
- Message conversion with image support
- Finish reason mapping
- Warnings system
- In-memory session store TTL and capacity
- Structured output / JSON mode

### Phase 3: P2 — Polish and Completeness

Nice-to-have features for full parity.

- Model cost computation
- xhigh reasoning detection
- Plugin support (Claude Code)
- Sandbox support (Claude Code)
- Streaming input / message injection
- Stream recovery on truncation
- Nested tool preview events
- Tool input serialization with size limits
- Tool extraction helpers
- MCP custom/combined server creation
- Debug/verbose configuration
- Environment variable passthrough
- Settings validation warnings
- Image content handling
- Codex thread compaction
- Codex scheduler queue overflow protection
- Codex environment sanitization
- Provider family gateway classification
- Compound session key lookup
- Configuration validation with rich schemas
- Runtime error union type
- Native event utility helpers

## Alternatives Considered

### Alternative 1: Bottom-Up by Layer

- **Description**: Phase 1 foundation (types, error), Phase 2 providers, Phase 3 infrastructure
- **Pros**: Each layer testable before building on top
- **Cons**: Visible value only in Phase 2; foundation work without consumers feels speculative
- **Why rejected**: Delays value delivery; consumers can't use anything until Phase 2

### Alternative 2: Vertical Slices by Provider

- **Description**: Phase 1 Claude Code complete, Phase 2 Codex complete, Phase 3 infrastructure
- **Pros**: One fully functional provider early
- **Cons**: Shared infrastructure (error classifier, usage tracking) built for one provider, potentially reworked for the second
- **Why rejected**: Risk of rework when shared concerns are designed for one provider only

## Consequences

### Positive

- Most critical gaps (error classification, usage tracking, reasoning) closed first across all providers
- Each phase delivers a testable, measurably better SDK
- Shared infrastructure built for all consumers simultaneously

### Negative

- Context switching between crates within each phase
- Requires careful dependency ordering within phases
- Phase boundaries may shift as implementation reveals dependencies

### Risks

- Phase 1 scope may be too large for a single sprint
- Mitigation: within Phase 1, order by dependency (arky-usage and arky-error first, then protocol, then providers, then server)
- Cross-crate changes may cause integration issues
- Mitigation: `make verify` at every checkpoint

## References

- Gap analyses: `tasks/prd-gaps/analysis_*.md`
- All ADRs in this directory
