# Porting Reference Map: `prd-rust-providers`

## Read This First

This PRD is grounded in the TypeScript provider system that already exists in
the sibling repository at:

- Relative path from this repo: `../compozy-code/providers`
- Absolute path: `/Users/pedronauck/Dev/compozy/compozy-code/providers`

Agents working from `techspec.md`, `_tasks.md`, `_task_*.md`, or the ADRs
should treat that TypeScript provider tree as the main reference corpus for
behavior, edge cases, and migration context. The Rust documents define the
target architecture and crate boundaries, and they may intentionally improve on
the upstream design.

Preserve the important behavior and integration requirements revealed by the
TypeScript implementation, but do **not** treat its API shape, module layout,
or limitations as mandatory. Prefer the Rust tech spec, ADRs, stronger typing,
cleaner crate boundaries, and better APIs whenever they produce a higher
quality result.

Use the upstream TypeScript packages to recover:

- Process models and event flows
- Tool bridge behavior and MCP integration details
- Session lifecycle, resume, and replay behavior
- Edge cases, failure handling, and regression fixtures
- Naming, export surfaces, and example coverage

## Primary Upstream Packages

- `../compozy-code/providers/core`
  Shared hooks, MCP server helpers, tool bridge, error classification, and
  token consumption.
- `../compozy-code/providers/runtime`
  Runtime orchestration, protocol types, tool registry, sessions, usage, and
  HTTP/SSE server behavior.
- `../compozy-code/providers/claude-code`
  Claude CLI wrapper provider, event normalization, tool lifecycle, session
  passthrough, and spawn-failure handling.
- `../compozy-code/providers/codex`
  Codex App Server wrapper, JSON-RPC transport, scheduler, thread routing,
  notification dispatch, and approval handling.
- `../compozy-code/providers/opencode`
  Secondary reference only. Use it when a task needs additional examples for
  hooks, streaming, session handling, or subprocess lifecycle patterns.

## Local Documents To Pair With Upstream Code

- `tasks/prd-rust-providers/techspec.md`
  Target Rust architecture, crate boundaries, invariants, and contracts.
- `tasks/prd-rust-providers/_tasks.md`
  Parent task breakdown and execution order.
- `tasks/prd-rust-providers/_task_*.md`
  Executable implementation slices for agents.
- `tasks/prd-rust-providers/adrs/*.md`
  Accepted architecture decisions for the Rust port.
- `tasks/prd-rust-providers/analysis_core.md`
  Deep analysis of `providers/core`.
- `tasks/prd-rust-providers/analysis_runtime.md`
  Deep analysis of `providers/runtime`.
- `tasks/prd-rust-providers/analysis_claude_code.md`
  Deep analysis of `providers/claude-code`.
- `tasks/prd-rust-providers/analysis_codex.md`
  Deep analysis of `providers/codex`.
- `tasks/prd-rust-providers/analysis_opencode.md`
  Secondary analysis of `providers/opencode`.
- `tasks/prd-rust-providers/analysis_pi_agent.md`
  External architectural reference used to shape the Rust SDK API and
  examples.

## Agent Workflow

1. Read the assigned `_task_<n>.md`, `techspec.md`, and the referenced ADRs.
2. Open the matching section in this file and inspect the upstream TypeScript
   folders/files listed there.
3. Use the local `analysis_*.md` documents to understand why those upstream
   files matter and where the Rust design intentionally diverges.
4. Implement the Rust crate or feature in `crates/` by preserving the required
   behavior and edge cases while still improving the API and implementation
   when the Rust design is stronger.

## Package Crosswalk

| Rust target | Primary TypeScript source | Notes |
| --- | --- | --- |
| `arky-error` | `providers/core`, `providers/runtime/src/errors/*` | Error classification and shared error shape live across core/runtime. |
| `arky-protocol` | `providers/runtime/src/protocol/*`, provider stream/event types | Protocol is assembled from runtime and provider event models. |
| `arky-config` | provider config modules plus `runtime/src/types/runtime-options.ts` | No single TS package owns all config concerns. |
| `arky-tools` | `providers/core/src/tools-bridge.ts`, `providers/runtime/src/tools/*` | Canonical naming, descriptors, and bridge behavior span core/runtime. |
| `arky-tools-macros` | No 1:1 upstream package | Derive macro behavior from how TS tool descriptors and executors are consumed. |
| `arky-hooks` | `providers/core/src/hooks.ts` | `opencode` adds useful secondary patterns. |
| `arky-session` | `providers/runtime/src/session/*` | Resume/replay behavior also touches runtime/adapters and provider sessions. |
| `arky-provider` | provider packages plus `runtime/src/services/provider-registry.ts` | Shared contract layer assembled from runtime and concrete providers. |
| `arky-mcp` | `providers/core/src/mcp-server.ts`, `mcp-http-server.ts` | Tool exposure/import behavior also appears in provider packages. |
| `arky-claude-code` | `providers/claude-code` | Direct port target. |
| `arky-codex` | `providers/codex` | Direct port target. |
| `arky-core` | `providers/runtime` plus Pi analysis | High-level orchestration is Rust-specific but grounded in runtime behavior. |
| `arky-server` | `providers/runtime/src/server/*` | HTTP/SSE behavior lives in runtime server code. |
| `arky` facade | package export surfaces from all TS packages | Model the re-export ergonomics, not the exact package topology. |
| examples/docs/CI | provider examples, tests, and package configs | Use upstream examples and fixtures as behavioral coverage guides. |

## Task Crosswalk

### Task 1.0: Workspace Scaffolding & `arky-error`

- Primary upstream packages:
  `../compozy-code/providers/core`,
  `../compozy-code/providers/runtime`
- Start with these files:
  `providers/core/src/error-classifier.ts`,
  `providers/core/src/errorContext.ts`,
  `providers/core/src/error-context.ts`,
  `providers/runtime/src/errors/index.ts`,
  `providers/runtime/src/errors/provider-errors.ts`,
  `providers/runtime/src/errors/tool-errors.ts`,
  `providers/runtime/src/errors/session-errors.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-001-package-architecture.md`,
  `adrs/adr-006-error-handling.md`,
  `adrs/adr-010-naming.md`,
  `analysis_core.md`,
  `analysis_runtime.md`

### Task 2.0: `arky-protocol`

- Primary upstream packages:
  `../compozy-code/providers/runtime`,
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/runtime/src/protocol/index.ts`,
  `providers/runtime/src/protocol/branded.ts`,
  `providers/runtime/src/protocol/provider-family.ts`,
  `providers/runtime/src/usage/types.ts`,
  `providers/claude-code/src/stream/normalized-events.ts`,
  `providers/claude-code/src/types.ts`,
  `providers/codex/src/streaming/types.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-004-event-model.md`,
  `analysis_runtime.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`

### Task 3.0: `arky-config`

- Primary upstream packages:
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`,
  `../compozy-code/providers/runtime`
- Start with these files:
  `providers/claude-code/src/config.ts`,
  `providers/codex/src/config/CodexConfig.ts`,
  `providers/codex/src/config/CodexProcessConfig.ts`,
  `providers/codex/src/config/CodexStreamingConfig.ts`,
  `providers/codex/src/config/schemas.ts`,
  `providers/codex/src/util/config-merge.ts`,
  `providers/runtime/src/types/runtime-options.ts`,
  `providers/runtime/src/capabilities/capability-validator.ts`
- Pair with local docs:
  `techspec.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`,
  `analysis_runtime.md`

### Task 4.0: `arky-tools`

- Primary upstream packages:
  `../compozy-code/providers/core`,
  `../compozy-code/providers/runtime`,
  `../compozy-code/providers/codex`,
  `../compozy-code/providers/claude-code`
- Start with these files:
  `providers/core/src/tool-provider.ts`,
  `providers/core/src/tools-bridge.ts`,
  `providers/runtime/src/tools/registry.ts`,
  `providers/runtime/src/tools/bridge.ts`,
  `providers/runtime/src/tools/codec.ts`,
  `providers/runtime/src/tools/types.ts`,
  `providers/codex/src/bridge/CodexToolsBridge.ts`,
  `providers/claude-code/src/tools/bridge.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-005-tool-system.md`,
  `analysis_core.md`,
  `analysis_runtime.md`,
  `analysis_codex.md`,
  `analysis_claude_code.md`

### Task 5.0: `arky-tools-macros`

- No direct 1:1 upstream TypeScript package exists for the proc macro.
- Derive behavior from these consumer-side TS sources:
  `providers/core/src/tools-bridge.ts`,
  `providers/runtime/src/tools/types.ts`,
  `providers/runtime/src/tools/registry.ts`,
  `providers/claude-code/src/tools/serialization.ts`,
  `providers/codex/examples/tools/example-tools.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-005-tool-system.md`,
  `analysis_core.md`,
  `analysis_runtime.md`

### Task 6.0: `arky-hooks`

- Primary upstream packages:
  `../compozy-code/providers/core`
- Secondary upstream package:
  `../compozy-code/providers/opencode`
- Start with these files:
  `providers/core/src/hooks.ts`,
  `providers/core/src/__tests__/hooks.test.ts`,
  `providers/core/src/__tests__/hook-extractors.test.ts`,
  `providers/core/src/__tests__/hooks-command-runner.test.ts`,
  `providers/opencode/src/services/hooks/hook-service.ts`,
  `providers/opencode/src/services/hooks/hook-runners.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-009-hook-system.md`,
  `analysis_core.md`,
  `analysis_opencode.md`

### Task 7.0: `arky-session`

- Primary upstream packages:
  `../compozy-code/providers/runtime`,
  `../compozy-code/providers/claude-code`
- Secondary upstream package:
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/runtime/src/session/session-store.ts`,
  `providers/runtime/src/session/in-memory-store.ts`,
  `providers/runtime/src/session/sqlite-session-store.ts`,
  `providers/runtime/src/adapters/shared/session-manager.ts`,
  `providers/claude-code/src/services/session.ts`,
  `providers/codex/examples/resume-last.ts`,
  `providers/codex/examples/resume-session-id.ts`,
  `providers/codex/examples/resume-chain.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-007-session-management.md`,
  `analysis_runtime.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`

### Task 8.0: `arky-provider`

- Primary upstream packages:
  `../compozy-code/providers/runtime`,
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/runtime/src/services/provider-registry.ts`,
  `providers/runtime/src/adapters/adapter.ts`,
  `providers/runtime/src/adapters/shared/build-stream-options.ts`,
  `providers/runtime/src/adapters/shared/error-mapping.ts`,
  `providers/claude-code/src/services/provider.ts`,
  `providers/claude-code/src/services/language-model.ts`,
  `providers/codex/src/model/CodexProvider.ts`,
  `providers/codex/src/model/CodexLanguageModel.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-002-dual-layer-api.md`,
  `adrs/adr-003-cli-wrapper-providers.md`,
  `analysis_runtime.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`

### Task 9.0: `arky-mcp`

- Primary upstream packages:
  `../compozy-code/providers/core`,
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/core/src/mcp-server.ts`,
  `providers/core/src/mcp-http-server.ts`,
  `providers/core/src/tools-bridge.ts`,
  `providers/claude-code/src/mcp/custom-server.ts`,
  `providers/claude-code/src/mcp/combined-server.ts`,
  `providers/codex/examples/tools-bridge.ts`,
  `providers/codex/src/bridge/CodexBridge.ts`,
  `providers/codex/src/bridge/CodexToolsBridge.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-008-mcp-integration.md`,
  `analysis_core.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`

### Task 10.0: `arky-claude-code`

- Primary upstream package:
  `../compozy-code/providers/claude-code`
- Start with these files:
  `providers/claude-code/src/services/provider.ts`,
  `providers/claude-code/src/services/language-model.ts`,
  `providers/claude-code/src/services/session.ts`,
  `providers/claude-code/src/services/spawn-failure-tracker.ts`,
  `providers/claude-code/src/services/tools-bridge-registry.ts`,
  `providers/claude-code/src/stream/event-normalizer.ts`,
  `providers/claude-code/src/stream/tool-lifecycle.ts`,
  `providers/claude-code/src/stream/nested-tool-tracker.ts`,
  `providers/claude-code/src/stream/text-deduplicator.ts`,
  `providers/claude-code/src/stream/stream.ts`,
  `providers/claude-code/src/tools/bridge.ts`,
  `providers/claude-code/src/mcp/combined-server.ts`,
  `providers/claude-code/src/classifier/classifier.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-003-cli-wrapper-providers.md`,
  `analysis_claude_code.md`

### Task 11.0: `arky-codex`

- Primary upstream package:
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/codex/src/model/CodexProvider.ts`,
  `providers/codex/src/model/CodexLanguageModel.ts`,
  `providers/codex/src/server/CodexAppServer.ts`,
  `providers/codex/src/server/CodexRpcTransport.ts`,
  `providers/codex/src/server/CodexScheduler.ts`,
  `providers/codex/src/server/CodexThreadManager.ts`,
  `providers/codex/src/server/CodexNotificationRouter.ts`,
  `providers/codex/src/server/CodexApprovalHandler.ts`,
  `providers/codex/src/server/CodexProcessManager.ts`,
  `providers/codex/src/streaming/CodexStreamPipeline.ts`,
  `providers/codex/src/streaming/CodexToolTracker.ts`,
  `providers/codex/src/streaming/CodexTextAccumulator.ts`,
  `providers/codex/src/bridge/CodexBridge.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-003-cli-wrapper-providers.md`,
  `analysis_codex.md`,
  `analysis_codex_rs.md`

### Task 12.0: `arky-core`

- Primary upstream packages:
  `../compozy-code/providers/runtime`
- External architectural reference:
  `.resources/pi/packages/coding-agent/`
- Start with these files:
  `providers/runtime/src/runtime.ts`,
  `providers/runtime/src/runtime-async.ts`,
  `providers/runtime/src/services/layers.ts`,
  `providers/runtime/src/services/provider-registry.ts`,
  `providers/runtime/src/tools/registry.ts`,
  `providers/runtime/src/session/session-store.ts`,
  `providers/runtime/src/adapters/shared/session-manager.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-002-dual-layer-api.md`,
  `analysis_runtime.md`,
  `analysis_pi_agent.md`

### Task 13.0: `arky-server`

- Primary upstream package:
  `../compozy-code/providers/runtime`
- Start with these files:
  `providers/runtime/src/server/app.ts`,
  `providers/runtime/src/server/index.ts`,
  `providers/runtime/src/server/routes-models.ts`,
  `providers/runtime/src/server/sse-writer.ts`,
  `providers/runtime/src/server/http-utils.ts`,
  `providers/runtime/src/server/auth.ts`,
  `providers/runtime/src/client/runtime-client.ts`
- Pair with local docs:
  `techspec.md`,
  `analysis_runtime.md`

### Task 14.0: `arky` Facade Crate & Prelude

- Primary upstream packages:
  `../compozy-code/providers/core`,
  `../compozy-code/providers/runtime`,
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/core/src/index.ts`,
  `providers/runtime/src/index.ts`,
  `providers/claude-code/src/index.ts`,
  `providers/codex/src/index.ts`
- Pair with local docs:
  `techspec.md`,
  `adrs/adr-010-naming.md`,
  `analysis_core.md`,
  `analysis_runtime.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`

### Task 15.0: Runnable Examples Suite

- Primary upstream packages:
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`,
  `../compozy-code/providers/opencode`
- External structural reference:
  `.resources/pi/packages/coding-agent/examples/sdk/`
- Start with these files:
  `providers/claude-code/examples/*`,
  `providers/codex/examples/*`,
  `providers/opencode/examples/*`
- Pair with local docs:
  `techspec.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`,
  `analysis_opencode.md`,
  `analysis_pi_agent.md`

### Task 16.0: CI/CD, Hardening & Documentation

- Primary upstream packages:
  `../compozy-code/providers/core`,
  `../compozy-code/providers/runtime`,
  `../compozy-code/providers/claude-code`,
  `../compozy-code/providers/codex`
- Start with these files:
  `providers/core/package.json`,
  `providers/core/vitest.config.ts`,
  `providers/claude-code/package.json`,
  `providers/claude-code/src/__tests__/*`,
  `providers/codex/package.json`,
  `providers/codex/src/__tests__/*`,
  `providers/runtime/package.json`,
  `providers/runtime/src/__tests__/*`
- Pair with local docs:
  `techspec.md`,
  `_tasks.md`,
  `_task_15.md`,
  `analysis_core.md`,
  `analysis_runtime.md`,
  `analysis_claude_code.md`,
  `analysis_codex.md`
