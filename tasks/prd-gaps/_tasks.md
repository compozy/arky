# Arky SDK Full Parity — Implementation Task Summary

## Relevant Files

### Core Implementation Files

- `crates/arky-error/src/lib.rs` - ClassifiedError trait (enhance with ErrorClassifier)
- `crates/arky-error/src/classifier.rs` - NEW: ErrorClassifier, ErrorPattern, ErrorCategory
- `crates/arky-protocol/src/event.rs` - AgentEvent enum (add reasoning variants)
- `crates/arky-protocol/src/request.rs` - ReasoningEffort, FinishReason enums
- `crates/arky-usage/src/lib.rs` - NEW CRATE: NormalizedUsage, UsageAggregator
- `crates/arky-usage/src/aggregator.rs` - NEW: UsageAggregator
- `crates/arky-usage/src/cost.rs` - NEW: ModelCost
- `crates/arky-usage/src/extractor.rs` - NEW: ProviderMetadataExtractor trait
- `crates/arky-claude-code/src/classifier.rs` - NEW: Claude Code error patterns
- `crates/arky-claude-code/src/parser.rs` - Reasoning block parsing
- `crates/arky-claude-code/src/config.rs` - NEW: Full config schema (~60 fields)
- `crates/arky-claude-code/src/tool_bridge.rs` - NEW: MCP tool bridge wiring
- `crates/arky-claude-code/src/conversion.rs` - NEW: Message conversion, images, warnings
- `crates/arky-claude-code/src/generate.rs` - NEW: Generate override with truncation recovery
- `crates/arky-codex/src/registry.rs` - NEW: CodexServerRegistry
- `crates/arky-codex/src/app_server.rs` - NEW: CodexAppServer (long-lived)
- `crates/arky-codex/src/dispatcher.rs` - NEW: Event dispatcher (40+ events)
- `crates/arky-codex/src/config.rs` - NEW: Full config schema (~40 fields)
- `crates/arky-codex/src/model_service.rs` - NEW: Model listing via RPC
- `crates/arky-codex/src/pipeline.rs` - NEW: Stream pipeline (abort, finalization, state)
- `crates/arky-codex/src/dedup.rs` - NEW: Fingerprint-based dedup
- `crates/arky-codex/src/tool_payloads.rs` - NEW: Per-type tool payload builders
- `crates/arky-provider/src/traits.rs` - Provider trait (generate default enhanced)
- `crates/arky-provider/src/descriptor.rs` - ProviderCapabilities (expand + validate)
- `crates/arky-provider/src/registry.rs` - Model-prefix inference
- `crates/arky-provider/src/discovery.rs` - NEW: ModelDiscoveryService
- `crates/arky-provider/src/reasoning.rs` - NEW: Reasoning effort resolution
- `crates/arky-tools/src/truncation.rs` - NEW: Tool output truncation
- `crates/arky-core/src/turn.rs` - Turn loop (usage aggregation, capability validation)
- `crates/arky-core/src/agent.rs` - Agent (capability validation entry)
- `crates/arky-server/src/routes/chat.rs` - NEW: POST /v1/chat/stream
- `crates/arky-server/src/routes/models.rs` - NEW: GET /v1/models
- `crates/arky-server/src/routes/events.rs` - SSE sequence IDs + [DONE]
- `crates/arky-server/src/client.rs` - NEW: RuntimeClient
- `crates/arky-server/src/middleware.rs` - Bearer token auth
- `crates/arky-session/src/memory.rs` - TTL + capacity eviction
- `crates/arky-hooks/src/lib.rs` - Wiring into provider stream pipelines

### Integration Points

- `crates/arky-mcp/src/bridge.rs` - MCP tool bridge (provider wiring point)
- `crates/arky-usage/src/aggregator.rs` - Integration with core turn loop

### Reference Documentation

- `tasks/prd-gaps/techspec.md` - Technical specification
- `tasks/prd-gaps/adrs/adr-001-scope-full-parity.md` through `adr-010-phasing-by-priority.md`
- `tasks/prd-gaps/analysis_*.md` - Gap analysis documents

### TypeScript Reference (compozy-code)

- `providers/core/src/` - hooks, error classifier, token consumption, tools bridge
- `providers/runtime/src/` - server, session, usage, capabilities, reasoning, models, adapters
- `providers/claude-code/src/` - classifier, conversion, stream, tools, MCP, generate, services
- `providers/codex/src/` - server, streaming, config, errors, model, bridge, util

## Tasks

- [ ] 1.0 Phase 1 (P0): Production Blockers — Error Classifier, Protocol Types, Usage Crate, Provider P0s, Server P0 (complexity: critical)
- [ ] 2.0 Phase 2 (P1): Important Completeness — Provider Enhancements, Tool Truncation, Claude Code P1, Codex P1, Core Integration, Server/Session P1 (complexity: critical)
- [ ] 3.0 Phase 3 (P2): Polish — Claude Code P2, Codex P2, Cross-Cutting P2 (complexity: high)

## Task Design Rules

- Each parent task is a closed deliverable: independently shippable and reviewable
- Do not split one deliverable across multiple parent tasks; avoid cross-task coupling
- Each parent task must include unit test subtasks for this feature
- Each generated `/_task_<num>.md` must contain explicit Deliverables and Tests sections

## Execution Plan

- Critical Path: 1.0 -> 2.0 -> 3.0 (strictly sequential phases)
- Within Phase 1: ErrorClassifier -> Protocol Types -> arky-usage -> Claude Code P0 / Codex P0 (parallel) -> Server P0
- Within Phase 2: Provider Enhancements -> Claude Code P1 / Codex P1 / Truncation (parallel) -> Core Integration -> Server/Session P1
- Within Phase 3: Claude Code P2 / Codex P2 / Cross-cutting P2 (all parallel)

```
Phase 1 (P0): Production Blockers
  ErrorClassifier ────┐
                      ├─> arky-usage ─┬─> Claude Code P0 ─┐
  Protocol Types ─────┘               │                    ├─> Server P0
                                      └─> Codex P0 ───────┘

Phase 2 (P1): Important Completeness
  Provider ──┬──> Claude Code P1 ──┐
             ├──> Codex P1 ────────┤
             └──> Truncation       ├──> Core/Cross-cutting ──> Server/Session
                                   │

Phase 3 (P2): Polish
  Claude Code P2 ──┐
  Codex P2 ────────┼──> Done
  Cross-cutting P2 ┘
```

Notes:

- All runtime code MUST use `tracing` for logging (not `log` crate)
- Run `make fmt && make lint && make test` before marking any task as completed
- Use `cargo add` for new dependencies, never edit Cargo.toml by hand
- Check `CLAUDE.md` Agent Skill Dispatch Protocol before starting each task

## Batch Plan (Grouped Commits)

- [ ] Batch 1 — P0 Foundation: ErrorClassifier, Protocol Types, arky-usage crate
- [ ] Batch 2 — P0 Providers: Claude Code P0, Codex P0
- [ ] Batch 3 — P0 Server: Chat Stream, Models, Auth
- [ ] Batch 4 — P1 Foundation: Provider Enhancements, Tool Truncation
- [ ] Batch 5 — P1 Providers: Claude Code P1, Codex P1
- [ ] Batch 6 — P1 Integration: Core, Server/Session
- [ ] Batch 7 — P2 Polish: Claude Code P2, Codex P2, Cross-cutting P2
