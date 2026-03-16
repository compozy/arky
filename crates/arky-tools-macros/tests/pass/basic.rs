use arky_tools::Tool;
use arky_tools_macros::tool;

#[tool]
/// Greet a user by name.
async fn greet(name: String) -> Result<String, arky_tools::ToolError> {
    Ok(format!("hello {name}"))
}

fn main() {
    let tool = GreetTool;
    let descriptor = tool.descriptor();
    assert_eq!(descriptor.canonical_name, "mcp/local/greet");
}
