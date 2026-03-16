# ADR-005: Tool System with Trait Base + Proc Macro Convenience

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

The tool system is the most complex subsystem in the SDK. Tools are functions that agents can call to interact with the external world (read files, search, execute commands, etc.). We need to define how consumers create, register, and execute tools.

Key constraints from Rust skills:

- `rust-coding-guidelines`: "Avoid macros unless necessary. Prefer functions/generics"
- `rust-best-practices ch.6`: "Favor static dispatch until your trait needs to live behind a pointer" — tools need `Vec<Box<dyn Tool>>` (heterogeneous collection), so dynamic dispatch is appropriate here
- `rust-best-practices ch.7`: Type State Pattern could enforce required fields in tool builders
- `rust-engineer/traits.md`: "Keep traits small and focused"

Reference implementations:

- `rmcp` crate uses `#[tool]` proc macro for ergonomic tool definition
- codex-rs uses a `ToolRouter` + `ToolRegistry` with `ToolPayload` enum dispatch
- Pi uses TypeBox schemas with typed `execute()` functions
- Our TS providers use JSON Schema based tools with dynamic dispatch

## Decision

Implement a **dual approach**: a small, focused `Tool` trait as the fundamental API, plus a `#[tool]` proc macro for convenience. Both produce the same `Box<dyn Tool>`.

### Layer 1: `Tool` trait (crate `arky-tools`)

```rust
/// Core tool trait — small and focused per rust-engineer guidelines.
/// Object-safe for heterogeneous collections (Vec<Box<dyn Tool>>).
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name
    fn name(&self) -> &str;
    /// Human-readable description for LLM context
    fn description(&self) -> &str;
    /// JSON Schema for the tool's input parameters
    fn input_schema(&self) -> serde_json::Value;
    /// Execute the tool with validated arguments
    async fn execute(
        &self,
        call_id: &str,
        args: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError>;
}

/// Tool result supporting text and structured data
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

pub enum ToolContent {
    Text(String),
    Image { data: Vec<u8>, media_type: String },
    Json(serde_json::Value),
}
```

### Layer 2: `#[tool]` proc macro (crate `arky-tools-macros`)

```rust
use arky_tools_macros::tool;

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadFileArgs {
    /// File path to read
    path: String,
    /// Maximum number of lines
    max_lines: Option<u32>,
}

#[tool(name = "read_file", description = "Read a file from disk")]
async fn read_file(args: ReadFileArgs, cancel: CancellationToken) -> Result<String, ToolError> {
    let content = tokio::fs::read_to_string(&args.path).await?;
    Ok(match args.max_lines {
        Some(n) => content.lines().take(n as usize).collect::<Vec<_>>().join("\n"),
        None => content,
    })
}
// Expands to a struct `ReadFileTool` implementing `Tool` trait
```

### Tool Registry

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Box<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<&dyn Tool>;
    pub fn list(&self) -> Vec<&dyn Tool>;
    pub fn schemas(&self) -> Vec<serde_json::Value>;
}
```

## Alternatives Considered

### Alternative 1: Trait only with `schemars` derive (no proc macro)

- **Description**: Only the `Tool` trait, consumers use `schemars::JsonSchema` derive on args structs and manually implement the trait
- **Pros**: Follows "avoid macros unless necessary" guideline strictly, simpler build, no proc macro dependency
- **Cons**: Significant boilerplate per tool (6 methods to implement), discourages tool creation, higher barrier to entry
- **Why rejected**: While aligned with macro-avoidance guidelines, the boilerplate cost is too high for a tool-heavy SDK. The rmcp crate and codex-rs both validate that `#[tool]` macros dramatically improve ergonomics. The guideline says "unless necessary" — for dozens of tools, the macro IS necessary.

### Alternative 2: Closure-based helper functions (no trait, no macro)

- **Description**: `fn tool<A>(name, description, handler) -> Box<dyn Tool>` using closures
- **Pros**: Zero boilerplate, zero macros, functional style
- **Cons**: Loses IDE support (no struct to navigate to), poor error messages, schema generation becomes implicit, harder to test individual tools, harder to compose
- **Why rejected**: Sacrifices too much type safety and developer experience for convenience

## Consequences

### Positive

- Two clear paths: trait for control, macro for productivity
- `Tool` trait is small (4 methods) and object-safe
- JSON Schema via `schemars` is standard ecosystem practice
- `CancellationToken` support enables cooperative cancellation
- Both paths produce identical `Box<dyn Tool>` — consumers don't know which was used
- Proc macro lives in a separate crate — optional dependency

### Negative

- Proc macro adds build-time complexity (separate crate, syn/quote deps)
- Two ways to define tools may confuse new users
- Proc macro must be maintained alongside trait changes

### Risks

- Proc macro output diverges from manual trait impl behavior (mitigate: extensive macro expansion tests, document exactly what the macro generates)
- `schemars` crate compatibility issues (mitigate: pin version, test schema output)
- Tool schema validation at registration vs execution time (mitigate: validate at registration, fail fast)

## Implementation Notes

- `arky-tools` crate: `Tool` trait, `ToolRegistry`, `ToolResult`, `ToolError`
- `arky-tools-macros` crate: `#[tool]` proc macro (depends on syn, quote, proc-macro2)
- Args validation: deserialize `serde_json::Value` into typed struct at execution time, return `ToolError::InvalidArgs` on failure
- Schema generation: `schemars::schema_for!()` macro at tool registration, not per-call
- Dynamic dispatch: `Vec<Box<dyn Tool>>` for collections, `&dyn Tool` for references (per ch.6 guidance)
- Cancellation: `tokio_util::sync::CancellationToken` passed to all tool executions

## References

- rmcp `#[tool]` macro: `.resources/codex/codex-rs/rmcp-client/`
- codex-rs ToolRouter: `tasks/prd-rust-providers/analysis_codex_rs.md`
- Rust trait design: `.claude/skills/rust-engineer/references/traits.md`
- Static vs dynamic dispatch: `.claude/skills/rust-best-practices/references/chapter_06.md`
- Type state pattern: `.claude/skills/rust-best-practices/references/chapter_07.md`
