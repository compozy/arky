## markdown

## status: pending

<task_context>
<domain>engine/examples</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>high</complexity>
<dependencies>task1,task2,task3,task4,task5,task6,task7,task8,task9,task10,task11,task12,task13,task14</dependencies>
</task_context>

# Task 15.0: Runnable Examples Suite

## Overview

Create a comprehensive, progressive examples suite for the Arky SDK following the numbered-tutorial pattern used by the Pi Agent SDK (`.resources/pi/packages/coding-agent/examples/sdk/`). Examples are organized in `~/dev/compozy/arky/examples/` as numbered Rust files that progressively demonstrate SDK capabilities — from minimal zero-config usage up to full manual control. Each example is a standalone `[[example]]` binary runnable via `cargo run --example <name>`.

The Pi SDK examples (01-minimal through 12-full-control) serve as the structural reference. The Arky examples must cover the equivalent surface area in Rust: agent creation, model/provider selection, custom tools, hooks/extensions, MCP integration, session management, streaming events, and full-control construction.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md)
- **ALWAYS READ** the Pi SDK examples at `.resources/pi/packages/coding-agent/examples/sdk/` for structural reference
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Create numbered examples following progressive complexity (01 through 12)
- Each example is a standalone `fn main()` binary in `~/dev/compozy/arky/examples/`
- All examples must compile and run (at minimum: compile without errors; runtime requires provider binaries)
- Examples must use the `arky` facade crate (`use arky::prelude::*`)
- Each example must have a doc comment header explaining what it demonstrates
- Examples must cover: minimal usage, provider selection, custom system prompts, custom tools (`#[tool]` macro), hooks/extensions, MCP integration, session management (in-memory and persistent), event streaming/subscription, server exposure, and full-control construction
- Include a `README.md` in the examples directory explaining the progression
- Register all examples as `[[example]]` in the `arky` facade crate's `Cargo.toml`
- Reference patterns from Pi SDK examples (`01-minimal.ts` through `12-full-control.ts`) adapted to Rust idioms
</requirements>

## Subtasks

- [ ] 15.1 Create `~/dev/compozy/arky/examples/` directory and `README.md` with progression overview
- [ ] 15.2 Create `01_minimal.rs` — Simplest agent usage with all defaults (equivalent to Pi's 01-minimal)
- [ ] 15.3 Create `02_custom_provider.rs` — Provider selection and model configuration (equivalent to Pi's 02-custom-model)
- [ ] 15.4 Create `03_system_prompt.rs` — Custom system prompt injection (equivalent to Pi's 03-custom-prompt)
- [ ] 15.5 Create `04_custom_tools.rs` — Custom tool creation with `#[tool]` macro and manual `Tool` impl (equivalent to Pi's 05-tools)
- [ ] 15.6 Create `05_tool_registry.rs` — Tool registry management: built-in tools, call-scoped tools, collision handling
- [ ] 15.7 Create `06_hooks.rs` — Hook lifecycle: before/after tool call, session start/end, stop decision (equivalent to Pi's 06-extensions)
- [ ] 15.8 Create `07_event_streaming.rs` — Event subscription and streaming consumption with `subscribe()` (event handling pattern from all Pi examples)
- [ ] 15.9 Create `08_mcp_integration.rs` — MCP client connection, tool import, and MCP server exposure
- [ ] 15.10 Create `09_sessions.rs` — Session management: in-memory, SQLite persistence, resume, replay (equivalent to Pi's 11-sessions)
- [ ] 15.11 Create `10_steering_followup.rs` — Mid-conversation steering and follow-up patterns
- [ ] 15.12 Create `11_server_exposure.rs` — HTTP/SSE server exposing agent runtime (health, sessions, events)
- [ ] 15.13 Create `12_full_control.rs` — Complete manual construction: no defaults, explicit provider, tools, hooks, session store, config (equivalent to Pi's 12-full-control)
- [ ] 15.14 Register all examples as `[[example]]` entries in `~/dev/compozy/arky/crates/arky/Cargo.toml`
- [ ] 15.15 Verify all examples compile: `cargo build --examples`

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/examples/README.md`
- `~/dev/compozy/arky/examples/01_minimal.rs`
- `~/dev/compozy/arky/examples/02_custom_provider.rs`
- `~/dev/compozy/arky/examples/03_system_prompt.rs`
- `~/dev/compozy/arky/examples/04_custom_tools.rs`
- `~/dev/compozy/arky/examples/05_tool_registry.rs`
- `~/dev/compozy/arky/examples/06_hooks.rs`
- `~/dev/compozy/arky/examples/07_event_streaming.rs`
- `~/dev/compozy/arky/examples/08_mcp_integration.rs`
- `~/dev/compozy/arky/examples/09_sessions.rs`
- `~/dev/compozy/arky/examples/10_steering_followup.rs`
- `~/dev/compozy/arky/examples/11_server_exposure.rs`
- `~/dev/compozy/arky/examples/12_full_control.rs`
- `~/dev/compozy/arky/crates/arky/Cargo.toml` — `[[example]]` entries

### Dependent Files

- `~/dev/compozy/arky/crates/arky/src/lib.rs` — Facade crate prelude (all examples import from here)
- `~/dev/compozy/arky/crates/arky-core/` — `Agent`, `AgentBuilder`
- `~/dev/compozy/arky/crates/arky-provider/` — `Provider` trait
- `~/dev/compozy/arky/crates/arky-claude-code/` — Claude Code provider
- `~/dev/compozy/arky/crates/arky-codex/` — Codex provider
- `~/dev/compozy/arky/crates/arky-tools/` — `Tool` trait, `ToolRegistry`
- `~/dev/compozy/arky/crates/arky-tools-macros/` — `#[tool]` proc macro
- `~/dev/compozy/arky/crates/arky-hooks/` — `Hooks` trait
- `~/dev/compozy/arky/crates/arky-mcp/` — MCP client/server
- `~/dev/compozy/arky/crates/arky-session/` — `SessionStore`
- `~/dev/compozy/arky/crates/arky-server/` — HTTP/SSE server
- `.resources/pi/packages/coding-agent/examples/sdk/` — Structural reference (Pi SDK examples)
- `tasks/prd-rust-providers/techspec.md` — All API surfaces

### Reference: Pi SDK Examples Mapping

| Pi Example | Arky Equivalent | Key Pattern |
|------------|-----------------|-------------|
| `01-minimal.ts` | `01_minimal.rs` | Zero-config agent creation, `prompt()` call, basic output |
| `02-custom-model.ts` | `02_custom_provider.rs` | Provider/model selection, `AgentBuilder` config |
| `03-custom-prompt.ts` | `03_system_prompt.rs` | System prompt customization |
| `05-tools.ts` | `04_custom_tools.rs` | `#[tool]` macro, manual `Tool` impl, tool sets |
| — | `05_tool_registry.rs` | Registry management, call-scoped tools, codecs |
| `06-extensions.ts` | `06_hooks.rs` | Hook lifecycle, shell hooks, merge semantics |
| (all examples use subscribe) | `07_event_streaming.rs` | `subscribe()`, event consumption patterns |
| — | `08_mcp_integration.rs` | MCP client/server, tool bridge |
| `11-sessions.ts` | `09_sessions.rs` | In-memory, persistent, resume, replay |
| — | `10_steering_followup.rs` | `steer()`, `follow_up()`, mid-conversation control |
| — | `11_server_exposure.rs` | HTTP/SSE server, health, session routes |
| `12-full-control.ts` | `12_full_control.rs` | Complete manual construction, no defaults |

## Deliverables

- 12 numbered example files demonstrating progressive SDK usage
- `README.md` explaining the example progression and how to run them
- All examples registered as `[[example]]` in Cargo.toml
- All examples compile with `cargo build --examples`

## Tests

### Unit Tests (Required)

- [ ] Each example file compiles without errors (`cargo build --examples`)
- [ ] README.md exists and lists all 12 examples with descriptions

### Integration Tests (Required)

- [ ] `cargo build --examples` succeeds with zero errors and zero warnings
- [ ] Each example's `main()` function is well-formed (has correct async runtime setup, error handling)

### Regression and Anti-Pattern Guards

- [ ] Examples use `arky::prelude::*` — not direct crate imports
- [ ] Examples handle errors with `Result` and `?` — no `unwrap()` in example code
- [ ] Examples use `#[tokio::main]` for async entry points
- [ ] No example depends on external state (files, databases) without setup code

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo build --examples`

## Success Criteria

- All 12 examples compile and demonstrate progressive complexity
- README clearly explains the progression from minimal to full-control
- Examples cover all major SDK features: agent, provider, tools, hooks, MCP, sessions, streaming, server
- Examples follow Rust idioms (async/await, Result, proper error handling)
- `cargo build --examples` passes with zero warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_15.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
