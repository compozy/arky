//! # 03 Claude Resume
//!
//! Live, self-checking session-resume validation for Claude Code.

mod common;

use std::sync::Arc;

use arky::{
    Agent,
    InMemorySessionStore,
    SessionStore,
};
use common::{
    ExampleError,
    claude_model,
    claude_provider,
    pass,
    print_section,
    require,
    require_contains,
    temporary_workspace,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("claude-resume")?;
    let model = claude_model();
    let session_store = Arc::new(InMemorySessionStore::default());
    let token = "CLAUDE_RESUME_TOKEN_271828";

    let first_agent = Agent::builder()
        .provider(claude_provider(workspace.path()))
        .model(model.clone())
        .session_store_arc(session_store.clone())
        .build()?;

    print_section("Claude initial turn");
    println!("model: {model}");
    let first = first_agent
        .prompt(format!(
            "Remember this verification token for the next turn: {token}. Reply with ACK_CLAUDE_RESUME only."
        ))
        .await?;
    let first_text = text_from_message(&first.message);
    require_contains(&first_text, "ACK_CLAUDE_RESUME", "Claude first turn ack")?;
    let session_id = first
        .session
        .id
        .clone()
        .ok_or_else(|| std::io::Error::other("Claude response missing session id"))?;
    pass("Claude created a resumable session");

    print_section("Claude resumed turn");
    let resumed_agent = Agent::builder()
        .provider(claude_provider(workspace.path()))
        .model(model)
        .session_store_arc(session_store.clone())
        .build()?;
    resumed_agent.resume(session_id.clone()).await?;
    let resumed = resumed_agent
        .prompt("What verification token did I ask you to remember? Reply with the exact token only.")
        .await?;
    let resumed_text = text_from_message(&resumed.message);
    require_contains(&resumed_text, token, "Claude resumed response")?;

    let snapshot = session_store.load(&session_id).await?;
    require(
        snapshot.messages.len() >= 4,
        "Claude resume should persist both turns in the session store",
    )?;
    pass("Claude resumed the conversation and preserved prior context");

    Ok(())
}
