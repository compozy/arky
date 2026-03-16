# ADR-002: Dedicated `arky-usage` Crate for Token/Usage Tracking

## Status

Accepted

## Date

2026-03-16

## Context

Usage/token tracking is the single most consistently identified P0 gap across all four gap analyses (core_runtime, claude_code, codex, server_session_usage). The TS codebase has a comprehensive pipeline spanning 5+ files: `NormalizedTokenConsumption`, cache breakdown (read/write/noCache), cost in USD, per-provider metadata extraction, and incremental chunk accumulation.

The Rust `arky-protocol` already has a basic `Usage` struct with `input_tokens` and `output_tokens`, but no normalization, cost calculation, provider-specific metadata extraction, or aggregation logic.

Usage tracking is cross-cutting: providers emit raw usage, `arky-core` accumulates per-turn/session, `arky-server` exposes usage in API responses, and a future `arky-usage` consumer may need it for billing.

## Decision

Create a new **`arky-usage`** crate dedicated to token consumption tracking, normalization, cost calculation, and usage aggregation. Types live in this crate (not in `arky-protocol`), and the crate provides:

1. `NormalizedUsage` — normalized token consumption with cache breakdown and reasoning tokens
2. `UsageAggregator` — accumulates usage across chunks, turns, and sessions
3. `ProviderMetadataExtractor` — extracts provider-specific metadata (sessionId, costUsd, durationMs, warnings)
4. `ModelCost` — per-model pricing and cost computation
5. Provider-specific normalization (different providers expose usage in different metadata keys)

## Alternatives Considered

### Alternative 1: Extend `arky-protocol` (types) + module in `arky-core` (logic)

- **Description**: Put usage types in `arky-protocol` and normalization/aggregation logic as a module in `arky-core`
- **Pros**: Fewer crates, co-located with the turn loop that consumes it
- **Cons**: `arky-server` would need to depend on `arky-core` just for usage types, circular dependency risk, bloats `arky-core`
- **Why rejected**: Usage is cross-cutting — providers, core, and server all need it. A dedicated crate avoids dependency issues.

## Consequences

### Positive

- Clean dependency graph: providers -> arky-usage, core -> arky-usage, server -> arky-usage
- Testable in isolation — usage normalization can be tested without provider or core dependencies
- Clear ownership boundary for billing/metering concerns

### Negative

- One more crate in the workspace (15th crate)
- Need to wire the new crate into existing Cargo.toml dependencies

### Risks

- Over-engineering if usage tracking stays simple
- Mitigation: start with minimal API surface, expand as providers register their metadata keys

## Implementation Notes

- `arky-protocol::Usage` struct should be re-exported or replaced by `arky-usage::NormalizedUsage`
- Each provider crate registers its metadata key mappings (e.g., `anthropic.cacheCreationInputTokens`)
- Integration point: `arky-core/src/turn.rs` calls `UsageAggregator` after each stream chunk and on turn complete

## References

- TS source: `providers/runtime/src/usage/` (5 files)
- TS source: `providers/core/src/token-consumption.ts`
- Gap analysis: `tasks/prd-gaps/analysis_core_runtime.md` (Gap #2)
- Gap analysis: `tasks/prd-gaps/analysis_server_session_usage.md` (GAP-SSU-001)
