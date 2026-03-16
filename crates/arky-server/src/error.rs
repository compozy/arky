//! HTTP-facing error mapping for the runtime server.

use std::{
    io,
    time::Duration,
};

use arky_core::CoreError;
use arky_error::{
    ClassifiedError,
    HttpErrorMapping,
};
use arky_protocol::ProviderId;
use arky_session::SessionError;
use serde::Serialize;
use serde_json::{
    Value,
    json,
};
use thiserror::Error;

/// Errors returned by the runtime server surface.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Wrapped core runtime error.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Wrapped session persistence error.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// Session path parameter could not be parsed.
    #[error("invalid session id `{value}`")]
    InvalidSessionId {
        /// Raw invalid value.
        value: String,
    },
    /// Request query validation failed.
    #[error("invalid `{field}` query parameter: {message}")]
    InvalidQuery {
        /// Invalid field name.
        field: &'static str,
        /// Human-readable validation failure.
        message: String,
    },
    /// Request body validation failed.
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Human-readable validation failure.
        message: String,
    },
    /// Provider health was requested for an unknown provider.
    #[error("provider health for `{provider_id}` was not found")]
    ProviderHealthNotFound {
        /// Missing provider identifier.
        provider_id: ProviderId,
    },
    /// Generic internal runtime-server failure.
    #[error("{message}")]
    Internal {
        /// Human-readable detail.
        message: String,
        /// Optional structured context.
        details: Option<Value>,
    },
}

impl ServerError {
    /// Creates an invalid-session-id error.
    #[must_use]
    pub fn invalid_session_id(value: impl Into<String>) -> Self {
        Self::InvalidSessionId {
            value: value.into(),
        }
    }

    /// Creates a query validation error.
    #[must_use]
    pub fn invalid_query(field: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidQuery {
            field,
            message: message.into(),
        }
    }

    /// Creates a request validation error.
    #[must_use]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    /// Creates a provider-health not-found error.
    #[must_use]
    pub const fn provider_health_not_found(provider_id: ProviderId) -> Self {
        Self::ProviderHealthNotFound { provider_id }
    }

    /// Creates an internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            details: None,
        }
    }

    /// Creates an internal I/O error wrapper.
    #[must_use]
    pub fn io(error: &io::Error) -> Self {
        Self::Internal {
            message: format!("i/o failure: {error}"),
            details: Some(json!({
                "kind": error.kind().to_string(),
            })),
        }
    }
}

impl ClassifiedError for ServerError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.error_code(),
            Self::Session(error) => error.error_code(),
            Self::InvalidSessionId { .. } => "SERVER_INVALID_SESSION_ID",
            Self::InvalidQuery { .. } => "SERVER_INVALID_QUERY",
            Self::InvalidRequest { .. } => "SERVER_INVALID_REQUEST",
            Self::ProviderHealthNotFound { .. } => "SERVER_PROVIDER_HEALTH_NOT_FOUND",
            Self::Internal { .. } => "SERVER_INTERNAL",
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Self::Core(error) => error.is_retryable(),
            Self::Session(error) => error.is_retryable(),
            Self::InvalidSessionId { .. }
            | Self::InvalidQuery { .. }
            | Self::InvalidRequest { .. }
            | Self::ProviderHealthNotFound { .. }
            | Self::Internal { .. } => false,
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Core(error) => error.retry_after(),
            Self::Session(error) => error.retry_after(),
            Self::InvalidSessionId { .. }
            | Self::InvalidQuery { .. }
            | Self::InvalidRequest { .. }
            | Self::ProviderHealthNotFound { .. }
            | Self::Internal { .. } => None,
        }
    }

    fn http_status(&self) -> u16 {
        match self {
            Self::Core(error) => error.http_status(),
            Self::Session(error) => error.http_status(),
            Self::InvalidSessionId { .. }
            | Self::InvalidQuery { .. }
            | Self::InvalidRequest { .. } => 400,
            Self::ProviderHealthNotFound { .. } => 404,
            Self::Internal { .. } => 500,
        }
    }

    fn correction_context(&self) -> Option<Value> {
        match self {
            Self::Core(error) => error.correction_context(),
            Self::Session(error) => error.correction_context(),
            Self::InvalidSessionId { value } => Some(json!({
                "value": value,
            })),
            Self::InvalidQuery { field, message } => Some(json!({
                "field": field,
                "message": message,
            })),
            Self::InvalidRequest { message } => Some(json!({
                "message": message,
            })),
            Self::ProviderHealthNotFound { provider_id } => Some(json!({
                "provider_id": provider_id.as_str(),
            })),
            Self::Internal { details, .. } => details.clone(),
        }
    }
}

/// JSON error envelope returned by HTTP handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorEnvelope {
    /// Wrapped error payload.
    pub error: ErrorBody,
}

/// JSON error payload returned by HTTP handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Human-readable error detail.
    pub message: String,
    /// Suggested retry delay, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    /// Structured correction context for capable clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ErrorEnvelope {
    /// Builds a JSON envelope from a classified error.
    #[must_use]
    pub fn from_classified<E>(error: &E) -> Self
    where
        E: ClassifiedError + ?Sized,
    {
        let mapping = HttpErrorMapping::from_error(error);

        Self {
            error: ErrorBody {
                code: mapping.error_code,
                message: mapping.message,
                retry_after_ms: mapping.retry_after.map(duration_to_millis),
                details: mapping.correction_context,
            },
        }
    }
}

fn duration_to_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(feature = "server")]
impl axum::response::IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        use axum::{
            Json,
            http::{
                HeaderMap,
                HeaderValue,
                StatusCode,
                header::RETRY_AFTER,
            },
        };

        let mapping = HttpErrorMapping::from_error(&self);
        let status = StatusCode::from_u16(mapping.status)
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let envelope = ErrorEnvelope::from_classified(&self);
        let mut headers = HeaderMap::new();
        if let Some(retry_after) = mapping.retry_after {
            let seconds = retry_after.as_secs().max(1).to_string();
            if let Ok(value) = HeaderValue::from_str(&seconds) {
                headers.insert(RETRY_AFTER, value);
            }
        }

        (status, headers, Json(envelope)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use pretty_assertions::assert_eq;

    use super::ServerError;
    use arky_protocol::SessionId;
    use arky_session::SessionError;

    #[tokio::test]
    async fn error_formatting_should_map_classified_errors_to_json() {
        let response = ServerError::from(SessionError::NotFound {
            session_id: SessionId::new(),
        })
        .into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).expect("error body should deserialize");

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "SESSION_NOT_FOUND");
    }
}
