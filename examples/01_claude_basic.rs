//! # 01 Claude Basic
//!
//! Live, self-checking smoke test for Claude Code basic `stream` + `generate`
//! behavior.

mod common;

use arky::Provider;
use common::{
    ExampleError,
    claude_model,
    claude_provider,
    collect_provider_stream,
    final_turn_text,
    pass,
    print_section,
    request,
    require_contains,
    require_event,
    temporary_workspace,
    text_from_message,
};

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("claude-basic")?;
    let provider = claude_provider(workspace.path());
    let model = claude_model();

    print_section("Claude streaming");
    println!("model: {model}");
    println!("workspace: {}", workspace.path().display());

    let stream_events = collect_provider_stream(
        provider
            .stream(request(
                &model,
                "Reply with the exact token CLAUDE_BASIC_STREAM_OK and nothing else.",
                1,
            ))
            .await?,
    )
    .await?;
    let streamed_text = final_turn_text(&stream_events)?;
    require_contains(
        &streamed_text,
        "CLAUDE_BASIC_STREAM_OK",
        "Claude streaming response",
    )?;
    require_event(
        &stream_events,
        "Claude stream should end the turn",
        |event| matches!(event, arky::AgentEvent::TurnEnd { .. }),
    )?;
    pass("Claude stream produced the expected terminal message");

    print_section("Claude generate");
    let generated = provider
        .generate(request(
            &model,
            "Reply with the exact token CLAUDE_BASIC_GENERATE_OK and nothing else.",
            2,
        ))
        .await?;
    let generated_text = text_from_message(&generated.message);
    require_contains(
        &generated_text,
        "CLAUDE_BASIC_GENERATE_OK",
        "Claude generate response",
    )?;
    pass("Claude generate produced the expected terminal message");

    Ok(())
}
