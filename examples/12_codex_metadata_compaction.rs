//! # 12 Codex Metadata And Compaction
//!
//! Live, self-checking validation for Codex response metadata, message part
//! IDs, estimated cost calculation, and thread compaction.

mod common;

use arky::Provider;
use common::{
    ExampleError,
    codex_model,
    codex_provider,
    collect_provider_stream,
    custom_event_payloads,
    final_turn_text,
    pass,
    print_section,
    request_with_session,
    require,
    require_contains,
    require_estimated_cost,
    require_event,
    require_valid_message_part_ids,
    temporary_workspace,
};

fn metadata_thread_id(events: &[arky::AgentEvent]) -> Result<String, ExampleError> {
    custom_event_payloads(events, "response-metadata")
        .iter()
        .find_map(|payload| {
            payload
                .get("session_id")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .ok_or_else(|| {
            std::io::Error::other(
                "missing thread/session identifier in Codex response metadata",
            )
            .into()
        })
}

fn verify_first_turn(
    model: &str,
    events: &[arky::AgentEvent],
) -> Result<(String, f64), ExampleError> {
    let first_text = final_turn_text(events)?;

    require_contains(
        &first_text,
        "ACK_CODEX_COMPACTION",
        "Codex compaction setup response",
    )?;
    require_event(events, "Codex should emit stream-start metadata", |event| {
        matches!(
            event,
            arky::AgentEvent::Custom { event_type, .. }
                if event_type == "stream-start"
        )
    })?;
    require_event(
        events,
        "Codex should emit response-metadata payloads",
        |event| {
            matches!(
                event,
                arky::AgentEvent::Custom { event_type, .. }
                    if event_type == "response-metadata"
            )
        },
    )?;
    require_valid_message_part_ids(
        events,
        "Codex metadata example should expose valid part ids",
    )?;

    Ok((
        metadata_thread_id(events)?,
        require_estimated_cost(
            model,
            events,
            "Codex metadata example should produce a cost estimate",
        )?,
    ))
}

fn verify_post_compaction_turn(
    token: &str,
    events: &[arky::AgentEvent],
) -> Result<(), ExampleError> {
    let second_text = final_turn_text(events)?;

    require_contains(
        &second_text,
        token,
        "Codex response after compaction should retain context",
    )?;
    require_contains(
        &second_text,
        "CODEX_COMPACTION_OK",
        "Codex response after compaction",
    )?;
    require(
        !custom_event_payloads(events, "response-metadata").is_empty(),
        "Codex follow-up after compaction should still emit response metadata",
    )
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let workspace = temporary_workspace("codex-metadata-compaction")?;
    let model = codex_model();
    let provider = codex_provider(workspace.path());
    let session_id = arky::SessionId::new();
    let token = "CODEX_COMPACTION_TOKEN_737373";

    print_section("Codex metadata before compaction");
    println!("model: {model}");
    println!("workspace: {}", workspace.path().display());

    let first_events = collect_provider_stream(
        provider
            .stream(request_with_session(
                session_id.clone(),
                &model,
                format!(
                    "Remember this verification token for the next turn: {token}. \
                     Reply with ACK_CODEX_COMPACTION only."
                ),
                1,
            ))
            .await?,
    )
    .await?;
    let (thread_id, estimated_cost) = verify_first_turn(&model, &first_events)?;

    println!("thread id: {thread_id}");
    println!("estimated cost: ${estimated_cost:.6}");
    pass("Codex emitted response metadata, part ids, and billable usage");

    print_section("Codex thread compaction");
    provider.compact_thread(&thread_id).await?;
    pass("Codex compact_thread succeeded for the active thread");

    print_section("Codex follow-up after compaction");
    let second_events = collect_provider_stream(
        provider
            .stream(request_with_session(
                session_id,
                &model,
                format!(
                    "What verification token did I ask you to remember earlier? \
                     Reply with the exact token {token} and also include \
                     CODEX_COMPACTION_OK."
                ),
                2,
            ))
            .await?,
    )
    .await?;
    verify_post_compaction_turn(token, &second_events)?;
    pass("Codex follow-up still works after history compaction");

    Ok(())
}
