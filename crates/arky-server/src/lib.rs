//! HTTP and SSE runtime exposure for Arky agents.
//!
//! Enable the `server` feature to expose Arky agents over HTTP and SSE,
//! including session inspection, event streaming, and replay-oriented routes.

#[cfg(feature = "server")]
mod client;
#[cfg(feature = "server")]
mod error;
#[cfg(feature = "server")]
mod middleware;
#[cfg(feature = "server")]
mod routes;
#[cfg(feature = "server")]
mod state;

#[cfg(feature = "server")]
use std::{
    net::{
        IpAddr,
        SocketAddr,
    },
    sync::Arc,
};

#[cfg(feature = "server")]
use axum::{
    Router,
    middleware::from_fn_with_state,
    routing::{
        get,
        post,
    },
};
#[cfg(feature = "server")]
use tokio::{
    net::TcpListener,
    sync::Mutex,
};
#[cfg(feature = "server")]
use tokio_util::sync::CancellationToken;

#[cfg(feature = "server")]
pub use crate::{
    client::RuntimeClient,
    error::ServerError,
    state::{
        ComponentHealth,
        HealthStatus,
        ModelCard,
        ProviderHealthSnapshot,
        ReadinessSnapshot,
        RuntimeHealthRegistry,
        ServerState,
        SessionCompatibility,
    },
};
#[cfg(feature = "server")]
pub use arky_control::{
    RuntimeHandle,
    SessionStoreAdapter,
    SessionStoreHandle,
};

#[cfg(feature = "server")]
pub use arky_storage::SessionStore;

#[cfg(feature = "server")]
use crate::{
    middleware::cors_layer,
    routes::{
        chat,
        events,
        health,
        models,
        replay,
        sessions,
    },
};

#[cfg(feature = "server")]
/// Builds the application router for the Arky runtime server.
pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/providers/health", get(health::list_provider_health))
        .route(
            "/providers/{provider_id}/health",
            get(health::get_provider_health),
        )
        .route("/sessions", get(sessions::list_sessions))
        .route("/sessions/{session_id}", get(sessions::get_session))
        .route(
            "/sessions/{session_id}/messages",
            get(sessions::get_session_messages),
        )
        .route(
            "/sessions/{session_id}/events",
            get(events::stream_session_events),
        )
        .route("/sessions/{session_id}/replay", get(replay::replay_session))
        .route(
            "/v1/chat/stream",
            post(chat::chat_stream).route_layer(from_fn_with_state(
                state.clone(),
                crate::middleware::bearer_auth,
            )),
        )
        .route(
            "/v1/models",
            get(models::list_models).route_layer(from_fn_with_state(
                state.clone(),
                crate::middleware::bearer_auth,
            )),
        )
        .with_state(state)
        .layer(cors_layer())
}

#[cfg(feature = "server")]
/// Running server handle with graceful shutdown support.
#[derive(Debug)]
pub struct ServerHandle {
    local_addr: SocketAddr,
    cancellation: CancellationToken,
    join: Mutex<Option<tokio::task::JoinHandle<std::io::Result<()>>>>,
}

#[cfg(feature = "server")]
impl ServerHandle {
    /// Returns the listener address.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns the base URL clients can use for requests.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", socket_addr_host(self.local_addr))
    }

    /// Triggers graceful shutdown and waits for the server task to finish.
    pub async fn shutdown(&self) -> Result<(), ServerError> {
        self.cancellation.cancel();
        self.await_join().await
    }

    async fn await_join(&self) -> Result<(), ServerError> {
        let join = self.join.lock().await.take();
        let Some(join) = join else {
            return Ok(());
        };

        match join.await {
            Ok(result) => result.map_err(|error| ServerError::io(&error)),
            Err(error) => Err(ServerError::internal(format!(
                "server task crashed while shutting down: {error}"
            ))),
        }
    }
}

#[cfg(feature = "server")]
/// Starts serving the Arky runtime on the provided listener.
pub fn serve(
    listener: TcpListener,
    state: ServerState,
) -> Result<ServerHandle, ServerError> {
    let local_addr = listener
        .local_addr()
        .map_err(|error| ServerError::io(&error))?;
    let cancellation = CancellationToken::new();
    let app = router(state);
    let join = tokio::spawn({
        let cancellation = cancellation.clone();
        async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    cancellation.cancelled_owned().await;
                })
                .await
                .map_err(std::io::Error::other)
        }
    });

    Ok(ServerHandle {
        local_addr,
        cancellation,
        join: Mutex::new(Some(join)),
    })
}

#[cfg(feature = "server")]
fn socket_addr_host(addr: SocketAddr) -> String {
    match addr.ip() {
        IpAddr::V4(_) => addr.to_string(),
        IpAddr::V6(_) => format!("[{}]:{}", addr.ip(), addr.port()),
    }
}

#[cfg(feature = "server")]
impl
    From<(
        Arc<arky_runtime::Agent>,
        Arc<dyn arky_storage::SessionStore>,
    )> for ServerState
{
    fn from(
        value: (
            Arc<arky_runtime::Agent>,
            Arc<dyn arky_storage::SessionStore>,
        ),
    ) -> Self {
        Self::new(value.0, Arc::new(SessionStoreAdapter::new(value.1)))
    }
}

#[cfg(test)]
#[allow(dead_code, reason = "shared only by route unit tests")]
mod test_support {
    use async_trait::async_trait;
    use futures::stream;

    use arky_provider::{
        Provider,
        ProviderCapabilities,
        ProviderDescriptor,
        ProviderError,
        ProviderEventStream,
        ProviderFamily,
        ProviderRequest,
    };
    use arky_types::ProviderId;

    pub struct StaticProvider {
        descriptor: ProviderDescriptor,
    }

    impl StaticProvider {
        pub fn new() -> Self {
            Self {
                descriptor: ProviderDescriptor::new(
                    ProviderId::new("mock-server"),
                    ProviderFamily::Custom("mock-server".to_owned()),
                    ProviderCapabilities::new()
                        .with_streaming(true)
                        .with_generate(true)
                        .with_tool_calls(true)
                        .with_session_resume(true)
                        .with_steering(true)
                        .with_follow_up(true),
                ),
            }
        }
    }

    #[async_trait]
    impl Provider for StaticProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        async fn stream(
            &self,
            _request: ProviderRequest,
        ) -> Result<ProviderEventStream, ProviderError> {
            Ok(Box::pin(stream::empty()))
        }
    }
}
