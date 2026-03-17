# Subagent Configuration: Codex CLI Source Analysis

## Source

Codex CLI source at `~/Dev/compozy/compozy-code/.resources/codex/`

---

## 1. CLI Flags and Config

### No `--agents` CLI flag

Agent configuration is entirely config-file-based (`config.toml`).

### Config File: `[agents]` table

**File:** `codex-rs/core/src/config/mod.rs` (lines 1596-1648)

```toml
[agents]
max_threads = 6           # default: Some(6), max concurrent agent threads
max_depth = 1             # default: 1, max nesting depth (root = depth 0)
job_max_runtime_seconds   # optional, max seconds per CSV job worker

[agents.researcher]
description = "Research-focused role."
config_file = "./agents/researcher.toml"
nickname_candidates = ["Herodotus", "Ibn Battuta"]
```

**Types:**
- `AgentsToml` (line 1598) — deserialization target for `[agents]`
- `AgentRoleConfig` (line 1625) — runtime representation of a role
- `AgentRoleToml` (line 1637) — TOML deserialization form

### Feature Flags

**File:** `codex-rs/core/src/features.rs` (lines 145-148, 715-725)

| Feature key | Rust enum | Default | Stage |
|---|---|---|---|
| `multi_agent` | `Feature::Collab` | **enabled** | Stable |
| `enable_fanout` | `Feature::SpawnCsv` | **disabled** | UnderDevelopment |

Enabling `enable_fanout` auto-enables `multi_agent` (line 429-430).

### `approvals_reviewer` config key

**File:** `codex-rs/protocol/src/config_types.rs` (lines 79-83)

```toml
approvals_reviewer = "guardian_subagent"  # or "user" (default)
```

Spawns a dedicated guardian subagent for automatic approve/deny decisions.

---

## 2. App Server RPC

Two agent-config-migration RPC methods (NOT runtime orchestration):

**File:** `codex-rs/app-server/src/message_processor.rs` (lines 630-648)

| RPC Method | Purpose |
|---|---|
| `externalAgentConfig/detect` | Detect Claude Code configs for migration |
| `externalAgentConfig/import` | Execute migration items |

Migration types: `CONFIG`, `SKILLS`, `AGENTS_MD`, `MCP_SERVER_CONFIG`

`ThreadStartParams` does NOT include agent-specific fields. The `config` field is freeform (`additionalProperties: true`).

---

## 3. Agent Orchestration (Internal)

All agents run in-process as Tokio tasks — not subprocesses.

### AgentControl (`codex-rs/core/src/agent/control.rs`, line 69)

- `spawn_agent(config, items, session_source)` — spawn new agent thread
- `spawn_agent_with_options(config, items, session_source, options)` — with fork-context
- `resume_agent_from_rollout(config, thread_id, session_source)` — resume from history
- `send_input(thread_id, items)` — message an existing agent
- `get_status(thread_id)` — poll status
- `shutdown_agent(thread_id)` — terminate
- `interrupt_agent(thread_id)` — interrupt mid-turn

### Model-Facing Tools (gated by `Feature::Collab`)

**File:** `codex-rs/core/src/tools/spec.rs` (lines 2910-2945)

| Tool | Purpose |
|---|---|
| `spawn_agent` | Spawn subagent with message and optional role/model |
| `send_input` | Send follow-up message; supports `interrupt=true` |
| `resume_agent` | Resume a previously closed agent |
| `wait_agent` | Wait for one or more agents to finish |
| `close_agent` | Close an agent |

### CSV Batch Jobs (`Feature::SpawnCsv`)

**File:** `codex-rs/core/src/tools/handlers/agent_jobs.rs`

| Tool | Purpose |
|---|---|
| `spawn_agents_on_csv` | One worker per CSV row, collects results |
| `report_agent_job_result` | Workers submit results |

Constants: default concurrency 16, max 64, default timeout 30min.

### Depth Limits and Config Inheritance

**File:** `codex-rs/core/src/tools/handlers/multi_agents.rs` (lines 244-320)

When spawning:
1. New `Config` built from parent's effective config via `build_agent_spawn_config()`
2. `apply_spawn_agent_runtime_overrides()` copies `approval_policy`, `sandbox_policy`, `cwd`, etc.
3. Role overlay via `apply_role_to_config()` if specified
4. If `child_depth >= max_depth`, `Collab` and `SpawnCsv` features disabled in child

### Guardian Subagent

**File:** `codex-rs/core/src/guardian.rs`

When `approvals_reviewer = "guardian_subagent"`:
- Spawned for each `on-request` approval
- Prefers `gpt-5.4`, 90s timeout
- Returns `GuardianAssessment` with `risk_score` (>= 80 rejected)

---

## 4. Config Schema (JSON)

**File:** `codex-rs/core/config.schema.json` (lines 34-58)

```json
"AgentsToml": {
  "additionalProperties": { "$ref": "#/definitions/AgentRoleToml" },
  "properties": {
    "job_max_runtime_seconds": { "format": "uint64", "minimum": 1 },
    "max_depth": { "format": "int32", "minimum": 1 },
    "max_threads": { "format": "uint", "minimum": 1 }
  }
}
```

---

## 5. Agent Role Files

**File:** `codex-rs/core/src/config/agent_roles.rs`

Discovery from two sources:
1. Inline `[agents.<role_name>]` in `config.toml` (with optional `config_file`)
2. Auto-discovered `.toml` files under `<config_folder>/agents/`

**Built-in roles** (`codex-rs/core/src/agent/role.rs`, lines 238-295):

| Role | Description |
|---|---|
| `default` | Default agent (no config layer) |
| `explorer` | Fast codebase Q&A |
| `worker` | Execution/production work, parallel tasks |

Role file format:
```toml
name = "my-role"
description = "..."
developer_instructions = "..."
nickname_candidates = ["Alice", "Bob"]
# ...any config.toml fields (model, sandbox_mode, etc.)
```

---

## 6. SubAgentSource Protocol Type

**File:** `codex-rs/protocol/src/protocol.rs` (lines 2268-2284)

```rust
pub enum SubAgentSource {
    Review,
    Compact,
    ThreadSpawn {
        parent_thread_id: ThreadId,
        depth: i32,
        agent_nickname: Option<String>,
        agent_role: Option<String>,
    },
    MemoryConsolidation,
    Other(String),
}
```

---

## Key Architectural Facts

1. **No `--agents` CLI flag.** Config-file-only (`[agents]` in config.toml).
2. **In-process Tokio tasks**, not subprocesses. `AgentControl` manages all lifecycle.
3. **`multi_agent` is stable and ON by default.**
4. **`enable_fanout` (CSV batch) is under development and OFF by default.**
5. **Agent roles** are first-class: inline config or auto-discovered `.toml` files.
6. **Depth enforced**: default `max_depth = 1`.
7. **Guardian subagent** is the only system-level built-in subagent.

---

## Rust Gap Assessment (for arky-codex)

The Codex provider wrapper's role for subagents is minimal:

| Concern | What the provider should do |
|---|---|
| Feature flags | Pass `multi_agent: true`, `child_agents_md: true` via config overrides |
| Agent role config | Optionally allow `configOverrides` to carry `[agents]` keys |
| `collab_tool_call` observability | Parse `agent_id`, `agent_nickname`, `agent_role` from stream events |
| `approvals_reviewer` | Optionally expose as a typed config field |
| Depth/thread limits | Expose `max_threads`, `max_depth` as typed config fields |

The heavy lifting (orchestration, role resolution, tool registration, depth enforcement) is all internal to the Codex App Server binary. The provider just needs to configure and observe.
