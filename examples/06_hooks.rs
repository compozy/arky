//! # 06 Hooks
//!
//! Demonstrates hook composition, shell-backed hooks, prompt rewriting, result
//! overrides, and stop-decision handling.

mod common;

use std::{
    collections::BTreeMap,
    io,
};

use arky::{
    FailureMode,
    HookChain,
    PromptUpdate,
    SessionRef,
    SessionStartUpdate,
    ShellCommandHook,
    StopDecision,
    ToolCall,
    ToolContent,
    ToolResult,
    ToolResultOverride,
    Verdict,
    hooks::{
        AfterToolCallContext,
        BeforeToolCallContext,
        HookEvent,
        PromptSubmitContext,
        SessionEndContext,
        SessionStartContext,
        SessionStartSource,
        StopContext,
    },
    prelude::*,
};
use async_trait::async_trait;
use common::ExampleError;
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct AuditHook;

#[async_trait]
impl Hooks for AuditHook {
    async fn before_tool_call(
        &self,
        ctx: &BeforeToolCallContext,
        _cancel: CancellationToken,
    ) -> Result<Verdict, arky::HookError> {
        println!("before_tool_call -> {}", ctx.tool_call.name);
        Ok(Verdict::Allow)
    }

    async fn after_tool_call(
        &self,
        _ctx: &AfterToolCallContext,
        _cancel: CancellationToken,
    ) -> Result<Option<ToolResultOverride>, arky::HookError> {
        Ok(Some(ToolResultOverride::new().with_content(vec![
            ToolContent::text("tool result rewritten by hook"),
        ])))
    }

    async fn session_start(
        &self,
        _ctx: &SessionStartContext,
        _cancel: CancellationToken,
    ) -> Result<Option<SessionStartUpdate>, arky::HookError> {
        let env = BTreeMap::from([("HOOK_MODE".to_owned(), "audit".to_owned())]);
        Ok(Some(SessionStartUpdate::new().with_env(env).with_messages(
            vec![Message::system("audit hook injected a system message")],
        )))
    }

    async fn session_end(&self, ctx: &SessionEndContext) -> Result<(), arky::HookError> {
        println!("session_end -> {}", ctx.reason);
        Ok(())
    }

    async fn on_stop(
        &self,
        _ctx: &StopContext,
        _cancel: CancellationToken,
    ) -> Result<StopDecision, arky::HookError> {
        Ok(StopDecision::continue_with(
            "hold the session open for one more turn",
        ))
    }

    async fn user_prompt_submit(
        &self,
        ctx: &PromptSubmitContext,
        _cancel: CancellationToken,
    ) -> Result<Option<PromptUpdate>, arky::HookError> {
        Ok(Some(
            PromptUpdate::new()
                .rewrite(format!("[audited] {}", ctx.prompt))
                .with_messages(vec![Message::system("prompt was audited")]),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let chain = HookChain::new()
        .with_failure_mode(FailureMode::FailClosed)
        .with_hook(AuditHook)
        .with_hook(
            ShellCommandHook::new(HookEvent::UserPromptSubmit, "sh").with_args([
                "-c",
                "cat >/dev/null; printf '%s\\n' 'shell injected note'",
            ]),
        );

    let tool_call = ToolCall::new(
        "call-1",
        "mcp/local/read_file",
        json!({ "path": "Cargo.toml" }),
    );
    let session = SessionRef::default();

    let session_start = chain
        .session_start(
            &SessionStartContext::new(session.clone(), SessionStartSource::Startup),
            CancellationToken::new(),
        )
        .await?
        .ok_or_else(|| io::Error::other("session start should produce an update"))?;
    println!("session_start env: {:?}", session_start.env);
    println!("session_start messages: {:?}", session_start.messages);

    let before = chain
        .before_tool_call(
            &BeforeToolCallContext::new(session.clone(), tool_call.clone()),
            CancellationToken::new(),
        )
        .await?;
    println!("before_tool_call verdict: {before:?}");

    let after = chain
        .after_tool_call(
            &AfterToolCallContext::new(
                session.clone(),
                tool_call,
                ToolResult::success(
                    "call-1",
                    "mcp/local/read_file",
                    vec![ToolContent::text("original tool result")],
                ),
            ),
            CancellationToken::new(),
        )
        .await?;
    println!("after_tool_call override: {after:?}");

    let prompt = chain
        .user_prompt_submit(
            &PromptSubmitContext::new(session.clone(), "review the registry"),
            CancellationToken::new(),
        )
        .await?
        .ok_or_else(|| io::Error::other("prompt hook should produce an update"))?;
    println!("user_prompt_submit rewrite: {:?}", prompt.prompt);
    println!("user_prompt_submit messages: {:?}", prompt.messages);

    let stop = chain
        .on_stop(&StopContext::new(session.clone()), CancellationToken::new())
        .await?;
    println!("on_stop decision: {stop:?}");

    chain
        .session_end(&SessionEndContext::new(session, "example completed"))
        .await?;

    Ok(())
}
