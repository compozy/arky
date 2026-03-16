## markdown

## status: pending

<task_context>
<domain>engine/core</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>critical</complexity>
<dependencies>task1,task2,task3,task4,task6,task7,task8,task9</dependencies>
</task_context>

# Task 12.0: `arky-core` Crate — Agent Orchestration & Turn Loop

## Overview

Implement the `arky-core` crate — the central orchestration layer providing the `Agent` struct, `AgentBuilder`, command queue, single-turn execution loop, steering/follow-up orchestration, session replay, and tool cleanup. This is the high-level API layer that users interact with most directly. It enforces the critical invariant that the agent must never execute overlapping turns for the same session.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-002)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Agent` struct with methods: `prompt()`, `stream()`, `steer()`, `follow_up()`, `subscribe()`, `new_session()`, `resume()`, `abort()`
- Implement `AgentBuilder` for constructing `Agent` instances with provider, tools, hooks, session store, and config
- Implement internal command queue for serializing `prompt`, `stream`, `steer`, `follow_up`, and `abort` operations
- Enforce single active turn per session invariant (overlapping turns rejected or queued)
- Implement turn loop: receive request -> invoke provider -> process events -> execute tools -> persist state
- Implement steering: inject system-level guidance mid-conversation without creating a new turn
- Implement follow-up: continue conversation after a completed turn
- Implement session replay: load session snapshot, restore state, resume from checkpoint
- Implement tool cleanup: temporary tools unregistered at stream completion (including error and cancellation paths)
- Implement `EventSubscription` wrapper for typed event broadcast
- Implement `AgentResponse` and `AgentEventStream` types
- Implement `CoreError` enum with variants: `BusySession`, `Cancelled`, `InvalidState`, `ReplayFailed` implementing `ClassifiedError`
- Tracing span hierarchy: `agent > session > turn > provider_call > tool_call`
- Dependencies: `arky-error`, `arky-config`, `arky-protocol`, `arky-provider`, `arky-tools`, `arky-hooks`, `arky-session`, `arky-mcp`
</requirements>

## Subtasks

- [ ] 12.1 Implement `AgentBuilder` with provider, tools, hooks, session store, and config registration
- [ ] 12.2 Implement `Agent` struct with all public methods
- [ ] 12.3 Implement command queue for operation serialization (channel-based or actor-style)
- [ ] 12.4 Implement single-turn enforcement: reject or queue overlapping turns
- [ ] 12.5 Implement turn loop: request -> provider stream -> event processing -> tool execution -> state persistence
- [ ] 12.6 Implement `steer()` for injecting system guidance mid-conversation
- [ ] 12.7 Implement `follow_up()` for continuing after completed turns
- [ ] 12.8 Implement `abort()` for cancelling active turns
- [ ] 12.9 Implement session replay: load snapshot, restore state, resume from checkpoint
- [ ] 12.10 Implement tool cleanup on stream completion (success, error, and cancellation paths)
- [ ] 12.11 Implement `EventSubscription` with broadcast receiver
- [ ] 12.12 Implement tracing span hierarchy for observability
- [ ] 12.13 Implement `CoreError` enum with `ClassifiedError` implementation
- [ ] 12.14 Write unit tests for command queue, turn enforcement, and event subscription
- [ ] 12.15 Write integration tests for full turn loop with mock provider

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-core/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-core/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-core/src/agent.rs`
- `~/dev/compozy/arky/crates/arky-core/src/builder.rs`
- `~/dev/compozy/arky/crates/arky-core/src/queue.rs`
- `~/dev/compozy/arky/crates/arky-core/src/turn.rs`
- `~/dev/compozy/arky/crates/arky-core/src/replay.rs`
- `~/dev/compozy/arky/crates/arky-core/src/subscription.rs`
- `~/dev/compozy/arky/crates/arky-core/src/error.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/` — `ClassifiedError` trait
- `~/dev/compozy/arky/crates/arky-config/` — Configuration
- `~/dev/compozy/arky/crates/arky-protocol/` — All shared types
- `~/dev/compozy/arky/crates/arky-provider/` — `Provider` trait, `ProviderRequest`
- `~/dev/compozy/arky/crates/arky-tools/` — `ToolRegistry`, tool execution
- `~/dev/compozy/arky/crates/arky-hooks/` — `Hooks` trait, hook chain
- `~/dev/compozy/arky/crates/arky-session/` — `SessionStore`, replay
- `~/dev/compozy/arky/crates/arky-mcp/` — MCP tool bridge
- `tasks/prd-rust-providers/techspec.md` — Section: Agent Struct, Architectural Invariants
- `tasks/prd-rust-providers/adrs/adr-002-dual-layer-api.md` — Dual-layer API design

## Deliverables

- `Agent` struct with full public API
- `AgentBuilder` for construction
- Command queue with turn serialization
- Turn loop orchestration
- Steering, follow-up, abort, and replay functionality
- Event subscription system
- Tool cleanup on all termination paths
- Tracing instrumentation
- `CoreError` with `ClassifiedError` implementation
- Unit and integration tests

## Tests

### Unit Tests (Required)

- [ ] Command queue: operations are serialized, concurrent submits are queued
- [ ] Single-turn enforcement: second `prompt()` while first is active returns `BusySession`
- [ ] `abort()`: cancels active turn, cleans up resources
- [ ] Event subscription: subscriber receives all events in order
- [ ] Tool cleanup: temporary tools are unregistered after stream completion
- [ ] Tool cleanup on error: temporary tools are unregistered even when stream fails
- [ ] `CoreError` classification: each variant returns correct error codes

### Integration Tests (Required)

- [ ] Full turn loop: mock provider -> agent receives events -> tool execution -> response assembly
- [ ] Steering: inject guidance mid-conversation, verify it reaches provider
- [ ] Follow-up: complete turn, follow up, verify conversation continues
- [ ] Session replay: create session, add messages, save checkpoint, resume, verify state restoration
- [ ] Concurrency: overlapping turns on same session are rejected per invariant

### Regression and Anti-Pattern Guards

- [ ] No overlapping turns allowed for same session (invariant 1)
- [ ] Temporary tools always cleaned up (invariant 4)
- [ ] Session resume restores enough state to continue (invariant 5)
- [ ] No `unwrap()` in library code
- [ ] Tracing spans are properly nested

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-core`

## Success Criteria

- `Agent` API matches techspec exactly
- Single-turn invariant is enforced and tested
- Turn loop orchestrates provider, tools, hooks, and session correctly
- Steering, follow-up, abort, and replay all work
- Tool cleanup happens on all termination paths
- Tracing spans provide full observability
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_12.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
