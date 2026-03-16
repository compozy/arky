//! # 02 Claude Tools
//!
//! Live, self-checking provider-native tool validation for Claude Code.

mod common;

use std::fs;

use arky::{
    AgentEvent,
    Provider,
};
use common::{
    ExampleError,
    claude_model,
    claude_provider,
    collect_provider_stream,
    final_turn_text,
    pass,
    print_section,
    request,
    require_any_tool_execution,
    require_contains,
    require_event,
    temporary_workspace,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("claude-tools")?;
    let model = claude_model();
    let token = "CLAUDE_TOOL_OK_778899";
    let token_file = workspace.path().join("TOKEN.txt");
    fs::write(&token_file, format!("verification_token={token}\n"))?;
    let provider = claude_provider(workspace.path());

    print_section("Claude native tool execution");
    println!("model: {model}");
    println!("workspace: {}", workspace.path().display());
    println!("token file: {}", token_file.display());

    let prompt = format!(
        "Use Claude Code tools to inspect the file `{}`. \
         Read the verification token from that file and reply with the exact token only. \
         Do not guess and do not answer before using a tool.",
        token_file.display()
    );
    let events =
        collect_provider_stream(provider.stream(request(&model, prompt, 1)).await?)
            .await?;

    require_any_tool_execution(
        &events,
        "Claude tools example should observe a provider-native tool execution",
    )?;
    require_event(&events, "Claude should emit tool start", |event| {
        matches!(event, AgentEvent::ToolExecutionStart { .. })
    })?;

    let final_text = final_turn_text(&events)?;
    require_contains(&final_text, token, "Claude tool-assisted response")?;
    pass("Claude executed provider-native tools and returned the file token");

    Ok(())
}
