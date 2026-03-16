# ADR-003: Centralized Error Classifier with Pattern Registry

## Status

Accepted

## Date

2026-03-16

## Context

Error classification is P0 across multiple gap analyses. The TS codebase has sophisticated classifiers in both providers:
- Claude Code: 18 error types with regex patterns, `formatForAgent()` for self-correction
- Codex: 11 error types with regex-based classification (rate_limited, quota_exceeded, etc.)
- Core: `ErrorClassifier` with `isRetryable()`, `extractContext()`, `formatForAgent()`

The Rust `arky-error` crate defines the `ClassifiedError` trait with `error_code`, `is_retryable`, `retry_after`, `http_status`, and `correction_context`. However, there is no pattern-based classification of stderr/error messages, no `formatForAgent()` logic, and the `correction_context` field is never populated.

## Decision

Implement a **centralized `ErrorClassifier` struct in `arky-error`** that accepts a registry of patterns. Each provider registers its own pattern sets (regex + error code + retryability + metadata). The classifier provides:

1. `ErrorClassifier::new()` — creates classifier with empty pattern registry
2. `ErrorClassifier::register_patterns(provider_id, patterns)` — providers register their error patterns
3. `ErrorClassifier::classify(input: &ErrorInput) -> ClassifiedResult` — examines stderr, error messages, status codes, exit codes against registered patterns
4. `ErrorClassifier::format_for_agent(error, attempt) -> String` — produces structured self-correction messages with attempt number, field-level suggestions for validation errors, and actionable guidance

The `format_for_agent()` logic is shared across all providers since the output format is identical — only the input patterns differ.

## Alternatives Considered

### Alternative 1: Per-Provider Independent Classifiers

- **Description**: Each provider crate implements its own classifier with no shared framework
- **Pros**: Simple, no coupling between providers
- **Cons**: Duplicated retry logic, duplicated formatForAgent logic, inconsistent classification interfaces
- **Why rejected**: `formatForAgent()` and retry decision logic are identical across providers

### Alternative 2: Shared Framework + Per-Provider Implementations (Trait-Based)

- **Description**: Define an `ErrorClassifier` trait in `arky-error`, each provider implements it
- **Pros**: Flexible, providers can customize classification logic
- **Cons**: Still duplicates the pattern-matching and formatting infrastructure
- **Why rejected**: Less DRY than the registry approach; pattern matching and formatting are purely data-driven

## Consequences

### Positive

- DRY: retry logic, pattern matching, and `format_for_agent()` written once
- Extensible: new providers add patterns without changing core classifier
- Consistent: all providers produce identical error formatting for agent self-correction
- Testable: classifier tested with mock patterns, provider patterns tested independently

### Negative

- Slightly more upfront design for the registry API
- Providers have a runtime dependency on registering patterns (must be called during init)

### Risks

- Pattern registry might not cover all classification needs (some errors need semantic analysis, not just regex)
- Mitigation: `ClassifiedResult` has an `Unknown` variant; providers can post-process unknown errors with custom logic

## Implementation Notes

- `ErrorInput` struct: `{ stderr: Option<&str>, message: Option<&str>, status_code: Option<u16>, exit_code: Option<i32>, error_code: Option<&str> }`
- `ErrorPattern` struct: `{ regex: Regex, error_code: &str, is_retryable: bool, category: ErrorCategory }`
- `ErrorCategory` enum: `Authentication, RateLimit, QuotaExceeded, ContextWindowExceeded, InvalidRequest, Timeout, SpawnFailure, StreamCorruption, ToolExecution, Network, ApiError, Unknown`
- `format_for_agent()` extracts field-level suggestions from serde validation errors (similar to Zod error formatting in TS)

## References

- TS source: `providers/claude-code/src/classifier/` (3 files)
- TS source: `providers/codex/src/errors/classification.ts`
- TS source: `providers/core/src/error-classifier.ts`
- Gap analysis: `tasks/prd-gaps/analysis_claude_code.md` (Gap #1, #2)
- Gap analysis: `tasks/prd-gaps/analysis_codex.md` (GAP-CDX-005)
- Gap analysis: `tasks/prd-gaps/analysis_core_runtime.md` (Gap #1)
