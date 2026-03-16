# ADR-004: Flat Enum Event Model with Rich Metadata

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

The SDK needs a unified event protocol that both the low-level Provider layer and high-level Agent layer emit. Events drive UI updates, logging, metrics, and consumer-side logic.

Analysis of existing systems revealed:

- **Pi**: 11 flat event types covering agent/turn/message/tool lifecycle. Simple `match` dispatch, but no metadata beyond the event payload.
- **Compozy claude-code**: 13 normalized event types with a multi-stage transformation pipeline. Powerful but complex — multiple intermediate representations before final events.
- **codex-rs**: Push-based event queue (SQ/EQ pattern) with structs, not enums. Less ergonomic for Rust pattern matching.

Rust's enum system with pattern matching is ideal for event protocols — exhaustive matching catches missing handlers at compile time.

## Decision

Use a **flat enum with shared `EventMetadata`** and a `Custom` variant for extensibility.

```rust
/// Metadata attached to every event
pub struct EventMetadata {
    pub timestamp: u64,
    pub session_id: Option<String>,
    pub turn_index: u32,
}

/// All events emitted by Agent and Provider layers
pub enum AgentEvent {
    // Agent lifecycle
    AgentStart { meta: EventMetadata },
    AgentEnd { meta: EventMetadata, messages: Vec<Message> },

    // Turn lifecycle
    TurnStart { meta: EventMetadata },
    TurnEnd { meta: EventMetadata, message: Message, tool_results: Vec<ToolResult> },

    // Message lifecycle
    MessageStart { meta: EventMetadata, message: Message },
    MessageUpdate { meta: EventMetadata, message: Message, delta: StreamDelta },
    MessageEnd { meta: EventMetadata, message: Message },

    // Tool execution lifecycle
    ToolExecutionStart { meta: EventMetadata, tool_call_id: String, tool_name: String, args: Value },
    ToolExecutionUpdate { meta: EventMetadata, tool_call_id: String, tool_name: String, partial_result: Value },
    ToolExecutionEnd { meta: EventMetadata, tool_call_id: String, tool_name: String, result: Value, is_error: bool },

    // Extensibility
    Custom { meta: EventMetadata, event_type: String, payload: Value },
}
```

### Design principles:

1. Every variant carries `EventMetadata` for consistent context
2. `#[non_exhaustive]` on the enum to allow adding variants without breaking consumers
3. `Custom` variant for user-defined events (analogous to Pi's `CustomAgentMessages` declaration merging)
4. Provider layer emits Message* and ToolExecution* events; Agent layer wraps them with Agent* and Turn* lifecycle events

## Alternatives Considered

### Alternative 1: Flat enum without metadata (Pi style)

- **Description**: Simple enum variants with only domain-specific fields, no shared metadata
- **Pros**: Minimal, easy to construct
- **Cons**: No timestamp/session context on events, consumers must track this externally, harder to correlate events across turns
- **Why rejected**: Metadata is essential for logging, debugging, UI, and multi-session scenarios. Adding it later would be a breaking change.

### Alternative 2: Hierarchical normalized events (Compozy claude-code style)

- **Description**: Multiple intermediate event types with transformation pipelines between layers
- **Pros**: Each layer has its own clean event type, transformations are explicit
- **Cons**: Multiple event enums to maintain, complex pipeline, consumers must decide which layer to subscribe to, more allocations from event conversion
- **Why rejected**: Over-engineered for our needs. A single enum with metadata gives the same information without the pipeline complexity. The CLI wrappers already normalize events — we don't need a second normalization layer.

## Consequences

### Positive

- Exhaustive `match` catches missing event handlers at compile time
- Single event type for the entire SDK — no confusion about which event type to use
- `EventMetadata` enables consistent logging, tracing, and UI updates
- `Custom` variant enables extensibility without forking the enum
- `#[non_exhaustive]` allows future variants without semver breaks

### Negative

- Every variant carries `meta` field, slightly more memory per event
- `Custom` variant loses type safety (payload is `Value`)
- Single enum may grow large as features are added

### Risks

- Enum becomes too large with many variants (mitigate: `#[non_exhaustive]` + group related events if needed in future)
- `Custom` variant abused for what should be proper variants (mitigate: document when to use Custom vs requesting a new variant)

## Implementation Notes

- `AgentEvent` lives in `crates/protocol`
- Both `crates/arky-core` (Agent) and `crates/arky-provider` (Provider) depend on it
- Use `tokio::sync::broadcast` or `mpsc` channels for event delivery
- Consumers subscribe via callback or async stream
- Serialization via `serde` for logging/persistence

## References

- Pi event system: `tasks/prd-rust-providers/analysis_pi_agent.md` (Section 2: Event System)
- Claude-code normalized events: `tasks/prd-rust-providers/analysis_claude_code.md` (Section 4: Streaming Pipeline)
- codex-rs SQ/EQ pattern: `tasks/prd-rust-providers/analysis_codex_rs.md` (Section 3: Core Agent Architecture)
