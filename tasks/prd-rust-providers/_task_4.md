## markdown

## status: completed

<task_context>
<domain>engine/tools</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2</dependencies>
</task_context>

# Task 4.0: `arky-tools` Crate — Tool Trait, Registry & Codecs

## Overview

Implement the `arky-tools` crate providing the `Tool` trait, `ToolDescriptor`, `ToolRegistry`, canonical tool naming system, provider-specific name codecs, and lifecycle handles. Tools are a first-class concept in Arky — every imported or exposed tool has a canonical ID of the form `mcp/<server>/<tool>`, and provider-specific names are codecs, not identity. The registry must support long-lived registrations, call-scoped registrations with cleanup handles, and collision detection.

## Porting Context

This task uses the shared tool system in `../compozy-code/providers/core`,
`../compozy-code/providers/runtime`, and the provider-specific tool bridges in
`../compozy-code/providers/claude-code` and
`../compozy-code/providers/codex` as the main upstream reference for behavior
and edge cases. Do not copy the TypeScript API or module layout mechanically;
prefer the Rust architecture and quality bar defined in this PRD. Before
implementation, read `tasks/prd-rust-providers/porting-reference.md` and
inspect the Task 4.0 upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-005)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 4.0 upstream TypeScript files before implementation
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Tool` trait with `descriptor()` and `execute()` methods, where `execute` takes a `CancellationToken`
- Implement `ToolDescriptor` with `canonical_name`, `display_name`, `description`, `input_schema` (JSON), and `ToolOrigin` enum
- Implement `ToolOrigin` enum: `Local`, `Mcp { server_name }`, `ProviderScoped { provider_id }`
- Implement `ToolRegistry` supporting long-lived registrations, call-scoped registrations with cleanup handles, and collision detection
- Implement `ToolIdCodec` trait for canonical <-> provider-specific tool naming round-trips
- Implement `ToolError` enum with variants: `InvalidArgs`, `ExecutionFailed`, `Timeout`, `Cancelled`, `NameCollision` implementing `ClassifiedError`
- Canonical tool identity format: `mcp/<server>/<tool>`
- Tool registration must be call-scoped when needed: temporary tools must be unregistered at stream completion
- Dependencies: `arky-error`, `arky-protocol`
</requirements>

## Subtasks

- [x] 4.1 Implement `ToolDescriptor` struct with all fields and `ToolOrigin` enum
- [x] 4.2 Implement `Tool` trait with `descriptor()` and `execute(ToolCall, CancellationToken)` methods
- [x] 4.3 Implement `ToolRegistry` with registration, lookup, listing, and collision detection
- [x] 4.4 Implement call-scoped registration with cleanup handles (RAII-style or explicit)
- [x] 4.5 Implement `ToolIdCodec` trait for canonical <-> provider-specific name round-trips
- [x] 4.6 Implement canonical naming validation (`mcp/<server>/<tool>` format)
- [x] 4.7 Implement `ToolError` enum with `ClassifiedError` implementation
- [x] 4.8 Write unit tests for registry operations, codec round-trips, collision detection, and cleanup handles

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-tools/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-tools/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-tools/src/descriptor.rs`
- `~/dev/compozy/arky/crates/arky-tools/src/registry.rs`
- `~/dev/compozy/arky/crates/arky-tools/src/codec.rs`
- `~/dev/compozy/arky/crates/arky-tools/src/error.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/src/lib.rs` — `ClassifiedError` trait
- `~/dev/compozy/arky/crates/arky-protocol/src/tool.rs` — `ToolCall`, `ToolResult` types
- `tasks/prd-rust-providers/techspec.md` — Section: Tool Trait, Architectural Invariants 3-4
- `tasks/prd-rust-providers/adrs/adr-005-tool-system.md` — Tool system design

## Deliverables

- `Tool` trait with full API surface
- `ToolRegistry` with long-lived and call-scoped registration
- `ToolIdCodec` trait for name translation
- `ToolError` with `ClassifiedError` implementation
- Unit tests for all registry operations, codecs, and error handling

## Tests

### Unit Tests (Required)

- [x] `ToolDescriptor` construction with all `ToolOrigin` variants
- [x] `ToolRegistry`: register, lookup, list, remove operations
- [x] `ToolRegistry`: collision detection when registering duplicate canonical names
- [x] Call-scoped registration: tool is accessible during scope, removed after cleanup
- [x] `ToolIdCodec`: canonical -> provider-specific and back round-trip correctness
- [x] Canonical name validation: valid `mcp/server/tool` accepted, invalid formats rejected
- [x] `ToolError` classification: each variant returns correct error codes and retryability

### Integration Tests (Required)

- [x] Mock `Tool` implementation registered and executed through the registry
- [x] Concurrent registry access: multiple reads and writes are safe (using `DashMap` or `RwLock`)

### Regression and Anti-Pattern Guards

- [x] No `unwrap()` in library code
- [x] `Tool` trait is `Send + Sync`
- [x] Registry operations are thread-safe

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky-tools`

## Success Criteria

- `Tool` trait matches techspec API surface
- Registry supports both long-lived and call-scoped registrations
- Codec round-trips are lossless
- Collision detection prevents duplicate registrations
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_4.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
