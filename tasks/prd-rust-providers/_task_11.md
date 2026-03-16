## markdown

## status: pending

<task_context>
<domain>engine/codex</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>critical</complexity>
<dependencies>task1,task2,task4,task8,task9</dependencies>
</task_context>

# Task 11.0: `arky-codex` Crate — Codex App Server Provider

## Overview

Implement the `arky-codex` crate — the second concrete provider wrapping the Codex App Server as a subprocess-backed provider using newline-delimited JSON-RPC over stdio. This provider validates the JSON-RPC transport and thread routing contracts. It must handle process lifecycle, request/response correlation, serialized model access, multi-conversation thread management, approval flow, and notification dispatch.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-003, analysis_codex.md, analysis_codex_rs.md)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `Provider` trait for the Codex App Server wrapper
- Spawn Codex App Server as subprocess using `ProcessManager` from `arky-provider`
- Implement newline-delimited JSON-RPC transport over stdio
- Implement `RpcTransport` for request/response correlation (match responses to pending requests by ID)
- Implement `Scheduler` for serialized model access (one model request at a time)
- Implement `ThreadManager` for multi-conversation thread control
- Implement `NotificationRouter` for routing async notifications to correct stream consumers
- Implement text accumulator and tool tracker for normalized output assembly
- Implement approval flow handling
- Normalize Codex events to `AgentEvent` variants
- Handle failure modes: JSON-RPC transport desync, process crash, stale thread routing, approval timeout, notification stream drop
- Pass `ProviderContractTests` from `arky-provider`
- Dependencies: `arky-error`, `arky-protocol`, `arky-provider`, `arky-tools`, `arky-mcp`
</requirements>

## Subtasks

- [ ] 11.1 Implement Codex App Server binary discovery and validation
- [ ] 11.2 Implement subprocess spawning with correct startup parameters
- [ ] 11.3 Implement JSON-RPC transport: request serialization, response deserialization, newline framing
- [ ] 11.4 Implement `RpcTransport` with request/response correlation by ID
- [ ] 11.5 Implement `Scheduler` for serialized model access
- [ ] 11.6 Implement `ThreadManager` for multi-conversation routing
- [ ] 11.7 Implement `NotificationRouter` for async notification dispatch to streams
- [ ] 11.8 Implement text accumulator and tool tracker for normalized event assembly
- [ ] 11.9 Implement approval flow handling (approve/deny tool execution requests)
- [ ] 11.10 Implement event normalization: Codex notifications -> `AgentEvent` variants
- [ ] 11.11 Implement `ProviderDescriptor` and `ProviderCapabilities` for Codex
- [ ] 11.12 Pass `ProviderContractTests` shared test suite
- [ ] 11.13 Write unit tests for JSON-RPC transport, correlation, scheduling, and thread routing
- [ ] 11.14 Write integration tests spawning real Codex App Server (behind integration flag)

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-codex/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-codex/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/provider.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/rpc.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/scheduler.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/thread.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/notification.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/approval.rs`
- `~/dev/compozy/arky/crates/arky-codex/src/accumulator.rs`
- `~/dev/compozy/arky/crates/arky-codex/tests/fixtures/` — JSON-RPC fixture files

### Dependent Files

- `~/dev/compozy/arky/crates/arky-provider/` — `Provider` trait, `ProcessManager`, `StdioTransport`, contract tests
- `~/dev/compozy/arky/crates/arky-protocol/` — `AgentEvent`, `Message`, event types
- `~/dev/compozy/arky/crates/arky-tools/` — `ToolRegistry`, `ToolIdCodec`
- `~/dev/compozy/arky/crates/arky-mcp/` — MCP tool bridge
- `tasks/prd-rust-providers/techspec.md` — Section: Codex App Server Integration
- `tasks/prd-rust-providers/adrs/adr-003-cli-wrapper-providers.md` — CLI wrapper design
- `tasks/prd-rust-providers/analysis_codex.md` — Codex provider analysis
- `tasks/prd-rust-providers/analysis_codex_rs.md` — Codex-RS reference analysis

## Deliverables

- Complete Codex provider implementing `Provider` trait
- JSON-RPC transport with request/response correlation
- Scheduler, ThreadManager, NotificationRouter
- Approval flow handling
- Event normalization to `AgentEvent`
- Passes `ProviderContractTests`
- Unit and integration tests

## Tests

### Unit Tests (Required)

- [ ] JSON-RPC transport: serialize request, deserialize response, verify correlation by ID
- [ ] JSON-RPC framing: newline-delimited parsing of multiple messages
- [ ] Scheduler: serialized access (second request waits until first completes)
- [ ] ThreadManager: route messages to correct thread, stale thread detection
- [ ] NotificationRouter: dispatch notification to correct stream consumer
- [ ] Text accumulator: incremental text assembly from multiple notifications
- [ ] Tool tracker: track tool call lifecycle from notifications
- [ ] Approval flow: approve/deny tool requests, timeout handling

### Integration Tests (Required)

- [ ] Spawn real Codex App Server (behind `#[cfg(feature = "integration")]`), send request, receive response
- [ ] Thread routing end-to-end: create thread, send message, verify response routing
- [ ] Provider contract tests: pass `ProviderContractTests` from `arky-provider`

### Regression and Anti-Pattern Guards

- [ ] JSON-RPC desync: mismatched IDs produce `ProtocolViolation`, not data corruption
- [ ] Process crash: handled gracefully with `ProcessCrashed` error
- [ ] Stale thread: detected and reported as error
- [ ] No `unwrap()` in library code

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-codex`
- [ ] `cargo test -p arky-codex --features integration` (requires Codex App Server)

## Success Criteria

- Codex provider implements `Provider` trait correctly
- JSON-RPC transport handles request/response correlation reliably
- Thread routing and notification dispatch work correctly
- Approval flow is functional
- Passes all `ProviderContractTests`
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_11.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
