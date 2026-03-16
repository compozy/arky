# ADR-009: Trait-Based Hook System with All Lifecycle Events from Day One

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

Our TypeScript providers/core has a battle-tested hook system (813 lines) with 6 lifecycle events and 4 handler types. Hooks are essential for:

- Permission gating (blocking dangerous tool calls)
- Audit logging (session start/end tracking)
- Prompt injection (modifying context before LLM calls)
- Agent termination control (stop hooks)
- Tool result modification (post-tool-use hooks)
- Shell command integration (running external scripts on events)

The TS hook system supports: callbacks, descriptors (with matchers), shell commands (child process with timeout/cancellation), and prompt injection. All 6 events must be present from day one because CLI-wrapped providers depend on them for feature parity.

We want a design that is complete AND extensible — all hooks present immediately, but new hooks can be added without breaking consumers.

## Decision

Implement a **`Hooks` trait with default no-op methods** for all 6 lifecycle events, plus a **`HookHandler` enum** for the 4 handler types. All events from day one. Extensible via default methods and `#[non_exhaustive]`.

### Hooks trait (crate `arky-hooks`)

```rust
/// All lifecycle events emitted by the agent.
/// Every method has a default no-op implementation — consumers override only what they need.
#[async_trait]
pub trait Hooks: Send + Sync {
    /// Called before a tool is executed. Return `Verdict::Block(reason)` to prevent execution.
    async fn before_tool_call(
        &self,
        ctx: &BeforeToolCallContext,
        cancel: CancellationToken,
    ) -> Result<Verdict, HookError> {
        let _ = (ctx, cancel);
        Ok(Verdict::Allow)
    }

    /// Called after a tool finishes. Can modify the tool result.
    async fn after_tool_call(
        &self,
        ctx: &AfterToolCallContext,
        cancel: CancellationToken,
    ) -> Result<Option<ToolResultOverride>, HookError> {
        let _ = (ctx, cancel);
        Ok(None)
    }

    /// Called when a session starts. Can inject initial context or modify settings.
    async fn session_start(
        &self,
        ctx: &SessionStartContext,
    ) -> Result<Option<SessionStartUpdate>, HookError> {
        let _ = ctx;
        Ok(None)
    }

    /// Called when a session ends.
    async fn session_end(
        &self,
        ctx: &SessionEndContext,
    ) -> Result<(), HookError> {
        let _ = ctx;
        Ok(())
    }

    /// Called when the agent is about to stop. Return `StopDecision::Continue` to override.
    async fn on_stop(
        &self,
        ctx: &StopContext,
    ) -> Result<StopDecision, HookError> {
        let _ = ctx;
        Ok(StopDecision::Stop)
    }

    /// Called when the user submits a prompt. Can modify the prompt before processing.
    async fn user_prompt_submit(
        &self,
        ctx: &PromptSubmitContext,
    ) -> Result<Option<PromptUpdate>, HookError> {
        let _ = ctx;
        Ok(None)
    }
}
```

### Hook handler types

```rust
/// Verdicts for before_tool_call
pub enum Verdict {
    Allow,
    Block { reason: String },
}

/// Overrides for after_tool_call
pub struct ToolResultOverride {
    pub content: Option<Vec<ToolContent>>,
    pub is_error: Option<bool>,
}

/// Stop decisions
pub enum StopDecision {
    Stop,
    Continue { reason: String },
}

/// Prompt updates from user_prompt_submit
pub struct PromptUpdate {
    pub modified_prompt: Option<String>,
    pub injected_messages: Vec<Message>,
}

/// Session start updates
pub struct SessionStartUpdate {
    pub injected_system_prompt: Option<String>,
    pub settings_overrides: HashMap<String, Value>,
}
```

### Built-in hook implementations

```rust
/// Compose multiple Hooks implementations into one.
/// Hooks run in order; first Block/Continue verdict wins.
pub struct HookChain {
    hooks: Vec<Box<dyn Hooks>>,
}

/// Hook that executes a shell command on events.
/// Mirrors the TS command handler with timeout and cancellation.
pub struct ShellCommandHook {
    pub event: HookEvent,
    pub command: String,
    pub args: Vec<String>,
    pub timeout: Duration,
    pub matcher: Option<ToolMatcher>,
}

/// Hook that runs an async closure.
pub struct ClosureHook<F> { /* ... */ }

/// Filter which tools a hook applies to
pub struct ToolMatcher {
    pub tool_names: Vec<String>,  // exact match
    pub patterns: Vec<Regex>,     // glob/regex match
}

/// Which lifecycle event a hook handles
#[non_exhaustive]
pub enum HookEvent {
    BeforeToolCall,
    AfterToolCall,
    SessionStart,
    SessionEnd,
    Stop,
    UserPromptSubmit,
}
```

### Agent integration

```rust
// Implement custom hooks by overriding only what you need
struct MyHooks;

#[async_trait]
impl Hooks for MyHooks {
    async fn before_tool_call(&self, ctx: &BeforeToolCallContext, _cancel: CancellationToken) -> Result<Verdict, HookError> {
        if ctx.tool_name == "bash" && ctx.args.get("command").map_or(false, |c| c.as_str().map_or(false, |s| s.contains("rm -rf"))) {
            return Ok(Verdict::Block { reason: "Dangerous command blocked".into() });
        }
        Ok(Verdict::Allow)
    }
}

// Compose hooks
let hooks = HookChain::new()
    .add(MyHooks)
    .add(ShellCommandHook::new(HookEvent::AfterToolCall, "notify-send", &["Tool completed"], Duration::from_secs(5)))
    .add(ShellCommandHook::new(HookEvent::SessionEnd, "curl", &["-X", "POST", "https://webhook.example.com/session-end"], Duration::from_secs(10)));

let agent = Agent::builder()
    .provider(
        ClaudeCodeProvider::builder()
            .model("claude-sonnet-4-20250514")
            .build()?
    )
    .hooks(hooks)
    .build()?;
```

## Alternatives Considered

### Alternative 1: Simple closures only

- **Description**: `agent.on_before_tool(|ctx| async { ... })` — just closures, no trait
- **Pros**: Simplest API, no trait implementation needed
- **Cons**: No shell command support (critical for CLI integration), no composition, no matcher-based filtering, can't share hook implementations across agents, closure lifetime complexities in Rust
- **Why rejected**: Too limited. Shell command hooks are essential for CLI wrapper providers (the Claude Code and Codex CLIs expect hooks to run external commands). Closures also have ergonomic issues with async + lifetimes in Rust.

### Alternative 2: Full TS-style hook system (4 handler types as enum variants)

- **Description**: `HookHandler` enum with Callback, Descriptor, Command, Prompt variants — exactly mirroring TS
- **Pros**: 1:1 feature parity with TS
- **Cons**: Enum-based dispatch is less extensible than traits, mixing handler types in one enum is awkward in Rust, descriptor/prompt types don't map cleanly to Rust patterns
- **Why rejected**: The trait-based approach achieves the same functionality with better Rust ergonomics. `HookChain` provides composition. `ShellCommandHook` covers the command handler. The trait's default methods cover the descriptor pattern.

## Consequences

### Positive

- All 6 lifecycle events available from day one — full feature parity with TS
- Default no-op methods mean consumers implement only what they need
- `HookChain` enables composing multiple hook implementations
- `ShellCommandHook` covers the critical shell command use case
- `ToolMatcher` enables selective hook targeting (per-tool filtering)
- New lifecycle events can be added as default methods without breaking existing consumers
- `#[non_exhaustive]` on `HookEvent` allows future event types

### Negative

- More upfront implementation work than simple closures
- `ShellCommandHook` requires careful subprocess lifecycle management
- `HookChain` ordering semantics must be clearly documented (first-wins for verdicts)

### Risks

- Hook execution adds latency to every tool call (mitigate: hooks are async, empty defaults are zero-cost, benchmark with tracing)
- Shell command hooks hang or leak (mitigate: mandatory timeout, `CancellationToken`, process group kill on timeout)
- Hook panics crash the agent (mitigate: catch_unwind wrapper in HookChain, convert panics to HookError)

## Implementation Notes

- `arky-hooks` crate: `Hooks` trait, `HookChain`, `ShellCommandHook`, `ClosureHook`, context types, verdict types
- `ShellCommandHook` uses `tokio::process::Command` with `kill_on_drop(true)` and `tokio::time::timeout`
- `HookChain::before_tool_call` runs all hooks sequentially; first `Block` verdict short-circuits
- `HookChain::after_tool_call` runs all hooks sequentially; overrides are merged (last wins per field)
- Shell command hooks receive context as JSON on stdin, parse stdout as JSON response (same protocol as TS)
- Hook timeout default: 30 seconds (configurable per hook)
- All context structs are `#[non_exhaustive]` for future field additions

## References

- TS hooks system: `tasks/prd-rust-providers/analysis_core.md` (Section 3: Hooks System — 813 lines, 6 events, 4 handler types)
- TS hook extractors: `providers/core/src/hooks.ts`
- codex-rs hooks: `tasks/prd-rust-providers/analysis_codex_rs.md` (Section 10: Hook System)
- Pi tool hooks: `tasks/prd-rust-providers/analysis_pi_agent.md` (beforeToolCall/afterToolCall)
