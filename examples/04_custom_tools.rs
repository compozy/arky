//! # 04 Custom Tools
//!
//! Demonstrates Arky's two primary tool-authoring styles:
//! `#[tool]` for concise schema-backed tools and a manual `Tool` implementation
//! when you need complete control.

mod common;

use arky::{
    ContentBlock,
    ProviderCapabilities,
    ProviderDescriptor,
    ProviderError,
    ProviderEventStream,
    ProviderFamily,
    ProviderId,
    ProviderRequest,
    Role,
    ToolCall,
    ToolContent,
    ToolOrigin,
    prelude::*,
};
use async_trait::async_trait;
use common::{
    ExampleError,
    final_message_stream,
    text_from_message,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tool]
/// Normalize a title into compact title case.
async fn sanitize_title(title: String) -> Result<String, ToolError> {
    tokio::task::yield_now().await;
    let normalized = title
        .split_whitespace()
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ");

    Ok(normalized)
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
}

struct CurrentDirectoryTool {
    descriptor: ToolDescriptor,
}

impl CurrentDirectoryTool {
    fn new() -> Result<Self, ToolError> {
        Ok(Self {
            descriptor: ToolDescriptor::new(
                "mcp/local/current_directory",
                "Current Directory",
                "Return the current working directory for the running process.",
                json!({
                    "type": "object",
                    "properties": {},
                }),
                ToolOrigin::Local,
            )?,
        })
    }
}

#[async_trait]
impl Tool for CurrentDirectoryTool {
    fn descriptor(&self) -> ToolDescriptor {
        self.descriptor.clone()
    }

    async fn execute(
        &self,
        call: ToolCall,
        _cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError> {
        let current_directory = std::env::current_dir().map_err(|error| {
            ToolError::execution_failed(
                format!("failed to resolve current directory: {error}"),
                Some(call.name.clone()),
            )
        })?;

        Ok(ToolResult::success(
            call.id,
            call.name,
            vec![ToolContent::text(current_directory.display().to_string())],
        ))
    }
}

#[derive(Clone)]
struct ToolDemoProvider {
    descriptor: ProviderDescriptor,
}

impl ToolDemoProvider {
    fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor::new(
                ProviderId::new("tool-demo"),
                ProviderFamily::Custom("tool-demo".to_owned()),
                ProviderCapabilities::new()
                    .with_streaming(true)
                    .with_generate(true)
                    .with_tool_calls(true),
            ),
        }
    }
}

#[async_trait]
impl Provider for ToolDemoProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn stream(
        &self,
        request: ProviderRequest,
    ) -> Result<ProviderEventStream, ProviderError> {
        let message = if request
            .messages
            .iter()
            .any(|message| message.role == Role::Tool)
        {
            let tool_outputs = request
                .messages
                .iter()
                .filter(|message| message.role == Role::Tool)
                .map(text_from_message)
                .collect::<Vec<_>>()
                .join(" | ");

            Message::assistant(format!("tool summary: {tool_outputs}"))
        } else {
            Message::builder(Role::Assistant)
                .block(ContentBlock::tool_use(ToolCall::new(
                    "call-1",
                    "mcp/local/sanitize_title",
                    json!({ "title": "  arky example suite  " }),
                )))
                .block(ContentBlock::tool_use(ToolCall::new(
                    "call-2",
                    "mcp/local/current_directory",
                    json!({}),
                )))
                .build()
        };

        Ok(final_message_stream(&request, &self.descriptor.id, message))
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let agent = Agent::builder()
        .provider(ToolDemoProvider::new())
        .model("demo-model")
        .tool(SanitizeTitleTool)
        .tool(CurrentDirectoryTool::new()?)
        .build()?;

    let response = agent
        .prompt("Use the registered tools and then summarise the results.")
        .await?;

    println!("assistant: {}", text_from_message(&response.message));
    println!("tool_results: {}", response.tool_results.len());

    Ok(())
}
