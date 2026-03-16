//! Turn-loop execution for the high-level agent runtime.

use std::sync::Arc;

use arky_hooks::{
    AfterToolCallContext,
    BeforeToolCallContext,
    Hooks,
    PromptSubmitContext,
    PromptUpdate,
    StopContext,
    StopDecision,
};
use arky_protocol::{
    AgentEvent,
    AgentResponse,
    ContentBlock,
    EventMetadata,
    Message,
    ModelRef,
    PersistedEvent,
    ProviderRequest,
    ProviderSettings,
    SessionRef,
    ToolCall,
    ToolContext,
    ToolDefinition,
    ToolResult,
    TurnCheckpoint,
    TurnContext,
    TurnId,
};
use arky_provider::Provider;
use arky_session::SessionStore;
use arky_tools::{
    Tool,
    ToolContent,
    ToolRegistrationHandle,
    ToolRegistry,
};
use futures::StreamExt;
use serde_json::{
    Value,
    json,
};
use tokio::sync::{
    broadcast,
    mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{
    Instrument,
    info_span,
};

use crate::{
    CoreError,
    replay::SessionState,
};

#[derive(Clone)]
pub struct TurnRuntime {
    pub provider: Arc<dyn Provider>,
    pub tools: ToolRegistry,
    pub temporary_tools: Vec<Arc<dyn Tool>>,
    pub hooks: Arc<dyn Hooks>,
    pub session_store: Arc<dyn SessionStore>,
    pub model: ModelRef,
    pub system_prompt: Option<String>,
    pub provider_settings: ProviderSettings,
    pub events: broadcast::Sender<AgentEvent>,
}

pub struct TurnControl {
    pub cancel: CancellationToken,
    pub steering_rx: mpsc::UnboundedReceiver<String>,
    pub follow_up_rx: mpsc::UnboundedReceiver<String>,
}

pub struct TurnRunResult {
    pub session: SessionState,
    pub response: Result<AgentResponse, CoreError>,
}

struct ToolExecutionOutcome {
    tool_results: Vec<ToolResult>,
    steering_inputs: Vec<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "the turn loop keeps the session state machine in one place"
)]
pub async fn run_turn(
    runtime: Arc<TurnRuntime>,
    session: SessionState,
    input: String,
    stream_tx: Option<mpsc::UnboundedSender<Result<AgentEvent, CoreError>>>,
    control: TurnControl,
) -> TurnRunResult {
    let agent_span = info_span!("agent", provider_id = %runtime.provider.descriptor().id);
    async move {
        let mut session = session;
        let mut stream_tx = stream_tx;
        let mut control = control;
        let provider_id = runtime.provider.descriptor().id.clone();

        let session_span = info_span!("session", session_id = %session.id);
        let response = async {
            let _temporary_handle = register_temporary_tools(&runtime)?;
            let mut event_log = Vec::new();
            let mut run_messages = Vec::new();
            let mut run_tool_results = Vec::new();
            let mut pending_messages = prepare_user_messages(
                &runtime,
                &session.session_ref(),
                vec![input],
            )
            .await?;
            let mut continue_from_context = false;
            let mut final_response: Option<AgentResponse> = None;

            emit_event(
                &runtime.session_store,
                &runtime.events,
                &mut stream_tx,
                &mut session,
                &mut event_log,
                &provider_id,
                None,
                |meta| AgentEvent::AgentStart { meta },
            )
            .await?;

            loop {
                check_cancelled(&control.cancel)?;

                if pending_messages.is_empty() && !continue_from_context {
                    let session_ref = session.session_ref();
                    let follow_up_messages = prepare_user_messages(
                        &runtime,
                        &session_ref,
                        drain_queue(&mut control.follow_up_rx),
                    )
                    .await?;
                    if follow_up_messages.is_empty() {
                        break;
                    }
                    pending_messages = follow_up_messages;
                }

                let turn_id = TurnId::new();
                let turn_sequence = session.next_turn_sequence;
                let turn_context = TurnContext::new(turn_id.clone(), turn_sequence);
                session.next_turn_sequence =
                    session.next_turn_sequence.saturating_add(1);
                let turn_span = info_span!(
                    "turn",
                    session_id = %session.id,
                    turn_id = %turn_id,
                    turn_sequence = turn_context.sequence,
                );

                let mut assistant_message: Option<Message> = None;
                let mut tool_results = Vec::new();
                let mut steering_messages = Vec::new();

                async {
                    emit_event(
                        &runtime.session_store,
                        &runtime.events,
                        &mut stream_tx,
                        &mut session,
                        &mut event_log,
                        &provider_id,
                        Some(&turn_id),
                        |meta| AgentEvent::TurnStart { meta },
                    )
                    .await?;

                    if !pending_messages.is_empty() {
                        persist_messages(
                            &runtime,
                            &session.id,
                            &pending_messages,
                        )
                        .await?;
                        for message in std::mem::take(&mut pending_messages) {
                            let decorated = decorate_message(
                                message,
                                &session.id,
                                &turn_id,
                                &provider_id,
                            );
                            emit_event(
                                &runtime.session_store,
                                &runtime.events,
                                &mut stream_tx,
                                &mut session,
                                &mut event_log,
                                &provider_id,
                                Some(&turn_id),
                                |meta| AgentEvent::MessageStart {
                                    meta,
                                    message: decorated.clone(),
                                },
                            )
                            .await?;
                            emit_event(
                                &runtime.session_store,
                                &runtime.events,
                                &mut stream_tx,
                                &mut session,
                                &mut event_log,
                                &provider_id,
                                Some(&turn_id),
                                |meta| AgentEvent::MessageEnd {
                                    meta,
                                    message: decorated.clone(),
                                },
                            )
                            .await?;
                            session.messages.push(decorated.clone());
                            run_messages.push(decorated);
                        }
                    }

                    assistant_message = Some(stream_provider_turn(
                        &runtime,
                        &mut session,
                        &mut event_log,
                        &mut stream_tx,
                        &turn_id,
                        &turn_context,
                        &control.cancel,
                    )
                    .await?);

                    let assistant_message = assistant_message.clone().ok_or_else(|| {
                        CoreError::invalid_state(
                            "provider stream completed without yielding an assistant message",
                            Some(json!({
                                "session_id": session.id.to_string(),
                                "turn_id": turn_id.to_string(),
                            })),
                        )
                    })?;

                    persist_messages(
                        &runtime,
                        &session.id,
                        std::slice::from_ref(&assistant_message),
                    )
                    .await?;
                    session.messages.push(assistant_message.clone());
                    run_messages.push(assistant_message.clone());

                    let tool_calls = tool_calls_from_message(&assistant_message);
                    if tool_calls.is_empty() {
                        let session_ref = session.session_ref();
                        steering_messages = prepare_steering_messages(
                            &session_ref,
                            drain_queue(&mut control.steering_rx),
                        );
                    } else {
                        let execution = execute_tool_calls(
                            &runtime,
                            &mut session,
                            &mut event_log,
                            &mut stream_tx,
                            &turn_id,
                            &control.cancel,
                            &mut control.steering_rx,
                            tool_calls,
                        )
                        .await?;
                        tool_results = execution.tool_results;
                        run_tool_results.extend(tool_results.clone());

                        for result in &tool_results {
                            let message = decorate_message(
                                Message::tool(result.clone()),
                                &session.id,
                                &turn_id,
                                &provider_id,
                            );
                            persist_messages(
                                &runtime,
                                &session.id,
                                std::slice::from_ref(&message),
                            )
                            .await?;
                            emit_event(
                                &runtime.session_store,
                                &runtime.events,
                                &mut stream_tx,
                                &mut session,
                                &mut event_log,
                                &provider_id,
                                Some(&turn_id),
                                |meta| AgentEvent::MessageStart {
                                    meta,
                                    message: message.clone(),
                                },
                            )
                            .await?;
                            emit_event(
                                &runtime.session_store,
                                &runtime.events,
                                &mut stream_tx,
                                &mut session,
                                &mut event_log,
                                &provider_id,
                                Some(&turn_id),
                                |meta| AgentEvent::MessageEnd {
                                    meta,
                                    message: message.clone(),
                                },
                            )
                            .await?;
                            session.messages.push(message.clone());
                            run_messages.push(message);
                        }

                        let session_ref = session.session_ref();
                        let mut steering_inputs = execution.steering_inputs;
                        steering_inputs.extend(drain_queue(&mut control.steering_rx));
                        steering_messages =
                            prepare_steering_messages(&session_ref, steering_inputs);
                    }

                    emit_event(
                        &runtime.session_store,
                        &runtime.events,
                        &mut stream_tx,
                        &mut session,
                        &mut event_log,
                        &provider_id,
                        Some(&turn_id),
                        |meta| AgentEvent::TurnEnd {
                            meta,
                            message: assistant_message.clone(),
                            tool_results: tool_results.clone(),
                        },
                    )
                    .await?;

                    let checkpoint = TurnCheckpoint::new(
                        turn_id.clone(),
                        session.next_event_sequence.saturating_sub(1),
                    )
                    .with_message(assistant_message.clone())
                    .with_tool_results(tool_results.clone())
                    .with_provider_id(provider_id.clone())
                    .mark_completed(now_ms());
                    runtime
                        .session_store
                        .save_turn_checkpoint(&session.id, checkpoint.clone())
                        .await
                        .map_err(|error| {
                            CoreError::invalid_state(
                                format!("failed to save turn checkpoint: {error}"),
                                Some(json!({
                                    "session_id": session.id.to_string(),
                                    "turn_id": checkpoint.turn_id.to_string(),
                                    "error_code": arky_error::ClassifiedError::error_code(&error),
                                })),
                            )
                        })?;
                    session.replay_cursor = Some(
                        arky_protocol::ReplayCursor::from_checkpoint(checkpoint.sequence),
                    );
                    session
                        .provider_session_id
                        .clone_from(&checkpoint.provider_session_id);
                    session.last_checkpoint = Some(checkpoint);

                    final_response = Some(
                        AgentResponse::new(
                            session.session_ref(),
                            turn_context.clone(),
                            assistant_message,
                        )
                        .with_tool_results(run_tool_results.clone())
                        .with_events(event_log.clone()),
                    );

                    Ok::<(), CoreError>(())
                }
                .instrument(turn_span)
                .await?;

                if !steering_messages.is_empty() {
                    pending_messages = steering_messages;
                    continue_from_context = false;
                    continue;
                }

                if !tool_results.is_empty() {
                    continue_from_context = true;
                    continue;
                }

                match runtime
                    .hooks
                    .on_stop(
                        &StopContext::new(session.session_ref()),
                        control.cancel.child_token(),
                    )
                    .await
                {
                    Ok(StopDecision::Continue { reason }) => {
                        pending_messages = vec![Message::system(reason)];
                        continue_from_context = false;
                        continue;
                    }
                    Ok(StopDecision::Stop) => {}
                    Err(error) => {
                        return Err(CoreError::invalid_state(
                            format!("stop hook failed: {error}"),
                            Some(json!({
                                "session_id": session.id.to_string(),
                                "error_code": arky_error::ClassifiedError::error_code(&error),
                            })),
                        ));
                    }
                }

                let session_ref = session.session_ref();
                let follow_up_messages = prepare_user_messages(
                    &runtime,
                    &session_ref,
                    drain_queue(&mut control.follow_up_rx),
                )
                .await?;
                if !follow_up_messages.is_empty() {
                    pending_messages = follow_up_messages;
                    continue_from_context = false;
                    continue;
                }

                break;
            }

            let response = final_response.ok_or_else(|| {
                CoreError::invalid_state(
                    "turn loop completed without producing a response",
                    Some(json!({
                        "session_id": session.id.to_string(),
                    })),
                )
            })?;
            emit_event(
                &runtime.session_store,
                &runtime.events,
                &mut stream_tx,
                &mut session,
                &mut event_log,
                &provider_id,
                None,
                |meta| AgentEvent::AgentEnd {
                    meta,
                    messages: run_messages.clone(),
                },
            )
            .await?;

            Ok(response)
        }
        .instrument(session_span)
        .await;

        if let Err(error) = &response
            && let Some(sender) = stream_tx.as_mut()
        {
            let _ = sender.send(Err(error.clone()));
        }

        TurnRunResult { session, response }
    }
    .instrument(agent_span)
    .await
}

async fn prepare_user_messages(
    runtime: &TurnRuntime,
    session: &SessionRef,
    inputs: Vec<String>,
) -> Result<Vec<Message>, CoreError> {
    let mut prepared = Vec::new();

    for input in inputs {
        let update = runtime
            .hooks
            .user_prompt_submit(
                &PromptSubmitContext::new(session.clone(), input.clone()),
                CancellationToken::new(),
            )
            .await
            .map_err(|error| {
                CoreError::invalid_state(
                    format!("prompt hook failed: {error}"),
                    Some(json!({
                        "session_id": session.id.as_ref().map(ToString::to_string),
                        "error_code": arky_error::ClassifiedError::error_code(&error),
                    })),
                )
            })?;

        let prompt = update
            .as_ref()
            .and_then(|PromptUpdate { prompt, .. }| prompt.clone())
            .unwrap_or(input);
        if let Some(update) = update {
            prepared.extend(update.messages);
        }
        prepared.push(Message::user(prompt));
    }

    Ok(prepared)
}

fn prepare_steering_messages(_session: &SessionRef, inputs: Vec<String>) -> Vec<Message> {
    inputs.into_iter().map(Message::system).collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "provider stream normalization needs to handle every event variant explicitly"
)]
async fn stream_provider_turn(
    runtime: &TurnRuntime,
    session: &mut SessionState,
    event_log: &mut Vec<AgentEvent>,
    stream_tx: &mut Option<mpsc::UnboundedSender<Result<AgentEvent, CoreError>>>,
    turn_id: &TurnId,
    turn_context: &TurnContext,
    cancel: &CancellationToken,
) -> Result<Message, CoreError> {
    check_cancelled(cancel)?;

    let messages =
        request_messages(session.messages.clone(), runtime.system_prompt.as_deref());
    let provider_call_span = info_span!(
        "provider_call",
        session_id = %session.id,
        turn_id = %turn_id,
        provider_id = %runtime.provider.descriptor().id,
    );

    async {
        let request = ProviderRequest::new(
            session.session_ref(),
            turn_context.clone(),
            runtime.model.clone(),
            messages,
        )
        .with_tools(ToolContext::new().with_definitions(tool_definitions(&runtime.tools)))
        .with_settings(runtime.provider_settings.clone());

        let mut stream = runtime.provider.stream(request).await.map_err(|error| {
            CoreError::invalid_state(
                format!("provider stream failed to start: {error}"),
                Some(json!({
                    "provider_id": runtime.provider.descriptor().id.as_str(),
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })?;

        let provider_id = runtime.provider.descriptor().id.clone();
        let mut terminal_message: Option<Message> = None;

        loop {
            let next = tokio::select! {
                () = cancel.cancelled() => {
                    return Err(CoreError::cancelled("active turn was aborted"));
                }
                item = stream.next() => item,
            };
            let Some(item) = next else {
                break;
            };
            let event = item.map_err(|error| {
                CoreError::invalid_state(
                    format!("provider stream failed: {error}"),
                    Some(json!({
                        "provider_id": provider_id.as_str(),
                        "error_code": arky_error::ClassifiedError::error_code(&error),
                    })),
                )
            })?;

            match event {
                AgentEvent::MessageStart { message, .. } => {
                    let message =
                        decorate_message(message, &session.id, turn_id, &provider_id);
                    terminal_message = Some(message.clone());
                    emit_event(
                        &runtime.session_store,
                        &runtime.events,
                        stream_tx,
                        session,
                        event_log,
                        &provider_id,
                        Some(turn_id),
                        |meta| AgentEvent::MessageStart {
                            meta,
                            message: message.clone(),
                        },
                    )
                    .await?;
                }
                AgentEvent::MessageUpdate { message, delta, .. } => {
                    let message =
                        decorate_message(message, &session.id, turn_id, &provider_id);
                    terminal_message = Some(message.clone());
                    emit_event(
                        &runtime.session_store,
                        &runtime.events,
                        stream_tx,
                        session,
                        event_log,
                        &provider_id,
                        Some(turn_id),
                        |meta| AgentEvent::MessageUpdate {
                            meta,
                            message: message.clone(),
                            delta: delta.clone(),
                        },
                    )
                    .await?;
                }
                AgentEvent::MessageEnd { message, .. } => {
                    let message =
                        decorate_message(message, &session.id, turn_id, &provider_id);
                    terminal_message = Some(message.clone());
                    emit_event(
                        &runtime.session_store,
                        &runtime.events,
                        stream_tx,
                        session,
                        event_log,
                        &provider_id,
                        Some(turn_id),
                        |meta| AgentEvent::MessageEnd {
                            meta,
                            message: message.clone(),
                        },
                    )
                    .await?;
                }
                AgentEvent::Custom {
                    event_type,
                    payload,
                    ..
                } => {
                    emit_event(
                        &runtime.session_store,
                        &runtime.events,
                        stream_tx,
                        session,
                        event_log,
                        &provider_id,
                        Some(turn_id),
                        |meta| AgentEvent::Custom {
                            meta,
                            event_type: event_type.clone(),
                            payload: payload.clone(),
                        },
                    )
                    .await?;
                }
                AgentEvent::TurnEnd { message, .. } => {
                    terminal_message = Some(decorate_message(
                        message,
                        &session.id,
                        turn_id,
                        &provider_id,
                    ));
                }
                _ => {}
            }
        }

        terminal_message.ok_or_else(|| {
            CoreError::invalid_state(
                "provider stream completed without a final assistant message",
                Some(json!({
                    "provider_id": provider_id.as_str(),
                    "turn_id": turn_id.to_string(),
                })),
            )
        })
    }
    .instrument(provider_call_span)
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "tool execution coordinates runtime state, event sinks, and cancellation"
)]
#[expect(
    clippy::too_many_lines,
    reason = "tool execution keeps hook, runtime, and steering interactions together"
)]
async fn execute_tool_calls(
    runtime: &TurnRuntime,
    session: &mut SessionState,
    event_log: &mut Vec<AgentEvent>,
    stream_tx: &mut Option<mpsc::UnboundedSender<Result<AgentEvent, CoreError>>>,
    turn_id: &TurnId,
    cancel: &CancellationToken,
    steering_rx: &mut mpsc::UnboundedReceiver<String>,
    tool_calls: Vec<ToolCall>,
) -> Result<ToolExecutionOutcome, CoreError> {
    let provider_id = runtime.provider.descriptor().id.clone();
    let mut results = Vec::new();
    let mut steering_inputs = Vec::new();

    for tool_call in tool_calls {
        check_cancelled(cancel)?;

        let tool_span = info_span!(
            "tool_call",
            session_id = %session.id,
            turn_id = %turn_id,
            tool_name = %tool_call.name,
            tool_call_id = %tool_call.id,
        );
        let tool_call = async {
            let verdict = runtime
                .hooks
                .before_tool_call(
                    &BeforeToolCallContext::new(
                        session.session_ref(),
                        tool_call.clone(),
                    ),
                    cancel.child_token(),
                )
                .await
                .map_err(|error| {
                    CoreError::invalid_state(
                        format!("before_tool_call hook failed: {error}"),
                        Some(json!({
                            "tool_name": tool_call.name,
                            "error_code": arky_error::ClassifiedError::error_code(&error),
                        })),
                    )
                })?;

            emit_event(
                &runtime.session_store,
                &runtime.events,
                stream_tx,
                session,
                event_log,
                &provider_id,
                Some(turn_id),
                |meta| AgentEvent::ToolExecutionStart {
                    meta,
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    args: tool_call.input.clone(),
                },
            )
            .await?;

            let result = match verdict {
                arky_hooks::Verdict::Allow => {
                    runtime
                        .tools
                        .execute(tool_call.clone(), cancel.child_token())
                        .await
                        .map_err(|error| {
                            CoreError::invalid_state(
                                format!("tool execution failed: {error}"),
                                Some(json!({
                                    "tool_name": tool_call.name,
                                    "error_code": arky_error::ClassifiedError::error_code(&error),
                                })),
                            )
                        })?
                }
                arky_hooks::Verdict::Block { reason } => ToolResult::failure(
                    tool_call.id.clone(),
                    tool_call.name.clone(),
                    vec![ToolContent::text(reason)],
                ),
            };

            let result = apply_tool_override(
                runtime,
                session,
                cancel,
                tool_call.clone(),
                result,
            )
            .await?;
            emit_event(
                &runtime.session_store,
                &runtime.events,
                stream_tx,
                session,
                event_log,
                &provider_id,
                Some(turn_id),
                |meta| AgentEvent::ToolExecutionEnd {
                    meta,
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    result: tool_result_value(&result),
                    is_error: result.is_error,
                },
            )
            .await?;

            Ok::<ToolResult, CoreError>(decorate_tool_result(
                result,
                &provider_id,
            ))
        }
        .instrument(tool_span)
        .await?;

        results.push(tool_call);
        let queued_steering = drain_queue(steering_rx);
        if !queued_steering.is_empty() {
            steering_inputs.extend(queued_steering);
            break;
        }
    }

    Ok(ToolExecutionOutcome {
        tool_results: results,
        steering_inputs,
    })
}

async fn apply_tool_override(
    runtime: &TurnRuntime,
    session: &SessionState,
    cancel: &CancellationToken,
    tool_call: ToolCall,
    mut result: ToolResult,
) -> Result<ToolResult, CoreError> {
    let override_result = runtime
        .hooks
        .after_tool_call(
            &AfterToolCallContext::new(session.session_ref(), tool_call, result.clone()),
            cancel.child_token(),
        )
        .await
        .map_err(|error| {
            CoreError::invalid_state(
                format!("after_tool_call hook failed: {error}"),
                Some(json!({
                    "tool_name": result.name,
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })?;
    if let Some(override_result) = override_result {
        if let Some(content) = override_result.content {
            result.content = content;
        }
        if let Some(is_error) = override_result.is_error {
            result.is_error = is_error;
        }
    }
    Ok(result)
}

fn request_messages(
    mut messages: Vec<Message>,
    system_prompt: Option<&str>,
) -> Vec<Message> {
    if let Some(system_prompt) = system_prompt {
        messages.insert(0, Message::system(system_prompt));
    }
    messages
}

fn tool_definitions(registry: &ToolRegistry) -> Vec<ToolDefinition> {
    registry
        .list()
        .into_iter()
        .map(|descriptor| {
            ToolDefinition::new(
                descriptor.canonical_name,
                descriptor.description,
                descriptor.input_schema,
            )
        })
        .collect()
}

fn tool_calls_from_message(message: &Message) -> Vec<ToolCall> {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { call } => Some(call.clone()),
            ContentBlock::Text { .. }
            | ContentBlock::ToolResult { .. }
            | ContentBlock::Image { .. } => None,
        })
        .collect()
}

fn tool_result_value(result: &ToolResult) -> Value {
    json!({
        "id": result.id,
        "name": result.name,
        "content": result.content,
        "is_error": result.is_error,
        "parent_id": result.parent_id,
    })
}

fn decorate_message(
    mut message: Message,
    session_id: &arky_protocol::SessionId,
    turn_id: &TurnId,
    provider_id: &arky_protocol::ProviderId,
) -> Message {
    let metadata = message.metadata.take().unwrap_or_default();
    message.metadata = Some(
        metadata
            .with_session_id(session_id.clone())
            .with_turn_id(turn_id.clone())
            .with_provider_id(provider_id.clone())
            .with_timestamp_ms(now_ms()),
    );
    message
}

const fn decorate_tool_result(
    result: ToolResult,
    _provider_id: &arky_protocol::ProviderId,
) -> ToolResult {
    result
}

async fn persist_messages(
    runtime: &TurnRuntime,
    session_id: &arky_protocol::SessionId,
    messages: &[Message],
) -> Result<(), CoreError> {
    runtime
        .session_store
        .append_messages(session_id, messages)
        .await
        .map_err(|error| {
            CoreError::invalid_state(
                format!("failed to persist messages: {error}"),
                Some(json!({
                    "session_id": session_id.to_string(),
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })
}

fn register_temporary_tools(
    runtime: &TurnRuntime,
) -> Result<Option<ToolRegistrationHandle>, CoreError> {
    if runtime.temporary_tools.is_empty() {
        return Ok(None);
    }

    runtime
        .tools
        .register_many_call_scoped(runtime.temporary_tools.iter().cloned())
        .map(Some)
        .map_err(|error| {
            CoreError::invalid_state(
                format!("failed to register temporary tools: {error}"),
                Some(json!({
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })
}

fn check_cancelled(cancel: &CancellationToken) -> Result<(), CoreError> {
    if cancel.is_cancelled() {
        return Err(CoreError::cancelled("active turn was aborted"));
    }
    Ok(())
}

fn drain_queue(receiver: &mut mpsc::UnboundedReceiver<String>) -> Vec<String> {
    let mut drained = Vec::new();
    while let Ok(message) = receiver.try_recv() {
        drained.push(message);
    }
    drained
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[expect(
    clippy::too_many_arguments,
    reason = "event emission needs the active session, sinks, and routing metadata"
)]
async fn emit_event<F>(
    session_store: &Arc<dyn SessionStore>,
    broadcast: &broadcast::Sender<AgentEvent>,
    stream_tx: &mut Option<mpsc::UnboundedSender<Result<AgentEvent, CoreError>>>,
    session: &mut SessionState,
    log: &mut Vec<AgentEvent>,
    provider_id: &arky_protocol::ProviderId,
    turn_id: Option<&TurnId>,
    builder: F,
) -> Result<AgentEvent, CoreError>
where
    F: FnOnce(EventMetadata) -> AgentEvent,
{
    let meta = EventMetadata::new(now_ms(), session.next_event_sequence)
        .with_session_id(session.id.clone())
        .with_provider_id(provider_id.clone());
    let meta = if let Some(turn_id) = turn_id {
        meta.with_turn_id(turn_id.clone())
    } else {
        meta
    };
    session.next_event_sequence = session.next_event_sequence.saturating_add(1);

    let event = builder(meta);
    session_store
        .append_events(&session.id, &[PersistedEvent::new(event.clone())])
        .await
        .map_err(|error| {
            CoreError::invalid_state(
                format!("failed to persist event: {error}"),
                Some(json!({
                    "session_id": session.id.to_string(),
                    "sequence": event.sequence(),
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })?;
    log.push(event.clone());
    let _ = broadcast.send(event.clone());
    if let Some(stream_tx) = stream_tx.as_mut() {
        let _ = stream_tx.send(Ok(event.clone()));
    }
    Ok(event)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arky_hooks::HookChain;
    use arky_protocol::{
        AgentEvent,
        EventMetadata,
        Message,
        ModelRef,
        ProviderId,
        ProviderSettings,
        ToolCall,
        ToolContent,
        ToolResult,
    };
    use arky_provider::{
        Provider,
        ProviderCapabilities,
        ProviderDescriptor,
        ProviderError,
        ProviderEventStream,
        ProviderFamily,
        ProviderRequest,
    };
    use arky_session::InMemorySessionStore;
    use arky_tools::{
        Tool,
        ToolDescriptor,
        ToolOrigin,
        ToolRegistry,
    };
    use async_trait::async_trait;
    use futures::stream;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::sync::broadcast;
    use tokio_util::sync::CancellationToken;

    use super::{
        SessionState,
        TurnControl,
        TurnRuntime,
        run_turn,
    };
    use crate::{
        CoreError,
        replay::create_session,
    };

    struct StaticProvider {
        descriptor: ProviderDescriptor,
        stream_factory: Arc<
            dyn Fn(ProviderRequest) -> Result<ProviderEventStream, ProviderError>
                + Send
                + Sync,
        >,
    }

    impl StaticProvider {
        fn new<F>(stream_factory: F) -> Self
        where
            F: Fn(ProviderRequest) -> Result<ProviderEventStream, ProviderError>
                + Send
                + Sync
                + 'static,
        {
            Self {
                descriptor: ProviderDescriptor::new(
                    ProviderId::new("turn-test"),
                    ProviderFamily::Custom("turn-test".to_owned()),
                    ProviderCapabilities::new()
                        .with_streaming(true)
                        .with_generate(true),
                ),
                stream_factory: Arc::new(stream_factory),
            }
        }
    }

    #[async_trait]
    impl Provider for StaticProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        async fn stream(
            &self,
            request: ProviderRequest,
        ) -> Result<ProviderEventStream, ProviderError> {
            (self.stream_factory)(request)
        }
    }

    struct TempTool;

    #[async_trait]
    impl Tool for TempTool {
        fn descriptor(&self) -> ToolDescriptor {
            ToolDescriptor::new(
                "mcp/test/temp",
                "Temp",
                "Temporary test tool",
                json!({ "type": "object" }),
                ToolOrigin::Local,
            )
            .expect("test descriptor should be valid")
        }

        async fn execute(
            &self,
            call: ToolCall,
            _cancel: CancellationToken,
        ) -> Result<ToolResult, arky_tools::ToolError> {
            Ok(ToolResult::success(
                call.id,
                call.name,
                vec![ToolContent::text("unused")],
            ))
        }
    }

    fn turn_runtime(provider: Arc<dyn Provider>) -> Arc<TurnRuntime> {
        let (events, _) = broadcast::channel(32);
        Arc::new(TurnRuntime {
            provider,
            tools: ToolRegistry::new(),
            temporary_tools: vec![Arc::new(TempTool)],
            hooks: Arc::new(HookChain::new()),
            session_store: Arc::new(InMemorySessionStore::default()),
            model: ModelRef::new("mock-model"),
            system_prompt: None,
            provider_settings: ProviderSettings::new(),
            events,
        })
    }

    async fn new_session(runtime: &TurnRuntime) -> SessionState {
        create_session(
            runtime.session_store.as_ref(),
            runtime.hooks.as_ref(),
            &runtime.model,
            &runtime.provider_settings,
        )
        .await
        .expect("session creation should succeed")
    }

    fn success_stream(request: &ProviderRequest) -> ProviderEventStream {
        Box::pin(stream::iter(vec![
            Ok(AgentEvent::MessageEnd {
                meta: EventMetadata::new(1, 1)
                    .with_session_id(
                        request
                            .session
                            .id
                            .clone()
                            .expect("request should include a session id"),
                    )
                    .with_turn_id(request.turn.id.clone())
                    .with_provider_id(ProviderId::new("turn-test")),
                message: Message::assistant("done"),
            }),
            Ok(AgentEvent::TurnEnd {
                meta: EventMetadata::new(2, 2)
                    .with_session_id(
                        request
                            .session
                            .id
                            .clone()
                            .expect("request should include a session id"),
                    )
                    .with_turn_id(request.turn.id.clone())
                    .with_provider_id(ProviderId::new("turn-test")),
                message: Message::assistant("done"),
                tool_results: Vec::new(),
            }),
        ]))
    }

    #[tokio::test]
    async fn temporary_tools_should_cleanup_after_success() {
        let runtime = turn_runtime(Arc::new(StaticProvider::new(|request| {
            Ok(success_stream(&request))
        })));
        let session = new_session(runtime.as_ref()).await;

        let result = run_turn(
            Arc::clone(&runtime),
            session,
            "hello".to_owned(),
            None,
            TurnControl {
                cancel: CancellationToken::new(),
                steering_rx: tokio::sync::mpsc::unbounded_channel().1,
                follow_up_rx: tokio::sync::mpsc::unbounded_channel().1,
            },
        )
        .await;

        assert!(result.response.is_ok());
        assert_eq!(runtime.tools.contains("mcp/test/temp"), false);
    }

    #[tokio::test]
    async fn temporary_tools_should_cleanup_after_provider_error() {
        let runtime = turn_runtime(Arc::new(StaticProvider::new(|_| {
            Err(ProviderError::stream_interrupted("boom"))
        })));
        let session = new_session(runtime.as_ref()).await;

        let result = run_turn(
            Arc::clone(&runtime),
            session,
            "hello".to_owned(),
            None,
            TurnControl {
                cancel: CancellationToken::new(),
                steering_rx: tokio::sync::mpsc::unbounded_channel().1,
                follow_up_rx: tokio::sync::mpsc::unbounded_channel().1,
            },
        )
        .await;

        assert!(result.response.is_err());
        assert_eq!(runtime.tools.contains("mcp/test/temp"), false);
    }

    #[tokio::test]
    async fn temporary_tools_should_cleanup_after_cancellation() {
        let runtime = turn_runtime(Arc::new(StaticProvider::new(|request| {
            Ok(success_stream(&request))
        })));
        let session = new_session(runtime.as_ref()).await;
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = run_turn(
            Arc::clone(&runtime),
            session,
            "hello".to_owned(),
            None,
            TurnControl {
                cancel,
                steering_rx: tokio::sync::mpsc::unbounded_channel().1,
                follow_up_rx: tokio::sync::mpsc::unbounded_channel().1,
            },
        )
        .await;

        assert!(matches!(result.response, Err(CoreError::Cancelled { .. })));
        assert_eq!(runtime.tools.contains("mcp/test/temp"), false);
    }
}
