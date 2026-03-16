## markdown

## status: completed

<task_context>
<domain>engine/server</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2,task12</dependencies>
</task_context>

# Task 13.0: `arky-server` Crate — HTTP/SSE Runtime Exposure

## Overview

Implement the `arky-server` crate providing an HTTP server that exposes the agent runtime state, session management, health checks, and Server-Sent Events (SSE) for real-time event delivery. This crate uses `axum` (feature-gated under `server`) and provides the external interface for monitoring, controlling, and observing running agents.

## Porting Context

This task uses the runtime server surface in
`../compozy-code/providers/runtime/src/server/*`, with
`runtime/src/client/runtime-client.ts` as a companion consumer reference, as
the main upstream reference for behavior and edge cases. Do not copy the
TypeScript API or module layout mechanically; prefer the Rust architecture and
quality bar defined in this PRD. Before implementation, read
`tasks/prd-rust-providers/porting-reference.md` and inspect the Task 13.0
upstream files listed there.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md)
- **ALWAYS READ** `tasks/prd-rust-providers/porting-reference.md` and inspect the Task 13.0 upstream TypeScript files before implementation
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement HTTP server using `axum` (feature-gated under `server`)
- Implement health routes: `GET /health` and `GET /ready`
- Implement session routes: list sessions, get session detail, get session messages
- Implement SSE endpoint for real-time event streaming from active agent sessions
- Implement replay endpoint: retrieve historical events for a completed session
- Expose provider health state (binary status, transport status, session compatibility)
- Implement proper CORS, error response formatting, and request validation
- Server must be non-blocking and integrate with the `Agent` instance from `arky-core`
- Dependencies: `arky-error`, `arky-protocol`, `arky-core`
- Feature-gated: `axum` under `server` feature
</requirements>

## Subtasks

- [x] 13.1 Set up `axum` server boilerplate with feature gate
- [x] 13.2 Implement `GET /health` and `GET /ready` endpoints
- [x] 13.3 Implement session routes: `GET /sessions`, `GET /sessions/:id`, `GET /sessions/:id/messages`
- [x] 13.4 Implement SSE endpoint: `GET /sessions/:id/events` for real-time event streaming
- [x] 13.5 Implement replay endpoint: `GET /sessions/:id/replay` for historical event retrieval
- [x] 13.6 Implement provider health exposure endpoints
- [x] 13.7 Implement error response formatting and request validation middleware
- [x] 13.8 Implement CORS configuration
- [x] 13.9 Integrate server with `Agent` instance (shared state via `Arc`)
- [x] 13.10 Write unit tests for route handlers with mock agent state
- [x] 13.11 Write integration tests for HTTP request/response round-trips

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-server/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-server/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-server/src/routes/health.rs`
- `~/dev/compozy/arky/crates/arky-server/src/routes/sessions.rs`
- `~/dev/compozy/arky/crates/arky-server/src/routes/events.rs`
- `~/dev/compozy/arky/crates/arky-server/src/routes/replay.rs`
- `~/dev/compozy/arky/crates/arky-server/src/middleware.rs`
- `~/dev/compozy/arky/crates/arky-server/src/state.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/` — Error formatting for HTTP responses
- `~/dev/compozy/arky/crates/arky-protocol/` — `AgentEvent`, `Message`, `SessionId`, session types
- `~/dev/compozy/arky/crates/arky-core/` — `Agent`, `EventSubscription`, session management
- `tasks/prd-rust-providers/techspec.md` — Section: Server, Health

## Deliverables

- HTTP server with health, session, SSE, and replay routes
- Feature-gated under `server` feature flag
- Error response formatting and CORS
- Integration with `Agent` from `arky-core`
- Unit and integration tests

## Tests

### Unit Tests (Required)

- [x] Health endpoint: returns 200 with correct body
- [x] Ready endpoint: returns 200 when agent is ready, 503 when not
- [x] Session list: returns sessions from mock store
- [x] Session detail: returns session snapshot for valid ID, 404 for invalid
- [x] Error formatting: errors map to correct HTTP status codes and JSON bodies

### Integration Tests (Required)

- [x] Full HTTP round-trip: start server, make requests with `reqwest`, verify responses
- [x] SSE endpoint: connect to event stream, inject events through agent, verify SSE delivery
- [x] Replay endpoint: create session with events, request replay, verify event sequence

### Regression and Anti-Pattern Guards

- [x] SSE connections are properly cleaned up on client disconnect
- [x] No `unwrap()` in library code
- [x] Server shutdown is graceful (in-flight requests complete)
- [x] CORS headers are present on all responses

### Verification Commands

- [x] `cargo fmt --check`
- [x] `cargo clippy -D warnings`
- [x] `cargo test -p arky-server --features server`

## Success Criteria

- All HTTP endpoints respond correctly
- SSE delivers real-time events from active sessions
- Replay endpoint returns historical events
- Health/ready endpoints reflect actual agent state
- Server integrates cleanly with `Agent` instance
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_13.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
