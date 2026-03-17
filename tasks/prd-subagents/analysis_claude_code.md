# Subagent Configuration: Claude Code Provider Analysis

## Source

TypeScript codebase at `~/Dev/compozy/compozy-code/providers/claude-code/`

---

## 1. Schema / Types

### Provider Schema (`src/schemas.ts`, lines 245-258)

The `agents` field in `ClaudeCodeSettingsSchema` is `Optional<Record<string, AgentStruct>>`.

**Per-agent struct fields:**

| Field | Type | Required | Notes |
|---|---|---|---|
| `description` | `NonEmptyString` | **Yes** | When to use this agent |
| `prompt` | `NonEmptyString` | **Yes** | System prompt |
| `tools` | `string[]` | No | Allowlist of tool names |
| `disallowedTools` | `string[]` | No | Denylist of tool names |
| `model` | `"sonnet" \| "opus" \| "haiku" \| "inherit"` | No | Model override; omit = inherit |
| `mcpServers` | `Array<unknown>` | No | Passthrough array (loosely typed) |
| `criticalSystemReminder_EXPERIMENTAL` | `string` | No | Extra system prompt injection |

**SDK-only fields NOT in provider schema** (from `sdk.d.ts` `AgentDefinition`):
- `skills: string[]` — preload named skills
- `maxTurns: number` — per-agent turn limit

These cannot be passed through even via `sdkOptions`.

### Top-level `agents` key

`Schema.optional(Schema.Record({ key: Schema.String, value: ... }))` — empty record accepted but `toAgents` returns `undefined` for empty.

---

## 2. Conversion / Serialization (`src/conversion/options.ts`, lines 366-397)

`toAgents` is a pure map over each agent entry:

- `description` -> verbatim
- `prompt` -> verbatim
- `model` -> conditional spread (only if defined)
- `tools` -> cloned array
- `disallowedTools` -> cloned array
- `criticalSystemReminder_EXPERIMENTAL` -> conditional spread
- `mcpServers` -> spread-cloned, cast to `AgentMcpServers` (no element validation)

Called in `mapSessionSettings` (lines 543-545):
```typescript
const agents = toAgents(settings.agents);
if (agents !== undefined) { opts.agents = agents; }
```

**No CLI arg building** — passes structured `Options` object directly to `@anthropic-ai/claude-agent-sdk`. There is no `--agents` flag serialization.

---

## 3. Runtime Behavior

- **No validation** of `mcpServers` elements — `Schema.Array(Schema.Unknown)` with cast
- **No merging** of parent MCP servers into agent MCP servers
- **`sdkOptions` override** (lines 685-689): merged on top of `opts`, can overwrite `agents`
- **Shallow settings merge** in `resolveMergedSettings` (`services/provider.ts`, lines 28-35): per-model `agents` **completely replaces** default `agents` (no deep merge)

---

## 4. Integration with Other Features

### MCP Servers
- Parent MCP servers NOT inherited into subagent configs
- Tool bridge MCP servers injected into top-level `opts.mcpServers` only, NOT per-agent
- `AgentMcpServerSpec` is `string | Record<string, McpServerConfigForProcessTransport>`

### Tools
- Per-agent allow/deny lists are independent from top-level
- No inheritance or merging logic
- `canUseTool` callback is top-level only

### Hooks
- Global only (`opts.hooks`), no per-subagent hooks
- SDK defines `SubagentStart` and `SubagentStop` hook events (fire at parent level)
- `SubagentStartHookSpecificOutput` allows injecting `additionalContext`
- Provider's `HOOK_EVENTS` includes both events

### Permissions
- `permissionMode` is top-level only
- Tool filtering per subagent is the primary restriction mechanism

### Model Selection
- Fixed literal union: `"sonnet" | "opus" | "haiku" | "inherit"`
- Custom model IDs NOT supported in agent definitions

---

## 5. Tests

**File:** `src/__tests__/conversion/options.test.ts` (lines 191-427)

Assertions at lines 278-282:
- `description`, `tools`, `disallowedTools`, `model` verified
- `mcpServers` passthrough NOT asserted
- `criticalSystemReminder_EXPERIMENTAL` NOT asserted
- `"inherit"` model value NOT tested
- No `schemas.test.ts` coverage of agents field

**Example:** `examples/11-sub-agents.ts` — 4 usage patterns (not a test)

---

## 6. Edge Cases

- `description` and `prompt` required (NonEmptyString)
- Empty `agents: {}` returns `undefined` (agents not set on opts)
- Excess properties cause `ParseError` (`onExcessProperty: "error"`)
- `model` constrained to literal union (not arbitrary model IDs)

---

## Rust Gap Assessment

| Feature | TS Status | Rust Status | Gap |
|---|---|---|---|
| Typed `AgentConfig` struct | Full schema with 7 fields | `Option<Value>` (raw JSON) | **Typed struct needed** |
| `toAgents` conversion | Maps to SDK options | Raw passthrough to `--agents` | **Needs structured mapping** |
| Model literal validation | `"sonnet"\|"opus"\|"haiku"\|"inherit"` | None | **Missing** |
| MCP servers per agent | Loosely typed passthrough | Not handled | **Missing** |
| SubagentStart/Stop hooks | Supported in HOOK_EVENTS | Not in arky-hooks | **Missing** |
| Settings merge (shallow) | Per-model replaces default | Not applicable yet | **Needs design** |
| Config schema validation | NonEmptyString, excess props | None | **Missing** |
