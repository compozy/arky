# Arky Rust AI Agent SDK — Implementation Task Summary

## Porting Context

This task set is grounded in the TypeScript provider implementation in
`../compozy-code/providers` (absolute path:
`/Users/pedronauck/Dev/compozy/compozy-code/providers`).

Before starting any task, read `tasks/prd-rust-providers/porting-reference.md`
and inspect the upstream package folders/files mapped to that task. Use the
local `analysis_*.md` documents and ADRs to understand the important upstream
behavior, and prefer the Rust design whenever this PRD intentionally improves
the API, typing, crate boundaries, or operational quality.

## Relevant Files

### Core Implementation Files

- `~/dev/compozy/arky/Cargo.toml` — Workspace root, shared deps and lint config
- `~/dev/compozy/arky/crates/arky-error/` — Shared error contracts and conventions
- `~/dev/compozy/arky/crates/arky-protocol/` — Shared types (messages, events, IDs, DTOs)
- `~/dev/compozy/arky/crates/arky-config/` — Configuration loading, merging, validation
- `~/dev/compozy/arky/crates/arky-tools/` — Tool trait, descriptors, registry, canonical naming
- `~/dev/compozy/arky/crates/arky-tools-macros/` — `#[tool]` proc macro
- `~/dev/compozy/arky/crates/arky-hooks/` — Hooks trait, hook chain, shell hooks, merge semantics
- `~/dev/compozy/arky/crates/arky-session/` — SessionStore trait, snapshots, replay log, SQLite backend
- `~/dev/compozy/arky/crates/arky-provider/` — Provider trait, provider registry, contract tests
- `~/dev/compozy/arky/crates/arky-mcp/` — MCP client, server, bidirectional bridge
- `~/dev/compozy/arky/crates/arky-claude-code/` — Claude Code CLI wrapper provider
- `~/dev/compozy/arky/crates/arky-codex/` — Codex App Server wrapper provider
- `~/dev/compozy/arky/crates/arky-core/` — Agent orchestration, command queue, turn loop
- `~/dev/compozy/arky/crates/arky-server/` — HTTP/SSE server exposing runtime state
- `~/dev/compozy/arky/crates/arky/` — Facade crate, prelude, top-level re-exports

### Documentation Files

- `tasks/prd-rust-providers/porting-reference.md` — Upstream TypeScript package
  and task-to-file mapping
- `tasks/prd-rust-providers/techspec.md` — Technical specification
- `tasks/prd-rust-providers/adrs/` — Architecture Decision Records (ADR-001 through ADR-010)
- `tasks/prd-rust-providers/analysis_*.md` — Analysis documents

## Tasks

- [x] 1.0 Workspace Scaffolding & `arky-error` Crate (complexity: medium)
- [x] 2.0 `arky-protocol` Crate — Shared Types & Event Model (complexity: high)
- [x] 3.0 `arky-config` Crate — Configuration Loading & Validation (complexity: medium)
- [x] 4.0 `arky-tools` Crate — Tool Trait, Registry & Codecs (complexity: high)
- [x] 5.0 `arky-tools-macros` Crate — `#[tool]` Proc Macro (complexity: high)
- [x] 6.0 `arky-hooks` Crate — Hook System & Merge Semantics (complexity: high)
- [x] 7.0 `arky-session` Crate — Session Store, Snapshots & Replay (complexity: high)
- [x] 8.0 `arky-provider` Crate — Provider Trait & Contract Test Suite (complexity: high)
- [x] 9.0 `arky-mcp` Crate — MCP Client, Server & Bridge (complexity: critical)
- [x] 10.0 `arky-claude-code` Crate — Claude Code CLI Provider (complexity: critical)
- [x] 11.0 `arky-codex` Crate — Codex App Server Provider (complexity: critical)
- [x] 12.0 `arky-core` Crate — Agent Orchestration & Turn Loop (complexity: critical)
- [x] 13.0 `arky-server` Crate — HTTP/SSE Runtime Exposure (complexity: high)
- [x] 14.0 `arky` Facade Crate & Prelude (complexity: medium)
- [x] 15.0 Runnable Examples Suite (complexity: high)
- [x] 16.0 CI/CD, Hardening & Documentation (complexity: medium)

Notes on complexity:

- **low**: Simple, straightforward changes (configuration, text updates, single-file modifications)
- **medium**: Standard development work (new components, API endpoints, moderate integration)
- **high**: Complex implementations (multi-step features, architectural changes, complex data flows)
- **critical**: Mission-critical or blocking work (security, core architecture, major refactors)

## Task Design Rules

- Each parent task is a closed deliverable: independently shippable and reviewable
- Do not split one deliverable across multiple parent tasks; avoid cross-task coupling
- Each parent task must include unit test subtasks for this feature
- Each generated `/_task_<num>.md` must contain explicit Deliverables and Tests sections

## Execution Plan

- Critical Path: 1.0 -> 2.0 -> 3.0 -> 4.0 -> 6.0 -> 7.0 -> 8.0 -> 12.0 -> 14.0
- Parallel Track A (after 4.0): 5.0 (`#[tool]` proc macro)
- Parallel Track B (after 8.0): 9.0 (MCP), 10.0 (Claude Code), 11.0 (Codex) — can run in parallel
- Parallel Track C (after 12.0): 13.0 (Server)
- Examples (after 14.0): 15.0 (Runnable examples suite)
- Final: 16.0 (CI/CD, hardening, docs) — after all other tasks including examples

Notes

- All Rust code MUST use `tracing` for structured logging
- Run `cargo fmt && cargo clippy -D warnings && cargo test` before marking any task as completed
- Rust Edition 2024, `[workspace.dependencies]` for version unification

## Batch Plan (Grouped Commits)

- [ ] Batch 1 — Foundations: 1.0, 2.0, 3.0
- [ ] Batch 2 — Tool System: 4.0, 5.0
- [x] Batch 3 — Durable Infrastructure: 6.0, 7.0, 8.0
- [x] Batch 4 — External Integrations: 9.0, 10.0, 11.0
- [x] Batch 5 — Orchestration & Exposure: 12.0, 13.0, 14.0
- [ ] Batch 6 — Examples: 15.0
- [x] Batch 7 — Hardening: 16.0
