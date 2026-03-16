use arky_tools_macros::tool;

#[tool(name = "custom")]
/// This should fail because the macro does not accept attribute arguments.
async fn invalid_attr_args() -> Result<(), arky_tools::ToolError> {
    Ok(())
}

fn main() {}
