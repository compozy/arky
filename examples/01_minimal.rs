//! # 01 Minimal
//!
//! Demonstrates the smallest useful Arky agent setup: one provider, one model,
//! and a single `prompt()` call. Everything else uses provider and agent
//! defaults.

mod common;

use arky::{
    ClaudeCodeProvider,
    prelude::*,
};
use common::{
    ExampleError,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let agent = Agent::builder()
        .provider(ClaudeCodeProvider::new())
        .model("sonnet")
        .build()?;

    let response = agent
        .prompt("Explain what the Arky SDK does in one sentence.")
        .await?;
    let session_label = response
        .session
        .id
        .iter()
        .map(ToString::to_string)
        .next()
        .unwrap_or_else(|| "transient".to_owned());

    println!("session: {session_label}");
    println!("assistant: {}", text_from_message(&response.message));

    Ok(())
}
