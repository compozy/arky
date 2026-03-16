//! Session inspection routes.

use std::collections::BTreeMap;

use arky_protocol::{
    Message,
    SessionId,
};
use arky_session::{
    SessionFilter,
    SessionMetadata,
    SessionSnapshot,
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
        parse_optional_label,
        parse_optional_limit,
        parse_optional_u64,
        parse_session_id,
    },
};

/// Response returned by `GET /sessions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionListResponse {
    /// Matching session summaries.
    pub sessions: Vec<SessionMetadata>,
}

/// Response returned by `GET /sessions/:id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionDetailResponse {
    /// Full session snapshot.
    pub session: SessionSnapshot,
}

/// Response returned by `GET /sessions/:id/messages`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionMessagesResponse {
    /// Target session identifier.
    pub session_id: SessionId,
    /// Full stored transcript.
    pub messages: Vec<Message>,
}

/// Lists sessions with optional filtering.
pub async fn list_sessions(
    State(state): State<ServerState>,
    Query(params): Query<BTreeMap<String, String>>,
) -> Result<Json<SessionListResponse>, ServerError> {
    let sessions = state
        .session_store()
        .list(SessionFilter {
            label: parse_optional_label(&params)?,
            since: parse_optional_u64(&params, "since")?,
            limit: parse_optional_limit(&params, "limit")?,
        })
        .await?;

    Ok(Json(SessionListResponse { sessions }))
}

/// Loads one session snapshot.
pub async fn get_session(
    State(state): State<ServerState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionDetailResponse>, ServerError> {
    let session_id = parse_session_id(&session_id)?;
    let session = state.session_store().load(&session_id).await?;

    Ok(Json(SessionDetailResponse { session }))
}

/// Loads the stored transcript for one session.
pub async fn get_session_messages(
    State(state): State<ServerState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionMessagesResponse>, ServerError> {
    let session_id = parse_session_id(&session_id)?;
    let session = state.session_store().load(&session_id).await?;

    Ok(Json(SessionMessagesResponse {
        session_id,
        messages: session.messages,
    }))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::Arc,
    };

    use arky_core::Agent;
    use arky_session::{
        InMemorySessionStore,
        NewSession,
        SessionFilter,
        SessionStore,
    };
    use axum::{
        Json,
        extract::{
            Path,
            Query,
            State,
        },
        response::IntoResponse,
    };
    use pretty_assertions::assert_eq;

    use super::{
        SessionDetailResponse,
        SessionListResponse,
        SessionMessagesResponse,
        get_session,
        get_session_messages,
        list_sessions,
    };
    use crate::ServerState;

    fn state() -> (ServerState, Arc<InMemorySessionStore>) {
        let store = Arc::new(InMemorySessionStore::default());
        let agent = Arc::new(
            Agent::builder()
                .provider_arc(Arc::new(crate::tests::StaticProvider::new()))
                .session_store_arc(store.clone())
                .model("mock-model")
                .build()
                .expect("agent should build"),
        );

        (ServerState::new(agent, store.clone()), store)
    }

    #[tokio::test]
    async fn session_list_should_return_sessions_from_store() {
        let (state, store) = state();
        let session_id = store
            .create(NewSession::default())
            .await
            .expect("session should be created");

        let Json(response) = list_sessions(State(state), Query(BTreeMap::new()))
            .await
            .expect("list should succeed");

        assert_eq!(
            response,
            SessionListResponse {
                sessions: vec![
                    store
                        .list(SessionFilter::default())
                        .await
                        .expect("sessions should list")
                        .into_iter()
                        .find(|session| session.id == session_id)
                        .expect("session should exist")
                ],
            }
        );
    }

    #[tokio::test]
    async fn session_detail_should_return_not_found_for_unknown_session() {
        let (state, _store) = state();
        let response = get_session(
            State(state),
            Path(arky_protocol::SessionId::new().to_string()),
        )
        .await
        .expect_err("missing session should fail")
        .into_response();

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn session_messages_should_return_full_transcript() {
        let (state, store) = state();
        let session_id = store
            .create(NewSession::default())
            .await
            .expect("session should be created");
        store
            .append_messages(
                &session_id,
                &[
                    arky_protocol::Message::user("hello"),
                    arky_protocol::Message::assistant("world"),
                ],
            )
            .await
            .expect("messages should append");

        let Json(response) =
            get_session_messages(State(state.clone()), Path(session_id.to_string()))
                .await
                .expect("messages should load");
        let Json(detail) = get_session(State(state), Path(session_id.to_string()))
            .await
            .expect("session should load");

        assert_eq!(
            response,
            SessionMessagesResponse {
                session_id: session_id.clone(),
                messages: vec![
                    arky_protocol::Message::user("hello"),
                    arky_protocol::Message::assistant("world"),
                ],
            }
        );
        assert_eq!(
            detail,
            SessionDetailResponse {
                session: store.load(&session_id).await.expect("snapshot should load")
            }
        );
    }
}
