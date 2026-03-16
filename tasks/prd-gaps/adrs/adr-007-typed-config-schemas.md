# ADR-007: Fully Typed Configuration Schemas with Validation

## Status

Accepted

## Date

2026-03-16

## Context

The TS providers have extensive configuration schemas validated via Effect Schema / Zod:
- Claude Code: ~60 settings (permissionMode, hooks, agents, reasoningEffort, maxTurns, allowedTools, plugins, sandbox, etc.)
- Codex: ~40 settings (approvalMode, sandboxMode, featureFlags, mcpServers, compaction, shellEnvPolicy, etc.)

The Rust providers have minimal configs:
- `ClaudeCodeProviderConfig`: 9 fields (binary, cwd, extra_args, env, version_args, verbose, max_frame_len, spawn_failure_policy)
- `CodexProviderConfig`: ~15 fields (binary, allow_npx, cwd, env, timeouts, approval_mode)

Missing settings are passed through untyped `BTreeMap<String, Value>` extras, which provides no validation, no compile-time safety, and no documentation.

## Decision

Implement **fully typed configuration structs** for both providers with all settings from the TS schemas. Each field is a named Rust struct field with appropriate types, serde attributes, and documentation.

Validation is performed at config construction time via a `validate()` method that returns `Vec<ConfigIssue>`, consistent with the existing `arky-config` validation approach.

Mandatory settings enforcement is implemented as a `enforce_mandatory_settings()` method that applies required overrides (e.g., Codex: approvalMode=never, sandboxMode=danger-full-access).

## Alternatives Considered

### Alternative 1: Core Typed + Extras Map (Hybrid)

- **Description**: Type the most-used fields (~20 per provider), keep rare fields in `BTreeMap<String, Value>`
- **Pros**: Less code, faster to implement, lower maintenance
- **Cons**: Extras have no validation, no autocomplete, error-prone, consumers can misspell keys
- **Why rejected**: Full parity decision requires all fields typed; extras map is a maintenance trap

## Consequences

### Positive

- Type safety: compile-time errors for invalid config
- Documentation: every field has doc comments and defaults
- Validation: structured error reporting on invalid configs
- IDE support: autocomplete for all config fields

### Negative

- ~600-800 lines per provider for config structs
- Maintenance burden: must track TS config changes
- Breaking changes when fields are added/renamed

### Risks

- Config drift: TS adds new fields that Rust doesn't have
- Mitigation: config structs use `#[serde(deny_unknown_fields)]` in strict mode or `#[serde(flatten)] extras: BTreeMap` as escape hatch for forward compatibility

## Implementation Notes

### Claude Code Config Fields (key additions)

```
system_prompt, append_system_prompt, max_turns, max_thinking_tokens,
reasoning_effort, permission_mode, allowed_tools, disallowed_tools,
tools, hooks, mcp_servers, agents, plugins, sandbox, session_id,
resume_session_at, continue_session, persist_session, fork_session,
max_budget_usd, fallback_model, additional_directories,
enable_file_checkpointing, include_partial_messages, betas,
max_tool_result_size, tool_output_limits, debug, debug_file
```

### Codex Config Fields (key additions)

```
approval_mode, sandbox_mode, full_auto, feature_flags, mcp_servers,
reasoning_effort, max_thinking_tokens, compaction_token_limit,
model_context_window, compact_prompt, shell_environment_policy,
exec_policy, web_search, startup_timeout_ms, idle_shutdown_ms,
max_in_flight_requests, max_queued_requests, model_cache_ttl_ms,
shared_app_server_key, sanitize_environment
```

### Validation

- `validate() -> Vec<ConfigIssue>` with `ConfigIssue { field, message, severity }`
- Severity levels: Error (blocks startup), Warning (logged but continues)
- Cross-field validation (e.g., `max_thinking_tokens` requires `reasoning_effort >= high`)

## References

- TS source: `providers/claude-code/src/schemas.ts` (~60 fields)
- TS source: `providers/codex/src/config/schemas.ts` (~40 fields)
- Gap analysis: `tasks/prd-gaps/analysis_claude_code.md` (Gap #6)
- Gap analysis: `tasks/prd-gaps/analysis_codex.md` (GAP-CDX-004)
