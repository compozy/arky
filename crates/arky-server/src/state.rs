//! Shared runtime state exposed through the HTTP server.

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::Duration,
};

use arky_error::ClassifiedError;
use arky_protocol::ProviderId;
use arky_session::SessionStore;
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::RwLock;

/// Top-level application state shared by all handlers.
#[derive(Clone)]
pub struct ServerState {
    agent: Arc<arky_core::Agent>,
    session_store: Arc<dyn SessionStore>,
    health: RuntimeHealthRegistry,
}

impl ServerState {
    /// Creates server state from a shared agent and session store.
    #[must_use]
    pub fn new(
        agent: Arc<arky_core::Agent>,
        session_store: Arc<dyn SessionStore>,
    ) -> Self {
        Self {
            agent,
            session_store,
            health: RuntimeHealthRegistry::default(),
        }
    }

    /// Returns the shared agent instance.
    #[must_use]
    pub fn agent(&self) -> Arc<arky_core::Agent> {
        Arc::clone(&self.agent)
    }

    /// Returns the shared session store.
    #[must_use]
    pub fn session_store(&self) -> Arc<dyn SessionStore> {
        Arc::clone(&self.session_store)
    }

    /// Returns the mutable runtime health registry.
    #[must_use]
    pub fn health(&self) -> RuntimeHealthRegistry {
        self.health.clone()
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
