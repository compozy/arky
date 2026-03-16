## markdown

## status: completed

<task_context>
<domain>infra/workspace</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>medium</complexity>
<dependencies>none</dependencies>
</task_context>

# Task 1.0: Workspace Scaffolding & `arky-error` Crate

## Overview

Bootstrap the Arky Cargo workspace at `~/dev/compozy/arky/` and implement the foundational `arky-error` crate. This is the first deliverable in the SDK — every other crate depends on the workspace structure and the shared error classification trait living in `arky-error`.

The workspace root `Cargo.toml` must define all shared dependencies via `[workspace.dependencies]`, enforce unified lint config, and declare all 15 crate members. The `arky-error` crate provides the `ClassifiedError` trait, shared error-code conventions, retryability classification, and helper structs for logging and API mapping.

## Porting Context

This task uses the shared workspace and error foundation in
`../compozy-code/providers/core` and `../compozy-code/providers/runtime` as
the main upstream reference for behavior and edge cases. Do not copy the
TypeScript API or module layout mechanically; prefer the Rust architecture and
quality bar defined in this PRD. Before implementation, read
`tasks/prd-rust-providers/porting-reference.md` and inspect the Task 1.0
upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-001, ADR-006, ADR-010)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 1.0 upstream TypeScript files before implementation
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Create Cargo workspace at `~/dev/compozy/arky/` with all 15 crate members declared
- Configure `[workspace.dependencies]` with all shared dependencies from techspec (tokio, serde, thiserror, tracing, async-trait, tokio-util, futures, schemars, rmcp, reqwest, dashmap, uuid, regex, syn, quote, proc-macro2)
- Configure feature-gated dependencies (tokio-rusqlite under `sqlite`, axum under `server`)
- Set Rust Edition 2024 in workspace
- Implement `ClassifiedError` trait in `arky-error` with methods: `error_code()`, `is_retryable()`, `retry_after()`, `http_status()`, `correction_context()`
- Create stub `Cargo.toml` for every crate member (empty `lib.rs` is acceptable for non-error crates)
- Enforce `cargo clippy -D warnings` workspace-wide
- All public types in `arky-error` must implement `Debug`, `Clone` where appropriate, and `Send + Sync`
</requirements>

## Subtasks

- [x] 1.1 Create workspace directory structure with all 15 crate directories under `crates/`
- [x] 1.2 Write root `Cargo.toml` with `[workspace]` members, `[workspace.dependencies]`, and shared lint/profile config
- [x] 1.3 Create stub `Cargo.toml` and `src/lib.rs` for each of the 15 crates (minimal, compilable)
- [x] 1.4 Implement `ClassifiedError` trait in `arky-error/src/lib.rs` with all required methods and default implementations
- [x] 1.5 Add helper structs in `arky-error` for structured error logging and HTTP status mapping
- [x] 1.6 Write unit tests for `ClassifiedError` default implementations and helper structs
- [x] 1.7 Verify full workspace compiles: `cargo build`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test`

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-error/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-error/src/lib.rs`
- All 14 other `crates/*/Cargo.toml` and `crates/*/src/lib.rs` (stubs)

### Dependent Files

- `tasks/prd-rust-providers/techspec.md` — Section: Repository & Workspace, Crate Dependency Graph, Rust Dependency Stack
- `tasks/prd-rust-providers/adrs/adr-001-package-architecture.md` — Workspace layout decisions
- `tasks/prd-rust-providers/adrs/adr-006-error-handling.md` — Error classification design
- `tasks/prd-rust-providers/adrs/adr-010-naming.md` — `arky-*` naming convention

## Deliverables

- Working Cargo workspace with 15 crate members that compiles cleanly
- `ClassifiedError` trait with full API surface as specified in techspec
- Helper types for error logging and HTTP status mapping
- Unit tests for error trait defaults and helpers
- All workspace lint checks passing

## Tests

### Unit Tests (Required)

- [x] `ClassifiedError` default method behavior: `is_retryable()` returns `false`, `retry_after()` returns `None`, `http_status()` returns `500`, `correction_context()` returns `None`
- [x] Custom error enum implementing `ClassifiedError` with overridden methods
- [x] Helper struct construction and field access for logging and API mapping
- [x] Error display formatting via `thiserror`

### Integration Tests (Required)

- [x] Full workspace `cargo build` succeeds with all 15 crates
- [x] `cargo clippy -D warnings` produces zero warnings across all crates
- [x] `cargo test` runs and passes for `arky-error`

### Regression and Anti-Pattern Guards

- [x] No `unwrap()` in library code (enforced by clippy config)
- [x] All public async traits are `Send + Sync`
- [x] `#[non_exhaustive]` on public enums where specified

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky-error`
- [x] `cargo build --workspace`

## Success Criteria

- All 15 crates declared and compilable in the workspace
- `ClassifiedError` trait API matches techspec exactly
- Zero warnings from clippy across entire workspace
- All unit tests pass
- Crate dependency graph matches techspec (verified by stub Cargo.toml deps)

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_1.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
