# ADR-001: Full Parity Scope Strategy

## Status

Accepted

## Date

2026-03-16

## Context

The Rust arky SDK was ported from the TypeScript compozy-code providers but covers only 25-60% of the TS feature surface depending on the domain. We needed to decide how much of the TS codebase to port: full parity, functional parity (critical features only), or MVP (P0 gaps only).

Gap analysis results:
- Core & Runtime: ~55-60% coverage
- Claude Code provider: ~25-30% coverage
- Codex provider: ~35-40% coverage
- Server/Session/Usage: ~40-45% coverage

## Decision

Pursue **full parity** with the TypeScript compozy-code providers. All features present in the TS codebase will be ported to Rust, including OpenAI-compatible endpoints, runtime client SDK, xhigh reasoning detection, model cost tracking, and all configuration options.

Estimated effort: ~15-20k additional lines of Rust.

## Alternatives Considered

### Alternative 1: Functional Parity (Critical Features Only)

- **Description**: Port only features that block real SDK usage — error classification, usage tracking, reasoning blocks, config expansion, tool bridge wiring, subagents
- **Pros**: Lower effort (~8-12k lines), faster time to usable SDK
- **Cons**: Missing convenience features, incomplete API surface, consumers hit gaps
- **Why rejected**: Incomplete parity creates friction for consumers migrating from TS

### Alternative 2: MVP Pragmatic (P0 Only)

- **Description**: Only close P0 gaps — error classification, usage tracking, streaming event dispatcher, minimal config
- **Pros**: Minimal effort (~4-6k lines), fast delivery
- **Cons**: Many important features missing (reasoning, generate, auth, model discovery), not production-ready
- **Why rejected**: Too limited for production use cases

## Consequences

### Positive

- Complete API surface parity with TS — consumers can migrate without feature loss
- Rust SDK becomes a drop-in replacement for all TS provider functionality
- No "missing feature" surprises for consumers

### Negative

- Significant implementation effort (~15-20k lines)
- Longer timeline to completion
- Some ported features may have low immediate usage

### Risks

- Scope creep if TS codebase continues evolving during port
- Mitigation: snapshot the TS feature set at time of decision, track delta separately

## References

- Gap analyses: `tasks/prd-gaps/analysis_*.md`
- Original techspec: `compozy-code/tasks/prd-rust-providers/techspec.md`
