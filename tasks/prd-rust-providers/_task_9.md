## markdown

## status: pending

<task_context>
<domain>engine/mcp</domain>
<type>implementation</type>
<scope>core_feature</scope>
<complexity>critical</complexity>
<dependencies>task1,task2,task4</dependencies>
</task_context>

# Task 9.0: `arky-mcp` Crate — MCP Client, Server & Bridge

## Overview

Implement the `arky-mcp` crate providing the MCP client, MCP server, and bidirectional tool bridge. MCP is not an optional add-on — it is required both for importing external tools into the SDK and for exposing Arky-managed tools back to CLI subprocesses. The implementation uses the `rmcp` crate (v0.16+) as the foundation and must handle canonical naming, schema translation, connection lifecycle, and auth.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md, ADR-008)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Implement `McpClient` for connecting to external MCP servers (stdio and streamable-HTTP transports)
- Implement `McpServer` for exposing SDK tools as MCP tools to external consumers
- Implement `McpToolBridge` for bidirectional tool import/export with canonical naming translation
- Canonical tool naming: imported tools get `mcp/<server_name>/<tool_name>` canonical IDs
- Schema translation: MCP tool schemas to/from `ToolDescriptor.input_schema` (JSON Schema)
- Connection lifecycle management: connect, disconnect, reconnect, keepalive pings
- Auth support: bearer token and OAuth for HTTP servers
- Implement `McpError` enum with variants: `ConnectionFailed`, `ProtocolError`, `AuthFailed`, `ServerCrashed`, `SchemaMismatch` implementing `ClassifiedError`
- Use `rmcp` 0.16.x as the underlying MCP implementation
- Dependencies: `arky-error`, `arky-protocol`, `arky-tools`
</requirements>

## Subtasks

- [ ] 9.1 Set up `rmcp` integration and verify basic client/server connectivity
- [ ] 9.2 Implement `McpClient` for stdio transport connections
- [ ] 9.3 Implement `McpClient` for streamable-HTTP transport connections
- [ ] 9.4 Implement tool import: discover remote tools, translate schemas, register with canonical names
- [ ] 9.5 Implement `McpServer` exposing local `ToolRegistry` tools as MCP tools
- [ ] 9.6 Implement `McpToolBridge` for bidirectional canonical naming and schema translation
- [ ] 9.7 Implement connection lifecycle: connect, disconnect, reconnect, keepalive
- [ ] 9.8 Implement auth support: bearer token and OAuth for HTTP transport
- [ ] 9.9 Implement `McpError` enum with `ClassifiedError` implementation
- [ ] 9.10 Write unit tests for schema translation, canonical naming, and error classification
- [ ] 9.11 Write integration tests for client-server round-trip over stdio

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/crates/arky-mcp/Cargo.toml`
- `~/dev/compozy/arky/crates/arky-mcp/src/lib.rs`
- `~/dev/compozy/arky/crates/arky-mcp/src/client.rs`
- `~/dev/compozy/arky/crates/arky-mcp/src/server.rs`
- `~/dev/compozy/arky/crates/arky-mcp/src/bridge.rs`
- `~/dev/compozy/arky/crates/arky-mcp/src/naming.rs`
- `~/dev/compozy/arky/crates/arky-mcp/src/auth.rs`
- `~/dev/compozy/arky/crates/arky-mcp/src/error.rs`

### Dependent Files

- `~/dev/compozy/arky/crates/arky-error/` — `ClassifiedError` trait
- `~/dev/compozy/arky/crates/arky-protocol/` — Shared types
- `~/dev/compozy/arky/crates/arky-tools/` — `Tool` trait, `ToolRegistry`, `ToolDescriptor`, `ToolIdCodec`
- `tasks/prd-rust-providers/techspec.md` — Section: MCP Integration
- `tasks/prd-rust-providers/adrs/adr-008-mcp-integration.md` — MCP integration design

## Deliverables

- `McpClient` with stdio and HTTP transport support
- `McpServer` exposing local tools as MCP tools
- `McpToolBridge` for bidirectional tool import/export
- Canonical naming and schema translation
- Auth support (bearer token, OAuth)
- `McpError` with `ClassifiedError` implementation
- Unit and integration tests

## Tests

### Unit Tests (Required)

- [ ] Canonical naming: `mcp/<server>/<tool>` generation and parsing
- [ ] Schema translation: MCP tool schema to `ToolDescriptor.input_schema` and back
- [ ] `McpError` classification: each variant returns correct error codes
- [ ] Connection lifecycle state transitions: connected, disconnected, reconnecting

### Integration Tests (Required)

- [ ] Client-server stdio round-trip: start MCP server in subprocess, connect client, list tools, call tool
- [ ] Tool import: connect to fixture MCP server, import tools, verify canonical names in registry
- [ ] Tool export: register local tools, expose via `McpServer`, connect external client, call tool
- [ ] Bidirectional bridge: import from one server, export to another, verify end-to-end

### Regression and Anti-Pattern Guards

- [ ] MCP connections properly cleaned up on drop
- [ ] No `unwrap()` in library code
- [ ] Schema translation is lossless for supported JSON Schema subsets
- [ ] Keepalive pings prevent stale connections

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test -p arky-mcp`

## Success Criteria

- MCP client connects to stdio and HTTP servers
- MCP server exposes local tools correctly
- Bidirectional bridge works for tool import and export
- Canonical naming follows `mcp/<server>/<tool>` format
- Schema translation is correct and tested
- All tests pass, zero clippy warnings

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_9.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
