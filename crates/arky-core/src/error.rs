//! Core orchestration errors.

use arky_error::ClassifiedError;
use arky_protocol::SessionId;
use serde_json::{
    Value,
    json,
};
use thiserror::Error;

/// Errors produced by the agent orchestration layer.
#[derive(Debug, Clone, Error)]
pub enum CoreError {
    /// Another turn is already active for the same session.
    #[error("session `{session_id}` is already processing `{operation}`")]
    BusySession {
        /// Session that rejected the overlapping operation.
        session_id: SessionId,
        /// User-facing operation label.
        operation: &'static str,
    },
    /// The active turn was cancelled explicitly.
    #[error("{message}")]
    Cancelled {
        /// Human-readable cancellation detail.
        message: String,
    },
    /// The requested operation is not valid in the current runtime state.
    #[error("{message}")]
    InvalidState {
        /// Human-readable failure detail.
        message: String,
        /// Optional structured context.
        details: Option<Value>,
    },
    /// Session resume or replay restoration failed.
    #[error("{message}")]
    ReplayFailed {
        /// Human-readable replay failure detail.
        message: String,
        /// Optional structured context.
        details: Option<Value>,
    },
}

impl CoreError {
    /// Creates a busy-session error.
    #[must_use]
    pub const fn busy_session(session_id: SessionId, operation: &'static str) -> Self {
        Self::BusySession {
            session_id,
            operation,
        }
    }

    /// Creates a cancelled error.
    #[must_use]
    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::Cancelled {
            message: message.into(),
        }
    }

    /// Creates an invalid-state error.
    #[must_use]
    pub fn invalid_state(message: impl Into<String>, details: Option<Value>) -> Self {
        Self::InvalidState {
            message: message.into(),
            details,
        }
    }

    /// Creates a replay failure.
    #[must_use]
    pub fn replay_failed(message: impl Into<String>, details: Option<Value>) -> Self {
        Self::ReplayFailed {
            message: message.into(),
            details,
        }
    }
}

impl ClassifiedError for CoreError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::BusySession { .. } => "CORE_BUSY_SESSION",
            Self::Cancelled { .. } => "CORE_CANCELLED",
            Self::InvalidState { .. } => "CORE_INVALID_STATE",
            Self::ReplayFailed { .. } => "CORE_REPLAY_FAILED",
        }
    }

    fn http_status(&self) -> u16 {
        match self {
            Self::Cancelled { .. } => 499,
            Self::BusySession { .. } | Self::InvalidState { .. } => 409,
            Self::ReplayFailed { .. } => 422,
        }
    }

    fn correction_context(&self) -> Option<Value> {
        match self {
            Self::BusySession {
                session_id,
                operation,
            } => Some(json!({
                "session_id": session_id.to_string(),
                "operation": operation,
            })),
            Self::Cancelled { message } => Some(json!({
                "message": message,
            })),
            Self::InvalidState { message, details }
            | Self::ReplayFailed { message, details } => Some(
                details
                    .clone()
                    .unwrap_or_else(|| json!({ "message": message })),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use arky_error::ClassifiedError;
    use pretty_assertions::assert_eq;

    use super::CoreError;
    use arky_protocol::SessionId;

    #[test]
    fn core_error_classification_should_match_contract() {
        let session_id = SessionId::new();
        let cases = vec![
            (
                CoreError::busy_session(session_id, "prompt"),
                "CORE_BUSY_SESSION",
                409,
            ),
            (
                CoreError::cancelled("request cancelled"),
                "CORE_CANCELLED",
                499,
            ),
            (
                CoreError::invalid_state("missing active turn", None),
                "CORE_INVALID_STATE",
                409,
            ),
            (
                CoreError::replay_failed("checkpoint unavailable", None),
                "CORE_REPLAY_FAILED",
                422,
            ),
        ];

        for (error, expected_code, expected_status) in cases {
            let actual = (
                error.error_code(),
                error.http_status(),
                error.correction_context().is_some(),
            );
            let expected = (expected_code, expected_status, true);

            assert_eq!(actual, expected);
        }
    }
}
