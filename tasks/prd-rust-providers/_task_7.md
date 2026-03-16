## markdown

## status: pending

<task_context>
<domain>engine/session</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2</dependencies>
</task_context>

# Task 7.0: `arky-session` Crate — Session Store, Snapshots & Replay

## Overview

Implement the `arky-session` crate providing the `SessionStore` trait, session snapshots, replay log persistence, and both in-memory and SQLite backends. Session persistence must support **resume and replay**, not just transcript loading — message history alone is insufficient. Replay metadata, last turn outcome, provider/session identifiers, and persisted event checkpoints are part of the persistence contract.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-007)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `SessionStore` trait with methods: `create`, `load`, `append_messages`, `append_events`, `save_turn_checkpoint`, `list`, `delete`
- Implement `SessionSnapshot` struct with: `metadata`, `messages`, `last_checkpoint`, `replay_cursor`
- Implement `SessionMetadata` with stable identifiers and summary fields (separate from `NewSession` input type)
- Implement `NewSession` input type for `create()`
- Implement `TurnCheckpoint` for persisting turn state
- Implement `ReplayCursor` for replay position tracking
- Implement `PersistedEvent` storage and retrieval
- Implement `SessionFilter` for listing sessions
- Implement in-memory `SessionStore` backend (replay persistence optionally disabled by config)
- Implement SQLite `SessionStore` backend behind `sqlite` feature flag (WAL mode, single-writer discipline)
- Implement `SessionError` enum with variants: `NotFound`, `StorageFailure`, `ReplayUnavailable`, `Expired` implementing `ClassifiedError`
- Dependencies: `arky-error`, `arky-protocol`
- Feature-gated: `tokio-rusqlite` under `sqlite` feature
</requirements>

## Subtasks

- [ ] 7.1 Define `SessionStore` trait with all required methods
- [ ] 7.2 Define `SessionSnapshot`, `SessionMetadata`, `NewSession`, `SessionFilter` types
- [ ] 7.3 Define `TurnCheckpoint` and `ReplayCursor` types
- [ ] 7.4 Implement in-memory `SessionStore` backend with configurable replay persistence
- [ ] 7.5 Implement SQLite `SessionStore` backend (feature-gated under `sqlite`)
- [ ] 7.6 Configure SQLite with WAL mode and single-writer discipline
- [ ] 7.7 Implement `SessionError` enum with `ClassifiedError` implementation
- [ ] 7.8 Write unit tests for in-memory backend: create, load, append, checkpoint, list, delete
- [ ] 7.9 Write integration tests for SQLite backend: full lifecycle with real database file
- [ ] 7.10 Write replay tests: checkpoint synthesis and replay cursor behavior

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-session/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-session/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-session/src/store.rs`
- `~/dev/compozy/arky/crates/arky-session/src/snapshot.rs`
- `~/dev/compozy/arky/crates/arky-session/src/memory.rs`
- `~/dev/compozy/arky/crates/arky-session/src/sqlite.rs`
- `~/dev/compozy/arky/crates/arky-session/src/error.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/src/lib.rs` — `ClassifiedError` trait
- `~/dev/compozy/arky/crates/arky-protocol/` — `Message`, `PersistedEvent`, `SessionId`, `TurnCheckpoint`, `ReplayCursor`
- `tasks/prd-rust-providers/techspec.md` — Section: SessionStore Trait
- `tasks/prd-rust-providers/adrs/adr-007-session-management.md` — Session management design

## Deliverables

- `SessionStore` trait with full API surface
- In-memory backend implementation
- SQLite backend implementation (feature-gated)
- All snapshot, checkpoint, and replay types
- `SessionError` with `ClassifiedError` implementation
- Unit and integration tests for both backends

## Tests

### Unit Tests (Required)

- [ ] In-memory: create session, load returns correct snapshot
- [ ] In-memory: append_messages updates message list
- [ ] In-memory: append_events stores persisted events
- [ ] In-memory: save_turn_checkpoint and load returns latest checkpoint
- [ ] In-memory: list with filter returns matching sessions
- [ ] In-memory: delete removes session, subsequent load returns `NotFound`
- [ ] Replay cursor: position tracking and advancement
- [ ] `SessionError` classification: each variant returns correct error codes

### Integration Tests (Required)

- [ ] SQLite: full lifecycle (create, append, checkpoint, load, list, delete) with real temp database
- [ ] SQLite: WAL mode verification
- [ ] SQLite: concurrent reads during write (single-writer discipline)
- [ ] Resume flow: create session, add messages, save checkpoint, load and verify replay cursor

### Regression and Anti-Pattern Guards

- [ ] `SessionStore` trait is `Send + Sync`
- [ ] No `unwrap()` in library code
- [ ] SQLite operations handle busy/locked errors with bounded retry
- [ ] `SessionMetadata` is NOT the input type for `create()` — `NewSession` is

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-session`
- [ ] `cargo test -p arky-session --features sqlite`

## Success Criteria

- `SessionStore` trait matches techspec API exactly
- In-memory backend passes all contract tests
- SQLite backend passes all contract tests with real database
- Resume and replay semantics are fully functional
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_7.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
