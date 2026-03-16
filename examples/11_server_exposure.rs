//! # 11 Server Exposure
//!
//! Demonstrates the HTTP/SSE runtime surface for health, sessions, and live
//! session events.

mod common;

use common::ExampleError;

#[cfg(not(feature = "server"))]
#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    println!(
        "enable the `server` feature to run this example: cargo run -p arky --example 11_server_exposure --features server"
    );
    Ok(())
}

#[cfg(feature = "server")]
mod enabled {
    use std::{
        sync::Arc,
        time::Duration,
    };

    use arky::{
        InMemorySessionStore,
        ProviderHealthSnapshot,
        ProviderId,
        ServerState,
        prelude::*,
        serve,
    };
    use reqwest::Client;
    use serde_json::Value;

    use crate::common::{
        EchoProvider,
        ExampleError,
    };

    pub async fn run() -> Result<(), ExampleError> {
        let store = Arc::new(InMemorySessionStore::default());
        let agent = Arc::new(
            Agent::builder()
                .provider(EchoProvider::new("http-demo", "server"))
                .session_store_arc(store.clone())
                .model("demo-model")
                .build()?,
        );
        let state = ServerState::new(agent.clone(), store);
        state
            .health()
            .set_provider_health(ProviderHealthSnapshot::healthy(ProviderId::new(
                "http-demo",
            )))
            .await;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let handle = serve(listener, state)?;
        let client = Client::builder().build()?;
        let session_id = agent.new_session().await?;
        let events_url = format!("{}/sessions/{session_id}/events", handle.base_url());
        let client_for_events = client.clone();

        let events_task = tokio::spawn(async move {
            client_for_events.get(events_url).send().await?.text().await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = agent
            .prompt("stream this through the runtime server")
            .await?;

        let sse_payload = events_task.await??;
        let health: Value = client
            .get(format!("{}/health", handle.base_url()))
            .send()
            .await?
            .json()
            .await?;
        let sessions: Value = client
            .get(format!("{}/sessions", handle.base_url()))
            .send()
            .await?
            .json()
            .await?;
        let messages: Value = client
            .get(format!(
                "{}/sessions/{session_id}/messages",
                handle.base_url()
            ))
            .send()
            .await?
            .json()
            .await?;

        println!("health status: {}", health["status"]);
        let session_count: usize = sessions["sessions"]
            .as_array()
            .into_iter()
            .map(Vec::len)
            .sum();
        let message_count: usize = messages["messages"]
            .as_array()
            .into_iter()
            .map(Vec::len)
            .sum();
        println!("session count: {session_count}");
        println!("message count: {message_count}");
        println!(
            "sse observed agent_start: {}",
            sse_payload.contains("event: agent_start")
        );
        println!(
            "sse observed agent_end: {}",
            sse_payload.contains("event: agent_end")
        );

        handle.shutdown().await?;

        Ok(())
    }
}

#[cfg(feature = "server")]
#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    enabled::run().await
}
