## markdown

## status: completed

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

## Porting Context

This task uses the shared hook system in
`../compozy-code/providers/core/src/hooks.ts`, with `opencode` as a secondary
reference for additional hook execution patterns, as the main upstream
reference for behavior and edge cases. Do not copy the TypeScript API or
module layout mechanically; prefer the Rust architecture and quality bar
defined in this PRD. Before implementation, read
`tasks/prd-rust-providers/porting-reference.md` and inspect the Task 6.0
upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-009)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 6.0 upstream TypeScript files before implementation
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

- [x] 6.1 Define all hook context types (`BeforeToolCallContext`, `AfterToolCallContext`, etc.)
- [x] 6.2 Define all hook result types (`Verdict`, `ToolResultOverride`, `StopDecision`, etc.)
- [x] 6.3 Implement `Hooks` trait with all 6 methods and `CancellationToken` support
- [x] 6.4 Implement `HookChain` with concurrent invocation and registration-order result reordering
- [x] 6.5 Implement merge semantics for each hook method (first-Block-wins, last-write-wins, append, any-Continue)
- [x] 6.6 Implement shell hook execution (spawn subprocess, JSON on stdin, parse response)
- [x] 6.7 Implement timeout handling and cancellation propagation for hook execution
- [x] 6.8 Implement configurable fail-open/fail-closed behavior with structured diagnostics
- [x] 6.9 Implement `HookError` enum with `ClassifiedError` implementation
- [x] 6.10 Write unit tests for merge semantics, timeout handling, fail-open/fail-closed, and shell hooks

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

- [x] `before_tool_call` merge: first `Block` verdict wins, `Allow` verdicts are ignored after block
- [x] `after_tool_call` merge: overrides merge in registration order, last write wins per field
- [x] `session_start` merge: env/settings merge shallowly, injected messages append in order
- [x] `user_prompt_submit` merge: last prompt rewrite wins, injected messages append
- [x] `on_stop` merge: any `Continue` decision blocks termination
- [x] Concurrent invocation: hooks run concurrently, results re-ordered by registration order
- [x] Timeout: hook exceeding timeout returns `HookError::Timeout`
- [x] Cancellation: cancellation token propagates to running hooks
- [x] Fail-open: hook error with fail-open config logs diagnostic and continues
- [x] Fail-closed: hook error with fail-closed config propagates error
- [x] Shell hook: JSON input/output round-trip through subprocess

### Integration Tests (Required)

- [x] `HookChain` with 3+ hooks: full merge pipeline for each hook method
- [x] Shell hook: spawn real subprocess, send JSON, parse response

### Regression and Anti-Pattern Guards

- [x] All hook methods are `Send + Sync`
- [x] No `unwrap()` in library code
- [x] Shell hook process cleanup on timeout/cancellation

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky-hooks`

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
