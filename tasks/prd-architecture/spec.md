# Arky Architecture Refactor Spec

## Problem Statement

The current workspace is high quality, but it is optimized for a narrower
problem: a provider-backed conversational runtime centered on sessions and
turns. That is a good execution substrate, but it is not yet a good product
architecture for the next phase of the project.

Today the main issues are:

| Concern | Current location | Current issue | Cost today |
|---|---|---|---|
| Runtime identity | `crates/arky-core`, `crates/arky-protocol` | Runtime is modeled mostly around `SessionId` and `TurnId` | Hard to add future product domains without polluting execution contracts |
| Server coupling | `crates/arky-server/src/state.rs` | `ServerState` depends directly on `arky_core::Agent` | API layer is tied to one runtime implementation |
| Persistence scope | `crates/arky-session` | Session storage is focused on transcripts, replay, and checkpoints | Future workflow/task/product state would likely get shoved into the wrong layer |
| Workspace topology | current 14-crate workspace | Some crates are appropriately isolated, others are too fine-grained or execution-centric | Refactors require touching too many surfaces |
| Future product evolution | not yet modeled | Product direction is still fluid | Premature domain crates would lock in the wrong abstractions |

This spec deliberately does **not** define the final product domain model for
workflows, issues, networks, governance, or organizations. That would be
premature. The goal here is to reshape the codebase so that those domains can
be added later without forcing them into `Agent`, `SessionStore`, or the server
routes.

## Design: Layered Runtime Architecture

### Key Principle

Treat the current Arky codebase as an execution foundation, not as the final
product surface.

That means:

- preserve the strong provider/tool/runtime work already done
- introduce clearer technical layers
- reduce direct dependencies on concrete runtime types
- avoid creating product-specific crates before the product domain stabilizes

### High-Level Architecture

```text
                           +----------------------+
                           |         arky         |
                           |     minimal facade   |
                           +----------+-----------+
                                      |
             +------------------------+------------------------+
             |                         |                        |
             v                         v                        v
   +------------------+      +------------------+      +------------------+
   |   arky-server    |      |  arky-control    |      |   arky-config    |
   | HTTP / SSE / API |      | app composition  |      | config loading   |
   +--------+---------+      +--------+---------+      +------------------+
            |                         |
            |                  +------+------+
            |                  |             |
            v                  v             v
   +------------------+  +------------------+  +------------------+
   |   arky-types     |  |  arky-runtime    |  |  arky-storage    |
   | shared contracts |  | execution engine |  | sessions/replay  |
   +--------+---------+  +--------+---------+  +--------+---------+
            |                     |                     |
            |             +-------+-------+             |
            |             |               |             |
            v             v               v             v
   +------------------+  +------------------+  +------------------+
   |  arky-provider   |  |   arky-tools     |  | arky-integrations|
   | model/processes  |  | tool registry    |  | MCP/hooks/bridges|
   +------------------+  +------------------+  +------------------+

Foundation:
- arky-error
```

## New Files

This spec proposes the following target file and crate structure.

```text
crates/
  arky/
  arky-error/
  arky-config/
  arky-types/
  arky-storage/
  arky-runtime/
  arky-provider/
  arky-tools/
  arky-tools-macros/
  arky-integrations/
  arky-control/
  arky-server/

apps/
  api/
  desktop/

tasks/
  prd-architecture/
    spec.md
```

This spec does **not** require creating all of these crates immediately. The
first phase is allowed to introduce the boundaries as modules and traits first,
then promote them into crates when the dependency direction is stable.

## 1. Layer Responsibilities

### 1.1 `arky-types`

Purpose:

- execution-layer IDs, requests, events, replay types, model references
- no runtime orchestration logic
- no HTTP DTOs specific to future product surfaces

Should contain:

- session and turn references
- execution event payloads
- tool call/result contracts
- provider request/usage types

Should not contain:

- workflow definitions
- issue/task domain types
- org/company/network types

### 1.2 `arky-storage`

Purpose:

- persistence for execution state only
- transcripts
- replay events
- checkpoints
- snapshots

Should contain:

- current `SessionStore`
- memory and sqlite backends
- snapshot/filter metadata

Should not contain:

- workflow state machines
- scheduling metadata
- governance or approval history
- future product entities unrelated to execution persistence

### 1.3 `arky-runtime`

Purpose:

- in-process agent execution engine
- turn loop
- event fanout
- resume/abort/stream behavior

Should contain:

- current `arky-core` runtime behavior
- command queue
- turn runtime
- replay-aware restore logic

Should not contain:

- HTTP concerns
- application bootstrap
- future workflow orchestration runtime
- ownership of product-level business policies

### 1.4 `arky-control`

Purpose:

- application composition layer
- runtime service assembly
- internal ports used by adapters

Should contain:

- traits such as `ChatRuntime`, `SessionReader`, `ModelCatalog`
- adapters that wrap concrete runtime/storage implementations
- service composition glue

Should not contain:

- transport concerns
- route handlers
- low-level provider logic

### 1.5 `arky-server`

Purpose:

- API adapter
- HTTP/SSE request parsing and response shaping
- auth middleware
- health/readiness exposure

Must depend on:

- service ports, not directly on `arky_core::Agent`

### 1.6 `arky-integrations`

Purpose:

- technical bridges around the runtime
- MCP
- hooks
- future non-product integration adapters

Rationale:

The current workspace has useful integration pieces, but they are scattered in
ways that make future system composition harder than necessary.

## 2. Crate Mapping

### 2.1 Current to Target

| Current crate | Target destination | Notes |
|---|---|---|
| `arky-core` | `arky-runtime` | main execution engine |
| `arky-session` | `arky-storage` | execution persistence only |
| `arky-protocol` | `arky-types` | execution contracts |
| `arky-provider` | `arky-provider` | keep |
| `arky-tools` | `arky-tools` | keep |
| `arky-tools-macros` | `arky-tools-macros` | keep |
| `arky-mcp` + `arky-hooks` | `arky-integrations` | merge by technical concern |
| `arky-server` | `arky-server` | keep as adapter layer |
| `arky-config` | `arky-config` | keep |
| `arky-error` | `arky-error` | keep |
| `arky` | `arky` | reduce to minimal facade |

### 2.2 Transitional Rule

Before moving crates physically, the team should first introduce boundaries by
trait and module so the move is structural, not cosmetic.

## 3. Recommended First Refactors

These are low-regret changes that improve the architecture immediately without
locking in the future product.

### 3.1 Decouple `arky-server` from `arky_core::Agent`

Introduce a server-facing runtime port:

```rust
#[async_trait]
pub trait RuntimeHandle: Send + Sync {
    async fn stream(
        &self,
        input: String,
    ) -> Result<arky_core::AgentEventStream, arky_core::CoreError>;

    async fn new_session(&self) -> Result<arky_protocol::SessionId, arky_core::CoreError>;

    async fn resume(
        &self,
        session_id: arky_protocol::SessionId,
    ) -> Result<(), arky_core::CoreError>;

    fn subscribe(&self) -> arky_core::EventSubscription;
}
```

This keeps the routes stable while removing direct ownership of the concrete
runtime implementation from the server state.

### 3.2 Reduce façade exports

The `arky` crate should export only stable, intentional surfaces. Avoid
re-exporting everything by default.

### 3.3 Preserve execution-only semantics in storage

Do not broaden `SessionStore` into a general application database interface.
When future domains arrive, they should get their own stores.

### 3.4 Split execution contracts from external API payloads

`arky-types` should remain the internal execution contract surface. If the
future product needs richer API DTOs, those should live in the adapter layer or
in future product-specific crates.

## 4. Interaction With Existing Systems

| Existing system | What changes | What stays the same |
|---|---|---|
| `arky_core::Agent` | stops being the direct dependency of the server | continues to power execution |
| `SessionStore` | becomes explicitly “execution persistence” | current replay/checkpoint semantics stay |
| routes in `arky-server` | start calling a runtime port | request/response shapes stay stable |
| provider integration | no design change in this phase | current provider model stays |
| tool registry | no design change in this phase | current registry model stays |

## 5. Public API Surface

### 5.1 `arky-server`

Public server API should expose:

- `ServerState`
- `ServerHandle`
- `ServerError`
- health/model structs already used externally

It should not require consumers to know which runtime implementation is behind
the state object.

### 5.2 `arky`

The root façade should export only:

- stable entry types
- runtime builder if intentionally supported
- server surface only if explicitly feature-gated

## 6. High-Level Implementation Overview

### 6.1 Before

```text
routes -> ServerState -> arky_core::Agent
                      -> SessionStore
```

### 6.2 After

```text
routes -> ServerState -> RuntimeHandle
                      -> SessionStore
                      -> model registry / health state

RuntimeHandle -> arky_core::Agent (temporary adapter)
```

### 6.3 Transitional Example

```rust
let agent = Arc::new(
    Agent::builder()
        .provider_arc(provider)
        .session_store_arc(store.clone())
        .model("mock-model")
        .build()?,
);

let state = ServerState::new(agent, store.clone());
```

This construction can remain source-compatible if `ServerState::new(...)`
accepts any `Arc<T>` where `T: RuntimeHandle + 'static`.

## 7. Testing Strategy

| Test level | Focus |
|---|---|
| Unit | route handlers use server-facing ports instead of concrete runtime types |
| Unit | `ServerState` accepts runtime trait implementations and preserves behavior |
| Integration | existing HTTP round-trips still pass |
| Regression | SSE/session behavior unchanged after server decoupling |

Mandatory verification for implementation work:

1. `make fmt`
2. `make lint`
3. `make test`

## 8. Implementation Order

| Step | What | Depends On | Parallelizable |
|---|---|---|---|
| 1 | Write this architecture spec | none | no |
| 2 | Introduce server-facing runtime trait | 1 | no |
| 3 | Update `ServerState` to depend on trait object | 2 | no |
| 4 | Adapt route handlers and tests | 3 | limited |
| 5 | Verify fmt/lint/tests | 4 | no |
| 6 | Split crate/module boundaries further (`types`, `storage`, `runtime`) | 2-5 | partially |
| 7 | Rework façade exports | 6 | yes |
| 8 | Introduce `arky-control` composition layer | 6 | partially |

## 9. Migration Path

### Phase 1: Port Decoupling

- add traits/ports
- keep concrete implementation the same
- avoid breaking the current user-facing server API

### Phase 2: Naming and Ownership Cleanup

- clarify execution-layer naming
- reduce façade exports
- stop `arky-server` from owning runtime-specific assumptions

### Phase 3: Crate Promotion

- promote stable module boundaries into target crates
- move `protocol` to `types`
- move `session` to `storage`
- move `core` to `runtime`

### Phase 4: Product-Layer Addition

- only after the product domain stabilizes
- add product-specific crates or services on top of this technical substrate

## 10. What Does NOT Move

| Concern | Current location | Why it stays |
|---|---|---|
| Provider contracts | `crates/arky-provider` | strong stable technical boundary |
| Tool registry and macros | `crates/arky-tools`, `crates/arky-tools-macros` | reusable foundation and already well-scoped |
| Shared error taxonomy | `crates/arky-error` | foundational concern with good reuse characteristics |
| Product-specific domain modeling | not yet present | intentionally deferred until scope stabilizes |

## 11. Open Questions

These questions are intentionally left unresolved by this spec:

- whether the future product is local-first or server-first
- whether workflow authoring begins as code-first or declarative graph-first
- whether future product concepts deserve their own crates or just services
- whether the root `arky` façade should preserve broad backward compatibility

## 12. Final Recommendation

Adopt the OpenFang-style **macro split** but not the OpenFang-style
**oversized coordinator files**.

For Arky, the right near-term move is:

- yes to technical subcrates
- no to product-domain subcrates yet
- yes to a server/runtime decoupling port now
- yes to eventually converging toward `types`, `storage`, `runtime`,
  `integrations`, `control`, and `server`
- no to forcing future workflows/issues/networks into the current execution
  model before the product domain is better understood
