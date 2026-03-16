## markdown

## status: completed

<task_context>
<domain>infra/protocol</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1</dependencies>
</task_context>

# Task 2.0: `arky-protocol` Crate — Shared Types & Event Model

## Overview

Implement the `arky-protocol` crate containing all shared types used across the Arky SDK: messages, events, IDs, request/response DTOs, and persisted event records. This crate is the lingua franca of the workspace — nearly every other crate depends on it. The event model must be rich enough for replay, routing, and observability while remaining a flat enum with `#[non_exhaustive]`.

## Porting Context

This task uses the TypeScript runtime and provider event models in
`../compozy-code/providers/runtime`,
`../compozy-code/providers/claude-code`, and
`../compozy-code/providers/codex` as the main upstream reference for behavior
and edge cases. Do not copy the TypeScript API or module layout mechanically;
prefer the Rust architecture and quality bar defined in this PRD. Before
implementation, read `tasks/prd-rust-providers/porting-reference.md` and
inspect the Task 2.0 upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-004)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 2.0 upstream TypeScript files before implementation
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Message` struct with `Role` enum (`User`, `Assistant`, `System`, `Tool`) and `ContentBlock` enum (`Text`, `ToolUse`, `ToolResult`, `Image`)
- Implement `AgentEvent` enum with all variants from techspec: `AgentStart`, `AgentEnd`, `TurnStart`, `TurnEnd`, `MessageStart`, `MessageUpdate`, `MessageEnd`, `ToolExecutionStart`, `ToolExecutionUpdate`, `ToolExecutionEnd`, `Custom`
- Implement `EventMetadata` struct with `timestamp_ms`, `sequence`, `session_id`, `turn_id`, `provider_id`
- Define ID types: `SessionId`, `ProviderId`, `TurnId` (consider newtype wrappers around `uuid::Uuid` or `String`)
- Implement `StreamDelta` type for incremental message updates
- Implement `PersistedEvent` and `TurnCheckpoint` types for session persistence
- Implement `ToolCall`, `ToolResult`, `ToolContent` types
- All public types must derive `Debug`, `Clone`, `Serialize`, `Deserialize` where crossing process/storage boundaries
- `AgentEvent` must be `#[non_exhaustive]`
- Event `sequence` must be strictly monotonic within a session (documented contract)
- Dependency: `arky-error` only
</requirements>

## Subtasks

- [x] 2.1 Define ID newtypes (`SessionId`, `ProviderId`, `TurnId`) with constructors, display, and serialization
- [x] 2.2 Implement `Role`, `ContentBlock`, `MessageMetadata`, and `Message` types
- [x] 2.3 Implement `ToolCall`, `ToolResult`, `ToolContent` types
- [x] 2.4 Implement `EventMetadata` struct
- [x] 2.5 Implement `AgentEvent` enum with all variants and `#[non_exhaustive]`
- [x] 2.6 Implement `StreamDelta` type for incremental streaming updates
- [x] 2.7 Implement persistence types: `PersistedEvent`, `TurnCheckpoint`, `ReplayCursor`
- [x] 2.8 Implement request/response DTOs: `ProviderRequest` fields, `GenerateResponse`, `AgentResponse`
- [x] 2.9 Write unit tests for serialization round-trips, event ordering, and ID generation
- [x] 2.10 Write unit tests for `ContentBlock` variant construction and `Message` builder patterns

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-protocol/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-protocol/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-protocol/src/id.rs`
- `~/dev/compozy/arky/crates/arky-protocol/src/message.rs`
- `~/dev/compozy/arky/crates/arky-protocol/src/event.rs`
- `~/dev/compozy/arky/crates/arky-protocol/src/tool.rs`
- `~/dev/compozy/arky/crates/arky-protocol/src/session.rs`
- `~/dev/compozy/arky/crates/arky-protocol/src/request.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/src/lib.rs` — `ClassifiedError` trait
- `tasks/prd-rust-providers/techspec.md` — Sections: Event Model, Message Types, Data Models
- `tasks/prd-rust-providers/adrs/adr-004-event-model.md` — Event design decisions

## Deliverables

- Complete `arky-protocol` crate with all shared types from the techspec
- Serialization support (`serde`) for all types crossing process/storage boundaries
- Unit tests for serialization, ID generation, event metadata, and type construction
- Documentation comments on all public types and methods

## Tests

### Unit Tests (Required)

- [x] ID types: creation, display, equality, serialization round-trip
- [x] Message: construction with all `ContentBlock` variants, serialization round-trip
- [x] AgentEvent: construction of each variant, serialization round-trip, metadata population
- [x] EventMetadata: sequence ordering invariant documentation and helper methods
- [x] PersistedEvent and TurnCheckpoint: serialization round-trip
- [x] StreamDelta: construction and serialization

### Integration Tests (Required)

- [x] Cross-crate usage: `arky-error` types used correctly in protocol error paths
- [x] JSON serialization compatibility: verify event JSON shape matches expected format for downstream consumers

### Regression and Anti-Pattern Guards

- [x] All public enums marked `#[non_exhaustive]` where specified
- [x] No `unwrap()` in library code
- [x] All types that cross process boundaries implement `Serialize`/`Deserialize`

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky-protocol`

## Success Criteria

- All shared types from techspec implemented and compilable
- Serialization round-trips pass for all types
- `#[non_exhaustive]` on `AgentEvent` and other specified enums
- Zero clippy warnings
- All unit tests pass

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_2.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
