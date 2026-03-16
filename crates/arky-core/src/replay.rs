//! Session restore helpers for the agent runtime.

use arky_hooks::{
    Hooks,
    SessionStartContext,
    SessionStartSource,
};
use arky_protocol::{
    Message,
    ModelRef,
    ProviderSettings,
    ReplayCursor,
    SessionId,
    SessionRef,
    TurnCheckpoint,
};
use arky_session::{
    NewSession,
    SessionSnapshot,
    SessionStore,
};
use serde_json::{
    Value,
    json,
};
use tokio_util::sync::CancellationToken;

use crate::CoreError;

#[derive(Debug, Clone)]
pub struct SessionState {
    pub id: SessionId,
    pub messages: Vec<Message>,
    pub replay_cursor: Option<ReplayCursor>,
    pub last_checkpoint: Option<TurnCheckpoint>,
    pub provider_session_id: Option<String>,
    pub next_event_sequence: u64,
    pub next_turn_sequence: u64,
}

impl SessionState {
    pub fn session_ref(&self) -> SessionRef {
        let mut session = SessionRef::new(Some(self.id.clone()));
        if let Some(provider_session_id) = &self.provider_session_id {
            session = session.with_provider_session_id(provider_session_id.clone());
        }
        if let Some(replay_cursor) = &self.replay_cursor {
            session = session.with_replay_cursor(replay_cursor.clone());
        }
        session
    }
}

pub async fn create_session(
    session_store: &dyn SessionStore,
    hooks: &dyn Hooks,
    model: &ModelRef,
    settings: &ProviderSettings,
) -> Result<SessionState, CoreError> {
    let session_id = session_store
        .create(NewSession {
            model_id: Some(model.model_id.clone()),
            ..NewSession::default()
        })
        .await
        .map_err(|error| {
            CoreError::invalid_state(
                format!("failed to create session: {error}"),
                Some(json!({
                    "operation": "create",
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })?;

    let mut state = SessionState {
        id: session_id,
        messages: Vec::new(),
        replay_cursor: None,
        last_checkpoint: None,
        provider_session_id: None,
        next_event_sequence: 1,
        next_turn_sequence: 1,
    };

    apply_session_start(
        &mut state,
        session_store,
        hooks,
        SessionStartSource::Startup,
        model,
        settings,
    )
    .await?;

    Ok(state)
}

pub async fn restore_session(
    session_store: &dyn SessionStore,
    hooks: &dyn Hooks,
    session_id: &SessionId,
    model: &ModelRef,
    settings: &ProviderSettings,
) -> Result<SessionState, CoreError> {
    let snapshot = session_store.load(session_id).await.map_err(|error| {
        CoreError::replay_failed(
            format!("failed to load session `{session_id}`: {error}"),
            Some(json!({
                "session_id": session_id.to_string(),
                "error_code": arky_error::ClassifiedError::error_code(&error),
            })),
        )
    })?;
    let mut state = session_state_from_snapshot(snapshot);

    apply_session_start(
        &mut state,
        session_store,
        hooks,
        SessionStartSource::Resume,
        model,
        settings,
    )
    .await?;

    Ok(state)
}

#[expect(
    clippy::option_if_let_else,
    reason = "workspace lint policy disallows map_or/map_or_else for Option handling"
)]
fn session_state_from_snapshot(snapshot: SessionSnapshot) -> SessionState {
    let provider_session_id = snapshot
        .last_checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.provider_session_id.clone())
        .or_else(|| snapshot.metadata.provider_session_id.clone());
    let last_sequence = snapshot.metadata.last_sequence.unwrap_or(0);
    let next_event_floor = last_sequence.saturating_add(1);
    let next_event_sequence = if let Some(cursor) = snapshot.replay_cursor.as_ref() {
        cursor.next_sequence.max(next_event_floor)
    } else {
        next_event_floor
    };
    let next_turn_sequence = if let Some(checkpoint) = snapshot.last_checkpoint.as_ref() {
        checkpoint.sequence.saturating_add(1)
    } else {
        1
    };

    SessionState {
        id: snapshot.metadata.id,
        messages: snapshot.messages,
        replay_cursor: snapshot.replay_cursor,
        last_checkpoint: snapshot.last_checkpoint,
        provider_session_id,
        next_event_sequence,
        next_turn_sequence,
    }
}

async fn apply_session_start(
    state: &mut SessionState,
    session_store: &dyn SessionStore,
    hooks: &dyn Hooks,
    source: SessionStartSource,
    model: &ModelRef,
    settings: &ProviderSettings,
) -> Result<(), CoreError> {
    let settings = provider_settings_json(settings);
    let context = SessionStartContext::new(state.session_ref(), source)
        .with_settings(settings)
        .with_messages(state.messages.clone());
    let update = hooks
        .session_start(&context, CancellationToken::new())
        .await
        .map_err(|error| {
            CoreError::invalid_state(
                format!("session_start hook failed: {error}"),
                Some(json!({
                    "session_id": state.id.to_string(),
                    "source": source,
                    "model_id": model.model_id.as_str(),
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })?;

    let Some(update) = update else {
        return Ok(());
    };
    if update.messages.is_empty() {
        return Ok(());
    }

    state.messages.extend(update.messages.clone());
    session_store
        .append_messages(&state.id, &update.messages)
        .await
        .map_err(|error| {
            CoreError::invalid_state(
                format!("failed to persist session-start messages: {error}"),
                Some(json!({
                    "session_id": state.id.to_string(),
                    "error_code": arky_error::ClassifiedError::error_code(&error),
                })),
            )
        })
}

fn provider_settings_json(
    settings: &ProviderSettings,
) -> std::collections::BTreeMap<String, Value> {
    let mut values = std::collections::BTreeMap::new();
    if let Some(temperature) = settings.temperature {
        values.insert("temperature".to_owned(), json!(temperature));
    }
    if let Some(max_tokens) = settings.max_tokens {
        values.insert("max_tokens".to_owned(), json!(max_tokens));
    }
    if !settings.stop_sequences.is_empty() {
        values.insert("stop_sequences".to_owned(), json!(settings.stop_sequences));
    }
    values.extend(settings.extra.clone());
    values
}
