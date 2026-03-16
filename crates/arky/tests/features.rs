//! Integration coverage for optional facade feature surfaces.

use std::any::TypeId;

#[cfg(feature = "claude-code")]
#[test]
fn claude_code_feature_should_expose_provider_types() {
    let _ = TypeId::of::<arky::ClaudeCodeProvider>();
    let _ = TypeId::of::<arky::ClaudeCodeProviderConfig>();
}

#[cfg(feature = "codex")]
#[test]
fn codex_feature_should_expose_provider_types() {
    let _ = TypeId::of::<arky::CodexProvider>();
    let _ = TypeId::of::<arky::CodexProviderConfig>();
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_feature_should_expose_session_backend_types() {
    let _ = TypeId::of::<arky::SqliteSessionStore>();
    let _ = TypeId::of::<arky::SqliteSessionStoreConfig>();
}

#[cfg(feature = "server")]
#[test]
fn server_feature_should_expose_runtime_surface() {
    let _ = TypeId::of::<arky::ServerError>();
    let _ = TypeId::of::<arky::ServerHandle>();
    let _ = TypeId::of::<arky::ServerState>();
}
