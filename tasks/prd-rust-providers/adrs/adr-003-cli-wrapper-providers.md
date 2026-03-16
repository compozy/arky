# ADR-003: CLI Wrapper Providers (Claude Code CLI + Codex App Server)

## Status

Accepted

## Date

2026-03-15

## Porting Context

This ADR uses the TypeScript provider stack in `../compozy-code/providers` as
upstream reference material. Use `../porting-reference.md` to find the closest
packages and files, but prefer the Rust decision recorded here when it
intentionally improves on the upstream design.

## Context

We need to decide how the Arky Rust SDK communicates with LLM providers. Our existing TypeScript providers wrap CLI tools (Claude Code CLI, Codex CLI) as subprocesses rather than calling LLM APIs directly. This gives us access to all the built-in features of those CLIs (MCP support, tool execution, hooks, permissions, session management).

The codex-rs reference implementation calls the OpenAI API directly, but our TypeScript providers use a different approach: the Codex provider spawns the Codex App Server (JSON-RPC over stdio) and the Claude Code provider uses the Claude Agent SDK / CLI.

## Decision

Use **CLI wrapper providers** for the MVP:

### Claude Code Provider

- Spawn the `claude` CLI as a subprocess
- Communicate via the Claude Agent SDK protocol (stdin/stdout)
- Mirror the approach from `providers/claude-code` (TypeScript) which uses `@anthropic-ai/claude-agent-sdk`
- Handle streaming events, tool calls, session management through the CLI protocol

### Codex Provider

- Spawn the **Codex App Server** as a subprocess
- Communicate via **JSON-RPC over stdio** (as done in `providers/codex` TypeScript)
- The App Server handles model calls, tool execution, and approval workflows
- Mirror the server architecture: ProcessManager, RpcTransport, Scheduler, ThreadManager

### Provider trait must support both patterns:

```rust
pub trait Provider: Send + Sync {
    /// Stream a response from the provider
    async fn stream(&self, request: ProviderRequest)
        -> Result<ProviderEventStream, ProviderError>;

    /// Provider capabilities (tools, MCP, sessions, etc.)
    fn descriptor(&self) -> &ProviderDescriptor;
}
```

`ProviderEventStream` is a stream of `Result<AgentEvent, ProviderError>`, so
mid-stream process crashes and protocol failures are represented in-band rather
than as panics.

Both CLI wrappers implement this trait by translating between the unified
protocol and the CLI-specific protocol.

## Alternatives Considered

### Alternative 1: Direct LLM API calls (Anthropic Messages API + OpenAI Responses API)

- **Description**: Call HTTP APIs directly using reqwest, parse SSE streams
- **Pros**: No external binary dependency, more portable, lower latency (no subprocess overhead), full control over requests
- **Cons**: Must reimplement all CLI features (MCP, tools, hooks, permissions, sandboxing), massive scope increase, duplicates work that CLIs already do well
- **Why rejected**: The CLIs provide enormous value (MCP integration, tool execution, sandboxing, approval workflows) that would take months to reimplement. The subprocess overhead is negligible compared to LLM response times.

### Alternative 2: Both approaches (API direct first, CLI wrappers later)

- **Description**: Start with direct API calls, add CLI wrappers as optional providers
- **Pros**: More portable MVP, no external dependencies
- **Cons**: Direct API providers are useful but lack the rich features (MCP, tools, hooks) that make our SDK valuable. Would deliver a half-baked product initially.
- **Why rejected**: Our differentiation IS the rich integration with these CLIs. Starting without it misses the point.

## Consequences

### Positive

- Leverages all built-in CLI features (MCP, tools, hooks, permissions, sandboxing)
- Proven architecture — mirrors our TypeScript providers that work in production
- Smaller implementation scope — we normalize events, not reimplement LLM clients
- Both CLIs are actively maintained by Anthropic and OpenAI respectively

### Negative

- External dependency on `claude` and `codex` CLI binaries being installed
- Subprocess communication adds a small amount of latency and complexity
- Must handle process lifecycle (spawn, health check, restart, cleanup)
- Version compatibility concerns with CLI updates

### Risks

- CLI protocol changes breaking our wrapper (mitigate: version pinning, integration tests against specific CLI versions)
- Process spawn failures on different platforms (mitigate: clear error messages, platform-specific spawn configuration)
- CLI binary not found at runtime (mitigate: clear prerequisites documentation, `which` check at startup)

## Implementation Notes

### Claude Code Provider

- Use `tokio::process::Command` to spawn `claude` with appropriate flags
- Parse stdout as streaming events (Claude Agent SDK protocol)
- Handle MCP server configuration passthrough
- Session management via CLI flags

### Codex Provider

- Use `tokio::process::Command` to spawn the Codex App Server
- Implement JSON-RPC client over stdio (similar to `CodexRpcTransport` in TS)
- Handle request correlation (ID-based matching)
- Implement scheduler for serialized model access
- Support thread management for multi-conversation

### Shared infrastructure

- `ProcessManager` trait for subprocess lifecycle (spawn, health, restart, kill)
- `StdioTransport` for reading/writing to subprocess stdin/stdout
- Event normalization layer that maps CLI-specific events to unified `AgentEvent`

## References

- Claude Code provider analysis: `tasks/prd-rust-providers/analysis_claude_code.md`
- Codex provider analysis: `tasks/prd-rust-providers/analysis_codex.md`
- Codex App Server architecture: `providers/codex/src/server/`
- codex-rs process management: `.resources/codex/codex-rs/core/`
