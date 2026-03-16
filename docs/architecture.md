# Architecture Overview

Arky is a Cargo workspace that splits the SDK into narrowly-scoped crates.
That structure is intentional: the provider wrappers, orchestration runtime,
session persistence, tooling, MCP integration, and HTTP exposure all evolve at
different speeds and should not force cycles through a monolith.

## Design Goals

- Keep foundational contracts reusable by higher layers without back edges.
- Model CLI-backed providers as first-class streaming runtimes, not thin text
  generators.
- Preserve enough session state for safe resume and replay, not only message
  history.
- Normalize events, tools, and errors before they reach application code.
- Ship one ergonomic facade crate without hiding the workspace boundaries.

## Workspace Layers

| Layer | Crates | Responsibility |
| --- | --- | --- |
| Leaf | `arky-error`, `arky-protocol`, `arky-config`, `arky-tools-macros` | Shared contracts, IDs, protocol shapes, configuration, proc-macro support |
| Foundation | `arky-tools`, `arky-hooks`, `arky-session`, `arky-provider` | Tool registry, lifecycle hooks, persistence contracts, provider abstractions |
| Integration | `arky-mcp` | MCP client/server bridges and canonical tool naming |
| Providers | `arky-claude-code`, `arky-codex` | CLI-backed provider implementations and protocol normalization |
| Orchestration | `arky-core` | Agent queue, turn runtime, steering, follow-up, replay, event fanout |
| Exposure | `arky-server` | HTTP and SSE runtime exposure |
| Facade | `arky` | Curated public API and feature-gated re-exports |

## Key Runtime Flows

### Provider Streaming

1. A caller constructs a `ProviderRequest` or uses `Agent::prompt` /
   `Agent::stream`.
2. The selected provider starts a subprocess-backed stream.
3. Provider-specific parsers normalize native output into `AgentEvent` values.
4. Mid-stream failures remain in-band as `Result<AgentEvent, ProviderError>`.
5. The runtime decorates, persists, broadcasts, and optionally replays those
   events.

### Tool Execution

1. Providers emit canonical tool lifecycle events.
2. `arky-core` resolves tool calls through `ToolRegistry`.
3. Hooks run before and after tool execution.
4. Tool results are folded back into the turn transcript and replay log.

### Session Resume and Replay

1. `SessionStore` persists transcript messages, replay events, and the last
   turn checkpoint.
2. Resume restores the provider session identifier, replay cursor, and next
   sequencing state.
3. Replay endpoints and internal recovery routines read persisted
   `PersistedEvent` records in sequence order.

## Provider Notes

### Claude Code

- Reads Claude CLI `stream-json` lines from stdout.
- Tracks nested tool invocations and assistant snapshot reconciliation.
- Converts malformed lines or crashes into structured protocol/process errors.

### Codex

- Speaks newline-delimited JSON-RPC with the Codex app server.
- Separates notifications, server requests, and correlated responses.
- Routes thread-scoped notifications so concurrent sessions do not cross-talk.

## Quality Gates

- CI checks formatting, clippy, tests, docs, examples, benchmarks, and the
  dependency graph.
- Fixture corpora capture provider protocol regressions without requiring live
  provider binaries for every case.
- Benchmarks establish baselines for stream processing, subprocess startup, and
  replay access overhead.

## Why The Dependency Graph Is Enforced

The workspace architecture only stays healthy if lower layers remain lower
layers. A cycle between `arky-core` and any foundational crate would leak
orchestration concerns into contracts that should stay reusable and stable.
`scripts/check-deps.sh` codifies that rule so CI catches architectural drift
before it lands.
