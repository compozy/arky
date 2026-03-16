# ADR-008: Reasoning/Thinking Blocks as First-Class `AgentEvent` Variants

## Status

Accepted

## Date

2026-03-16

## Context

Extended thinking / reasoning is a core capability of modern LLMs:
- Claude with extended thinking emits `content_block_start` with `type: "thinking"` and `content_block_delta` with `type: "thinking_delta"`
- Codex / OpenAI models emit reasoning tokens with their own lifecycle (start/delta/complete)

The TS providers handle reasoning as first-class events:
- Claude Code: `ReasoningStart`, `ReasoningDelta`, `ReasoningComplete` normalized events
- Codex: reasoning text accumulator with UUID-based part tracking and lifecycle

The Rust `arky-protocol` `AgentEvent` enum has 11 variants but none for reasoning. The Claude Code parser silently drops thinking blocks. The Codex accumulator has no reasoning support.

## Decision

Add **three new variants** to `AgentEvent` in `arky-protocol`:

```rust
enum AgentEvent {
    // ... existing variants ...
    ReasoningStart {
        reasoning_id: String,
        metadata: EventMetadata,
    },
    ReasoningDelta {
        reasoning_id: String,
        delta: String,
        metadata: EventMetadata,
    },
    ReasoningComplete {
        reasoning_id: String,
        full_text: String,
        metadata: EventMetadata,
    },
}
```

Each provider is responsible for detecting reasoning blocks in its stream and emitting these events:
- `arky-claude-code`: detect `thinking` content blocks in `parse_stream_content_block_start/delta/stop`
- `arky-codex`: detect reasoning notifications in the event dispatcher

## Alternatives Considered

### Alternative 1: Metadata Flag on Existing Events

- **Description**: Add `is_reasoning: bool` to `AgentEvent::TextDelta` and use existing text events
- **Pros**: No new variants, simpler enum
- **Cons**: Loses lifecycle semantics (start/complete), consumers can't distinguish reasoning text boundaries, no reasoning_id for correlation
- **Why rejected**: Reasoning has its own lifecycle independent of regular text; mixing them loses important semantic information

## Consequences

### Positive

- First-class reasoning support: consumers can filter, display, or track reasoning independently
- Lifecycle semantics: clear start/delta/complete boundaries with correlation ID
- Consistent across providers: same event types from Claude Code and Codex

### Negative

- `AgentEvent` enum grows from 11 to 14 variants
- All event consumers (server SSE, session store, replay) must handle new variants
- Serialization format grows

### Risks

- Provider inconsistency: different providers may emit reasoning with different granularity
- Mitigation: normalize at the provider level before emitting AgentEvent

## Implementation Notes

- `reasoning_id`: UUID generated at `ReasoningStart`, reused for subsequent Delta/Complete
- Claude Code parser changes: `parse_stream_content_block_start` handles `type: "thinking"`, `parse_stream_content_block_delta` handles `type: "thinking_delta"`
- Codex accumulator changes: add reasoning text lifecycle tracking similar to TS `CodexTextAccumulator`
- SSE mapping: `reasoning-start`, `reasoning-delta`, `reasoning-complete` event names
- Session store: reasoning events persisted alongside other events

## References

- TS source: `providers/claude-code/src/stream/normalized-events.ts` (ReasoningStart/Delta/Complete)
- TS source: `providers/claude-code/src/stream/event-normalizer.ts` (thinking block handling)
- TS source: `providers/codex/src/streaming/CodexTextAccumulator.ts` (reasoning lifecycle)
- Gap analysis: `tasks/prd-gaps/analysis_claude_code.md` (Gap #3)
- Gap analysis: `tasks/prd-gaps/analysis_codex.md` (GAP-CDX-001, GAP-CDX-003)
