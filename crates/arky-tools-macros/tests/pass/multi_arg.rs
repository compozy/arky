use arky_tools::Tool;
use arky_tools_macros::tool;
use tokio_util::sync::CancellationToken;

#[tool]
/// Format a line using multiple parameters.
async fn format_line(
    prefix: String,
    value: u32,
    cancel: CancellationToken,
) -> Result<String, arky_tools::ToolError> {
    let _ = cancel;
    Ok(format!("{prefix}:{value}"))
}

fn main() {
    let tool = FormatLineTool;
    let descriptor = tool.descriptor();
    assert_eq!(descriptor.input_schema["type"], "object");
}
