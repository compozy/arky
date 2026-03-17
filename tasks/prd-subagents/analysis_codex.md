# Subagent Configuration: Codex Provider Analysis

## Source

TypeScript codebase at `~/Dev/compozy/compozy-code/providers/codex/`

---

## 1. Schema / Types: No `agents` Field Exists

The Codex provider has **zero subagent configuration support** at the schema/type level.

`CodexCliSettings` (`src/config/schemas.ts`, lines 59-108) and `BaseCodexCliSettingsStruct` (lines 322-371) have no `agents` field. Same for `CodexCliProviderOptions` (lines 110-135).

The `strictObjectSchema` validation (lines 159-180) **rejects** unknown keys — passing `agents` throws: `"Unknown properties: agents"`.

---

## 2. Config Override Building: No Agent Passthrough

`buildCodexConfigOverrides` (`src/util/args.ts`, lines 277-305) calls:
- `applyGenericConfigOverrides` — raw passthrough
- `applyReasoningSettings` — reasoning config
- `applyShellEnvironmentPolicy` — shell env
- `applyFeatureFlags` — feature flags
- `applyMcpSettings` — MCP servers
- `applyCompactionSettings` — compaction

**No `applyAgentSettings` exists.**

However, mandatory feature flags (`src/util/args.ts`, lines 36-41) include:
- `child_agents_md: true`
- `multi_agent: true`

These enable Codex's internal subagent system but expose no external configuration surface.

---

## 3. Runtime Behavior: No Subagent Awareness

`toTurnStartPayload` (`src/server/CodexThreadManager.ts`, lines 32-73) includes: `threadId`, `input`, `model`, `effort`, `summary`, `outputSchema`. **No agent identifier or config.**

`toThreadOpenPayload` (lines 75-94) includes: `threadId`, `model`, `config`. The `config` is flat `Record<string, unknown>` from `buildCodexConfigOverrides`. No agent routing.

`CodexLanguageModel.doStream` (lines 560-623) uses only a `scopeId` (per-instance UUID) for notification routing. No agent selection.

---

## 4. Integration with Codex App Server

`CodexRegistry` (`src/server/CodexRegistry.ts`, lines 46-66) deduplicates by: `sharedAppServerKey`, `codexPath`, `cwd`, `env`, timeouts, concurrency. **No agent name in registry key.**

`CodexThreadManager` exposes `startThread`, `resumeThread`, `startTurn`, `compactThread`. Thread start accepts `ThreadOpenParams` with only `model` and `configOverrides`. No `agentId` or agent config.

---

## 5. Integration with Other Features

### MCP Servers
- Global only via `applyMcpSettings`. No per-agent MCP server scoping.

### Tool Filtering
- Per-MCP-server `enabledTools`/`disabledTools` exists but is global. No per-agent tool filtering.

### Model Override
- Single `model` field per thread/turn. No per-agent model.

### Approval/Permission
- **Hard-coded to `never`** via `MANDATORY_CODEX_CONFIG_OVERRIDES` (line 27: `approval_policy: "never"`).
- `enforceMandatoryCodexSettings` (lines 43-62) forces: `approvalMode: "never"`, `sandboxMode: "danger-full-access"`.
- Per-agent permission scoping is structurally impossible.

---

## 6. `collab_tool_call` — Partial Observability

The streaming layer surfaces `agentId` from Codex App Server's `collab_tool_call` item type (`src/streaming/tool-payloads.ts`, lines 122-134, 225-234).

This is **read-only observation** of what Codex internally decided. It represents the only bridge between the App Server's internal agent routing and the external consumer.

---

## 7. Tests: None

Zero tests covering subagent configuration in the Codex provider. The word `agent` appears only in:
- Stream event types (`item.agent_message.delta`)
- Reasoning events (`agent.reasoning.section.break`)
- `collab_tool_call` with `agent_id` (observational only)

---

## Rust Gap Assessment

| Feature | TS Status | Rust Status | Gap |
|---|---|---|---|
| `agents` config field | **Not present** | Not present | **No gap** (neither has it) |
| Feature flags `multi_agent`/`child_agents_md` | Hard-coded `true` | Not present | **Needs passthrough** |
| `collab_tool_call` agent_id parsing | Partial (stream parse) | Not present | **Missing observability** |
| Agent config override via `configOverrides` | Not supported | Not supported | **No gap** |
| `approvals_reviewer` config key | Not exposed | Not present | **Future consideration** |

### Key Insight

The Codex **provider wrapper** (TS) does NOT configure subagents. The Codex **App Server** (Rust binary) handles all subagent orchestration internally via its own `[agents]` config table, role files, feature flags, and `AgentControl`. The provider simply enables the feature flags and lets the App Server do its thing.

The implication for arky: **Codex subagent config is a config-file-level concern, not a provider-API-level concern.** The provider's job is to:
1. Pass through `multi_agent: true` and `child_agents_md: true` feature flags
2. Optionally allow `configOverrides` to carry agent-related keys
3. Parse `collab_tool_call` events for agent observability
