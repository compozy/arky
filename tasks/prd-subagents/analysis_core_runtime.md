# Subagent Configuration: Core/Runtime Layer Analysis

## Source

TypeScript codebase at `~/Dev/compozy/compozy-code/providers/core/` and `~/Dev/compozy/compozy-code/providers/runtime/`

---

## 1. Core Abstractions (`providers/core/`)

**Core has NO subagent types.** The `@compozy/provider-core` package exports: error classifiers, token consumption, tool bridge, MCP server, and hooks. Zero mention of `agent`, `subagent`, `AgentConfig`.

The only agent-adjacent concept is the hooks system (`hooks.ts`) via `SessionStartHookInput` / `SessionEndHookInput` — no subagent-specific fields.

---

## 2. Shared Config Schema: `RuntimeAgentConfig`

**Location:** `providers/runtime/src/types/runtime-options.ts`, lines 53-64

```typescript
export class RuntimeAgentConfig extends Schema.Class<RuntimeAgentConfig>("RuntimeAgentConfig")({
  name: Schema.NonEmptyString,
  model: Schema.optional(Schema.String),
  instructions: Schema.optional(Schema.String),
  tools: Schema.optional(Schema.Array(Schema.NonEmptyString)),
  maxTurns: Schema.optional(Schema.Int.pipe(Schema.positive())),
}) {}

export type RuntimeAgentOptions = {
  readonly subagents?: readonly RuntimeAgentConfig[];
  readonly agents?: readonly RuntimeAgentConfig[];
};
```

This is the **cross-provider abstraction** for subagent config. Lives in `runtime` (not `core`).

- `subagents` — base agent registry (takes priority over defaults)
- `agents` — override patches applied on top
- Both are arrays of `RuntimeAgentConfig`

Exported from `providers/runtime/src/index.ts` at lines 188, 198.

---

## 3. Capability Flag: `agentSupport`

**Location:** `providers/runtime/src/types/capabilities.ts`

`RuntimeCapabilities` carries `agentSupport: boolean` (line 27).

| Provider | `agentSupport` |
|---|---|
| Claude Code | `true` (line 56) |
| OpenCode | `true` (line 73) |
| Codex | `false` (line 90) |

**Capability validator** (`capability-validator.ts`, lines 67-73):
```typescript
if (options.hasAgentOptions && !options.capabilities.agentSupport) {
  return yield* failUnsupported(providerId, "agentSupport", "Agent delegation is not supported");
}
```

`hasAgentOptions()` (line 115) checks both `.agents` and `.subagents` lengths.

---

## 4. Provider-Level Translation: Two Divergent Patterns

### 4a. Claude Code Adapter (`runtime/src/adapters/claude-code/index.ts`, lines 71-113)

`toClaudeAgents()` converts `RuntimeAgentConfig[]` into `ClaudeCodeSettingsInput["agents"]` map.

Merging logic (lines 152-162):
- `input.subagents` -> base layer (overrides `defaultSettings.agents`)
- `input.agents` -> top-layer override
- `extensions.agents` -> middle layer

Model values validated against `"haiku" | "inherit" | "opus" | "sonnet"` only. Other strings silently dropped.

### 4b. OpenCode Adapter (`runtime/src/adapters/opencode/adapter.ts`)

**Critical difference: subagents configured at adapter creation time, NOT per-call.**

- Lines 112-132: `buildAgentConfigFromSubagents()` converts to `Record<string, AgentConfigEntry>` with `mode: "subagent"` hardcoded
- Lines 156-191: Agent config merged into `serverConfig.agent` during `createOpenCodeAdapter()`
- Lines 243-246: At call time, `input.subagents` and `input.agents` are **explicitly ignored**

When subagents exist, OpenCode's native `task` tool is enabled (lines 316-319).

---

## 5. OpenCode's Native `AgentConfig` (richer than `RuntimeAgentConfig`)

**Location:** `providers/opencode/src/schemas/config-schema.ts`, lines 92-114

Full native fields: `model`, `variant`, `temperature`, `top_p`, `prompt`, `tools`, `disable`, `description`, `mode` (`"subagent" | "primary" | "all"`), `hidden`, `steps`, `maxSteps`, `options`, `color`, `permission`, `reasoningEffort`, `reasoningSummary`, `textVerbosity`.

`RuntimeAgentConfig` exposes only `{name, model, instructions, tools, maxTurns}`. All other fields only accessible via `providerSettings.serverConfig.agent`.

---

## 6. Subagent Event Model (OpenCode Streaming)

**Location:** `providers/opencode/src/services/streaming/streaming-service.ts`

Sophisticated subagent session tracking:
- `session.created` events with `parentID` parsed for child session detection (lines 204-257)
- Session allowlist (`Ref<Set<string>>`) tracks which session IDs pass through
- `SubagentManagerService` correlates child sessions to parent tool calls via bounded `Queue`

**SubagentManagerService** (`streaming/subagent-manager.ts`):
- `Queue.bounded<PendingSubagentSession>` (capacity 64)
- `HashMap<childSessionId, parentToolCallId>` for resolved mappings
- API: `enqueuePendingSession`, `takePendingSession`, `resolveParentToolCall`, `registerChildSession`

---

## 7. SSE Stream / Server API

`StreamRequestBody` (`runtime/src/server/app.ts`, lines 86-97) does **NOT** expose `subagents` or `agents` to HTTP clients. No subagent-specific SSE event types. Child session events merged transparently into unified stream.

---

## 8. Usage / Billing

No per-subagent token breakdown. All child session tokens flow through same `onUsageUpdate` callback. `SessionMetadata` has no `childSessionIds` field.

---

## 9. Hooks

Six event types in core: `PreToolUse`, `PostToolUse`, `SessionStart`, `SessionEnd`, `Stop`, `UserPromptSubmit`. None subagent-specific.

OpenCode fires `SessionStart`/`SessionEnd` for parent session only. Child sessions do not trigger separate hook events.

---

## 10. Tests

| Test File | Coverage |
|---|---|
| `runtime/__tests__/runtime-options.test.ts` | `RuntimeAgentConfig` schema decoding, `maxTurns` validation |
| `runtime/__tests__/capability-validator.test.ts` (lines 112-180) | `agentSupport` rejection, `hasAgentOptions()` |
| `runtime/__tests__/claude-adapter.test.ts` (lines 59-192) | `subagents`/`agents` merging with defaults |
| `runtime/__tests__/opencode-adapter.test.ts` (lines 294-371) | Creation-time subagents, `task` tool enabled |
| `opencode/streaming/__tests__/subagent-manager.test.ts` | Queue ops, session-to-toolcall correlation |

---

## Rust Gap Assessment

| Feature | TS Status | Rust Status | Gap |
|---|---|---|---|
| `RuntimeAgentConfig` shared type | In runtime package | Missing from `arky-provider` | **Full gap** |
| `RuntimeAgentOptions` (subagents+agents) | In runtime package | Missing | **Full gap** |
| `agentSupport` capability flag | Per-provider preset | Missing from capabilities | **Full gap** |
| `hasAgentOptions()` validation | In capability validator | Missing | **Full gap** |
| Claude Code `toClaudeAgents()` | In Claude adapter | Missing from `arky-claude-code` | **Full gap** |
| OpenCode creation-time baking | In OpenCode adapter | No OpenCode adapter exists | **Future** |
| `SubagentManagerService` | In OpenCode streaming | Missing | **Future** |
| Server API excludes subagents | Consistent | Consistent | **No gap** |
| No per-subagent billing | Consistent | Consistent | **No gap** |
| No subagent lifecycle hooks | Consistent | Consistent | **No gap** |
