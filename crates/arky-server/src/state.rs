//! Shared runtime state exposed through the HTTP server.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::Duration,
};

use arky_control::{
    RuntimeHandle,
    SessionStoreHandle,
};
use arky_error::ClassifiedError;
use arky_types::{
    ProviderId,
    SessionId,
};
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::{
    Mutex,
    RwLock,
};

/// Top-level application state shared by all handlers.
#[derive(Clone)]
pub struct ServerState {
    runtime: Arc<dyn RuntimeHandle>,
    session_store: Arc<dyn SessionStoreHandle>,
    health: RuntimeHealthRegistry,
    auth_token: Option<String>,
    models: Arc<RwLock<Vec<ModelCard>>>,
    session_keys: Arc<RwLock<BTreeMap<String, SessionId>>>,
    chat_start_lock: Arc<Mutex<()>>,
}

impl ServerState {
    /// Creates server state from a shared runtime handle and session store.
    #[must_use]
    pub fn new<R, S>(runtime: Arc<R>, session_store: Arc<S>) -> Self
    where
        R: RuntimeHandle + 'static,
        S: SessionStoreHandle + 'static,
    {
        Self {
            runtime,
            session_store,
            health: RuntimeHealthRegistry::default(),
            auth_token: None,
            models: Arc::new(RwLock::new(Vec::new())),
            session_keys: Arc::new(RwLock::new(BTreeMap::new())),
            chat_start_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Returns the shared runtime handle.
    #[must_use]
    pub fn runtime(&self) -> Arc<dyn RuntimeHandle> {
        Arc::clone(&self.runtime)
    }

    /// Returns the shared session store.
    #[must_use]
    pub fn session_store(&self) -> Arc<dyn SessionStoreHandle> {
        Arc::clone(&self.session_store)
    }

    /// Returns the mutable runtime health registry.
    #[must_use]
    pub fn health(&self) -> RuntimeHealthRegistry {
        self.health.clone()
    }

    /// Stores a bearer token required by protected API routes.
    #[must_use]
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Returns the protected API bearer token when configured.
    #[must_use]
    pub fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    /// Replaces the model catalog exposed by `/v1/models`.
    pub async fn set_models(&self, models: Vec<ModelCard>) {
        *self.models.write().await = models;
    }

    /// Lists the configured models.
    pub async fn models(&self) -> Vec<ModelCard> {
        self.models.read().await.clone()
    }

    /// Resolves a stable chat session mapping by caller-supplied key.
    pub async fn session_id_for_key(&self, session_key: &str) -> Option<SessionId> {
        self.session_keys.read().await.get(session_key).cloned()
    }

    /// Stores a stable chat session mapping.
    pub async fn set_session_key(
        &self,
        session_key: impl Into<String>,
        session_id: SessionId,
    ) {
        self.session_keys
            .write()
            .await
            .insert(session_key.into(), session_id);
    }

    /// Returns the guard used to serialize chat-session routing and turn starts.
    #[must_use]
    pub fn chat_start_lock(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.chat_start_lock)
    }
}

/// Model metadata exposed by the server's model-listing endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCard {
    /// Model identifier.
    pub id: String,
    /// Provider owner or family label.
    pub owned_by: String,
    /// Creation timestamp used by OpenAI-compatible responses.
    pub created: u64,
    /// Provider identifier.
    pub provider_id: ProviderId,
    /// Optional user-facing display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional model context window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Optional max output token budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    /// Whether the model supports tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_tools: Option<bool>,
    /// Whether the model supports reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning: Option<bool>,
    /// Optional free-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ModelCard {
    /// Creates a minimal model card.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        owned_by: impl Into<String>,
        provider_id: ProviderId,
    ) -> Self {
        Self {
            id: id.into(),
            owned_by: owned_by.into(),
            created: 0,
            provider_id,
            display_name: None,
            context_window: None,
            max_output_tokens: None,
            supports_tools: None,
            supports_reasoning: None,
            description: None,
        }
    }
}

/// Runtime readiness and provider health storage.
#[derive(Clone)]
pub struct RuntimeHealthRegistry {
    readiness: Arc<RwLock<ReadinessSnapshot>>,
    providers: Arc<RwLock<BTreeMap<ProviderId, ProviderHealthSnapshot>>>,
}

impl RuntimeHealthRegistry {
    /// Returns the current readiness snapshot.
    pub async fn readiness(&self) -> ReadinessSnapshot {
        self.readiness.read().await.clone()
    }

    /// Marks the runtime as ready.
    pub async fn set_ready(&self) {
        *self.readiness.write().await = ReadinessSnapshot::ready();
    }

    /// Marks the runtime as not ready with a reason.
    pub async fn set_not_ready(&self, reason: impl Into<String>) {
        *self.readiness.write().await = ReadinessSnapshot::not_ready(reason);
    }

    /// Stores or replaces provider health information.
    pub async fn set_provider_health(&self, snapshot: ProviderHealthSnapshot) {
        self.providers
            .write()
            .await
            .insert(snapshot.provider_id.clone(), snapshot);
    }

    /// Removes provider health information.
    pub async fn remove_provider_health(
        &self,
        provider_id: &ProviderId,
    ) -> Option<ProviderHealthSnapshot> {
        self.providers.write().await.remove(provider_id)
    }

    /// Returns one provider snapshot when present.
    pub async fn provider_health(
        &self,
        provider_id: &ProviderId,
    ) -> Option<ProviderHealthSnapshot> {
        self.providers.read().await.get(provider_id).cloned()
    }

    /// Returns provider snapshots sorted by provider identifier.
    pub async fn provider_health_list(&self) -> Vec<ProviderHealthSnapshot> {
        self.providers.read().await.values().cloned().collect()
    }
}

impl Default for RuntimeHealthRegistry {
    fn default() -> Self {
        Self {
            readiness: Arc::new(RwLock::new(ReadinessSnapshot::ready())),
            providers: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }
}

/// Overall health classification for runtime components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Component is healthy.
    Healthy,
    /// Component is usable but degraded.
    Degraded,
    /// Component is unavailable or failed.
    Unhealthy,
}

/// Health information for one provider subcomponent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Current health status.
    pub status: HealthStatus,
    /// Human-readable detail when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Suggested retry delay for recovery, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl ComponentHealth {
    /// Creates a healthy component snapshot.
    #[must_use]
    pub const fn healthy() -> Self {
        Self {
            status: HealthStatus::Healthy,
            message: None,
            retry_after_ms: None,
        }
    }

    /// Creates a degraded component snapshot.
    #[must_use]
    pub fn degraded(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            retry_after_ms: None,
        }
    }

    /// Creates an unhealthy component snapshot.
    #[must_use]
    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            retry_after_ms: None,
        }
    }

    /// Creates component health from a classified error.
    #[must_use]
    pub fn from_error<E>(status: HealthStatus, error: &E) -> Self
    where
        E: ClassifiedError + ?Sized,
    {
        Self {
            status,
            message: Some(error.to_string()),
            retry_after_ms: error.retry_after().map(duration_to_millis),
        }
    }
}

/// Session compatibility projection for a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCompatibility {
    /// Whether the provider can resume or continue the current runtime session.
    pub compatible: bool,
    /// Human-readable explanation when compatibility is limited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl SessionCompatibility {
    /// Creates a compatible session state.
    #[must_use]
    pub const fn compatible() -> Self {
        Self {
            compatible: true,
            message: None,
        }
    }

    /// Creates an incompatible session state with context.
    #[must_use]
    pub fn incompatible(message: impl Into<String>) -> Self {
        Self {
            compatible: false,
            message: Some(message.into()),
        }
    }
}

/// Provider health snapshot exposed by the runtime server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderHealthSnapshot {
    /// Provider identifier.
    pub provider_id: ProviderId,
    /// Overall status for the provider.
    pub overall: HealthStatus,
    /// Binary validation state.
    pub binary: ComponentHealth,
    /// Transport state.
    pub transport: ComponentHealth,
    /// Session compatibility projection.
    pub session_compatibility: SessionCompatibility,
}

impl ProviderHealthSnapshot {
    /// Creates a healthy provider snapshot.
    #[must_use]
    pub const fn healthy(provider_id: ProviderId) -> Self {
        Self {
            provider_id,
            overall: HealthStatus::Healthy,
            binary: ComponentHealth::healthy(),
            transport: ComponentHealth::healthy(),
            session_compatibility: SessionCompatibility::compatible(),
        }
    }
}

/// Readiness state for the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessSnapshot {
    /// Whether the runtime is ready to serve traffic.
    pub ready: bool,
    /// Optional reason when not ready.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ReadinessSnapshot {
    /// Creates a ready snapshot.
    #[must_use]
    pub const fn ready() -> Self {
        Self {
            ready: true,
            reason: None,
        }
    }

    /// Creates a not-ready snapshot with context.
    #[must_use]
    pub fn not_ready(reason: impl Into<String>) -> Self {
        Self {
            ready: false,
            reason: Some(reason.into()),
        }
    }
}

fn duration_to_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
