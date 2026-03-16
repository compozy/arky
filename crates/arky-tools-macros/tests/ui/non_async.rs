use arky_tools_macros::tool;

#[tool]
/// This should fail because the function is not async.
fn not_async() -> Result<(), arky_tools::ToolError> {
    Ok(())
}

fn main() {}
