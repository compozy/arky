# ADR-002: Dual-Layer API Design (Provider + Agent)

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

We need to decide the primary API abstraction for the Arky Rust SDK. Analysis of both the existing compozy providers/\* (provider-centric) and Pi's agent framework (agent-centric) revealed two valid but different design philosophies.

- **Pi's Agent-centric approach**: Simple, productive, opinionated. The `Agent` owns state, tools, events, streaming. Great for CLI tools and single-agent use cases. But less flexible for server-side, multi-tenant, or headless scenarios.
- **Compozy's Provider-centric approach**: Flexible, composable, multi-provider. The `Runtime` orchestrates adapters and exposes `stream_text`. Better for server-side and infrastructure. But requires more boilerplate for common agent use cases.

Users of an AI agent SDK range from "I want to build a quick agent" to "I need a multi-provider server with custom session management."

## Decision

Implement a **dual-layer API**:

### Low-Level Layer: Provider Traits + Streaming Protocol

- `Provider` trait with `stream()` and `generate()` methods
- `ProviderRegistry` for multi-provider resolution
- Raw streaming via `Stream<Item = Result<AgentEvent, ProviderError>>`
- Tool calling protocol types
- For consumers who want full control over the LLM interaction

### High-Level Layer: Agent Framework

- `Agent` struct that orchestrates providers, tools, hooks, state, and events
- Builder pattern for ergonomic construction
- `agent.prompt()` / `agent.stream()` as the primary interaction
- Built-in event system, steering, follow-ups (inspired by Pi)
- Uses the low-level Provider layer internally

### Usage examples:

```rust
// High-level: Agent (most users)
let provider = ClaudeCodeProvider::builder()
    .model("claude-sonnet-4-20250514")
    .build()?;

let agent = Agent::builder()
    .provider(provider)
    .system_prompt("You are helpful")
    .tool(my_tool)
    .build()?;

agent.prompt("Hello").await?;

// Low-level: Provider (advanced users)
let provider = ClaudeCodeProvider::builder()
    .model("claude-sonnet-4-20250514")
    .build()?;

let stream = provider.stream(request).await?;
while let Some(event) = stream.next().await {
    match event? {
        AgentEvent::MessageUpdate { .. } => {}
        _ => {}
    }
}
```

## Alternatives Considered

### Alternative 1: Agent-Centric Only (Pi style)

- **Description**: `Agent` as the only public abstraction. Providers are internal implementation details.
- **Pros**: Simple API surface, easy to learn, less decision fatigue
- **Cons**: Inflexible for server-side/multi-tenant, forces opinionated patterns, hard to use providers standalone
- **Why rejected**: Too restrictive for infrastructure use cases. Server-side consumers need provider-level access without the agent overhead.

### Alternative 2: Provider-Centric Only (current compozy style)

- **Description**: `Runtime`/`Provider` as the primary abstraction. No built-in agent concept.
- **Pros**: Maximum flexibility, composable, good for servers
- **Cons**: Too much boilerplate for common agent use cases, users must build their own agent loop, poor DX for getting started
- **Why rejected**: Most consumers want an agent, not a raw provider. Forcing everyone to build their own agent loop is wasteful.

## Consequences

### Positive

- Serves both "quick agent" and "infrastructure" use cases
- Agent layer validates and proves the provider layer's design
- Clear upgrade path: start with Agent, drop to Provider when needed
- Each layer can be tested independently
- Follows the principle of progressive disclosure of complexity

### Negative

- Two APIs to maintain and document
- Risk of leaky abstractions between layers
- More initial design work to get the boundaries right

### Risks

- Agent layer becomes a thin wrapper that doesn't add enough value (mitigate: invest in built-in behaviors like steering, hooks, session management)
- Provider layer is too low-level to use standalone (mitigate: provide sensible defaults and helpers)

## Implementation Notes

- `crates/arky-core` contains the Agent framework
- `crates/arky-provider` contains the Provider trait and registry
- Individual provider implementations (`arky-claude-code`, `arky-codex`) implement the Provider trait
- The Agent imports and composes providers internally
- Both layers share the same `AgentEvent` enum and `Tool` trait from `crates/arky-protocol`

## References

- Pi agent analysis: `tasks/prd-rust-providers/analysis_pi_agent.md`
- Runtime analysis: `tasks/prd-rust-providers/analysis_runtime.md`
- Rig framework: uses a similar dual-layer with `CompletionModel` trait + `Agent` builder
