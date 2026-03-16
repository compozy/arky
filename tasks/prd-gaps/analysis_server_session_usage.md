# Gap Analysis: Server, Session & Usage Infrastructure

## Summary

The TypeScript `compozy-code` runtime infrastructure is a comprehensive, production-grade system spanning five major subsystems: an HTTP server with SSE streaming and OpenAI-compatible endpoints, bearer token authentication middleware, a session store abstraction with both in-memory and SQLite backends, a complete usage/token tracking and cost calculation pipeline, and a runtime client SDK wrapping the server as a typed async API. In addition, the Codex provider ships its own full application server layer with JSON-RPC over stdio, subprocess lifecycle management, thread/turn orchestration, auto-approval, model discovery with pagination, and a shared registry with ref-counted idle shutdown. This infrastructure totals approximately 50+ source files across 6 directories (server, session, usage, client in `runtime/src/`; server in `codex/src/`).

The Rust `arky` codebase addresses the server and session domains with a different -- and in several respects more capable -- architectural approach. `arky-server` exposes an axum-based HTTP server with health/readiness endpoints, per-provider health monitoring, full session CRUD, SSE event streaming, and replay support. `arky-session` provides a `SessionStore` trait with in-memory and SQLite backends that store complete session data including messages, events, turn checkpoints, labels, expiration, and replay cursors -- substantially richer than the TS metadata-only model. `arky-mcp` delivers a full MCP client/server/bridge with stdio and HTTP transports, tool schema translation, OAuth flow, and keepalive. `arky-config` provides TOML/YAML/env configuration loading with deep merge and validation.

However, several critical subsystems present in the TS codebase are entirely absent in Rust: there is no usage/token tracking or cost calculation system, no streaming chat endpoint (`/v1/chat/stream`), no OpenAI-compatible model listing endpoint (`/v1/models`), no HTTP-level bearer token authentication middleware, no runtime client abstraction, and the Codex application server's process lifecycle, JSON-RPC transport, thread management, scheduling, and approval handling are not represented as standalone infrastructure (though some concerns are addressed differently through `arky-codex`'s provider-level subprocess management). The overall functional coverage of the TS server/session/usage infrastructure is estimated at 40-45% in Rust, with the session store being the one area where Rust is significantly ahead of TypeScript.

## Feature Comparison Matrix

| Feature | compozy-code (TS) | arky (Rust) | Status | Priority |
|---------|-------------------|-------------|--------|----------|
| **HTTP Server** | | | | |
| Server lifecycle (start/stop) | Full: `createRuntimeServer` returns `{fetch, dispose}` | Full: `serve()` returns `ServerHandle` with graceful shutdown via `CancellationToken` | Complete | -- |
| Health endpoint | `GET /health` returns status | `GET /health` + `GET /ready` + per-provider health | Complete+ | -- |
| CORS middleware | Not explicitly present | Full: allow any origin, GET+OPTIONS | Complete+ | -- |
| Error response mapping | `withErrorResponses`: 13 error types mapped to HTTP status codes | `ErrorEnvelope` with `ClassifiedError` trait for HTTP mapping | Complete | -- |
| Chat streaming endpoint | `POST /v1/chat/stream` with SSE output, model/messages/system/sessionKey/resumeSession | None | Missing | P0 |
| Models listing endpoint | `GET /v1/models` with OpenAI-compatible format + compozy metadata | None | Missing | P1 |
| SSE writer/formatting | `RuntimeSseWriter`: sequence IDs, `[DONE]` sentinel, error payloads | SSE via `axum::response::sse` with event name mapping, 10s keepalive | Partial | P1 |
| Session CRUD endpoints | Not present (sessions managed client-side) | Full: `GET /sessions`, `GET /sessions/{id}`, `GET /sessions/{id}/messages` | Complete+ | -- |
| Session event streaming | Not present as server route | Full: `GET /sessions/{id}/events` SSE stream | Complete+ | -- |
| Session replay endpoint | Not present | Full: `GET /sessions/{id}/replay` | Complete+ | -- |
| Provider health endpoints | Not present | Full: `GET /providers/health`, `GET /providers/{id}/health` | Complete+ | -- |
| Readiness checks | Not present | Full: `GET /ready` with reason reporting | Complete+ | -- |
| **Authentication** | | | | |
| Bearer token auth middleware | Full: `AuthService.authorizeRequest` with `timingSafeCompare` | None on HTTP server (MCP-level auth exists in `arky-mcp`) | Missing | P1 |
| No-auth bypass layer | Full: `noAuthLayer` for optional auth bypass | N/A | Missing | P1 |
| Timing-safe comparison | Full: Node.js `crypto.timingSafeEqual` | None | Missing | P1 |
| **Session Store** | | | | |
| Store trait/interface | `SessionStoreService`: getByTaskId, getByKey, get, set, delete, touch | `SessionStore` trait: create, load, append_messages, append_events, save_turn_checkpoint, replay_events, list, delete | Complete+ | -- |
| Session metadata | sessionId, providerId, taskId, sessionKey, createdAt, lastAccessedAt, workingDirectory | sessionId, providerId, labels, message_count, event_count, created_at, updated_at, expires_at | Complete+ | -- |
| In-memory backend | Full: dual-map (byId + byKey), TTL eviction, capacity eviction (max 500) | Full: BTreeMap + VecDeque, TTL + capacity eviction | Complete | -- |
| SQLite backend | Wraps `SqliteSessionRepository`, metadata-only, legacy provider compat | Full: WAL mode, reader/writer separation, write_gate, retry on busy/locked, 5-table schema (sessions, labels, messages, events, checkpoints) | Complete+ | -- |
| Message persistence | Not in session store (consumers manage history) | Full: messages stored in session with append + load | Complete+ | -- |
| Event persistence | Not present | Full: events stored with append + replay_events | Complete+ | -- |
| Turn checkpoints | Not present | Full: `save_turn_checkpoint` + `TurnCheckpoint` with cursor support | Complete+ | -- |
| Replay support | Not present | Full: `ReplayCursor` + `replay_events` with after-cursor filtering | Complete+ | -- |
| Session filtering | Not present | Full: `SessionFilter` with provider_id, label, created_after/before, limit | Complete+ | -- |
| Session labels | Not present | Full: label support in metadata + filter by label | Complete+ | -- |
| Compound key lookup (taskId/sessionKey) | Full: `getByTaskId`, `getByKey` with reverse index | Not present (lookup by SessionId only) | Missing | P2 |
| Touch/last-access tracking | Full: `touch` updates `lastAccessedAt` | Partial: `updated_at` tracked but no explicit `touch` method | Partial | P2 |
| **Usage / Token Tracking** | | | | |
| Usage type definitions | Full: `RuntimeUsage` with inputTokens, outputTokens, totalTokens, inputDetails (cacheRead/cacheWrite/noCache), outputDetails (text/reasoning), costUsd, durationMs | None | Missing | P0 |
| Token consumption resolution | Full: `RuntimeConsumption` with `resolve()`, `fromStreamResult()`, `fromChunk()` static methods | None | Missing | P0 |
| Normalized token consumption | Full: `NormalizedTokenConsumption` resolving from usage, result, and chunk | None | Missing | P0 |
| Provider metadata extraction | Full: `extractProviderMetadata` resolving sessionId, costUsd, durationMs, rawUsage, modelUsage, warnings | None | Missing | P0 |
| Cost calculation | Full: costUsd resolution from provider metadata | None | Missing | P0 |
| Metering shape normalization | Full: resolves V3 shape and LanguageModel shape | None | Missing | P0 |
| Native event utility helpers | Full: `asRecord`, `asString`, `asInteger`, `asNumber` | None | Missing | P2 |
| **Runtime Client** | | | | |
| Client service interface | Full: `RuntimeClientService` with streamText, registerTools, createSession, resumeSession, resumeOrCreateSession, dispose | None | Missing | P1 |
| Async wrapper | Full: `RuntimeClientAsync` wrapping Effect service with Promise API | None | Missing | P1 |
| Dispose lifecycle | Full: semaphore-guarded cleanup | None | Missing | P1 |
| **MCP Server** | | | | |
| MCP server (stdio) | Present in TS but outside examined scope | Full: `McpServer` with stdio transport via rmcp | Complete | -- |
| MCP server (HTTP) | Present in TS but outside examined scope | Full: `McpServer` with streamable HTTP transport | Complete | -- |
| MCP client (stdio) | Present in TS but outside examined scope | Full: `McpClient` with connection lifecycle, keepalive, tool import/refresh | Complete | -- |
| MCP client (HTTP) | Present in TS but outside examined scope | Full: `McpClient` with HTTP transport | Complete | -- |
| MCP tool bridge | Present in TS but outside examined scope | Full: `McpToolBridge` with bidirectional tool import/export + schema translation | Complete | -- |
| MCP auth (Bearer/OAuth) | Present in TS but outside examined scope | Full: `McpAuth` with Bearer + OAuth flow | Complete | -- |
| **Config** | | | | |
| Config file loading | Not explicitly in examined scope | Full: TOML/YAML file loading with `ConfigLoader` | Complete | -- |
| Environment variable overrides | Not explicitly in examined scope | Full: `ARKY_` prefix env var resolution | Complete | -- |
| Deep merge (file + env + builder) | Not explicitly in examined scope | Full: `merge_config` with workspace/provider/agent merge | Complete | -- |
| Validation with issue collection | Not explicitly in examined scope | Full: required field checks, binary path validation, issue accumulation | Complete | -- |
| **Codex App Server** | | | | |
| Process lifecycle management | Full: `CodexProcessManager` with spawn, SIGTERM/SIGKILL, respawn | Addressed differently via `arky-codex` provider subprocess (ProcessManager + StdioTransport) | Partial | P1 |
| JSON-RPC transport (stdio) | Full: `CodexRpcTransport` with full JSON-RPC 2.0, request/response correlation, notification routing | Not present as standalone; arky-codex uses line-delimited JSON notifications | Missing | P1 |
| Thread management | Full: `CodexThreadManager` with start/resume, turn lifecycle, notification streaming, compaction | Not present; single-session model in arky-codex | Missing | P1 |
| Request scheduler | Full: `CodexScheduler` with semaphore permits, overflow detection, timeouts | Not present | Missing | P2 |
| Auto-approval handler | Full: `CodexApprovalHandler` for command exec + file change approvals | Not present (approval handled differently via provider config) | Missing | P2 |
| Model service with pagination | Full: `CodexModelService` with caching, fallback IDs, pagination | Not present | Missing | P2 |
| App-server registry | Full: `CodexRegistry` with ref-counting, idle shutdown | Not present | Missing | P2 |
| Runtime config with respawn | Full: `CodexRuntimeConfig` with reconfigure/respawn support | Not present | Missing | P2 |
| Notification router | Full: `CodexNotificationRouter` routing by threadId/scopeId | Not present (arky-codex handles notifications in provider stream) | Missing | P2 |

## Detailed Gap Analysis

### GAP-SSU-001: Usage / Token Tracking System

- **TS Location**: `providers/runtime/src/usage/` (5 files: `types.ts`, `consumption.ts`, `metadata-extractor.ts`, `native-event-utils.ts`, `token-consumption.ts`)
- **Rust Status**: Entirely absent. No crate or module addresses usage tracking.
- **Complexity**: Medium-High. Requires defining usage types in `arky-protocol`, a consumption resolver, provider metadata extraction hooks, and cost calculation. Must integrate with the agent event stream.
- **Description**: The TS codebase provides a complete token consumption pipeline: `RuntimeUsage` captures input/output tokens with cache breakdown (cacheRead, cacheWrite, noCache) and output detail (text vs reasoning tokens), plus cost in USD and duration in ms. `RuntimeConsumption` resolves usage from three sources (direct usage object, stream result metadata, individual stream chunks). `NormalizedTokenConsumption` normalizes across provider-specific shapes (V3 vs LanguageModel). `extractProviderMetadata` pulls sessionId, costUsd, durationMs, rawUsage, modelUsage, and warnings from provider-specific metadata keys. The Rust codebase has `TokenUsage` in `arky-protocol` with `input_tokens` and `output_tokens` fields but no cache breakdown, no cost calculation, no metering resolution, and no consumption accumulation pipeline.
- **Dependencies**: `arky-protocol` (types), `arky-provider` (metadata extraction hook), `arky-core` (integration with agent loop events)

### GAP-SSU-002: Chat Streaming Endpoint (`/v1/chat/stream`)

- **TS Location**: `providers/runtime/src/server/app.ts` (route definition), `providers/runtime/src/server/sse-writer.ts` (SSE formatting)
- **Rust Status**: Missing. `arky-server` has session event SSE streaming but no chat input endpoint.
- **Complexity**: High. Requires request validation (messages, model, system prompt, sessionKey, maxSteps, reasoningEffort), provider dispatch, session creation/resume, SSE response formatting with sequence IDs, `[DONE]` sentinel, and error payloads.
- **Description**: The TS `POST /v1/chat/stream` endpoint accepts a `StreamRequestBody` with messages, model, system prompt, maxSteps, sessionKey, resumeSession flag, and reasoningEffort. It pipes through authentication, resolves the provider, creates or resumes a session, and streams SSE events back. `RuntimeSseWriter` formats events with sequence IDs and sends a `[DONE]` sentinel on completion. The Rust server currently only exposes read-only endpoints (health, sessions, events, replay) with no write/command endpoints.
- **Dependencies**: GAP-SSU-001 (usage tracking for response metadata), GAP-SSU-004 (auth middleware), `arky-core` (agent dispatch), `arky-session` (session create/resume)

### GAP-SSU-003: OpenAI-Compatible Model Listing Endpoint (`/v1/models`)

- **TS Location**: `providers/runtime/src/server/routes-models.ts`
- **Rust Status**: Missing.
- **Complexity**: Low-Medium. Requires iterating registered providers, collecting model metadata, and formatting OpenAI-compatible responses with compozy extensions (context_window, cost, supports_tools, etc.).
- **Description**: The TS `GET /v1/models` endpoint returns an OpenAI-compatible model list with optional `provider_id` filter. Each model includes standard fields plus compozy metadata: `context_window`, `cost` (input/output per token), `supports_tools`, `supports_streaming`, `supports_structured_output`. The Rust `arky-provider` has `ProviderDescriptor` with capabilities but no endpoint exposing this.
- **Dependencies**: `arky-provider` (ProviderRegistry access), `arky-server` (route addition)

### GAP-SSU-004: HTTP Bearer Token Authentication Middleware

- **TS Location**: `providers/runtime/src/server/auth.ts`
- **Rust Status**: Missing at HTTP server level. `arky-mcp` has `McpAuth` with Bearer + OAuth but this is MCP-transport specific, not HTTP middleware.
- **Complexity**: Low. Requires an axum middleware layer extracting `Authorization: Bearer <token>`, timing-safe comparison, and a bypass option.
- **Description**: The TS `AuthService` extracts the bearer token from the `Authorization` header, performs timing-safe comparison using Node.js `crypto.timingSafeEqual`, and returns 401 on failure. A `noAuthLayer` allows optional bypass. The Rust server has no authentication on any endpoint.
- **Dependencies**: `arky-server` (middleware integration). Consider using `subtle` crate for constant-time comparison.

### GAP-SSU-005: Runtime Client Abstraction

- **TS Location**: `providers/runtime/src/client/runtime-client.ts`
- **Rust Status**: Entirely absent.
- **Complexity**: Medium. Requires a typed client struct wrapping HTTP calls to the arky-server endpoints, with streaming support for SSE consumption, session management helpers, and disposal lifecycle.
- **Description**: The TS `RuntimeClientService` provides `streamText`, `registerTools`, `createSession`, `resumeSession`, `resumeOrCreateSession`, and `dispose`. `RuntimeClientAsync` wraps the Effect-based service with a Promise API for non-Effect consumers. The dispose lifecycle uses a semaphore guard. No equivalent exists in Rust -- consumers must construct raw HTTP requests to `arky-server`.
- **Dependencies**: `arky-server` (must expose the endpoints the client calls), GAP-SSU-002 (chat stream endpoint)

### GAP-SSU-006: Codex JSON-RPC Transport

- **TS Location**: `providers/codex/src/server/CodexRpcTransport.ts`
- **Rust Status**: Missing. `arky-codex` uses line-delimited JSON notifications over stdio, not a bidirectional JSON-RPC protocol.
- **Complexity**: High. Full JSON-RPC 2.0 implementation: request/response correlation with IDs, notification routing, server-initiated requests (reverse RPC), batch support, and error handling.
- **Description**: The TS `CodexRpcTransport` implements complete JSON-RPC 2.0 over stdio with: pending request map for correlation, notification queue partitioned by type, server-request handler for reverse RPC (e.g., approval requests from Codex), line-based framing with buffer management, and typed parse/serialize. The Rust `arky-codex` reads notifications line-by-line without request/response correlation.
- **Dependencies**: `arky-codex` (provider architecture), potentially a shared `arky-rpc` crate

### GAP-SSU-007: Codex Thread & Turn Management

- **TS Location**: `providers/codex/src/server/CodexThreadManager.ts`, `providers/codex/src/server/CodexScheduler.ts`
- **Rust Status**: Missing. `arky-codex` operates in a single-session model.
- **Complexity**: High. Thread lifecycle (create/resume/compact), turn start with notification streaming, request scheduling with semaphore permits and overflow detection, timeout enforcement.
- **Description**: The TS `CodexThreadManager` manages multiple concurrent threads, each with its own turn lifecycle. `startThread` initiates a conversation, `resumeThread` continues one, `startTurn` sends a message and streams notifications until turn completion. `CodexScheduler` gates concurrent requests with semaphore permits, detects overflow (too many pending requests), and enforces per-request timeouts. Compaction support allows thread history compression. None of this multi-thread orchestration exists in Rust.
- **Dependencies**: GAP-SSU-006 (JSON-RPC transport), `arky-codex`

### GAP-SSU-008: Codex Approval Handler

- **TS Location**: `providers/codex/src/server/CodexApprovalHandler.ts`
- **Rust Status**: Missing as standalone handler. `arky-codex` config has `auto_approve` flag but no handler for server-initiated approval requests.
- **Complexity**: Low-Medium. Respond to server-initiated JSON-RPC requests for command execution and file change approvals based on policy configuration.
- **Description**: The TS `CodexApprovalHandler` receives reverse-RPC requests from the Codex process asking for approval to execute commands or modify files. It auto-approves based on configuration, supporting both command execution approval and file change approval with different policies. The Rust `arky-codex` passes `--full-auto` flag to the CLI but cannot handle interactive approval requests.
- **Dependencies**: GAP-SSU-006 (JSON-RPC transport for receiving server requests)

### GAP-SSU-009: Codex Model Service & Registry

- **TS Location**: `providers/codex/src/server/CodexModelService.ts`, `providers/codex/src/server/CodexRegistry.ts`
- **Rust Status**: Missing.
- **Complexity**: Medium. Model listing with RPC call, pagination, caching, fallback model IDs. Registry with ref-counted app-server instances and idle shutdown timer.
- **Description**: `CodexModelService` lists available models via RPC to the Codex process with pagination and response caching, providing fallback model IDs when the process is unavailable. `CodexRegistry` maintains a shared map of `CodexAppServer` instances keyed by configuration hash, with ref-counting for shared access and idle shutdown (instances are terminated after a configurable period with no active references).
- **Dependencies**: GAP-SSU-006 (JSON-RPC transport), GAP-SSU-007 (thread management)

### GAP-SSU-010: Compound Session Key Lookup

- **TS Location**: `providers/runtime/src/session/session-store.ts` (`getByTaskId`, `getByKey`), `providers/runtime/src/session/sqlite-session-store.ts` (reverse index with `SESSION_REVERSE_KEY_PREFIX`)
- **Rust Status**: Missing. `arky-session` supports lookup by `SessionId` only.
- **Complexity**: Low. Add secondary index support (by task ID, by compound key) to `SessionStore` trait and both backends.
- **Description**: The TS session store supports three lookup paths: by session ID, by task ID, and by a compound session key (provider + working directory + user-defined key). The SQLite backend uses a reverse index with a key prefix for efficient compound lookups. The Rust `SessionStore` trait only supports `load(session_id)` and `list(filter)`.
- **Dependencies**: `arky-session` (trait extension + backend implementations)

### GAP-SSU-011: SSE Writer with Sequence IDs and Sentinel

- **TS Location**: `providers/runtime/src/server/sse-writer.ts`
- **Rust Status**: Partial. `arky-server` uses axum's SSE with event name mapping but no sequence IDs or `[DONE]` sentinel.
- **Complexity**: Low. Add monotonic sequence ID to each SSE event, emit `[DONE]` on stream completion, format error payloads as SSE events.
- **Description**: The TS `RuntimeSseWriter` assigns monotonic sequence IDs to every SSE event, emits structured error payloads as SSE events on failure, and sends a `[DONE]` sentinel event when the stream completes (matching the OpenAI SSE convention). The Rust SSE implementation maps `AgentEvent` variants to named events but lacks sequence tracking and completion signaling.
- **Dependencies**: `arky-server` (routes/events.rs modification)

## Files Reference

### TypeScript (compozy-code)

**Server:**
- `providers/runtime/src/server/app.ts` - HTTP server with routes, error mapping, stream request schema
- `providers/runtime/src/server/auth.ts` - Bearer token auth with timing-safe comparison
- `providers/runtime/src/server/sse-writer.ts` - SSE formatting with sequence IDs and `[DONE]` sentinel
- `providers/runtime/src/server/http-utils.ts` - JSON response helper
- `providers/runtime/src/server/routes-models.ts` - OpenAI-compatible model listing
- `providers/runtime/src/server/index.ts` - Module exports

**Session:**
- `providers/runtime/src/session/session-store.ts` - `SessionStoreService` interface
- `providers/runtime/src/session/in-memory-store.ts` - In-memory backend with TTL + capacity eviction
- `providers/runtime/src/session/sqlite-session-store.ts` - SQLite backend with reverse index
- `providers/runtime/src/session/index.ts` - Module exports

**Usage:**
- `providers/runtime/src/usage/types.ts` - `RuntimeUsage` with token breakdown and cost
- `providers/runtime/src/usage/consumption.ts` - `RuntimeConsumption` resolver class
- `providers/runtime/src/usage/metadata-extractor.ts` - `extractProviderMetadata` function
- `providers/runtime/src/usage/native-event-utils.ts` - Helper extraction functions
- `providers/runtime/src/usage/token-consumption.ts` - `NormalizedTokenConsumption` resolver
- `providers/runtime/src/usage/index.ts` - Module exports

**Client:**
- `providers/runtime/src/client/runtime-client.ts` - `RuntimeClientService` and `RuntimeClientAsync`

**Codex Server:**
- `providers/codex/src/server/CodexAppServer.ts` - Orchestrator: process, transport, notifications, threads
- `providers/codex/src/server/CodexServerLayer.ts` - Effect Layer composition
- `providers/codex/src/server/CodexProcessManager.ts` - Subprocess lifecycle (spawn, SIGTERM/SIGKILL)
- `providers/codex/src/server/CodexThreadManager.ts` - Thread start/resume, turn lifecycle, compaction
- `providers/codex/src/server/CodexScheduler.ts` - Request scheduling with semaphore permits
- `providers/codex/src/server/CodexRpcTransport.ts` - Full JSON-RPC 2.0 over stdio
- `providers/codex/src/server/CodexApprovalHandler.ts` - Auto-approval for commands and file changes
- `providers/codex/src/server/CodexModelService.ts` - Model listing with pagination and caching
- `providers/codex/src/server/CodexRegistry.ts` - Ref-counted app-server registry with idle shutdown
- `providers/codex/src/server/CodexRuntimeConfig.ts` - Runtime config with reconfigure/respawn
- `providers/codex/src/server/CodexNotificationRouter.ts` - Notification routing by thread/scope
- `providers/codex/src/server/types.ts` - Type definitions for turns, threads, models, approvals

### Rust (arky)

**arky-server:**
- `crates/arky-server/src/lib.rs` - Router definition, `ServerHandle`, `serve()` function
- `crates/arky-server/src/state.rs` - `ServerState`, `RuntimeHealthRegistry`, health types
- `crates/arky-server/src/middleware.rs` - CORS layer, parse helpers
- `crates/arky-server/src/error.rs` - `ServerError` enum, `ErrorEnvelope`, classified error mapping
- `crates/arky-server/src/routes/health.rs` - Health, readiness, provider health endpoints
- `crates/arky-server/src/routes/sessions.rs` - Session list, detail, messages endpoints
- `crates/arky-server/src/routes/events.rs` - SSE session event streaming
- `crates/arky-server/src/routes/replay.rs` - Session replay endpoint

**arky-session:**
- `crates/arky-session/src/lib.rs` - Crate root, re-exports
- `crates/arky-session/src/store.rs` - `SessionStore` trait definition
- `crates/arky-session/src/memory.rs` - `InMemorySessionStore` with BTreeMap + VecDeque
- `crates/arky-session/src/sqlite.rs` - `SqliteSessionStore` with WAL, retry, 5-table schema
- `crates/arky-session/src/snapshot.rs` - `NewSession`, `SessionMetadata`, `SessionSnapshot`, `SessionFilter`
- `crates/arky-session/src/error.rs` - `SessionError` enum

**arky-mcp:**
- `crates/arky-mcp/src/lib.rs` - Crate root, re-exports
- `crates/arky-mcp/src/server.rs` - `McpServer` with stdio and HTTP transports
- `crates/arky-mcp/src/client.rs` - `McpClient` with connection lifecycle and keepalive
- `crates/arky-mcp/src/bridge.rs` - `McpToolBridge` for bidirectional tool import/export
- `crates/arky-mcp/src/auth.rs` - `McpAuth` with Bearer token and OAuth flow
- `crates/arky-mcp/src/naming.rs` - Canonical name encoding/decoding
- `crates/arky-mcp/src/error.rs` - `McpError` enum

**arky-config:**
- `crates/arky-config/src/lib.rs` - Crate root, re-exports
- `crates/arky-config/src/loader.rs` - `ConfigLoader` with TOML/YAML/env/builder support
- `crates/arky-config/src/merge.rs` - Deep merge functions
- `crates/arky-config/src/validate.rs` - Validation with issue collection
- `crates/arky-config/src/error.rs` - `ConfigError` enum
