//! # 11 Claude Runtime Config
//!
//! Live, self-checking validation for Claude environment passthrough and debug
//! logging.

mod common;

use std::collections::BTreeMap;

use arky::{
    ClaudeCodeProviderConfig,
    Provider,
    claude_code::{
        ClaudeCliBehaviorConfig,
        ClaudePermissionConfig,
    },
};
use common::{
    ExampleError,
    claude_model,
    claude_provider_with_config,
    collect_provider_stream,
    final_turn_text,
    pass,
    print_section,
    request,
    require_any_tool_execution,
    require_contains,
    require_file_nonempty,
    temporary_workspace,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("claude-runtime-config")?;
    let model = claude_model();
    let token = "CLAUDE_ENV_TOKEN_626262";
    let debug_file = workspace.path().join("claude.debug.log");

    print_section("Claude runtime config");
    println!("model: {model}");
    println!("workspace: {}", workspace.path().display());
    println!("debug file: {}", debug_file.display());

    let provider = claude_provider_with_config(
        workspace.path(),
        ClaudeCodeProviderConfig {
            env: BTreeMap::from([("ARKY_EXAMPLE_TOKEN".to_owned(), token.to_owned())]),
            permission: ClaudePermissionConfig {
                mode: Some("dontAsk".to_owned()),
                ..ClaudePermissionConfig::default()
            },
            allowed_tools: vec!["Bash".to_owned()],
            cli_behavior: ClaudeCliBehaviorConfig {
                debug: true,
                debug_file: Some(debug_file.clone()),
                ..ClaudeCliBehaviorConfig::default()
            },
            ..ClaudeCodeProviderConfig::default()
        },
    );
    let prompt = "Use Claude Code tools to print the environment variable \
         ARKY_EXAMPLE_TOKEN from the shell. After reading it, reply with the \
         exact token only. Do not guess.";
    let events =
        collect_provider_stream(provider.stream(request(&model, prompt, 1)).await?)
            .await?;
    let final_text = final_turn_text(&events)?;

    require_any_tool_execution(
        &events,
        "Claude runtime-config example should observe a tool execution",
    )?;
    require_contains(&final_text, token, "Claude runtime-config response")?;
    require_file_nonempty(&debug_file, "Claude debug log file should be written")?;
    pass("Claude inherited provider env and wrote a debug log file");

    Ok(())
}
