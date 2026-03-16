## markdown

## status: completed

<task_context>
<domain>infra/facade</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>medium</complexity>
<dependencies>task1,task2,task3,task4,task5,task6,task7,task8,task9,task10,task11,task12,task13</dependencies>
</task_context>

# Task 14.0: `arky` Facade Crate & Prelude

## Overview

Implement the `arky` facade crate that re-exports common types from all workspace crates and provides the `arky::prelude::*` module. This crate is the primary user-facing entry point — consumers should be able to `use arky::prelude::*` and have access to the most commonly needed types. It also defines the top-level `ArkyError` enum that unifies all crate-specific error types.

## Porting Context

This task does not map to one upstream TypeScript package. Use the export
surfaces in `../compozy-code/providers/core`, `runtime`, `claude-code`, and
`codex` together to shape the Rust facade. Treat them as reference material
for behavior and ergonomics, not as a requirement to mirror TypeScript
structure. Before implementation, read
`tasks/prd-rust-providers/porting-reference.md` and inspect the Task 14.0
upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-010)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 14.0 upstream TypeScript files before implementation
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Re-export common types from all workspace crates
- Implement `arky::prelude` module with the most commonly used types
- Implement `ArkyError` enum unifying all crate-specific errors: `Core`, `Provider`, `Tool`, `Session`, `Mcp`, `Hook`, `Config` with `#[from]` conversions
- `ArkyError` must implement `ClassifiedError` by delegating to inner error
- Provide feature flags to optionally include specific providers (`claude-code`, `codex`) and backends (`sqlite`, `server`)
- Documentation on the facade crate explaining the re-export structure
- Dependencies: all workspace crates
</requirements>

## Subtasks

- [x] 14.1 Define `ArkyError` enum with `#[from]` conversions for all crate error types
- [x] 14.2 Implement `ClassifiedError` for `ArkyError` by delegating to inner error variants
- [x] 14.3 Create `prelude` module re-exporting commonly used types (Agent, AgentBuilder, Provider, Tool, etc.)
- [x] 14.4 Re-export modules from all workspace crates under organized namespaces
- [x] 14.5 Configure feature flags: `claude-code`, `codex`, `sqlite`, `server` for optional dependencies
- [x] 14.6 Add crate-level documentation explaining the structure and re-exports
- [x] 14.7 Write compile tests verifying that `use arky::prelude::*` provides expected types
- [x] 14.8 Write tests verifying `ArkyError` conversions from all crate error types

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky/Cargo.toml`
- `~/dev/compozy/arky/crates/arky/src/lib.rs`
- `~/dev/compozy/arky/crates/arky/src/prelude.rs`
- `~/dev/compozy/arky/crates/arky/src/error.rs`

### Dependent Files

- All workspace crates (re-exported)
- `tasks/prd-rust-providers/techspec.md` — Section: Facade, ArkyError
- `tasks/prd-rust-providers/adrs/adr-010-naming.md` — Naming convention

## Deliverables

- `arky` facade crate with re-exports from all workspace crates
- `arky::prelude` module with commonly used types
- `ArkyError` unified error enum with `ClassifiedError` implementation
- Feature flags for optional providers and backends
- Documentation and compile tests

## Tests

### Unit Tests (Required)

- [x] `ArkyError::from(CoreError)` conversion works
- [x] `ArkyError::from(ProviderError)` conversion works
- [x] `ArkyError::from(ToolError)` conversion works
- [x] `ArkyError::from(SessionError)` conversion works
- [x] `ArkyError::from(McpError)` conversion works
- [x] `ArkyError::from(HookError)` conversion works
- [x] `ArkyError::from(ConfigError)` conversion works
- [x] `ArkyError` delegates `ClassifiedError` methods to inner variant correctly

### Integration Tests (Required)

- [x] `use arky::prelude::*` compiles and provides: `Agent`, `AgentBuilder`, `Provider`, `Tool`, `ToolDescriptor`, `Message`, `AgentEvent`, `SessionStore`
- [x] Feature flag combinations compile: default, `claude-code`, `codex`, `sqlite`, `server`, all features

### Regression and Anti-Pattern Guards

- [x] No re-export conflicts (multiple types with same name from different crates)
- [x] Feature flags properly gate optional dependencies
- [x] Prelude does not re-export overly specific internal types

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky`
- [x] `cargo test -p arky --all-features`

## Success Criteria

- `use arky::prelude::*` provides all commonly needed types
- `ArkyError` unifies all crate errors with correct `From` conversions
- Feature flags work correctly for optional components
- All tests pass with default and all-features configurations
- Zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_14.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
