# ADR-009: Generic `generate_from_stream` Helper with Provider Override

## Status

Accepted

## Date

2026-03-16

## Context

The TS providers implement `doGenerate()` — a non-streaming endpoint that runs the stream internally and accumulates results into a single response (final text, tool calls, usage, structured output). This is used for batch processing, simple tool calls, and cases where streaming is unnecessary.

The Rust `Provider` trait has `generate()` declared but not implemented. The `GenerateResponse` type exists in `arky-protocol`.

Claude Code's generate has provider-specific logic: truncation recovery (when stream ends with incomplete JSON, it attempts to salvage partial results).

## Decision

Implement a **two-layer approach**:

1. **Generic helper** in `arky-provider`: `async fn generate_from_stream(provider, request) -> Result<GenerateResponse>` that consumes any provider's `stream()` output and accumulates text, tool calls, usage, and finish reason into `GenerateResponse`

2. **Provider override**: the `Provider` trait's `generate()` method has a default implementation that calls `generate_from_stream()`, but providers can override it with custom logic. Claude Code overrides to add truncation recovery.

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    async fn stream(&self, request: ProviderRequest)
        -> Result<ProviderEventStream>;

    async fn generate(&self, request: ProviderRequest)
        -> Result<GenerateResponse> {
        generate_from_stream(self, request).await
    }
}
```

## Alternatives Considered

### Alternative 1: Implement Generate in Each Provider Independently

- **Description**: Each provider writes its own generate logic
- **Pros**: Full control per provider
- **Cons**: Duplicated accumulation logic (~200 lines per provider), inconsistent behavior
- **Why rejected**: 90% of generate logic is identical (consume stream, accumulate)

### Alternative 2: Generic Helper Only (No Override)

- **Description**: Single `generate_from_stream()` function, no provider customization
- **Pros**: Simplest, DRY
- **Cons**: Cannot handle provider-specific concerns (Claude Code truncation recovery)
- **Why rejected**: Claude Code needs truncation recovery which is provider-specific

## Consequences

### Positive

- DRY: accumulation logic written once
- Extensible: providers can override for custom behavior
- Default works for most cases: Codex and future providers get generate for free

### Negative

- Two code paths to maintain (generic + override)
- Override providers must be careful not to diverge from generic behavior

### Risks

- Accumulation edge cases (e.g., interleaved text and tool calls, partial JSON)
- Mitigation: comprehensive tests with fixture streams

## Implementation Notes

- `generate_from_stream()` accumulates: `text: String`, `tool_calls: Vec<ToolCall>`, `usage: NormalizedUsage`, `finish_reason: FinishReason`, `reasoning_text: Option<String>`
- Handles all `AgentEvent` variants including `ReasoningDelta` (ADR-008)
- Claude Code override: wraps generic helper with `catch` for `StreamCorrupted` errors, attempts truncation recovery by parsing partial JSON
- Default trait implementation uses `generate_from_stream` so providers only implement `stream()`

## References

- TS source: `providers/claude-code/src/generate/generate.ts`
- TS source: `providers/claude-code/src/utils.ts` (isClaudeCodeTruncationError)
- Gap analysis: `tasks/prd-gaps/analysis_claude_code.md` (Gap #5)
- Gap analysis: `tasks/prd-gaps/analysis_core_runtime.md` (Agent orchestration)
