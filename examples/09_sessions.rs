//! # 09 Sessions
//!
//! Demonstrates in-memory sessions, replay, resume, and optional SQLite-backed
//! persistence.

mod common;

use std::{
    io,
    sync::Arc,
};

use arky::{
    InMemorySessionStore,
    PersistedEvent,
    prelude::*,
};
use common::{
    EchoProvider,
    ExampleError,
    text_from_message,
};

#[cfg(feature = "sqlite")]
use arky::{
    SessionFilter,
    SqliteSessionStore,
};

#[cfg(feature = "sqlite")]
use tempfile::tempdir;

fn require_session_id(response: &arky::AgentResponse) -> Result<SessionId, ExampleError> {
    response.session.id.clone().ok_or_else(|| {
        io::Error::other("agent response did not expose a session id").into()
    })
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let in_memory_store = Arc::new(InMemorySessionStore::default());
    let agent = Agent::builder()
        .provider(EchoProvider::new("session-demo", "session"))
        .session_store_arc(in_memory_store.clone())
        .model("demo-model")
        .build()?;

    let first = agent.prompt("remember the first turn").await?;
    let session_id = require_session_id(&first)?;
    let snapshot = in_memory_store.load(&session_id).await?;
    let replay: Vec<PersistedEvent> = in_memory_store
        .replay_events(&session_id, None, Some(32))
        .await?;

    println!("first assistant: {}", text_from_message(&first.message));
    println!("session id: {session_id}");
    println!("snapshot messages: {}", snapshot.messages.len());
    println!("replay events: {}", replay.len());

    let resumed_agent = Agent::builder()
        .provider(EchoProvider::new("session-demo", "session"))
        .session_store_arc(in_memory_store.clone())
        .model("demo-model")
        .build()?;
    resumed_agent.resume(session_id.clone()).await?;
    let resumed = resumed_agent.prompt("continue the conversation").await?;
    println!("resumed assistant: {}", text_from_message(&resumed.message));

    #[cfg(feature = "sqlite")]
    {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("arky-examples.sqlite3");
        let sqlite_store = Arc::new(SqliteSessionStore::open(&db_path).await?);
        let sqlite_agent = Agent::builder()
            .provider(EchoProvider::new("sqlite-demo", "sqlite"))
            .session_store_arc(sqlite_store.clone())
            .model("demo-model")
            .build()?;

        let persisted = sqlite_agent.prompt("persist this session").await?;
        let sqlite_session_id = require_session_id(&persisted)?;
        let sqlite_sessions = sqlite_store.list(SessionFilter::default()).await?;
        let sqlite_replay = sqlite_store
            .replay_events(&sqlite_session_id, None, Some(32))
            .await?;

        println!("sqlite db: {}", db_path.display());
        println!("sqlite sessions: {}", sqlite_sessions.len());
        println!("sqlite replay events: {}", sqlite_replay.len());
    }

    #[cfg(not(feature = "sqlite"))]
    println!(
        "sqlite example skipped; run with `cargo run -p arky --example 09_sessions --features sqlite`."
    );

    Ok(())
}
