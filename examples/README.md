# Arky Runnable Examples

The examples in this directory follow the same numbered, progressive shape as
the Pi Agent SDK tutorials, but they adapt the flow to Arky's current Rust
surface.

Each file is a standalone example target registered in `crates/arky/Cargo.toml`
and can be run with:

```bash
cargo run -p arky --example 01_minimal
```

Examples that need optional crate features have feature-specific notes in the
table below.

| Example | What it demonstrates | Notes |
| --- | --- | --- |
| `01_minimal` | Minimal high-level agent usage with the default Claude provider config | Requires the `claude` binary |
| `02_custom_provider` | Switching between Claude Code and Codex, plus explicit model selection | Requires `claude` or `codex` |
| `03_system_prompt` | Injecting a custom system prompt through `AgentBuilder` | Fully local demo |
| `04_custom_tools` | `#[tool]` macro expansion plus a manual `Tool` implementation | Fully local demo |
| `05_tool_registry` | Long-lived tools, call-scoped tools, collision handling, and tool-name codecs | Fully local demo |
| `06_hooks` | Hook lifecycle composition, shell hooks, prompt rewriting, and stop decisions | Fully local demo |
| `07_event_streaming` | Consuming live agent events via `subscribe()` while a prompt runs | Fully local demo |
| `08_mcp_integration` | Local MCP server/client round-trips plus bridge-based import/export | Fully local demo |
| `09_sessions` | In-memory sessions, replay, resume, and optional SQLite persistence | Add `--features sqlite` for the SQLite section |
| `10_steering_followup` | Mid-turn steering and post-turn follow-up scheduling | Fully local demo |
| `11_server_exposure` | HTTP/SSE server routes for health, sessions, and live events | Run with `--features server` |
| `12_full_control` | Explicit config, provider registry, hooks, session store, tools, and event buffer sizing | Fully local demo |

Recommended validation commands for the suite:

```bash
cargo build --examples
cargo build --examples --features full
```
