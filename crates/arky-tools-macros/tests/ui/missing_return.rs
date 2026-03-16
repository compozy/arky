use arky_tools_macros::tool;

#[tool]
/// This should fail because it omits the explicit return type.
async fn missing_return() {}

fn main() {}
