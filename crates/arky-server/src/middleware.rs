//! Shared request parsing and middleware helpers.

use std::collections::BTreeMap;

use arky_protocol::{
    ProviderId,
    SessionId,
};
use axum::{
    extract::{
        Request,
        State,
    },
    http::{
        HeaderValue,
        Method,
        StatusCode,
        header::AUTHORIZATION,
    },
    middleware::Next,
    response::{
        IntoResponse,
        Response,
    },
};
use subtle::ConstantTimeEq;
use tower_http::cors::{
    Any,
    CorsLayer,
};

use crate::ServerError;

/// Parses a session identifier path segment.
pub fn parse_session_id(value: &str) -> Result<SessionId, ServerError> {
    SessionId::parse_str(value).map_err(|_| ServerError::invalid_session_id(value))
}

/// Parses a provider identifier path segment.
#[must_use]
pub fn parse_provider_id(value: &str) -> ProviderId {
    ProviderId::new(value)
}

/// Parses an optional unsigned integer query parameter.
pub fn parse_optional_u64(
    params: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<u64>, ServerError> {
    params
        .get(key)
        .map(|value| {
            value.parse::<u64>().map_err(|error| {
                ServerError::invalid_query(key, format!("expected u64: {error}"))
            })
        })
        .transpose()
}

/// Parses an optional positive limit query parameter.
pub fn parse_optional_limit(
    params: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<usize>, ServerError> {
    let value = params
        .get(key)
        .map(|raw| {
            raw.parse::<usize>().map_err(|error| {
                ServerError::invalid_query(key, format!("expected usize: {error}"))
            })
        })
        .transpose()?;
    if matches!(value, Some(0)) {
        return Err(ServerError::invalid_query(
            key,
            "limit must be greater than zero",
        ));
    }

    Ok(value)
}

/// Parses an optional label filter pair from query parameters.
pub fn parse_optional_label(
    params: &BTreeMap<String, String>,
) -> Result<Option<(String, String)>, ServerError> {
    let key = params.get("label_key").cloned();
    let value = params.get("label_value").cloned();
    match (key, value) {
        (Some(key), Some(value)) => Ok(Some((key, value))),
        (None, None) => Ok(None),
        _ => Err(ServerError::invalid_query(
            "label_key",
            "label_key and label_value must be supplied together",
        )),
    }
}

/// Builds the CORS layer applied to every route.
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
}

/// Extracts a bearer token from the authorization header.
#[must_use]
pub fn bearer_token(header_value: Option<&HeaderValue>) -> Option<String> {
    let header = header_value?.to_str().ok()?.trim();
    let (scheme, token) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    (!token.trim().is_empty()).then(|| token.trim().to_owned())
}

/// Compares bearer tokens using constant-time equality.
#[must_use]
pub fn timing_safe_compare(left: &str, right: &str) -> bool {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let max_len = left_bytes.len().max(right_bytes.len());
    let mut padded_left = vec![0_u8; max_len];
    let mut padded_right = vec![0_u8; max_len];
    padded_left[..left_bytes.len()].copy_from_slice(left_bytes);
    padded_right[..right_bytes.len()].copy_from_slice(right_bytes);

    let left_len = u64::try_from(left_bytes.len()).unwrap_or(u64::MAX);
    let right_len = u64::try_from(right_bytes.len()).unwrap_or(u64::MAX);
    bool::from(padded_left.ct_eq(&padded_right) & left_len.ct_eq(&right_len))
}

/// Validates bearer auth for protected server routes.
pub async fn bearer_auth(
    State(state): State<crate::ServerState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected_token) = state.auth_token() else {
        return next.run(request).await;
    };

    let actual_token = bearer_token(request.headers().get(AUTHORIZATION));
    match actual_token {
        None => (
            StatusCode::UNAUTHORIZED,
            axum::Json(crate::error::ErrorEnvelope {
                error: crate::error::ErrorBody {
                    code: "missing_token",
                    message: "Authorization header required.".to_owned(),
                    retry_after_ms: None,
                    details: None,
                },
            }),
        )
            .into_response(),
        Some(token) if timing_safe_compare(&token, expected_token) => {
            next.run(request).await
        }
        Some(_) => (
            StatusCode::FORBIDDEN,
            axum::Json(crate::error::ErrorEnvelope {
                error: crate::error::ErrorBody {
                    code: "invalid_token",
                    message: "Invalid API key.".to_owned(),
                    retry_after_ms: None,
                    details: None,
                },
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;
    use pretty_assertions::assert_eq;

    use super::{
        bearer_token,
        timing_safe_compare,
    };

    #[test]
    fn bearer_token_should_extract_case_insensitive_values() {
        let header = HeaderValue::from_static("Bearer test-token");
        let extracted = bearer_token(Some(&header));

        assert_eq!(extracted.as_deref(), Some("test-token"));

        let mixed_case = HeaderValue::from_static("bEaReR second-token");
        let extracted_mixed = bearer_token(Some(&mixed_case));

        assert_eq!(extracted_mixed.as_deref(), Some("second-token"));
    }

    #[test]
    fn timing_safe_compare_should_match_equal_and_unequal_tokens() {
        assert_eq!(timing_safe_compare("secret", "secret"), true);
        assert_eq!(timing_safe_compare("secret", "other"), false);
        assert_eq!(timing_safe_compare("secret", "secret-longer"), false);
    }
}
