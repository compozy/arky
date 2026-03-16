//! # 08 Codex Control Flow
//!
//! Live, self-checking validation for Codex follow-up control flow.

mod common;

use arky::Agent;
use common::{
    ExampleError,
    codex_model,
    codex_provider,
    collect_subscription_until_agent_end,
    final_turn_text,
    pass,
    print_section,
    require_contains,
    temporary_workspace,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("codex-control-flow")?;
    let model = codex_model();

    print_section("Codex follow-up");
    println!("model: {model}");
    let follow_up_agent = Agent::builder()
        .provider(codex_provider(workspace.path()))
        .model(model)
        .build()?;
    let memory_token = "CODEX_FIRST_MEMORY";
    let first = follow_up_agent
        .prompt(format!(
            "Remember this verification token for the next turn: {memory_token}. Reply with ACK_CODEX_FIRST only."
        ))
        .await?;
    let first_text = text_from_message(&first.message);
    require_contains(&first_text, "ACK_CODEX_FIRST", "Codex first follow-up turn")?;

    let mut subscription = follow_up_agent.subscribe();
    follow_up_agent
        .follow_up(
            format!(
                "What verification token did I ask you to remember earlier? Reply with the exact token {memory_token} and also include CODEX_FOLLOW_UP_OK."
            ),
        )
        .await?;
    let follow_up_events =
        collect_subscription_until_agent_end(&mut subscription).await?;
    let follow_up_text = final_turn_text(&follow_up_events)?;
    require_contains(
        &follow_up_text,
        "CODEX_FOLLOW_UP_OK",
        "Codex follow-up response",
    )?;
    require_contains(
        &follow_up_text,
        memory_token,
        "Codex follow-up should retain prior turn context",
    )?;
    pass("Codex follow_up scheduled a second turn that retained prior context");

    Ok(())
}
