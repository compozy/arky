//! # 05 Tool Registry
//!
//! Demonstrates direct `ToolRegistry` usage, call-scoped registrations,
//! collision handling, and provider-specific tool ID codecs.

mod common;

use std::sync::Arc;

use arky::{
    StaticToolIdCodec,
    ToolCall,
    ToolContent,
    ToolIdCodec,
    ToolOrigin,
    ToolRegistry,
    create_codex_tool_id_codec,
    prelude::*,
};
use async_trait::async_trait;
use common::ExampleError;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tool]
/// Reverse one line of text.
async fn reverse_line(text: String) -> Result<String, ToolError> {
    tokio::task::yield_now().await;
    Ok(text.chars().rev().collect())
}

struct NoteTool {
    descriptor: ToolDescriptor,
    note: String,
}

impl NoteTool {
    fn new(
        canonical_name: &str,
        display_name: &str,
        note: impl Into<String>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            descriptor: ToolDescriptor::new(
                canonical_name,
                display_name,
                format!("{display_name} tool"),
                json!({
                    "type": "object",
                    "properties": {},
                }),
                ToolOrigin::Local,
            )?,
            note: note.into(),
        })
    }
}

#[async_trait]
impl Tool for NoteTool {
    fn descriptor(&self) -> ToolDescriptor {
        self.descriptor.clone()
    }

    async fn execute(
        &self,
        call: ToolCall,
        _cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError> {
        Ok(ToolResult::success(
            call.id,
            call.name,
            vec![ToolContent::text(self.note.clone())],
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let registry = ToolRegistry::new();
    let codec = create_codex_tool_id_codec();
    let local_codec = StaticToolIdCodec::new(arky::ProviderId::new("local"), "local__");

    registry.register(ReverseLineTool)?;
    registry.register(NoteTool::new(
        "mcp/local/current_note",
        "Current Note",
        "long-lived registration",
    )?)?;

    let encoded = codec.encode("mcp/local/reverse_line")?;
    let decoded = codec.decode(&encoded)?;
    println!("codex codec: {encoded} -> {}", decoded.canonical_name);
    println!(
        "local codec: {}",
        local_codec.encode("mcp/local/current_note")?
    );

    let duplicate_error = registry
        .register(ReverseLineTool)
        .expect_err("duplicate tool registration should fail");
    println!("collision error: {}", duplicate_error.error_code());

    let scoped = registry.register_many_call_scoped(vec![Arc::new(NoteTool::new(
        "mcp/local/scoped_note",
        "Scoped Note",
        "call-scoped registration",
    )?) as Arc<dyn Tool>])?;
    println!("registered tools: {}", registry.list().len());

    let reverse = registry
        .execute(
            ToolCall::new(
                "call-1",
                "mcp/local/reverse_line",
                json!({ "text": "arky" }),
            ),
            CancellationToken::new(),
        )
        .await?;
    let scoped_note = registry
        .execute(
            ToolCall::new("call-2", "mcp/local/scoped_note", json!({})),
            CancellationToken::new(),
        )
        .await?;

    println!("reverse_line -> {:?}", reverse.content);
    println!("scoped_note -> {:?}", scoped_note.content);
    println!(
        "scoped tool present before cleanup: {}",
        registry.contains("mcp/local/scoped_note")
    );

    let removed = scoped.cleanup();
    println!("call-scoped cleanup removed {removed} tool(s)");
    println!(
        "scoped tool present after cleanup: {}",
        registry.contains("mcp/local/scoped_note")
    );

    Ok(())
}
