# ADR-005: Codex Server Registry with Ref-Counting and Idle Shutdown

## Status

Accepted

## Date

2026-03-16

## Context

The most impactful architectural difference between the TS and Rust Codex providers is server process lifecycle. The TS implementation maintains long-lived Codex app server processes via a registry with:

- **Key-based caching**: config normalized into a deterministic JSON key; same config = same process
- **Ref-counting**: `acquire()` increments refCount, `release()` decrements; multiple consumers share one process
- **Idle shutdown**: when refCount reaches 0, a configurable timer starts; if no new acquire before timeout, process is disposed; if someone acquires, timer is cancelled
- **Lazy init**: process only spawns on first `ensureReady()`
- **Reconfiguration**: critical fields (codexPath, cwd, env, timeouts) change triggers respawn; non-critical changes are soft updates
- **sharedAppServerKey**: override allowing multiple working directories to share one server

The Rust `arky-codex` spawns a **fresh process for every `stream()` call**, paying startup latency each time with no session continuity.

## Decision

Implement a **Codex server registry** in `arky-codex` matching the TS design:

1. `CodexServerRegistry` — manages `CodexAppServer` instances keyed by normalized config hash
2. `CodexAppServer` — wraps a long-lived process with `RpcTransport`, lazy init via `ensure_ready()`
3. Ref-counted leases: `acquire(config) -> CodexLease`, `CodexLease::release()` on drop
4. Idle shutdown: configurable `idle_shutdown_ms`, timer-based disposal when refCount reaches 0
5. Reconfiguration: `reconfigure(updates)` detects critical field changes, triggers respawn if needed
6. `shared_app_server_key`: optional override that normalizes cwd out of the registry key

The registry key is computed by serializing and hashing the critical config fields (codex_path, cwd, env, allow_npx, timeouts, max_in_flight, max_queued, auto_approve).

## Alternatives Considered

### Alternative 1: Single Long-Lived Process per Provider

- **Description**: `CodexProvider` maintains one process internally (lazy init, reuse across streams, restart on crash)
- **Pros**: Simpler (~150-200 lines), solves 90% of performance issue
- **Cons**: No multi-config support, no shared server across cwds, no ref-counting
- **Why rejected**: Does not match TS design; `sharedAppServerKey` and multi-config scenarios are real use cases

### Alternative 2: Keep Spawn-Per-Stream

- **Description**: Accept startup latency, use session resume for continuity
- **Pros**: Zero complexity
- **Cons**: Poor performance, no real session continuity, significant deviation from TS behavior
- **Why rejected**: Unacceptable performance and UX regression

## Consequences

### Positive

- Amortizes process startup across many requests
- Session continuity within a server instance
- Multiple consumers (threads, turns) share one process efficiently
- Matches proven TS design

### Negative

- Significant implementation effort (~400-500 lines)
- Lifecycle complexity (ref-counting, idle timers, reconfiguration)
- Must handle edge cases (process crash during idle, concurrent acquire/release)

### Risks

- Ref-counting bugs can leak processes or cause premature shutdown
- Mitigation: `CodexLease` implements `Drop` for automatic release; integration tests verify lifecycle
- Idle timer races with acquire
- Mitigation: all registry mutations go through a `tokio::sync::Mutex`; timer checks refCount atomically

## Implementation Notes

- Registry key: `serde_json::to_string(&normalized_config)` then hash
- `CodexLease` wraps `Arc<CodexAppServer>` and decrements refCount on `Drop`
- Idle shutdown: `tokio::time::sleep` spawned as a `JoinHandle`, cancelled via `AbortHandle` on re-acquire
- Critical fields for reconfiguration: `codex_path`, `cwd`, `allow_npx`, `env`, `sanitize_environment`, `auto_approve`, `request_timeout_ms`, `startup_timeout_ms`, `max_in_flight_requests`, `max_queued_requests`, `model_cache_ttl_ms`
- Non-critical fields: `idle_shutdown_ms`, `shared_app_server_key`, `compaction_token_limit`, `model_context_window`

## References

- TS source: `providers/codex/src/server/CodexRegistry.ts`
- TS source: `providers/codex/src/server/CodexAppServer.ts`
- TS source: `providers/codex/src/server/CodexRuntimeConfig.ts`
- TS source: `providers/codex/src/compat.ts` (registry usage)
- Gap analysis: `tasks/prd-gaps/analysis_codex.md` (GAP-CDX-007, GAP-CDX-012)
