//! # 03 System Prompt
//!
//! Demonstrates explicit system-prompt injection through `AgentBuilder`.

mod common;

use arky::prelude::*;
use common::{
    EchoProvider,
    ExampleError,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let agent = Agent::builder()
        .provider(EchoProvider::new("prompt-echo", "assistant"))
        .model("demo-model")
        .system_prompt("Respond like an operator checklist and mention that the system prompt is active.")
        .build()?;

    let response = agent
        .prompt("Summarise the current execution mode.")
        .await?;

    println!("assistant: {}", text_from_message(&response.message));

    Ok(())
}
