//! Error types and helpers re-exported by the Arky facade.

use arky_config::ConfigError;
use arky_integrations::{
    HookError,
    McpError,
};
use arky_provider::ProviderError;
use arky_runtime::CoreError;
use arky_storage::SessionError;
use arky_tools::ToolError;
use thiserror::Error;

pub use arky_error::{
    ClassifiedError,
    ErrorLogEntry,
    HttpErrorMapping,
    classify_error,
};

/// Unified error type for the facade crate.
#[derive(Debug, Error)]
pub enum ArkyError {
    /// Error returned by the agent orchestration layer.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// Error returned by the provider abstraction or concrete providers.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// Error returned by the tool registry or tool execution flow.
    #[error(transparent)]
    Tool(#[from] ToolError),
    /// Error returned by session storage and replay handling.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// Error returned by MCP connectivity or translation layers.
    #[error(transparent)]
    Mcp(#[from] McpError),
    /// Error returned by hook execution and isolation.
    #[error(transparent)]
    Hook(#[from] HookError),
    /// Error returned by configuration loading and validation.
    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl ClassifiedError for ArkyError {
    fn error_code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.error_code(),
            Self::Provider(error) => error.error_code(),
            Self::Tool(error) => error.error_code(),
            Self::Session(error) => error.error_code(),
            Self::Mcp(error) => error.error_code(),
            Self::Hook(error) => error.error_code(),
            Self::Config(error) => error.error_code(),
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Self::Core(error) => error.is_retryable(),
            Self::Provider(error) => error.is_retryable(),
            Self::Tool(error) => error.is_retryable(),
            Self::Session(error) => error.is_retryable(),
            Self::Mcp(error) => error.is_retryable(),
            Self::Hook(error) => error.is_retryable(),
            Self::Config(error) => error.is_retryable(),
        }
    }

    fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::Core(error) => error.retry_after(),
            Self::Provider(error) => error.retry_after(),
            Self::Tool(error) => error.retry_after(),
            Self::Session(error) => error.retry_after(),
            Self::Mcp(error) => error.retry_after(),
            Self::Hook(error) => error.retry_after(),
            Self::Config(error) => error.retry_after(),
        }
    }

    fn http_status(&self) -> u16 {
        match self {
            Self::Core(error) => error.http_status(),
            Self::Provider(error) => error.http_status(),
            Self::Tool(error) => error.http_status(),
            Self::Session(error) => error.http_status(),
            Self::Mcp(error) => error.http_status(),
            Self::Hook(error) => error.http_status(),
            Self::Config(error) => error.http_status(),
        }
    }

    fn correction_context(&self) -> Option<serde_json::Value> {
        match self {
            Self::Core(error) => error.correction_context(),
            Self::Provider(error) => error.correction_context(),
            Self::Tool(error) => error.correction_context(),
            Self::Session(error) => error.correction_context(),
            Self::Mcp(error) => error.correction_context(),
            Self::Hook(error) => error.correction_context(),
            Self::Config(error) => error.correction_context(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::{
            Path,
            PathBuf,
        },
        time::Duration,
    };

    use arky_error::ClassifiedError;
    use arky_integrations::HookEvent;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::ArkyError;
    use crate::{
        ConfigError,
        CoreError,
        HookError,
        McpError,
        ProviderError,
        SessionError,
        ToolError,
        ValidationIssue,
    };

    #[test]
    fn arky_error_from_core_error_should_wrap_core_variant() {
        let error = ArkyError::from(CoreError::cancelled("cancelled"));

        let actual = matches!(
            error,
            ArkyError::Core(CoreError::Cancelled { message }) if message == "cancelled"
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_from_provider_error_should_wrap_provider_variant() {
        let error = ArkyError::from(ProviderError::auth_failed("bad credentials"));

        let actual = matches!(
            error,
            ArkyError::Provider(ProviderError::AuthFailed { message })
                if message == "bad credentials"
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_from_tool_error_should_wrap_tool_variant() {
        let error = ArkyError::from(ToolError::name_collision("mcp/fs/read_file"));

        let actual = matches!(
            error,
            ArkyError::Tool(ToolError::NameCollision { canonical_name })
                if canonical_name == "mcp/fs/read_file"
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_from_session_error_should_wrap_session_variant() {
        let session_id = crate::SessionId::new();
        let error = ArkyError::from(SessionError::expired(session_id, Some(5_000)));

        let actual = matches!(
            error,
            ArkyError::Session(SessionError::Expired {
                expired_at_ms: Some(5_000),
                ..
            })
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_from_mcp_error_should_wrap_mcp_variant() {
        let error = ArkyError::from(McpError::schema_mismatch(
            "schema mismatch",
            Some(json!({ "tool": "read_file" })),
        ));

        let actual = matches!(
            error,
            ArkyError::Mcp(McpError::SchemaMismatch { message, .. })
                if message == "schema mismatch"
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_from_hook_error_should_wrap_hook_variant() {
        let error = ArkyError::from(HookError::execution_failed(
            "hook failed",
            Some(HookEvent::BeforeToolCall),
            Some("policy".to_owned()),
        ));

        let actual = matches!(
            error,
            ArkyError::Hook(HookError::ExecutionFailed {
                message,
                hook_name,
                ..
            }) if message == "hook failed" && hook_name.as_deref() == Some("policy")
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_from_config_error_should_wrap_config_variant() {
        let error = ArkyError::from(ConfigError::NotFound {
            path: PathBuf::from("arky.toml"),
        });

        let actual = matches!(
            error,
            ArkyError::Config(ConfigError::NotFound { path })
                if path == Path::new("arky.toml")
        );

        assert_eq!(actual, true);
    }

    #[test]
    fn arky_error_should_delegate_classification_to_inner_error() {
        let retry_after = Duration::from_secs(5);
        let error = ArkyError::from(ProviderError::rate_limited(
            "provider rate limited",
            Some(retry_after),
        ));

        let actual = (
            error.error_code(),
            error.is_retryable(),
            error.retry_after(),
            error.http_status(),
            error.correction_context(),
        );
        let expected = (
            "PROVIDER_RATE_LIMITED",
            true,
            Some(retry_after),
            429,
            Some(json!({ "retry_after_ms": retry_after.as_millis() })),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn arky_error_should_delegate_context_for_non_retryable_variants() {
        let details = vec![ValidationIssue::new(
            "providers.default.kind",
            "is required",
        )];
        let error = ArkyError::from(ConfigError::ValidationFailed {
            message: "configuration validation failed with 1 issue".to_owned(),
            issues: details,
        });

        let actual = (error.error_code(), error.http_status());
        let expected = ("CONFIG_VALIDATION_FAILED", 422);

        assert_eq!(actual, expected);
    }
}
