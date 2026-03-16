//! # 12 Full Control
//!
//! Demonstrates explicit configuration of the agent config, provider registry,
//! hooks, session store, tool registration, and event buffer sizing without
//! relying on discovery or hidden defaults.

mod common;

use std::{
    io,
    sync::Arc,
};

use arky::{
    AgentConfigBuilder,
    ArkyConfig,
    FailureMode,
    HookChain,
    InMemorySessionStore,
    InMemorySessionStoreConfig,
    PromptUpdate,
    ProviderConfigBuilder,
    ProviderId,
    ProviderRegistry,
    SessionStartUpdate,
    WorkspaceConfigBuilder,
    hooks::{
        PromptSubmitContext,
        SessionStartContext,
    },
    prelude::*,
};
use async_trait::async_trait;

use crate::common::{
    EchoProvider,
    ExampleError,
    text_from_message,
};

#[tool]
/// Attach a human-readable annotation to a conversation.
async fn annotate(note: String) -> Result<String, ToolError> {
    tokio::task::yield_now().await;
    Ok(format!("annotation: {note}"))
}

struct ExplicitHook;

#[async_trait]
impl Hooks for ExplicitHook {
    async fn session_start(
        &self,
        _ctx: &SessionStartContext,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Option<SessionStartUpdate>, arky::HookError> {
        Ok(Some(SessionStartUpdate::new().with_messages(vec![
            Message::system("session started via explicit hook"),
        ])))
    }

    async fn user_prompt_submit(
        &self,
        ctx: &PromptSubmitContext,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<Option<PromptUpdate>, arky::HookError> {
        Ok(Some(
            PromptUpdate::new().rewrite(format!("explicit prompt: {}", ctx.prompt)),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let config = ArkyConfig::builder()
        .workspace(
            WorkspaceConfigBuilder::new()
                .name("arky-examples")
                .default_provider("full-control")
                .env("RUNTIME_MODE", "manual"),
        )
        .provider(
            "full-control",
            ProviderConfigBuilder::new()
                .kind("custom-provider")
                .binary("example-provider")
                .model("demo-model")
                .args(["--demo"]),
        )
        .agent(
            "manual",
            AgentConfigBuilder::new()
                .provider("full-control")
                .model("demo-model")
                .instructions("Mention that the run was assembled explicitly.")
                .max_turns(4)
                .tools(["mcp/local/annotate"])
                .env("AGENT_MODE", "explicit"),
        )
        .build()?;

    let manual_agent_config = config
        .agent("manual")
        .cloned()
        .ok_or_else(|| io::Error::other("missing manual agent config"))?;
    let provider_registry = ProviderRegistry::new();
    provider_registry.register(EchoProvider::new("full-control", "manual"))?;
    let provider = provider_registry.get(&ProviderId::new("full-control"))?;

    let session_store = Arc::new(InMemorySessionStore::new(InMemorySessionStoreConfig {
        persist_replay: true,
        max_sessions: Some(4),
    }));
    let hooks = HookChain::new()
        .with_failure_mode(FailureMode::FailClosed)
        .with_hook(ExplicitHook);

    println!(
        "tool registered: {}",
        AnnotateTool.descriptor().canonical_name
    );
    println!(
        "workspace default provider: {}",
        config.workspace().default_provider().unwrap_or("unset")
    );

    let agent = Agent::builder()
        .config(manual_agent_config)
        .provider_arc(provider)
        .session_store_arc(session_store)
        .hooks(hooks)
        .tool(AnnotateTool)
        .event_buffer(512)
        .build()?;

    let response = agent.prompt("describe the final assembly").await?;

    println!("assistant: {}", text_from_message(&response.message));
    let session_label = response
        .session
        .id
        .iter()
        .map(ToString::to_string)
        .next()
        .unwrap_or_else(|| "transient".to_owned());
    println!("session: {session_label}");

    Ok(())
}
