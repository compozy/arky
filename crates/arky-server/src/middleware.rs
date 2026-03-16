//! Shared request parsing and middleware helpers.

use std::collections::BTreeMap;

use arky_protocol::{
    ProviderId,
    SessionId,
};
use axum::http::Method;
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
        .allow_methods([Method::GET, Method::OPTIONS])
}
