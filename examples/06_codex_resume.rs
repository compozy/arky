//! # 06 Codex Resume
//!
//! Live, self-checking session-resume validation for Codex.

mod common;

use std::sync::Arc;

use arky::{
    Agent,
    InMemorySessionStore,
    SessionStore,
};
use common::{
    ExampleError,
    codex_model,
    codex_provider,
    pass,
    print_section,
    require,
    require_contains,
    temporary_workspace,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("codex-resume")?;
    let model = codex_model();
    let session_store = Arc::new(InMemorySessionStore::default());
    let token = "CODEX_RESUME_TOKEN_161803";

    let first_agent = Agent::builder()
        .provider(codex_provider(workspace.path()))
        .model(model.clone())
        .session_store_arc(session_store.clone())
        .build()?;

    print_section("Codex initial turn");
    println!("model: {model}");
    let first = first_agent
        .prompt(format!(
            "Remember this verification token for the next turn: {token}. Reply with ACK_CODEX_RESUME only."
        ))
        .await?;
    let first_text = text_from_message(&first.message);
    require_contains(&first_text, "ACK_CODEX_RESUME", "Codex first turn ack")?;
    let session_id = first
        .session
        .id
        .clone()
        .ok_or_else(|| std::io::Error::other("Codex response missing session id"))?;
    pass("Codex created a resumable session");

    print_section("Codex resumed turn");
    let resumed_agent = Agent::builder()
        .provider(codex_provider(workspace.path()))
        .model(model)
        .session_store_arc(session_store.clone())
        .build()?;
    resumed_agent.resume(session_id.clone()).await?;
    let resumed = resumed_agent
        .prompt("What verification token did I ask you to remember? Reply with the exact token only.")
        .await?;
    let resumed_text = text_from_message(&resumed.message);
    require_contains(&resumed_text, token, "Codex resumed response")?;

    let snapshot = session_store.load(&session_id).await?;
    require(
        snapshot.messages.len() >= 4,
        "Codex resume should persist both turns in the session store",
    )?;
    pass("Codex resumed the conversation and preserved prior context");

    Ok(())
}
