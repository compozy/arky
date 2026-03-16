# ADR-004: Subagent Configuration as CLI Passthrough

## Status

Accepted

## Date

2026-03-16

## Context

The TS compozy-code providers support named subagent configurations with per-agent model, tools, prompt, disallowedTools, and mcpServers. These are passed to the Claude SDK / Codex CLI which handles the actual subagent orchestration.

The Rust `Agent` in `arky-core` already has first-class steering, follow-up, and abort capabilities that the TS side lacks. The question is whether the Rust SDK should also orchestrate subagents programmatically (spawn child `Agent` instances) or simply pass subagent configuration to the underlying CLI.

## Decision

Implement subagent support as **CLI config passthrough only**. The provider config structs (`ClaudeCodeProviderConfig`, `CodexProviderConfig`) gain an `agents` field that maps agent names to their configurations. These are serialized to the appropriate CLI flags (Claude Code `--agents`, Codex config overrides).

The Rust SDK does NOT orchestrate subagents — it delegates to the CLI process, which already knows how to manage them.

## Alternatives Considered

### Alternative 1: SDK-Level Subagent Orchestration

- **Description**: `Agent` in `arky-core` can spawn child `Agent` instances with independent configs, each with its own turn loop. Parent collects results and decides next steps.
- **Pros**: Full programmatic control, composable agents, leverage existing actor model
- **Cons**: High complexity (~1000+ lines), requires parent-child communication protocol, error propagation across agents, resource management
- **Why rejected**: Significant investment with unclear immediate demand; CLI already handles subagents well

### Alternative 2: Hybrid (Passthrough Now, Orchestration Later)

- **Description**: Phase 1 passthrough, Phase 2 orchestration
- **Pros**: Incremental value delivery
- **Cons**: Two-phase planning overhead
- **Why rejected**: No concrete demand for SDK-level orchestration yet; passthrough is sufficient

## Consequences

### Positive

- Simple implementation: just add config fields and CLI argument mapping
- Immediate subagent support via Claude Code and Codex CLIs
- No architectural changes to `arky-core` agent loop

### Negative

- No programmatic subagent control from Rust code
- Consumers cannot compose agents in ways the CLI doesn't support
- Subagent events may be opaque (nested inside parent stream)

### Risks

- Consumer demand for programmatic orchestration may emerge
- Mitigation: the `Agent` actor model is extensible; orchestration can be added later without breaking changes

## Implementation Notes

- `ClaudeCodeProviderConfig` gains `agents: Option<Vec<AgentConfig>>` where `AgentConfig` has `name`, `description`, `model`, `prompt`, `tools`, `disallowed_tools`, `mcp_servers`
- `CodexProviderConfig` gains equivalent config override field
- Config is serialized to CLI arguments in `build_process_config()` / `build_config_overrides()`

## References

- TS source: `providers/claude-code/src/schemas.ts` (agents field)
- TS source: `providers/claude-code/src/conversion/options.ts` (toAgents)
- Gap analysis: `tasks/prd-gaps/analysis_claude_code.md` (Gap #8)
- Gap analysis: `tasks/prd-gaps/analysis_core_runtime.md` (Gap #12)
