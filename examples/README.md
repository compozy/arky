# Arky Live Provider Examples

The examples in this directory are no longer generic SDK demos. They are live,
self-checking provider scenarios meant to validate that the real `Claude Code`
and `Codex` integrations are actually functioning end-to-end.

Every example:

- runs against a real provider binary;
- asserts concrete invariants;
- prints `PASS: ...` for each validated behavior; and
- exits non-zero when a binary, auth flow, stream shape, tool path, or final
  output violates the expected contract.

Compile the suite with:

```bash
cargo build -p arky --examples
```

Run individual scenarios with:

```bash
cargo run -p arky --example 01_claude_basic
```

Run the grouped matrix with:

```bash
cargo run -p arky --example 09_live_matrix -- all
make test-live
make test-live PROVIDER=claude
make test-live PROVIDER=codex
```

## Prerequisites

- An authenticated `claude` CLI session for Claude examples
- A working `codex` CLI/app-server setup for Codex examples
- Rust `1.94.0+`

The examples do not silently skip missing setup. If auth or binaries are not
available, the example fails and prints the provider error.

## Scenario Matrix

| Example | What it validates | Notes |
| --- | --- | --- |
| `01_claude_basic` | Claude `stream` and `generate` with exact-token assertions | Requires Claude auth |
| `02_claude_tools` | Claude provider-native tool execution against a real workspace file | Verifies tool lifecycle events and final token extraction |
| `03_claude_resume` | Claude session persistence and resume behavior | Uses `InMemorySessionStore` |
| `04_codex_basic` | Codex `stream` and `generate` with exact-token assertions | Requires Codex app-server access |
| `05_codex_tools` | Codex provider-native tool execution against a real workspace file | Verifies tool lifecycle events and final token extraction |
| `06_codex_resume` | Codex session persistence and resume behavior | Uses `InMemorySessionStore` |
| `07_codex_mcp` | Codex MCP passthrough against an in-process HTTP MCP server | Validates remote tool reachability |
| `08_codex_control_flow` | Codex `follow_up()` behavior in a real multi-turn session | `steer()` remains covered by deterministic non-live tests until the real-provider path has a stable trigger |
| `09_live_matrix` | Runs the applicable scenario set for `claude`, `codex`, or `all` | Good entrypoint for manual smoke runs |

## Model Overrides

You can override the default models used by the suite:

```bash
ARKY_CLAUDE_MODEL=sonnet cargo run -p arky --example 01_claude_basic
ARKY_CODEX_MODEL=gpt-5 cargo run -p arky --example 04_codex_basic
```

## Notes

- `examples/common.rs` contains the shared live harness and should remain
  assertion-oriented.
- Broader SDK behavior that is not provider-specific now belongs in tests and
  docs, not in `examples/`.
