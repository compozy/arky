//! Historical replay routes.

use std::collections::BTreeMap;

use arky_protocol::{
    PersistedEvent,
    SessionId,
};
use axum::{
    Json,
    extract::{
        Path,
        Query,
        State,
    },
};
use serde::Serialize;

use crate::{
    ServerError,
    ServerState,
    middleware::{
        parse_optional_limit,
        parse_optional_u64,
        parse_session_id,
    },
};

/// Response returned by `GET /sessions/:id/replay`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionReplayResponse {
    /// Target session identifier.
    pub session_id: SessionId,
    /// Historical persisted events.
    pub events: Vec<PersistedEvent>,
}

/// Loads historical replay events for one session.
pub async fn replay_session(
    State(state): State<ServerState>,
    Path(session_id): Path<String>,
    Query(params): Query<BTreeMap<String, String>>,
) -> Result<Json<SessionReplayResponse>, ServerError> {
    let session_id = parse_session_id(&session_id)?;
    let events = state
        .session_store()
        .replay_events(
            &session_id,
            parse_optional_u64(&params, "after_sequence")?,
            parse_optional_limit(&params, "limit")?,
        )
        .await?;

    Ok(Json(SessionReplayResponse { session_id, events }))
}
