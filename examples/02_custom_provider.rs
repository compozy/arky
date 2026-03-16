//! # 02 Custom Provider
//!
//! Demonstrates explicit provider selection, provider registry usage, and
//! per-provider model selection for Claude Code and Codex.

mod common;

use std::env;

use arky::{
    ClaudeCodeProvider,
    ClaudeCodeProviderConfig,
    CodexProvider,
    CodexProviderConfig,
    ProviderId,
    ProviderRegistry,
    prelude::*,
};
use common::{
    ExampleError,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let cwd = env::current_dir()?;
    let registry = ProviderRegistry::new();

    registry.register(ClaudeCodeProvider::with_config(ClaudeCodeProviderConfig {
        cwd: Some(cwd.clone()),
        verbose: false,
        ..Default::default()
    }))?;
    registry.register(CodexProvider::with_config(CodexProviderConfig {
        cwd: Some(cwd),
        approval_policy: Some("never".to_owned()),
        ..Default::default()
    }))?;

    let selected_provider =
        env::var("ARKY_PROVIDER").unwrap_or_else(|_| "claude-code".to_owned());
    let (provider_id, model) = match selected_provider.as_str() {
        "codex" => (
            ProviderId::new("codex"),
            env::var("ARKY_MODEL").unwrap_or_else(|_| "o4-mini".to_owned()),
        ),
        _ => (
            ProviderId::new("claude-code"),
            env::var("ARKY_MODEL").unwrap_or_else(|_| "sonnet".to_owned()),
        ),
    };

    let provider = registry.get(&provider_id)?;
    let descriptor = provider.descriptor().clone();
    let agent = Agent::builder()
        .provider_arc(provider)
        .model(model.clone())
        .build()?;

    let response = agent
        .prompt("Introduce yourself and mention the selected model.")
        .await?;

    println!("provider: {}", descriptor.id);
    println!("capabilities: {:?}", descriptor.capabilities);
    println!("model: {model}");
    println!("assistant: {}", text_from_message(&response.message));

    Ok(())
}
