# ADR-008: MCP Client + Server + Bidirectional Tool Bridge

## Status

Accepted

## Date

2026-03-15

## Context

The Model Context Protocol (MCP) is the emerging standard for tool interoperability between AI agents. Our TypeScript providers/core already implements MCP integration at three levels: creating MCP servers from tools (`createMcpServerFromTools`), HTTP transport (`createMcpHttpServer`), and bidirectional tool bridging (`ToolsBridge`).

The codex-rs reference implementation uses the `rmcp` crate (official Rust MCP SDK, 3200+ GitHub stars, v0.16) with both stdio and streamable HTTP transports. This validates that production MCP in Rust is viable.

Our CLI wrapper providers (ADR-003) need MCP integration critically:

- The Claude Code CLI supports MCP servers natively — we need to pass tool configurations through
- The Codex App Server needs tools exposed as MCP endpoints for the CLI to consume
- Both providers need to aggregate tools from multiple MCP servers the user configures

## Decision

Implement **MCP Client + Server + Bidirectional Tool Bridge** using the `rmcp` crate as the foundation.

### MCP Client (crate `arky-mcp`)

Connects to external MCP servers, discovers their tools, and makes them available as `Box<dyn Tool>`.

```rust
/// MCP client that connects to an MCP server and exposes its tools
pub struct McpClient {
    client: rmcp::Client,
    tools: Vec<McpToolAdapter>,
}

impl McpClient {
    /// Connect via stdio (spawn subprocess)
    pub async fn stdio(command: &str, args: &[&str]) -> Result<Self, McpError>;

    /// Connect via streamable HTTP
    pub async fn http(url: &str, auth: Option<McpAuth>) -> Result<Self, McpError>;

    /// Get all tools from this MCP server as Agent-compatible tools
    pub fn tools(&self) -> Vec<Box<dyn Tool>>;

    /// Call a specific tool on the MCP server
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<ToolResult, McpError>;
}
```

### MCP Server

Exposes agent tools as an MCP server that other agents or CLIs can consume.

```rust
/// MCP server that exposes Agent tools via MCP protocol
pub struct McpServer {
    tools: Arc<ToolRegistry>,
    transport: McpTransport,
}

pub enum McpTransport {
    Stdio,
    Http { bind: SocketAddr },
}

impl McpServer {
    /// Create server from a tool registry
    pub fn from_registry(registry: Arc<ToolRegistry>) -> Self;

    /// Start serving (blocks until shutdown)
    pub async fn serve(self, transport: McpTransport) -> Result<(), McpError>;
}
```

### Bidirectional Tool Bridge

Translates between Agent tools and MCP tools automatically, handling schema conversion and result mapping.

```rust
/// Bidirectional bridge between Agent Tool system and MCP protocol
pub struct McpToolBridge {
    /// Agent tools exposed to MCP clients
    server: Option<McpServer>,
    /// MCP servers whose tools are imported into the Agent
    clients: Vec<McpClient>,
}

impl McpToolBridge {
    pub fn builder() -> McpToolBridgeBuilder;

    /// Import all tools from connected MCP servers as Agent tools.
    /// Tool names are canonicalized as `mcp/<server_name>/<tool_name>`.
    pub fn imported_tools(&self) -> Vec<Box<dyn Tool>>;

    /// Start the bridge (MCP server + keep clients alive)
    pub async fn start(&self) -> Result<(), McpError>;

    /// Shutdown all connections
    pub async fn shutdown(&self) -> Result<(), McpError>;
}
```

### Canonical tool naming

MCP tools imported into the agent use canonical names following our TS convention:

```
mcp/<server_name>/<tool_name>
```

Example: `mcp/filesystem/read_file`, `mcp/github/create_issue`

### Agent integration

```rust
let provider = ClaudeCodeProvider::builder()
    .model("claude-sonnet-4-20250514")
    .build()?;

let agent = Agent::builder()
    .provider(provider)
    .mcp_server("filesystem", McpClient::stdio("npx", &["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]).await?)
    .mcp_server("github", McpClient::http("https://mcp.github.com", Some(auth)).await?)
    .expose_tools_as_mcp(McpTransport::Stdio) // Optional: expose agent tools via MCP
    .build()?;
// Agent now has both its own tools + all MCP server tools
```

## Alternatives Considered

### Alternative 1: MCP Client only

- **Description**: Only consume MCP servers, no server or bridge capability
- **Pros**: Simpler, covers the most common use case (connecting to tool servers)
- **Cons**: Cannot expose agent tools to CLIs that expect MCP, no tool bridging for CLI wrappers. The Codex provider NEEDS to expose tools as MCP for the CLI subprocess to use them.
- **Why rejected**: Our CLI wrapper architecture (ADR-003) requires bidirectional MCP — the CLI processes need to call tools that exist in the Rust SDK, and that happens via MCP.

### Alternative 2: MCP Client + Server (no bridge)

- **Description**: Client and server as separate components, manual wiring
- **Pros**: More explicit, consumer controls exactly what's exposed
- **Cons**: Boilerplate for the common case of "import MCP tools + expose my tools". No canonical naming. No automatic schema translation.
- **Why rejected**: The bridge is what makes MCP integration seamless. Without it, every consumer must manually wire MCP client tools into the agent and manually expose agent tools as MCP — repetitive and error-prone.

## Consequences

### Positive

- Full MCP interoperability — consume and expose tools via standard protocol
- Canonical tool naming (`mcp/<server>/<tool>`) prevents collisions and enables routing
- CLI wrapper providers can expose SDK tools to their subprocess via MCP bridge
- Users can compose agents by chaining MCP servers
- Built on battle-tested `rmcp` crate, same as codex-rs
- Bidirectional bridge automates the common case

### Negative

- `rmcp` is a significant dependency (pulls in tokio, serde, etc.)
- MCP protocol complexity (transports, auth, lifecycle) adds surface area
- Bridge must handle schema mismatches between Tool trait and MCP format

### Risks

- `rmcp` crate has breaking changes (mitigate: pin version, integration tests, contribute upstream)
- MCP server process leaks if not properly cleaned up (mitigate: `Drop` impl with graceful shutdown, `CancellationToken`)
- OAuth flow complexity for HTTP MCP servers (mitigate: reuse codex-rs OAuth implementation from `rmcp-client`)

## Implementation Notes

- `arky-mcp` crate: `McpClient`, `McpServer`, `McpToolBridge`, `McpToolAdapter`
- Depends on `rmcp` v0.16+ for protocol implementation
- `McpToolAdapter` wraps an MCP tool call into the `Tool` trait interface
- `McpServerAdapter` wraps a `Box<dyn Tool>` into an MCP tool handler
- Canonical naming: `McpToolBridge` prefixes imported tools with `mcp/<server_name>/`
- Transports: stdio (for subprocess-based servers) and streamable HTTP (for remote servers)
- Auth: support bearer token and OAuth (reuse patterns from codex-rs `rmcp-client/src/oauth.rs`)
- Lifecycle: `McpClient` implements `Drop` to kill subprocess, `McpServer` uses `CancellationToken` for shutdown

## References

- TS MCP integration: `tasks/prd-rust-providers/analysis_core.md` (Section 4: MCP Integration)
- TS ToolsBridge: `tasks/prd-rust-providers/analysis_core.md` (Section 5: Tools Bridge)
- codex-rs rmcp usage: `tasks/prd-rust-providers/analysis_codex_rs.md` (Section 5: MCP Integration)
- rmcp crate: `rmcp` v0.16, official Rust MCP SDK
- Canonical tool naming: `tasks/prd-rust-providers/analysis_runtime.md` (Section 13: Tools codec)
