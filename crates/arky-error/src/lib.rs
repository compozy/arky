//! Shared error contracts for Arky.
//!
//! This crate stays at the bottom of the internal dependency graph so every
//! other crate can implement common error-classification behavior without
//! introducing cycles through `arky-core`.

/// Classification metadata shared by all Arky library errors.
pub trait ClassifiedError: std::error::Error + Send + Sync {
    /// Stable machine-readable error code.
    fn error_code(&self) -> &'static str;

    /// Whether the operation can be retried safely.
    fn is_retryable(&self) -> bool {
        false
    }

    /// Recommended delay before retrying, when applicable.
    fn retry_after(&self) -> Option<std::time::Duration> {
        None
    }

    /// HTTP-style status code for server and transport mappings.
    fn http_status(&self) -> u16 {
        500
    }

    /// Optional structured data that can help higher-level recovery logic.
    fn correction_context(&self) -> Option<serde_json::Value> {
        None
    }
}
