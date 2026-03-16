//! Health and provider-state routes.

use axum::{
    Json,
    extract::{
        Path,
        State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;

use crate::{
    ServerError,
    ServerState,
    middleware::parse_provider_id,
    state::ProviderHealthSnapshot,
};

/// Response returned by `GET /health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthResponse {
    /// Server status.
    pub status: &'static str,
}

/// Response returned by `GET /ready`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadyResponse {
    /// Readiness status.
    pub status: &'static str,
    /// Optional reason when not ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Provider-health list response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderHealthListResponse {
    /// All known provider snapshots.
    pub providers: Vec<ProviderHealthSnapshot>,
}

/// Provider-health detail response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderHealthResponse {
    /// One provider snapshot.
    pub provider: ProviderHealthSnapshot,
}

/// Returns the liveness response.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Returns readiness based on runtime health state.
pub async fn ready(State(state): State<ServerState>) -> impl IntoResponse {
    let readiness = state.health().readiness().await;
    let body = ReadyResponse {
        status: if readiness.ready {
            "ready"
        } else {
            "not_ready"
        },
        reason: readiness.reason,
    };
    let status = if readiness.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(body))
}

/// Lists provider health snapshots.
pub async fn list_provider_health(
    State(state): State<ServerState>,
) -> Json<ProviderHealthListResponse> {
    let providers = state.health().provider_health_list().await;
    Json(ProviderHealthListResponse { providers })
}

/// Returns health for one provider.
pub async fn get_provider_health(
    State(state): State<ServerState>,
    Path(provider_id): Path<String>,
) -> Result<Json<ProviderHealthResponse>, ServerError> {
    let provider_id = parse_provider_id(&provider_id);
    let Some(provider) = state.health().provider_health(&provider_id).await else {
        return Err(ServerError::provider_health_not_found(provider_id));
    };

    Ok(Json(ProviderHealthResponse { provider }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arky_core::Agent;
    use arky_protocol::ProviderId;
    use arky_session::InMemorySessionStore;
    use axum::{
        Json,
        extract::{
            Path,
            State,
        },
        response::IntoResponse,
    };
    use pretty_assertions::assert_eq;

    use super::{
        HealthResponse,
        ProviderHealthListResponse,
        get_provider_health,
        health,
        list_provider_health,
        ready,
    };
    use crate::{
        ServerState,
        state::ProviderHealthSnapshot,
    };

    fn state() -> ServerState {
        let store = Arc::new(InMemorySessionStore::default());
        let agent = Arc::new(
            Agent::builder()
                .provider_arc(Arc::new(crate::test_support::StaticProvider::new()))
                .session_store_arc(store.clone())
                .model("mock-model")
                .build()
                .expect("agent should build"),
        );

        ServerState::new(agent, store)
    }

    #[tokio::test]
    async fn health_endpoint_should_return_ok_payload() {
        let Json(body) = health().await;

        assert_eq!(body, HealthResponse { status: "ok" });
    }

    #[tokio::test]
    async fn ready_endpoint_should_return_ok_when_runtime_is_ready() {
        let response = ready(State(state())).await.into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).expect("body should deserialize");

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            body,
            serde_json::json!({
                "status": "ready",
            })
        );
    }

    #[tokio::test]
    async fn ready_endpoint_should_return_service_unavailable_when_not_ready() {
        let state = state();
        state.health().set_not_ready("provider warming up").await;

        let response = ready(State(state)).await.into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).expect("body should deserialize");

        assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body,
            serde_json::json!({
                "status": "not_ready",
                "reason": "provider warming up",
            })
        );
    }

    #[tokio::test]
    async fn provider_health_routes_should_return_snapshots() {
        let state = state();
        state
            .health()
            .set_provider_health(ProviderHealthSnapshot::healthy(ProviderId::new(
                "codex",
            )))
            .await;

        let Json(list) = list_provider_health(State(state.clone())).await;
        let Json(detail) = get_provider_health(State(state), Path("codex".to_owned()))
            .await
            .expect("provider should exist");

        assert_eq!(
            list,
            ProviderHealthListResponse {
                providers: vec![ProviderHealthSnapshot::healthy(ProviderId::new(
                    "codex",
                ))],
            }
        );
        assert_eq!(detail.provider.provider_id.as_str(), "codex");
    }
}
