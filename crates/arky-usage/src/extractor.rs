//! Provider metadata extraction contracts.

use serde_json::Value;

/// Normalized provider metadata emitted alongside usage.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProviderMetadata {
    /// Provider-native session identifier.
    pub session_id: Option<String>,
    /// Estimated or provider-reported USD cost.
    pub cost_usd: Option<f64>,
    /// Total runtime in milliseconds.
    pub duration_ms: Option<f64>,
    /// Raw provider usage payload.
    pub raw_usage: Option<Value>,
    /// Non-fatal warnings encountered while extracting metadata.
    pub warnings: Vec<String>,
}

/// Extracts provider metadata from raw provider payloads.
pub trait ProviderMetadataExtractor: Send + Sync {
    /// Extracts normalized metadata from a raw JSON payload.
    fn extract_metadata(&self, raw: &Value) -> ProviderMetadata;
}
