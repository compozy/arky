//! OpenAI-compatible model-listing route.

use axum::{
    extract::State,
    response::Json,
};
use serde::Serialize;

use crate::ServerState;

/// OpenAI-compatible model list payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelListResponse {
    /// Collection envelope discriminator.
    pub object: &'static str,
    /// Listed models.
    pub data: Vec<ModelResponse>,
}

/// One OpenAI-compatible model entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelResponse {
    /// Model identifier.
    pub id: String,
    /// Object discriminator.
    pub object: &'static str,
    /// Creation timestamp.
    pub created: u64,
    /// Provider owner or family label.
    pub owned_by: String,
    /// Extended Arky metadata.
    pub compozy: ModelCompozyResponse,
}

/// Extended Arky model metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelCompozyResponse {
    /// Provider identifier.
    pub provider_id: String,
    /// Optional display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional context window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Optional max output tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    /// Optional tools support flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    /// Optional reasoning support flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning: Option<bool>,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Lists the configured models.
pub async fn list_models(State(state): State<ServerState>) -> Json<ModelListResponse> {
    let models = state.models().await;
    let data = models
        .into_iter()
        .map(|model| ModelResponse {
            id: model.id,
            object: "model",
            created: model.created,
            owned_by: model.owned_by,
            compozy: ModelCompozyResponse {
                provider_id: model.provider_id.to_string(),
                display_name: model.display_name,
                context_window: model.context_window,
                max_output_tokens: model.max_output_tokens,
                supports_tools: model.supports_tools,
                supports_reasoning: model.supports_reasoning,
                description: model.description,
            },
        })
        .collect();

    Json(ModelListResponse {
        object: "list",
        data,
    })
}
