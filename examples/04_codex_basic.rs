//! # 04 Codex Basic
//!
//! Live, self-checking smoke test for Codex basic `stream` + `generate`
//! behavior.

mod common;

use arky::Provider;
use common::{
    ExampleError,
    codex_model,
    codex_provider,
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
    let workspace = temporary_workspace("codex-basic")?;
    let provider = codex_provider(workspace.path());
    let model = codex_model();

    print_section("Codex streaming");
    println!("model: {model}");
    println!("workspace: {}", workspace.path().display());

    let stream_events = collect_provider_stream(
        provider
            .stream(request(
                &model,
                "Reply with the exact token CODEX_BASIC_STREAM_OK and nothing else.",
                1,
            ))
            .await?,
    )
    .await?;
    let streamed_text = final_turn_text(&stream_events)?;
    require_contains(
        &streamed_text,
        "CODEX_BASIC_STREAM_OK",
        "Codex streaming response",
    )?;
    require_event(
        &stream_events,
        "Codex stream should emit a turn start",
        |event| matches!(event, arky::AgentEvent::TurnStart { .. }),
    )?;
    pass("Codex stream produced the expected terminal message");

    print_section("Codex generate");
    let generated = provider
        .generate(request(
            &model,
            "Reply with the exact token CODEX_BASIC_GENERATE_OK and nothing else.",
            2,
        ))
        .await?;
    let generated_text = text_from_message(&generated.message);
    require_contains(
        &generated_text,
        "CODEX_BASIC_GENERATE_OK",
        "Codex generate response",
    )?;
    pass("Codex generate produced the expected terminal message");

    Ok(())
}
