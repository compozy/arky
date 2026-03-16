# ADR-007: Session Store Trait from Day One

## Status

Accepted

## Date

2026-03-15

## Context

AI agent conversations are stateful — messages, tool results, and context accumulate over time. We need to decide how session state is managed and whether persistence is a first-class concern or an afterthought.

Our TypeScript runtime already has two session store implementations (`InMemorySessionStore` with LRU+TTL and `SqliteSessionStore` with reverse-index), proving the abstraction is valuable and stable. Pi's Agent keeps state purely in-memory with no persistence story.

Key considerations:

- Agent conversations can be long-running (hours for coding agents)
- Users expect to resume sessions after CLI restart
- Multi-agent scenarios need shared session context
- Server-side deployments need persistent, queryable session storage

## Decision

Implement a **`SessionStore` trait from day one** as a required dependency of the Agent, with `InMemorySessionStore` as the default implementation and `SqliteSessionStore` as a first additional backend.

### Session Store trait (crate `arky-session`)

```rust
pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSession {
    pub model_id: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: SessionId,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    pub model_id: Option<String>,
    pub labels: HashMap<String, String>,
}

pub struct SessionSnapshot {
    pub metadata: SessionMetadata,
    pub messages: Vec<Message>,
    pub last_checkpoint: Option<TurnCheckpoint>,
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, new_session: NewSession) -> Result<SessionId, SessionError>;
    async fn load(&self, id: &SessionId) -> Result<SessionSnapshot, SessionError>;
    async fn append_messages(&self, id: &SessionId, messages: &[Message]) -> Result<(), SessionError>;
    async fn append_events(&self, id: &SessionId, events: &[PersistedEvent]) -> Result<(), SessionError>;
    async fn save_turn_checkpoint(&self, id: &SessionId, checkpoint: TurnCheckpoint) -> Result<(), SessionError>;
    async fn list(&self, filter: SessionFilter) -> Result<Vec<SessionMetadata>, SessionError>;
    async fn delete(&self, id: &SessionId) -> Result<(), SessionError>;
}

#[derive(Debug, Default)]
pub struct SessionFilter {
    pub label: Option<(String, String)>,
    pub since: Option<u64>,
    pub limit: Option<usize>,
}
```

### In-memory implementation (default)

```rust
/// In-memory session store with optional LRU eviction and TTL
pub struct InMemorySessionStore {
    sessions: DashMap<SessionId, SessionData>,
    max_sessions: Option<usize>,
    ttl: Option<Duration>,
}
```

### SQLite implementation

```rust
/// Persistent session store backed by SQLite
pub struct SqliteSessionStore {
    conn: tokio_rusqlite::Connection,
}
```

### Agent integration

```rust
let agent = Agent::builder()
    .provider(
        ClaudeCodeProvider::builder()
            .model("claude-sonnet-4-20250514")
            .build()?
    )
    .session_store(SqliteSessionStore::open("~/.arky/sessions.db").await?)
    .build()?;

// New session
let session_id = agent.new_session().await?;
agent.prompt("Hello").await?;

// Resume later
let agent = Agent::builder()
    .provider(
        ClaudeCodeProvider::builder()
            .model("claude-sonnet-4-20250514")
            .build()?
    )
    .session_store(SqliteSessionStore::open("~/.arky/sessions.db").await?)
    .resume(session_id)
    .build()?;

agent.prompt("Continue from where we left off").await?;
```

## Alternatives Considered

### Alternative 1: In-memory only (add persistence later)

- **Description**: Agent keeps `Vec<Message>` in memory, no store trait, no persistence
- **Pros**: Simplest MVP, less code upfront
- **Cons**: No resume, no crash recovery, retrofitting persistence later forces API redesign, consumers build their own storage
- **Why rejected**: Session persistence is not optional for coding agents. Our TS SDK already proved this — the SessionStore abstraction was needed immediately. Delaying it creates API debt.

### Alternative 2: In-memory default with opt-in store trait

- **Description**: Agent works without a store by default, `SessionStore` is optional
- **Pros**: Zero-config for simple use cases, progressive complexity
- **Cons**: Two code paths (with/without store), edge cases when switching from no-store to store mid-session, agent must handle "no persistence" mode everywhere
- **Why rejected**: The complexity of maintaining two code paths (persisted vs ephemeral) outweighs the simplicity of making the store always present. `InMemorySessionStore` is trivial and serves as the "zero config" default.

## Consequences

### Positive

- Session resume works from day one — critical for coding agents
- Clean `SessionStore` trait enables custom backends (Redis, Postgres, cloud storage)
- `InMemorySessionStore` is the "zero config" default — no setup burden
- SQLite store enables crash recovery for CLI agents
- Consistent API regardless of storage backend
- Proven abstraction from our TypeScript SDK

### Negative

- All agent operations go through the store trait — slight overhead for in-memory case
- More upfront implementation work (trait + 2 implementations + tests)
- SQLite dependency must be optional (feature-gated)

### Risks

- Store trait becomes a bottleneck for high-throughput scenarios (mitigate: batch append, async writes, write-behind caching)
- Message serialization format changes break stored sessions (mitigate: version field in session metadata, migration support)
- SQLite locking in multi-agent scenarios (mitigate: WAL mode, per-agent store instances)

## Implementation Notes

- `arky-session` crate: `SessionStore` trait, `InMemorySessionStore`, `SessionFilter`, `SessionMetadata`
- `arky-session` crate can expose a feature-gated SQLite backend using an
  async-friendly wrapper around `rusqlite`
- Messages and replay events are serialized as JSON stored in SQLite `TEXT`
  columns with JSON validation, not PostgreSQL-style `JSONB`
- `InMemorySessionStore` uses `dashmap` for concurrent access, optional LRU eviction via `max_sessions`
- Agent always has a session — `Agent::builder()` without explicit store uses `InMemorySessionStore::default()`
- Session IDs are UUIDs (`uuid` crate)

## References

- TS InMemorySessionStore: `tasks/prd-rust-providers/analysis_runtime.md` (Section 12)
- TS SqliteSessionStore: `providers/runtime/src/session/sqlite-session-store.ts`
- codex-rs state management: `tasks/prd-rust-providers/analysis_codex_rs.md` (Section 9)
- Pi session management: `tasks/prd-rust-providers/analysis_pi_agent.md` (no persistence)
