# ADR-006: HTTP Chat Streaming Endpoint (`POST /v1/chat/stream`)

## Status

Accepted

## Date

2026-03-16

## Context

The TS runtime server exposes `POST /v1/chat/stream` — an HTTP endpoint that accepts messages, model, system prompt, sessionKey, and reasoningEffort, then returns SSE events. This is the primary entry point for HTTP consumers and follows OpenAI-compatible conventions.

The Rust `arky-server` currently only exposes read-only endpoints: health, sessions, events (SSE), and replay. There is no write/command endpoint for initiating conversations.

## Decision

Implement `POST /v1/chat/stream` in `arky-server` with:

1. **Request body**: messages, model, system_prompt, session_key, resume_session, max_steps, reasoning_effort
2. **Auth**: bearer token validation via auth middleware (see ADR-007 implicit in auth gap)
3. **Provider dispatch**: resolve provider from model ID via `ProviderRegistry` (with model-prefix inference)
4. **Session handling**: create or resume session based on session_key
5. **SSE response**: stream `AgentEvent`s as SSE with monotonic sequence IDs and `[DONE]` sentinel
6. **Error handling**: structured error payloads as SSE events on failure

Also implement `GET /v1/models` for OpenAI-compatible model listing with provider metadata.

## Alternatives Considered

### Alternative 1: Programmatic API Only (No HTTP Endpoint)

- **Description**: Consumers use `Agent` directly in Rust code; server stays read-only
- **Pros**: Less code, simpler server
- **Cons**: No HTTP access for external clients (web UIs, non-Rust consumers, microservices)
- **Why rejected**: Full parity requires HTTP access; HTTP is the standard integration point

## Consequences

### Positive

- External clients can use the SDK via HTTP without Rust dependency
- OpenAI-compatible conventions enable broad tooling compatibility
- SSE streaming provides real-time event delivery

### Negative

- Additional ~300-400 lines in `arky-server`
- Requires auth middleware, request validation, provider dispatch logic
- SSE formatting needs sequence IDs and `[DONE]` sentinel

### Risks

- Performance: HTTP overhead vs direct Agent API
- Mitigation: HTTP endpoint is for external consumers; Rust consumers use Agent directly
- Security: endpoint must be authenticated
- Mitigation: bearer token auth middleware (constant-time comparison via `subtle` crate)

## Implementation Notes

- Route: `POST /v1/chat/stream` in `arky-server/src/routes/`
- Request schema: `ChatStreamRequest` struct with serde
- Response: `axum::response::sse::Sse` with `AgentEvent` -> SSE event mapping
- Sequence IDs: monotonic counter per stream, sent as `id` field in SSE
- `[DONE]` sentinel: final SSE event with `data: [DONE]` (OpenAI convention)
- Model listing: `GET /v1/models` returns `{ data: [{ id, object: "model", ... }] }`
- Auth: axum middleware layer extracting `Authorization: Bearer <token>`, constant-time comparison

## References

- TS source: `providers/runtime/src/server/app.ts`
- TS source: `providers/runtime/src/server/sse-writer.ts`
- TS source: `providers/runtime/src/server/routes-models.ts`
- TS source: `providers/runtime/src/server/auth.ts`
- Gap analysis: `tasks/prd-gaps/analysis_server_session_usage.md` (GAP-SSU-002, GAP-SSU-003, GAP-SSU-004)
