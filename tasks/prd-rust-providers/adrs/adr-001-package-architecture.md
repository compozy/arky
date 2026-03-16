# ADR-001: Cargo Workspace Multi-Crate Architecture

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

We are building a Rust SDK (codename "Arky") for creating AI agents with multiple LLM providers. The TypeScript `providers/*` packages are the closest upstream implementation reference, and the Pi agent framework is a secondary architectural influence. We need to decide the package structure.

The SDK will live in a separate repository at `~/dev/compozy/arky`, decoupled from the main compozy-code monorepo.

Key constraints:

- Multiple LLM providers (Anthropic, OpenAI, etc.) with different dependencies
- MCP integration (rmcp crate) is a significant dependency
- Session storage backends (in-memory, SQLite) should be optional
- Consumers should only compile what they need
- The codex-rs reference implementation uses a 71-crate workspace successfully

## Decision

Use a **Cargo workspace** with multiple crates, following the multi-crate pattern used by codex-rs. Each major concern gets its own crate with clear boundaries.

Proposed initial workspace structure:

```
arky/
  Cargo.toml          (workspace root)
  crates/
    arky/             (facade crate re-exporting everything)
    arky-error/       (shared error classification contracts)
    arky-protocol/    (shared API types, streaming protocol)
    arky-config/      (configuration management)
    arky-tools/       (tool registry, codec, bridge)
    arky-tools-macros/ (#[tool] proc macro)
    arky-hooks/       (hook system)
    arky-session/     (session store trait + implementations)
    arky-provider/    (provider trait + registry)
    arky-mcp/         (MCP client/server via rmcp)
    arky-claude-code/ (Claude Code CLI wrapper provider)
    arky-codex/       (Codex App Server wrapper provider)
    arky-core/        (agent loop and orchestration)
    arky-server/      (HTTP server for exposing runtime)
```

## Alternatives Considered

### Alternative 1: Single Crate with Feature Flags

- **Description**: One `compozy-sdk` crate with features like `anthropic`, `openai`, `mcp`, `session-sqlite`
- **Pros**: Simpler to consume (`cargo add compozy-sdk -F anthropic`), less Cargo.toml boilerplate
- **Cons**: Compilation is all-or-nothing per feature, module boundaries are conventions not enforced, harder to test independently
- **Why rejected**: For a project of this scope, enforced boundaries between providers and subsystems outweigh the convenience of a single crate

### Alternative 2: Hybrid (Core crate + feature flags for light providers + separate crates for heavy ones)

- **Description**: Core SDK crate with feature-gated providers, separate crates only for heavy dependencies (MCP, SQLite)
- **Pros**: Best of both worlds for consumers
- **Cons**: Inconsistent consumption patterns, unclear boundary between "light" and "heavy"
- **Why rejected**: User preference for full workspace approach, and consistency matters more than marginal convenience

## Consequences

### Positive

- Clear boundaries between subsystems enforced by the compiler
- Independent compilation and testing per crate
- Consumers can depend on only the crates they need
- Follows proven pattern from codex-rs (71 crates in production)
- Each crate can have its own version and changelog

### Negative

- More Cargo.toml boilerplate to maintain
- Inter-crate dependency management requires care
- Initial setup is more work than a single crate

### Risks

- Over-splitting into too many tiny crates (mitigate: start with ~12 crates, split only when justified)
- Circular dependency issues (mitigate: strict layering with `arky-error` and `arky-protocol` as leaf crates)

## Implementation Notes

- Repository location: `~/dev/compozy/arky`
- Workspace root Cargo.toml will define shared dependencies and settings
- Use `[workspace.dependencies]` for version unification
- Each crate will have a `README.md` explaining its role

## References

- codex-rs workspace: `.resources/codex/codex-rs/Cargo.toml` (71 crates)
- Analysis: `tasks/prd-rust-providers/analysis_codex_rs.md`
- Analysis: `tasks/prd-rust-providers/analysis_rust_ecosystem.md`
