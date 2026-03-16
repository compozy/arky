//! # 07 Codex MCP
//!
//! Live, self-checking MCP passthrough validation for Codex using an
//! in-process HTTP MCP server.

mod common;

use std::sync::Arc;

use arky::{
    McpServer,
    Provider,
    ToolRegistry,
};
use common::{
    ExampleError,
    RevealTokenTool,
    codex_mcp_settings,
    codex_model,
    codex_provider,
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
    let workspace = temporary_workspace("codex-mcp")?;
    let model = codex_model();
    let token = "CODEX_MCP_TOKEN_424242";

    let registry = Arc::new(ToolRegistry::new());
    registry.register(RevealTokenTool::new("mcp/runtime/reveal_token", token)?)?;

    let server = McpServer::from_registry("runtime", registry);
    let mut handle = server.serve(http_transport("/mcp")).await?;
    let url = handle_url(&handle)?;

    print_section("Codex MCP tool passthrough");
    println!("model: {model}");
    println!("mcp url: {url}");

    let provider = codex_provider(workspace.path());
    let request = request(
        &model,
        "Use the runtime MCP tool to retrieve the verification token. \
         After the tool succeeds, reply with the exact token only. Do not guess.",
        1,
    )
    .with_settings(codex_mcp_settings(&url));

    let events = collect_provider_stream(provider.stream(request).await?).await?;
    let final_text = final_turn_text(&events)?;

    require_any_tool_execution(
        &events,
        "Codex MCP example should observe a tool execution",
    )?;
    require_contains(&final_text, token, "Codex MCP response")?;
    pass("Codex reached the MCP server and returned the MCP tool token");

    handle.close().await?;

    Ok(())
}
