## markdown

## status: pending

<task_context>
<domain>engine/hooks</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2,task4</dependencies>
</task_context>

# Task 6.0: `arky-hooks` Crate — Hook System & Merge Semantics

## Overview

Implement the `arky-hooks` crate providing the `Hooks` trait, `HookChain` composition, shell hook execution, merge semantics, timeout and isolation rules. The hook system provides complete lifecycle coverage for agent execution: before/after tool calls, session start/end, stop decisions, and prompt submission. Composition rules are mandatory and must be implemented exactly as specified — they are not optional.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-009)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Hooks` trait with all 6 hook methods: `before_tool_call`, `after_tool_call`, `session_start`, `session_end`, `on_stop`, `user_prompt_submit`
- All hook methods receive a `CancellationToken` (except `session_end`)
- Implement `HookChain` for composing multiple hook implementations with defined merge semantics
- Mandatory merge rules: `before_tool_call` (first `Block` wins), `after_tool_call` (last write wins per field), `session_start` (env/settings merge shallowly, messages append), `user_prompt_submit` (last prompt rewrite wins, messages append), `on_stop` (any `Continue` blocks termination)
- Hooks are invoked concurrently per event, results re-ordered into registration order before merge
- Implement shell hook support: receive JSON on stdin, return JSON or plain text
- Implement timeout and cancellation for all hooks
- Implement configurable fail-open vs fail-closed behavior per hook chain (default: fail-open for shell hooks)
- Implement context types: `BeforeToolCallContext`, `AfterToolCallContext`, `SessionStartContext`, `SessionEndContext`, `StopContext`, `PromptSubmitContext`
- Implement result types: `Verdict`, `ToolResultOverride`, `SessionStartUpdate`, `StopDecision`, `PromptUpdate`
- Implement `HookError` enum with variants: `ExecutionFailed`, `Timeout`, `InvalidOutput`, `PanicIsolated` implementing `ClassifiedError`
- Dependencies: `arky-error`, `arky-protocol`, `arky-tools`
</requirements>

## Subtasks

- [ ] 6.1 Define all hook context types (`BeforeToolCallContext`, `AfterToolCallContext`, etc.)
- [ ] 6.2 Define all hook result types (`Verdict`, `ToolResultOverride`, `StopDecision`, etc.)
- [ ] 6.3 Implement `Hooks` trait with all 6 methods and `CancellationToken` support
- [ ] 6.4 Implement `HookChain` with concurrent invocation and registration-order result reordering
- [ ] 6.5 Implement merge semantics for each hook method (first-Block-wins, last-write-wins, append, any-Continue)
- [ ] 6.6 Implement shell hook execution (spawn subprocess, JSON on stdin, parse response)
- [ ] 6.7 Implement timeout handling and cancellation propagation for hook execution
- [ ] 6.8 Implement configurable fail-open/fail-closed behavior with structured diagnostics
- [ ] 6.9 Implement `HookError` enum with `ClassifiedError` implementation
- [ ] 6.10 Write unit tests for merge semantics, timeout handling, fail-open/fail-closed, and shell hooks

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-hooks/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-hooks/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-hooks/src/context.rs`
- `~/dev/compozy/arky/crates/arky-hooks/src/result.rs`
- `~/dev/compozy/arky/crates/arky-hooks/src/chain.rs`
- `~/dev/compozy/arky/crates/arky-hooks/src/shell.rs`
- `~/dev/compozy/arky/crates/arky-hooks/src/error.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/src/lib.rs` — `ClassifiedError` trait
- `~/dev/compozy/arky/crates/arky-protocol/` — Message, event, and tool types
- `~/dev/compozy/arky/crates/arky-tools/` — Tool types for `before/after_tool_call` context
- `tasks/prd-rust-providers/techspec.md` — Section: Hooks Trait
- `tasks/prd-rust-providers/adrs/adr-009-hook-system.md` — Hook system design

## Deliverables

- `Hooks` trait with all 6 hook methods
- `HookChain` with correct merge semantics for each hook type
- Shell hook execution support
- Timeout/cancellation handling
- Configurable fail-open/fail-closed behavior
- All context and result types
- Unit tests covering all merge rules and failure modes

## Tests

### Unit Tests (Required)

- [ ] `before_tool_call` merge: first `Block` verdict wins, `Allow` verdicts are ignored after block
- [ ] `after_tool_call` merge: overrides merge in registration order, last write wins per field
- [ ] `session_start` merge: env/settings merge shallowly, injected messages append in order
- [ ] `user_prompt_submit` merge: last prompt rewrite wins, injected messages append
- [ ] `on_stop` merge: any `Continue` decision blocks termination
- [ ] Concurrent invocation: hooks run concurrently, results re-ordered by registration order
- [ ] Timeout: hook exceeding timeout returns `HookError::Timeout`
- [ ] Cancellation: cancellation token propagates to running hooks
- [ ] Fail-open: hook error with fail-open config logs diagnostic and continues
- [ ] Fail-closed: hook error with fail-closed config propagates error
- [ ] Shell hook: JSON input/output round-trip through subprocess

### Integration Tests (Required)

- [ ] `HookChain` with 3+ hooks: full merge pipeline for each hook method
- [ ] Shell hook: spawn real subprocess, send JSON, parse response

### Regression and Anti-Pattern Guards

- [ ] All hook methods are `Send + Sync`
- [ ] No `unwrap()` in library code
- [ ] Shell hook process cleanup on timeout/cancellation

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-hooks`

## Success Criteria

- All 6 hook methods implemented with correct signatures
- Merge semantics match techspec exactly for each hook type
- Shell hooks work end-to-end
- Timeout and cancellation handling is robust
- Fail-open/fail-closed is configurable and tested
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_6.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
