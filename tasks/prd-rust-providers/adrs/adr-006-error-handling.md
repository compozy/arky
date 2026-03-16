# ADR-006: Error Handling with Per-Crate thiserror Enums + Common Error Trait

## Status

Accepted

## Date

2026-03-15

## Porting Context

This ADR uses the TypeScript provider stack in `../compozy-code/providers` as
upstream reference material. Use `../porting-reference.md` to find the closest
packages and files, but prefer the Rust decision recorded here when it
intentionally improves on the upstream design.

## Context

The SDK spans multiple crates, each with distinct failure modes. We need a consistent error handling strategy that:

- Allows each crate to define domain-specific errors
- Supports error classification (retryability, severity) for agent self-correction
- Follows Rust ecosystem conventions (`thiserror` for libraries)
- Enables error propagation via `?` across crate boundaries
- Mirrors the error classification system from our TypeScript `providers/core` (ErrorClassifier with retryability, structured context extraction)

Rust skill guidelines:

- `rust-best-practices ch.4`: "Use `thiserror` for library errors, `anyhow` for binaries only"
- `rust-engineer/error-handling.md`: "Use `#[from]` in thiserror for automatic conversions"
- `rust-coding-guidelines`: "Use `?` propagation, not `try!()` macro"

## Decision

Use **per-crate `thiserror` enums** plus a shared **`ClassifiedError` trait** that enables error classification across all crate boundaries.

### Common error trait (crate `arky-error`)

```rust
/// Classification metadata for all SDK errors.
/// Enables agent-level error handling decisions (retry, abort, self-correct).
pub trait ClassifiedError: std::error::Error + Send + Sync {
    /// Whether this error is transient and the operation can be retried
    fn is_retryable(&self) -> bool { false }

    /// Suggested retry delay, if retryable
    fn retry_after(&self) -> Option<Duration> { None }

    /// Machine-readable error code for programmatic handling
    fn error_code(&self) -> &str;

    /// HTTP-equivalent status code (for server layer mapping)
    fn http_status(&self) -> u16 { 500 }

    /// Structured context for agent self-correction
    fn correction_context(&self) -> Option<serde_json::Value> { None }
}
```

### Per-crate error enums

```rust
// crates/arky-provider/src/error.rs
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider not found: {provider_id}")]
    NotFound { provider_id: String },

    #[error("Stream interrupted: {reason}")]
    StreamInterrupted { reason: String, is_retryable: bool },

    #[error("Authentication failed for provider {provider_id}")]
    AuthFailed { provider_id: String },

    #[error("Rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },

    #[error(transparent)]
    Tool(#[from] ToolError),
}

impl ClassifiedError for ProviderError {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::RateLimited { .. } | Self::StreamInterrupted { is_retryable: true, .. })
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }

    fn error_code(&self) -> &str {
        match self {
            Self::NotFound { .. } => "PROVIDER_NOT_FOUND",
            Self::StreamInterrupted { .. } => "STREAM_INTERRUPTED",
            Self::AuthFailed { .. } => "AUTH_FAILED",
            Self::RateLimited { .. } => "RATE_LIMITED",
            Self::Tool(e) => e.error_code(),
        }
    }

    fn http_status(&self) -> u16 {
        match self {
            Self::NotFound { .. } => 404,
            Self::AuthFailed { .. } => 401,
            Self::RateLimited { .. } => 429,
            _ => 500,
        }
    }
}
```

### Crate error structure

| Crate           | Error Enum      | Key Variants                                         |
| --------------- | --------------- | ---------------------------------------------------- |
| `arky-core`     | `CoreError`     | AgentLoop, Cancelled, InvalidState                   |
| `arky-provider` | `ProviderError` | NotFound, StreamInterrupted, AuthFailed, RateLimited |
| `arky-tools`    | `ToolError`     | InvalidArgs, ExecutionFailed, Timeout, Cancelled     |
| `arky-session`  | `SessionError`  | NotFound, StorageFailure, Expired                    |
| `arky-mcp`      | `McpError`      | ConnectionFailed, ProtocolError, ServerCrashed       |
| `arky-hooks`    | `HookError`     | ExecutionFailed, Timeout, InvalidOutput              |
| `arky-config`   | `ConfigError`   | ParseFailed, ValidationFailed, NotFound              |

### Facade error (crate `arky`)

```rust
#[derive(Debug, Error)]
pub enum ArkyError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Mcp(#[from] McpError),
    #[error(transparent)]
    Hook(#[from] HookError),
    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl ClassifiedError for ArkyError {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Core(e) => e.is_retryable(),
            Self::Provider(e) => e.is_retryable(),
            Self::Tool(e) => e.is_retryable(),
            // ... delegate to inner
        }
    }
    // ... same delegation pattern for other methods
}
```

## Alternatives Considered

### Alternative 1: Per-crate thiserror enums without common trait

- **Description**: Each crate has `thiserror` enums, no shared classification interface
- **Pros**: Simplest approach, pure Rust convention
- **Cons**: No unified way to check retryability, error codes, or correction context across crate boundaries. Agent loop must pattern-match every concrete error type. No equivalent of our TS ErrorClassifier.
- **Why rejected**: Error classification is critical for agent self-correction. Without a shared trait, the agent loop needs to know about every error type from every crate — tight coupling that breaks as crates are added.

### Alternative 2: Single global error enum

- **Description**: One `ArkyError` with all variants from all crates
- **Pros**: Simple to use, one type everywhere
- **Cons**: Violates separation of concerns, forces every crate to depend on the root error type, circular dependency risk, massive enum that grows unboundedly
- **Why rejected**: Doesn't scale with a multi-crate workspace. Crates cannot define their own errors independently.

## Consequences

### Positive

- Each crate owns its errors — clear boundaries, no coupling
- `ClassifiedError` trait enables uniform error handling in the agent loop
- `#[from]` enables ergonomic `?` propagation across crate boundaries
- Retryability, error codes, and correction context are available everywhere
- Mirrors the battle-tested ErrorClassifier from our TypeScript providers/core
- Follows Rust ecosystem conventions (`thiserror`, `?`, `Error` trait)

### Negative

- Each crate must implement `ClassifiedError` for its error enum — some boilerplate
- Facade `ArkyError` requires delegation boilerplate for trait methods
- Two things to implement per error type: `thiserror` derive + `ClassifiedError` impl

### Risks

- `ClassifiedError` trait grows too many methods over time (mitigate: start minimal, use default implementations for optional methods)
- Inconsistent error code naming across crates (mitigate: document error code conventions, use `CRATE_ERROR_NAME` pattern)

## Implementation Notes

- `ClassifiedError` trait lives in `arky-error` so foundational crates can
  depend on it without introducing cycles through `arky-core`
- Default implementations on `ClassifiedError` so only `error_code()` is mandatory
- Use `#[non_exhaustive]` on all error enums for future variant additions
- Error codes follow pattern: `PROVIDER_RATE_LIMITED`, `TOOL_TIMEOUT`, `SESSION_EXPIRED`
- Consider a `classify()` helper that wraps any `ClassifiedError` into a structured log entry

## References

- TS ErrorClassifier: `tasks/prd-rust-providers/analysis_core.md` (Section 2)
- TS error hierarchy: `tasks/prd-rust-providers/analysis_runtime.md` (Section 5)
- Rust error handling: `.claude/skills/rust-engineer/references/error-handling.md`
- Apollo best practices ch.4: `.claude/skills/rust-best-practices/references/chapter_04.md`
