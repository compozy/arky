# Getting Started

This guide gets a new Arky consumer from `cargo add` to a runnable agent with
provider selection, streaming, and examples you can compile locally.

## Prerequisites

- Rust `1.94.0` or newer
- A supported provider binary:
  - Claude Code CLI for `arky-claude-code`
  - Codex app server for `arky-codex`
- A Tokio-based application if you plan to drive the SDK asynchronously

## Add The Facade Crate

Add the facade crate and opt into the features you need:

```toml
[dependencies]
arky = { path = "../arky/crates/arky", features = ["claude-code", "codex"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Use the `full` feature if you want the bundled server and SQLite session
support in addition to both providers.

## Minimal Example

```rust
use arky::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ClaudeCodeProvider::new();
    let agent = Agent::builder()
        .provider(provider)
        .model("sonnet")
        .build()?;

    let response = agent.prompt("Summarize the repository layout.").await?;
    println!("{:?}", response.message);

    Ok(())
}
```

## Choose A Provider

### Claude Code

Use `ClaudeCodeProvider` when you want Claude CLI features such as nested tools
and Claude-native session resume.

```rust
let provider = ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
    binary: "claude".to_owned(),
    ..ClaudeCodeProviderConfig::default()
});
```

### Codex

Use `CodexProvider` when you want JSON-RPC app-server integration and
thread-aware routing.

```rust
let provider = CodexProvider::with_config(CodexProviderConfig {
    binary: "codex".to_owned(),
    ..CodexProviderConfig::default()
});
```

## Stream Events

`Agent::stream` exposes the same turn as a stream of structured events:

```rust
let mut stream = agent.stream("Plan a release checklist.").await?;
while let Some(event) = stream.next().await {
    match event? {
        AgentEvent::MessageUpdate { delta, .. } => println!("{delta:?}"),
        AgentEvent::ToolExecutionStart { tool_name, .. } => {
            println!("tool: {tool_name}");
        }
        _ => {}
    }
}
```

## Sessions, Replay, And Resume

- Use `Agent::new_session` to force a fresh session identity.
- Use `Agent::resume` with a persisted `SessionId` to continue prior work.
- Pick `InMemorySessionStore` for tests or short-lived processes.
- Enable the `sqlite` feature and use `SqliteSessionStore` for durable replay.

## MCP And Tools

- Register local tools with `ToolRegistry`.
- Expose or import MCP tools through `arky-mcp`.
- Keep canonical tool identity stable: `mcp/<server>/<tool>`.

## Examples

The workspace includes runnable numbered examples under [`examples/`](../examples):

- `cargo run --example 01_minimal -p arky`
- `cargo run --example 07_event_streaming -p arky`
- `cargo run --example 12_full_control -p arky --features full`

## Verification Commands

Use the same commands locally that CI enforces:

```sh
cargo +nightly fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --workspace
cargo build --examples
cargo bench --no-run
./scripts/check-deps.sh
```

## Next Steps

- Read [`docs/architecture.md`](./architecture.md) for the crate layout and
  runtime boundaries.
- Start from `arky::prelude::*` unless you explicitly need a lower-level crate.
- Use the fixture-backed provider tests when changing protocol parsing or
  stream normalization logic.
