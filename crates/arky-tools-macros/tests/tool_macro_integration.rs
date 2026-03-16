//! Integration coverage for the `#[tool]` proc macro.

use arky_tools::{
    Tool,
    ToolCall,
    ToolContent,
    ToolRegistry,
};
use arky_tools_macros::tool;
use pretty_assertions::assert_eq;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchArgs {
    query: String,
    filters: Vec<String>,
    nested: SearchNested,
    mode: Option<SearchMode>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchNested {
    limit: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
enum SearchMode {
    Quick,
    Deep,
}

#[tool]
/// Echo the provided message and count.
async fn echo(
    message: String,
    count: u32,
    cancel: CancellationToken,
) -> Result<String, arky_tools::ToolError> {
    let _ = cancel;
    tokio::task::yield_now().await;
    Ok(format!("{message}:{count}"))
}

#[tool]
/// Build a structured summary for a query.
async fn summarize(args: SearchArgs) -> Result<serde_json::Value, arky_tools::ToolError> {
    tokio::task::yield_now().await;
    Ok(json!({
        "query": args.query,
        "filters": args.filters,
        "limit": args.nested.limit,
        "mode": args.mode.map(|mode| match mode {
            SearchMode::Quick => "quick",
            SearchMode::Deep => "deep",
        }),
    }))
}

#[tool]
/// Report whether execution was already cancelled.
async fn inspect_cancel(
    cancel: CancellationToken,
) -> Result<String, arky_tools::ToolError> {
    tokio::task::yield_now().await;
    Ok(cancel.is_cancelled().to_string())
}

#[tool]
/// Trim and uppercase a single string argument.
async fn normalize_text(text: String) -> Result<String, arky_tools::ToolError> {
    tokio::task::yield_now().await;
    Ok(text.trim().to_uppercase())
}

#[tokio::test]
async fn tool_macro_should_register_and_execute_generated_tool() {
    let registry = ToolRegistry::new();
    registry
        .register(EchoTool)
        .expect("generated tool should register");

    let result = registry
        .execute(
            ToolCall::new(
                "call-1",
                "mcp/local/echo",
                json!({
                    "message": "hello",
                    "count": 3,
                }),
            ),
            CancellationToken::new(),
        )
        .await
        .expect("generated tool should execute");

    assert_eq!(result.content, vec![ToolContent::text("hello:3")]);
}

#[tokio::test]
async fn tool_macro_should_accept_direct_json_for_single_primitive_argument() {
    let registry = ToolRegistry::new();
    registry
        .register(NormalizeTextTool)
        .expect("single-argument tool should register");

    let descriptor = NormalizeTextTool.descriptor();
    assert_eq!(descriptor.input_schema["type"], "string");

    let result = registry
        .execute(
            ToolCall::new("call-direct", "mcp/local/normalize_text", json!("  arky  ")),
            CancellationToken::new(),
        )
        .await
        .expect("single-argument tool should accept a direct string input");

    assert_eq!(result.content, vec![ToolContent::text("ARKY")]);
}

#[test]
fn tool_macro_should_generate_schema_for_complex_input_types() {
    let descriptor = SummarizeTool.descriptor();
    let properties = descriptor.input_schema["properties"]
        .as_object()
        .expect("schema should expose properties");
    let required = descriptor.input_schema["required"]
        .as_array()
        .expect("schema should list required fields");

    assert_eq!(descriptor.canonical_name, "mcp/local/summarize");
    assert_eq!(
        descriptor.description,
        "Build a structured summary for a query."
    );
    assert!(properties.contains_key("query"));
    assert!(properties.contains_key("filters"));
    assert!(properties.contains_key("nested"));
    assert!(properties.contains_key("mode"));
    assert!(required.iter().any(|value| value == "query"));
    assert_eq!(descriptor.input_schema["type"], "object");
}

#[tokio::test]
async fn tool_macro_should_exclude_cancellation_token_from_schema_and_pass_it_through() {
    let cancel = CancellationToken::new();
    cancel.cancel();

    let descriptor = InspectCancelTool.descriptor();
    let result = InspectCancelTool
        .execute(
            ToolCall::new("call-2", "mcp/local/inspect_cancel", json!({})),
            cancel,
        )
        .await
        .expect("cancel-only tool should execute");

    assert_eq!(descriptor.input_schema["type"], "object");
    assert!(
        descriptor.input_schema["properties"]
            .get("cancel")
            .is_none()
    );
    assert_eq!(result.content, vec![ToolContent::text("true")]);
}
