## markdown

## status: completed

<task_context>
<domain>engine/provider</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2,task4,task6,task7</dependencies>
</task_context>

# Task 8.0: `arky-provider` Crate — Provider Trait & Contract Test Suite

## Overview

Implement the `arky-provider` crate defining the `Provider` trait, `ProviderRequest`, `ProviderDescriptor`, capability descriptors, provider registry, and the shared contract test suite that every provider implementation must pass. The provider trait must be rich enough for CLI-wrapper providers to receive session, tool, hook, and turn context. This crate also provides shared infrastructure: `ProcessManager`, `StdioTransport`, `ToolIdCodec`, and `ReplayWriter`.

## Porting Context

This task uses the TypeScript runtime and concrete provider implementations in
`../compozy-code/providers/runtime`,
`../compozy-code/providers/claude-code`, and
`../compozy-code/providers/codex` as the main upstream reference for behavior
and edge cases. Do not copy the TypeScript API or module layout mechanically;
prefer the Rust architecture and quality bar defined in this PRD. Before
implementation, read `tasks/prd-rust-providers/porting-reference.md` and
inspect the Task 8.0 upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-002, ADR-003)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 8.0 upstream TypeScript files before implementation
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Provider` trait with `descriptor()`, `stream()`, and `generate()` methods
- `stream()` returns `ProviderEventStream` = `Pin<Box<dyn Stream<Item = Result<AgentEvent, ProviderError>> + Send>>`
- `generate()` returns `Result<GenerateResponse, ProviderError>` (optional in practice, providers may drain stream)
- Implement `ProviderDescriptor` with `id`, `family`, `capabilities`
- Implement `ProviderCapabilities` struct with flags: `streaming`, `generate`, `tool_calls`, `mcp_passthrough`, `session_resume`, `steering`, `follow_up`
- Implement `ProviderRequest` with: `session`, `turn`, `model`, `messages`, `tools`, `hooks`, `settings`
- Implement reference types: `SessionRef`, `TurnContext`, `ModelRef`, `ToolContext`, `HookContext`, `ProviderSettings`
- Implement `ProviderFamily` enum (e.g., `ClaudeCode`, `Codex`, `Custom`)
- Implement provider registry for looking up providers by ID
- Implement shared infrastructure: `ProcessManager` (subprocess spawn, restart, graceful shutdown, kill-on-drop), `StdioTransport` (buffered stdin/stdout, framing, backpressure, cancellation), `ReplayWriter` (persist events during active streams)
- Implement `ProviderError` enum with variants: `NotFound`, `BinaryNotFound`, `ProcessCrashed`, `StreamInterrupted`, `ProtocolViolation`, `AuthFailed`, `RateLimited` implementing `ClassifiedError`
- Implement shared contract test suite (`ProviderContractTests`) that any provider must pass
- Dependencies: `arky-error`, `arky-protocol`, `arky-tools`, `arky-hooks`, `arky-session`
</requirements>

## Subtasks

- [x] 8.1 Implement `ProviderDescriptor`, `ProviderCapabilities`, `ProviderFamily` types
- [x] 8.2 Implement `Provider` trait with `descriptor()`, `stream()`, `generate()` methods
- [x] 8.3 Define `ProviderEventStream` type alias
- [x] 8.4 Implement `ProviderRequest` with all context types (`SessionRef`, `TurnContext`, `ModelRef`, `ToolContext`, `HookContext`, `ProviderSettings`)
- [x] 8.5 Implement provider registry (register, lookup, list providers)
- [x] 8.6 Implement `ProcessManager`: subprocess spawn, restart policy, graceful shutdown, kill-on-drop fallback
- [x] 8.7 Implement `StdioTransport`: buffered stdin/stdout, framing, backpressure, cancellation
- [x] 8.8 Implement `ReplayWriter`: persist event log or compacted checkpoints during active streams
- [x] 8.9 Implement `ProviderError` enum with `ClassifiedError` implementation
- [x] 8.10 Implement `ProviderContractTests` shared test suite
- [x] 8.11 Write unit tests for registry, process manager, stdio transport, and error classification

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-provider/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-provider/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/traits.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/request.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/registry.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/process.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/transport.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/replay.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/error.rs`
- `~/dev/compozy/arky/crates/arky-provider/src/contract_tests.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/` — `ClassifiedError` trait
- `~/dev/compozy/arky/crates/arky-protocol/` — All shared types
- `~/dev/compozy/arky/crates/arky-tools/` — `ToolRegistry`, `ToolDescriptor`, `ToolIdCodec`
- `~/dev/compozy/arky/crates/arky-hooks/` — `Hooks` trait, hook context types
- `~/dev/compozy/arky/crates/arky-session/` — `SessionStore`, session types
- `tasks/prd-rust-providers/techspec.md` — Sections: Provider Trait, Shared Infrastructure
- `tasks/prd-rust-providers/adrs/adr-002-dual-layer-api.md` — Dual-layer API design
- `tasks/prd-rust-providers/adrs/adr-003-cli-wrapper-providers.md` — CLI wrapper approach

## Deliverables

- `Provider` trait with full API surface
- `ProviderRequest` and all context types
- Provider registry
- `ProcessManager`, `StdioTransport`, `ReplayWriter` shared infrastructure
- `ProviderError` with `ClassifiedError` implementation
- `ProviderContractTests` shared test suite
- Unit tests for all components

## Tests

### Unit Tests (Required)

- [x] `ProviderDescriptor` construction and capability flag checking
- [x] `ProviderRequest` construction with all context types populated
- [x] Provider registry: register, lookup, list, not-found error
- [x] `ProcessManager`: spawn, graceful shutdown sequence, kill-on-drop behavior
- [x] `StdioTransport`: write/read framing, backpressure simulation, cancellation
- [x] `ReplayWriter`: event persistence and checkpoint writing
- [x] `ProviderError` classification: each variant returns correct error codes, retryability, HTTP status

### Integration Tests (Required)

- [x] `ProcessManager`: spawn a real subprocess (e.g., `echo`), capture output, verify shutdown
- [x] `StdioTransport`: full round-trip with a real subprocess
- [x] Contract test suite: verify it can be applied to a mock provider implementation

### Regression and Anti-Pattern Guards

- [x] `Provider` trait is `Send + Sync`
- [x] `ProviderEventStream` items are `Result<AgentEvent, ProviderError>` (mid-stream failures expressible)
- [x] No `unwrap()` in library code
- [x] `ProcessManager` always cleans up child processes (kill-on-drop)

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky-provider`

## Success Criteria

- `Provider` trait matches techspec API surface exactly
- `ProviderRequest` carries full context (session, turn, model, tools, hooks, settings)
- Shared infrastructure components are tested and reusable by concrete providers
- Contract test suite is ready for use by `arky-claude-code` and `arky-codex`
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_8.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
