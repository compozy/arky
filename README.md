<p align="center">
  <h1 align="center">Arky</h1>
  <p align="center">A Rust SDK for building AI agents with first-class streaming, tool execution, and session management.</p>
</p>

<p align="center">
  <a href="https://github.com/compozy/arky/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.94%2B-orange.svg" alt="Rust Version"></a>
  <a href="https://github.com/compozy/arky"><img src="https://img.shields.io/badge/status-alpha-yellow.svg" alt="Status"></a>
</p>

---

Arky treats CLI-backed AI providers (Claude Code, Codex) as first-class streaming runtimes -- not thin text generators. It normalizes provider-specific protocols into a unified event stream, manages session persistence and replay, and gives you composable hooks, tools, and MCP integration out of the box.

## Features

- **Streaming-first architecture** -- real-time `AgentEvent` streams with text deltas, tool lifecycle, and extended thinking
- **Multiple providers** -- Claude Code CLI, Codex app server, plus Claude-compatible wrappers (Bedrock, Vertex, OpenRouter, Ollama, and more)
- **Tool system with `#[tool]` macro** -- turn any async function into an agent tool with auto-generated JSON schemas
- **MCP integration** -- client/server bridge for importing and exposing tools via the Model Context Protocol
- **Session persistence & replay** -- in-memory or SQLite-backed session stores with event-sourced replay and resume
- **Lifecycle hooks** -- intercept tool calls, session events, stop decisions, and prompt submissions
- **Error classification** -- structured error codes with retryability hints, HTTP status mapping, and correction context
- **Cooperative cancellation** -- `CancellationToken` throughout the entire stack for graceful abort
- **HTTP/SSE server** -- expose agents over HTTP with Server-Sent Events streaming

## Quick Start

Add Arky to your project:

```toml
[dependencies]
arky = { version = "0.1", features = ["claude-code"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Build and run an agent:

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

## Streaming Events

```rust
use arky::prelude::*;
use futures::StreamExt;

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

## Custom Tools

The `#[tool]` proc macro turns async functions into agent tools with zero boilerplate:

```rust
use arky::prelude::*;

#[tool]
async fn search(query: String) -> String {
    /// Search the web for information
    format!("Results for: {query}")
}

let agent = Agent::builder()
    .provider(provider)
    .tool(SearchTool)
    .build()?;
```

Tools support cancellation tokens, structured `ToolResult` returns, and JSON input schemas generated automatically via `schemars`.

## Providers

### Claude Code

Wraps the Claude CLI with streaming JSON parsing, nested tool tracking, and native session resume.

```rust
let provider = ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
    binary: "claude".to_owned(),
    ..Default::default()
});
```

Claude-compatible wrappers are also available: `BedrockProvider`, `VertexProvider`, `OpenRouterProvider`, `VercelProvider`, `OllamaProvider`, `ZaiProvider`, `MoonshotProvider`, and `MinimaxProvider`.

### Codex

Speaks newline-delimited JSON-RPC with the Codex app server, with thread-aware routing for concurrent sessions.

```rust
let provider = CodexProvider::with_config(CodexProviderConfig {
    binary: "codex".to_owned(),
    ..Default::default()
});
```

## Sessions & Replay

Arky persists transcript messages, replay events, and turn checkpoints for safe resume:

```rust
// In-memory for development
let store = InMemorySessionStore::new();

// SQLite for production (requires "sqlite" feature)
let store = SqliteSessionStore::new("sessions.db").await?;

let agent = Agent::builder()
    .provider(provider)
    .session_store(store)
    .build()?;

// Resume a previous session
agent.resume(session_id).await?;
```

## Lifecycle Hooks

Intercept and control agent behavior at every stage:

```rust
use arky::prelude::*;

struct SafetyHooks;

#[async_trait]
impl Hooks for SafetyHooks {
    async fn before_tool_call(
        &self,
        ctx: &BeforeToolCallContext,
        _cancel: CancellationToken,
    ) -> Result<Verdict, HookError> {
        if ctx.tool_call.name.contains("dangerous") {
            return Ok(Verdict::block("Tool not allowed"));
        }
        Ok(Verdict::Allow)
    }
}

let agent = Agent::builder()
    .provider(provider)
    .hooks(SafetyHooks)
    .build()?;
```

Hooks support `before_tool_call`, `after_tool_call`, `session_start`, `session_end`, `on_stop`, and `user_prompt_submit`. Compose multiple hooks with `HookChain` and configure per-hook failure modes (`FailOpen` / `FailClosed`).

## MCP Integration

Import tools from MCP servers or expose your tools as an MCP server:

```rust
// Import tools from an MCP server
let client = McpClient::connect(McpStdioClientConfig {
    command: "my-mcp-server".to_owned(),
    args: vec![],
    env: Default::default(),
}).await?;

// Expose tools as an MCP server
let server = McpServer::builder()
    .tools(tool_registry)
    .transport(McpServerTransport::Http { port: 3000 })
    .build()?;
```

Tools follow canonical naming: `mcp/<server>/<tool_name>`.

## Architecture

Arky is a Cargo workspace with narrowly-scoped crates that evolve independently:

```
Layer           Crates                              Responsibility
─────           ──────                              ──────────────
Leaf            arky-error, arky-protocol,          Shared contracts, IDs, protocol
                arky-config, arky-tools-macros       shapes, configuration, proc macros

Foundation      arky-tools, arky-hooks,             Tool registry, lifecycle hooks,
                arky-session, arky-provider          persistence, provider abstractions

Integration     arky-mcp                            MCP client/server bridge

Providers       arky-claude-code, arky-codex        CLI-backed provider implementations

Orchestration   arky-core                           Agent loop, turn runtime, replay,
                                                     event fanout

Exposure        arky-server                         HTTP and SSE runtime

Facade          arky                                Curated public API with
                                                     feature-gated re-exports
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `claude-code` | Claude Code CLI provider | Yes |
| `codex` | Codex app server provider | Yes |
| `sqlite` | SQLite session persistence | No |
| `server` | HTTP/SSE server runtime | No |
| `full` | All optional features | No |

## Development

```bash
# Format code (uses nightly for unstable rustfmt options)
make fmt

# Run lints (clippy with -D warnings)
make lint

# Run all tests
make test

# Full verification (fmt + lint + test)
make verify

# Build in release mode
make build

# Run live provider examples
cargo run -p arky --example 01_claude_basic
cargo run -p arky --example 09_live_matrix -- all
```

Requires Rust `1.94.0+`. The workspace uses nightly `cargo fmt` for unstable formatting options.

## Documentation

- [Getting Started](docs/getting-started.md) -- from `cargo add` to a running agent
- [Architecture](docs/architecture.md) -- crate layout, design goals, and runtime flows
- [Live Examples](examples/README.md) -- self-checking provider validation scenarios

## License

Apache-2.0
