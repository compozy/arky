## markdown

## status: pending

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

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-005)
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

- [ ] 4.1 Implement `ToolDescriptor` struct with all fields and `ToolOrigin` enum
- [ ] 4.2 Implement `Tool` trait with `descriptor()` and `execute(ToolCall, CancellationToken)` methods
- [ ] 4.3 Implement `ToolRegistry` with registration, lookup, listing, and collision detection
- [ ] 4.4 Implement call-scoped registration with cleanup handles (RAII-style or explicit)
- [ ] 4.5 Implement `ToolIdCodec` trait for canonical <-> provider-specific name round-trips
- [ ] 4.6 Implement canonical naming validation (`mcp/<server>/<tool>` format)
- [ ] 4.7 Implement `ToolError` enum with `ClassifiedError` implementation
- [ ] 4.8 Write unit tests for registry operations, codec round-trips, collision detection, and cleanup handles

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

- [ ] `ToolDescriptor` construction with all `ToolOrigin` variants
- [ ] `ToolRegistry`: register, lookup, list, remove operations
- [ ] `ToolRegistry`: collision detection when registering duplicate canonical names
- [ ] Call-scoped registration: tool is accessible during scope, removed after cleanup
- [ ] `ToolIdCodec`: canonical -> provider-specific and back round-trip correctness
- [ ] Canonical name validation: valid `mcp/server/tool` accepted, invalid formats rejected
- [ ] `ToolError` classification: each variant returns correct error codes and retryability

### Integration Tests (Required)

- [ ] Mock `Tool` implementation registered and executed through the registry
- [ ] Concurrent registry access: multiple reads and writes are safe (using `DashMap` or `RwLock`)

### Regression and Anti-Pattern Guards

- [ ] No `unwrap()` in library code
- [ ] `Tool` trait is `Send + Sync`
- [ ] Registry operations are thread-safe

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-tools`

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
