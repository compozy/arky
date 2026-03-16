## markdown

## status: pending

<task_context>
<domain>engine/tools-macros</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task4</dependencies>
</task_context>

# Task 5.0: `arky-tools-macros` Crate — `#[tool]` Proc Macro

## Overview

Implement the `arky-tools-macros` crate providing the `#[tool]` procedural macro that generates `Tool` trait implementations from annotated async functions. This macro removes real repeated boilerplate from tool authoring: it auto-generates `ToolDescriptor` (including JSON Schema from function arguments), the `execute` method dispatch, argument deserialization, and cancellation token plumbing. This crate is standalone (depends only on `syn`, `quote`, `proc-macro2`) and can be developed in parallel with other tasks after Task 4.0.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-005)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `#[tool]` attribute proc macro for annotating async functions
- Generate `Tool` trait implementation including `descriptor()` and `execute()` methods
- Auto-generate `ToolDescriptor` with: canonical_name derived from function name, description from doc comments, input_schema from function parameter types
- Auto-generate JSON Schema for tool input from Rust types (leveraging `schemars` or manual schema construction)
- Handle `CancellationToken` parameter transparently (not part of tool schema)
- Handle return type mapping: function return type maps to `ToolResult`
- Produce clear compile-time errors for invalid macro usage (wrong signature, missing attributes)
- Dependencies: `syn` 2.x, `quote` 2.x, `proc-macro2` 2.x only (standalone proc-macro crate)
</requirements>

## Subtasks

- [ ] 5.1 Set up proc-macro crate structure (`proc-macro = true` in Cargo.toml)
- [ ] 5.2 Implement function signature parsing: extract name, doc comments, parameters, return type
- [ ] 5.3 Implement `ToolDescriptor` code generation from parsed signature
- [ ] 5.4 Implement JSON Schema generation for tool input parameters
- [ ] 5.5 Implement `execute()` method code generation with argument deserialization and dispatch
- [ ] 5.6 Implement `CancellationToken` parameter detection and transparent handling
- [ ] 5.7 Implement compile-time error messages for invalid macro usage
- [ ] 5.8 Write expansion tests verifying generated code structure
- [ ] 5.9 Write schema output validation tests for complex arg types (nested structs, Options, Vecs)
- [ ] 5.10 Write error-message tests for invalid macro usage patterns

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-tools-macros/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-tools-macros/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-tools-macros/src/parse.rs`
- `~/dev/compozy/arky/crates/arky-tools-macros/src/codegen.rs`
- `~/dev/compozy/arky/crates/arky-tools-macros/src/schema.rs`
- `~/dev/compozy/arky/crates/arky-tools-macros/tests/` — expansion and compile-fail tests

### Dependent Files

- `~/dev/compozy/arky/crates/arky-tools/src/lib.rs` — `Tool` trait that generated code must implement
- `tasks/prd-rust-providers/techspec.md` — Section: Tools Macros, Tool Trait
- `tasks/prd-rust-providers/adrs/adr-005-tool-system.md` — Tool system design

## Deliverables

- `#[tool]` proc macro that generates valid `Tool` trait implementations
- JSON Schema generation for tool input parameters
- Compile-time error messages for invalid usage
- Expansion tests and compile-fail tests

## Tests

### Unit Tests (Required)

- [ ] Function signature parsing: name extraction, doc comment extraction, parameter extraction
- [ ] Schema generation: primitive types, Option<T>, Vec<T>, nested structs produce valid JSON Schema
- [ ] CancellationToken detection: parameter is excluded from schema and passed through correctly

### Integration Tests (Required)

- [ ] Compile-time expansion test: annotated function expands to valid `Tool` implementation
- [ ] End-to-end test: annotated function can be registered in `ToolRegistry` and executed
- [ ] Complex arg types: nested structs, enums, optional fields expand correctly

### Regression and Anti-Pattern Guards

- [ ] Compile-fail tests for: non-async functions, missing return type, unsupported parameter types
- [ ] Generated code compiles without warnings under `clippy -D warnings`
- [ ] No test-only production APIs introduced

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-tools-macros`

## Success Criteria

- `#[tool]` macro generates valid, compilable `Tool` implementations
- Generated `ToolDescriptor` includes correct name, description, and JSON Schema
- CancellationToken is handled transparently
- Invalid usage produces helpful compile-time errors
- All expansion and compile-fail tests pass

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_5.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
