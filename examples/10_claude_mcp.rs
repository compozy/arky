//! # 10 Claude MCP
//!
//! Live, self-checking MCP passthrough validation for Claude using an
//! in-process HTTP MCP server.

mod common;

use std::sync::Arc;

use arky::{
    ClaudeCodeProviderConfig,
    McpServer,
    Provider,
    ToolRegistry,
    claude_code::ClaudePermissionConfig,
};
use common::{
    ExampleError,
    RevealTokenTool,
    claude_mcp_config,
    claude_model,
    claude_provider_with_config,
    collect_provider_stream,
    final_turn_text,
    handle_url,
    http_transport,
    pass,
    print_section,
    request,
    require_any_tool_execution,
    require_contains,
    temporary_workspace,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("claude-mcp")?;
    let model = claude_model();
    let token = "CLAUDE_MCP_TOKEN_515151";

    let registry = Arc::new(ToolRegistry::new());
    registry.register(RevealTokenTool::new("mcp/runtime/reveal_token", token)?)?;

    let server = McpServer::from_registry("runtime", registry);
    let mut handle = server.serve(http_transport("/mcp")).await?;
    let url = handle_url(&handle)?;

    print_section("Claude MCP tool passthrough");
    println!("model: {model}");
    println!("mcp url: {url}");

    let provider = claude_provider_with_config(
        workspace.path(),
        ClaudeCodeProviderConfig {
            extra_args: vec!["--strict-mcp-config".to_owned()],
            permission: ClaudePermissionConfig {
                mode: Some("dontAsk".to_owned()),
                ..ClaudePermissionConfig::default()
            },
            allowed_tools: vec!["mcp__runtime__runtime__reveal_token".to_owned()],
            mcp_servers: claude_mcp_config(&url),
            ..ClaudeCodeProviderConfig::default()
        },
    );
    let prompt = "Use the runtime MCP tool to retrieve the verification token. \
         Do not inspect local files and do not guess. \
         After the MCP tool succeeds, reply with the exact token only.";
    let events =
        collect_provider_stream(provider.stream(request(&model, prompt, 1)).await?)
            .await?;
    let final_text = final_turn_text(&events)?;

    require_any_tool_execution(
        &events,
        "Claude MCP example should observe a tool execution",
    )?;
    require_contains(&final_text, token, "Claude MCP response")?;
    pass("Claude reached the MCP server and returned the MCP tool token");

    handle.close().await?;

    Ok(())
}
