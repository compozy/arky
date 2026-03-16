## markdown

## status: pending

<task_context>
<domain>infra/config</domain>
<type>implementation</type>
<scope>configuration</scope>
<complexity>medium</complexity>
<dependencies>task1</dependencies>
</task_context>

# Task 3.0: `arky-config` Crate — Configuration Loading & Validation

## Overview

Implement the `arky-config` crate responsible for loading, merging, and validating configuration from multiple sources: files, environment variables, builder overrides, and provider prerequisites. This crate is a leaf dependency — it depends only on `arky-error` and must stay that way.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement configuration loading from TOML/YAML files
- Implement environment variable override support
- Implement builder pattern for programmatic configuration
- Implement configuration merging with clear precedence rules (file < env < builder)
- Implement validation with structured error reporting via `ConfigError`
- Define `ConfigError` enum implementing `ClassifiedError` with variants: `ParseFailed`, `ValidationFailed`, `NotFound`, `MissingBinary`
- Support provider prerequisite checking (e.g., verifying `claude` or `codex` binaries exist on PATH)
- Dependency: `arky-error` only (leaf crate)
</requirements>

## Subtasks

- [ ] 3.1 Define configuration struct hierarchy (workspace config, provider config, agent config)
- [ ] 3.2 Implement file-based config loading (TOML and/or YAML with serde)
- [ ] 3.3 Implement environment variable overlay with prefix-based naming (`ARKY_*`)
- [ ] 3.4 Implement builder pattern for programmatic config construction
- [ ] 3.5 Implement config merging with clear precedence (file < env < builder)
- [ ] 3.6 Implement validation logic with structured `ConfigError` reporting
- [ ] 3.7 Implement binary prerequisite checking (PATH lookup for provider CLIs)
- [ ] 3.8 Define `ConfigError` enum with `ClassifiedError` implementation
- [ ] 3.9 Write unit tests for loading, merging, validation, and error classification

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-config/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-config/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-config/src/error.rs`
- `~/dev/compozy/arky/crates/arky-config/src/loader.rs`
- `~/dev/compozy/arky/crates/arky-config/src/merge.rs`
- `~/dev/compozy/arky/crates/arky-config/src/validate.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/src/lib.rs` — `ClassifiedError` trait
- `tasks/prd-rust-providers/techspec.md` — Section: Config component

## Deliverables

- Complete `arky-config` crate with file/env/builder loading and merging
- `ConfigError` enum with `ClassifiedError` implementation
- Binary prerequisite checking utility
- Unit tests for all loading, merging, and validation paths

## Tests

### Unit Tests (Required)

- [ ] File loading: valid TOML parses correctly, invalid TOML returns `ParseFailed`
- [ ] Env override: environment variables override file values correctly
- [ ] Builder: programmatic values override env values
- [ ] Merge precedence: file < env < builder ordering verified
- [ ] Validation: missing required fields produce `ValidationFailed`
- [ ] Binary check: missing binary returns `MissingBinary`, present binary returns Ok
- [ ] `ConfigError` classification: each variant returns correct `error_code()`, `is_retryable()`, `http_status()`

### Integration Tests (Required)

- [ ] End-to-end: load from temp file, apply env overrides, validate, and produce final config
- [ ] Prerequisite check: verify PATH lookup works for a known binary (e.g., `cargo`)

### Regression and Anti-Pattern Guards

- [ ] No `unwrap()` in library code
- [ ] Config struct fields are private with accessor methods where appropriate
- [ ] No cyclic dependencies introduced

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-config`

## Success Criteria

- Config loads correctly from files, env vars, and builder
- Merge precedence is deterministic and tested
- Validation produces clear, structured errors
- Binary prerequisite checking works for PATH lookups
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_3.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
