# ADR-010: Arky as SDK Name and Crate Prefix

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

We need a consistent naming convention for the repository, workspace, crates, and public API. The codename "Arky" was chosen for the project.

## Decision

Use **`arky`** as the SDK name, crate prefix, and facade crate name.

### Crate naming convention

| Crate        | Name                | Description                                         |
| ------------ | ------------------- | --------------------------------------------------- |
| Facade       | `arky`              | Re-exports everything, `use arky::prelude::*`       |
| Error        | `arky-error`        | Shared error classification contracts               |
| Core         | `arky-core`         | Agent loop, events, traits, errors, ClassifiedError |
| Provider     | `arky-provider`     | Provider trait, registry                            |
| Claude Code  | `arky-claude-code`  | Claude Code CLI wrapper provider                    |
| Codex        | `arky-codex`        | Codex App Server wrapper provider                   |
| Tools        | `arky-tools`        | Tool trait, registry, codec                         |
| Tools Macros | `arky-tools-macros` | `#[tool]` proc macro                                |
| MCP          | `arky-mcp`          | MCP client, server, tool bridge                     |
| Session      | `arky-session`      | SessionStore trait, InMemory, SQLite                |
| Hooks        | `arky-hooks`        | Hooks trait, HookChain, ShellCommandHook            |
| Config       | `arky-config`       | Configuration management                            |
| Protocol     | `arky-protocol`     | Shared types (Message, AgentEvent, etc.)            |
| Server       | `arky-server`       | HTTP server for runtime exposure                    |

### Repository structure

```
~/dev/compozy/arky/
  Cargo.toml
  crates/
    arky/              (facade)
    arky-error/
    arky-core/
    arky-provider/
    arky-claude-code/
    arky-codex/
    arky-tools/
    arky-tools-macros/
    arky-mcp/
    arky-session/
    arky-hooks/
    arky-config/
    arky-protocol/
    arky-server/
```

### Public API

```rust
// Facade usage
use arky::prelude::*;

// Or direct crate usage
use arky_core::Agent;
use arky_provider::Provider;
use arky_tools::Tool;
```

## Alternatives Considered

### Alternative 1: compozy-sdk as crate name

- **Description**: `compozy-sdk-core`, `compozy-sdk-provider`, etc.
- **Pros**: Brand alignment with compozy
- **Cons**: Verbose crate names, `compozy-sdk-` prefix is 12 characters before the actual module name
- **Why rejected**: `arky-` is shorter and the SDK is a separate product from the compozy app

## Consequences

### Positive

- Short, memorable, unique name
- Consistent `arky-*` prefix across all crates
- Clean facade: `use arky::prelude::*`

### Negative

- Name is not self-descriptive (doesn't say "AI agent SDK")
- Need good README/docs to explain what arky is

## Implementation Notes

- Repository: `~/dev/compozy/arky`
- Cargo workspace members use `crates/arky-*` paths
- Facade crate re-exports with `pub use arky_core::*` etc.
- Prelude module includes most commonly used types

## References

- ADR-001: Package architecture decision
