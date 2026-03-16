## markdown

## status: pending

<task_context>
<domain>engine/claude-code</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>critical</complexity>
<dependencies>task1,task2,task4,task8,task9</dependencies>
</task_context>

# Task 10.0: `arky-claude-code` Crate — Claude Code CLI Provider

## Overview

Implement the `arky-claude-code` crate — the first concrete provider wrapping the Claude Code CLI (`claude`) as a subprocess-backed provider. This provider validates the entire event contract and is the primary integration target. It must handle spawn-failure cooldown tracking, tool lifecycle finite-state machine, nested tool-call tracking, text deduplication between streamed and final assistant payloads, and session identifier passthrough.

This provider must NOT collapse all complexity into a single "read lines and map JSON" module. The tool FSM, nested tool tracking, and duplicate-text handling are core correctness requirements.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-003, analysis_claude_code.md)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Provider` trait for the Claude Code CLI wrapper
- Spawn `claude` binary as subprocess using `ProcessManager` from `arky-provider`
- Parse Claude CLI event protocol from stdout (stream-json format)
- Implement tool lifecycle finite-state machine for tracking tool call states
- Implement nested tool-call tracking (tools calling other tools)
- Implement text deduplication between streamed deltas and final assistant payloads
- Implement spawn-failure cooldown tracking (avoid rapid restart loops)
- Implement session identifier passthrough and reuse for `--session-id` flag
- Normalize Claude CLI events to `AgentEvent` variants
- Handle failure modes: binary missing, spawn failure, protocol corruption, partial stdout/stderr, invalid tool transition
- Pass `ProviderContractTests` from `arky-provider`
- Dependencies: `arky-error`, `arky-protocol`, `arky-provider`, `arky-tools`, `arky-mcp`
</requirements>

## Subtasks

- [ ] 10.1 Implement Claude CLI binary discovery and version validation
- [ ] 10.2 Implement subprocess spawning with correct CLI flags (`--print`, `--output-format stream-json`, `--verbose`, etc.)
- [ ] 10.3 Implement Claude CLI event protocol parser (stream-json line parsing)
- [ ] 10.4 Implement event normalization: Claude CLI events -> `AgentEvent` variants
- [ ] 10.5 Implement tool lifecycle FSM: track tool call states with valid transitions
- [ ] 10.6 Implement nested tool-call tracking
- [ ] 10.7 Implement text deduplication between streamed deltas and final assistant blocks
- [ ] 10.8 Implement spawn-failure cooldown tracking
- [ ] 10.9 Implement session identifier passthrough (`--session-id` flag reuse)
- [ ] 10.10 Implement `ProviderDescriptor` and `ProviderCapabilities` for Claude Code
- [ ] 10.11 Pass `ProviderContractTests` shared test suite
- [ ] 10.12 Write unit tests for event parsing, tool FSM, deduplication, and cooldown
- [ ] 10.13 Write integration tests spawning real `claude` binary (behind integration flag)

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-claude-code/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-claude-code/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/src/provider.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/src/parser.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/src/tool_fsm.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/src/dedup.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/src/cooldown.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/src/session.rs`
- `~/dev/compozy/arky/crates/arky-claude-code/tests/fixtures/` — CLI output fixture files

### Dependent Files

- `~/dev/compozy/arky/crates/arky-provider/` — `Provider` trait, `ProcessManager`, `StdioTransport`, contract tests
- `~/dev/compozy/arky/crates/arky-protocol/` — `AgentEvent`, `Message`, event types
- `~/dev/compozy/arky/crates/arky-tools/` — `ToolRegistry`, `ToolIdCodec`
- `~/dev/compozy/arky/crates/arky-mcp/` — MCP tool bridge for tool exposure
- `tasks/prd-rust-providers/techspec.md` — Section: Claude Code CLI Integration
- `tasks/prd-rust-providers/adrs/adr-003-cli-wrapper-providers.md` — CLI wrapper design
- `tasks/prd-rust-providers/analysis_claude_code.md` — Claude provider analysis

## Deliverables

- Complete Claude Code provider implementing `Provider` trait
- Event protocol parser with fixture-based tests
- Tool lifecycle FSM with validated state transitions
- Nested tool tracking and text deduplication
- Spawn-failure cooldown mechanism
- Session identifier passthrough
- Passes `ProviderContractTests`
- Unit tests and integration tests (real binary behind flag)

## Tests

### Unit Tests (Required)

- [ ] Event parser: parse each Claude CLI event type from fixture JSON, verify `AgentEvent` mapping
- [ ] Tool FSM: valid transitions (start -> update -> end), invalid transitions produce errors
- [ ] Nested tools: parent tool receives child tool results correctly
- [ ] Text deduplication: streamed text + final block produce clean output without duplicates
- [ ] Cooldown: spawn failure triggers cooldown, subsequent spawn within cooldown is delayed
- [ ] Session passthrough: session ID is correctly passed to `--session-id` flag

### Integration Tests (Required)

- [ ] Spawn real `claude` binary (behind `#[cfg(feature = "integration")]`), send prompt, receive events
- [ ] Tool lifecycle end-to-end: trigger tool call, verify FSM transitions in event stream
- [ ] Provider contract tests: pass `ProviderContractTests` from `arky-provider`

### Regression and Anti-Pattern Guards

- [ ] Protocol corruption: malformed JSON lines produce `ProtocolViolation`, not panics
- [ ] Process crash after first event: handled gracefully via stream `Result` items
- [ ] No `unwrap()` in library code
- [ ] Binary not found: produces `BinaryNotFound` error, not panic

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-claude-code`
- [ ] `cargo test -p arky-claude-code --features integration` (requires `claude` binary)

## Success Criteria

- Claude Code provider implements `Provider` trait correctly
- Event protocol parsing handles all known Claude CLI event types
- Tool FSM enforces valid state transitions
- Text deduplication eliminates duplicates between stream and final payloads
- Spawn-failure cooldown prevents rapid restart loops
- Passes all `ProviderContractTests`
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_10.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
