use arky_tools_macros::tool;

#[tool]
/// This should fail because borrowed parameters are unsupported.
async fn borrowed_argument(
    value: &str,
) -> Result<(), arky_tools::ToolError> {
    let _ = value;
    Ok(())
}

fn main() {}
