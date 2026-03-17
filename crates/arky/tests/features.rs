//! Integration coverage for optional facade feature surfaces.

use std::any::TypeId;

#[cfg(feature = "claude-code")]
#[test]
fn claude_code_feature_should_expose_provider_types() {
    let _ = TypeId::of::<arky::BedrockProvider>();
    let _ = TypeId::of::<arky::BedrockProviderConfig>();
    let _ = TypeId::of::<arky::ClaudeCompatibleProviderConfig>();
    let _ = TypeId::of::<arky::ClaudeCompatibleProviderKind>();
    let _ = TypeId::of::<arky::ClaudeCodeProvider>();
    let _ = TypeId::of::<arky::ClaudeCodeProviderConfig>();
    let _ = TypeId::of::<arky::MinimaxProvider>();
    let _ = TypeId::of::<arky::MinimaxProviderConfig>();
    let _ = TypeId::of::<arky::MoonshotProvider>();
    let _ = TypeId::of::<arky::MoonshotProviderConfig>();
    let _ = TypeId::of::<arky::OllamaProvider>();
    let _ = TypeId::of::<arky::OllamaProviderConfig>();
    let _ = TypeId::of::<arky::OpenRouterProvider>();
    let _ = TypeId::of::<arky::OpenRouterProviderConfig>();
    let _ = TypeId::of::<arky::VercelProvider>();
    let _ = TypeId::of::<arky::VercelProviderConfig>();
    let _ = TypeId::of::<arky::VertexProvider>();
    let _ = TypeId::of::<arky::VertexProviderConfig>();
    let _ = TypeId::of::<arky::ZaiProvider>();
    let _ = TypeId::of::<arky::ZaiProviderConfig>();
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
