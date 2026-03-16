# Arky SDK - Repository Guidelines

## Overview

Arky is a Rust SDK for building AI agents. Cargo workspace with 14 crates under `crates/`.

## HIGH PRIORITY

- **IF YOU DON'T CHECK SKILLS** your task will be invalidated and we will generate rework
- **YOU CAN ONLY** finish a task after `make fmt && make lint && make test` are **ALL passing at 100%**. No exceptions.
- **ALWAYS** check dependent crate APIs before writing tests to avoid wrong code
- **NEVER** use workarounds, especially in tests - always use the `no-workarounds` skill for any fix/debug task + `test-anti-patterns` for tests
- **ALWAYS** use the `no-workarounds` and `systematic-debugging` skills when fixing bugs or complex issues
- **ALWAYS** use `requirements-clarity` before implementing ambiguous multi-crate features or underspecified requests
- **USE** `qa-test-planner` when defining regression scope or test strategy for significant changes
- **USE** `adversarial-review` before closing large or high-risk diffs that deserve a critical second pass

## MANDATORY REQUIREMENTS

- **MUST** run `make fmt && make lint && make test` before completing ANY subtask. All three commands must exit with **zero errors and zero warnings**. If any command fails, fix the issues and re-run until all pass.
- **ALWAYS USE** the `rust-engineer` + `rust-best-practices` + `rust-coding-guidelines` skills for ALL Rust work
- **ALWAYS USE** `rust-async-patterns` skill when working with async code, tokio, channels, or streams
- **YOU SHOULD NEVER** add dependencies by hand to Cargo.toml - always use `cargo add` instead
- **THIRDY PARTY LIBRARIES** (just applied when needing external resources):
  - **MANDATORY** Always use `sourcebot` skill (5-7 times) to search code and find information about **EXTERNAL libraries, frameworks, and code patterns**
  - **YOU MUST** use Context7 (multiple times) when the library you research on `sourcebot` is not available
  - **NEVER use Sourcebot to search local project code**. For local code, use `codebase_search` or `Grep`/`Glob` instead

### CRITICAL: Git Commands Restriction

- **ABSOLUTELY FORBIDDEN**: **NEVER** run `git restore`, `git checkout`, `git reset`, `git clean`, `git rm`, or any other git commands that modify or discard working directory changes **WITHOUT EXPLICIT USER PERMISSION**.
- **DATA LOSS RISK**: These commands can **PERMANENTLY LOSE CODE CHANGES** and cannot be easily recovered.
- **REQUIRED ACTION**: If you need to revert or discard changes, **YOU MUST ASK THE USER FIRST** and wait for explicit permission before executing any destructive git command.

### Code Search and Discovery

- **TOOL HIERARCHY**: Use tools in this order:
  1. `codebase_search` (if available) - Preferred semantic search tool
  2. `Grep` or `Glob` (when exact string matching is needed)
- **FORBIDDEN**: Never use `grep` or `find` via Bash for semantic code discovery without first trying dedicated tools.

## Build, Test, and Development Commands

All commands run from repository root:

- `make fmt` - Format Rust code (uses nightly for unstable rustfmt options)
- `make fmt-check` - Check Rust formatting without modifying files
- `make lint` - Run all lints (fmt check + clippy with `-D warnings`)
- `make lint-clippy` - Run clippy lints only
- `make lint-fix` - Auto-fix clippy warnings where possible
- `make check` - Type-check without producing binaries
- `make build` - Build in release mode
- `make test` - Run all tests
- `make coverage` - Print code coverage summary
- `make verify` - Run fmt + lint + test (full verification)
- `make clean` - Clean build artifacts

**MANDATORY Verification (BLOCKING):** Before completing ANY task, you **MUST** run all three commands and they **MUST** all pass at 100%:

1. `make fmt` - Format all code. Must exit cleanly.
2. `make lint` - Must pass with **zero warnings and zero errors** (includes fmt check + clippy with `-D warnings`).
3. `make test` - All tests must pass with **zero failures**.

**If any of these commands fail, the task is NOT complete.** Fix all issues and re-run until all three pass.

## Coding Style & Naming Conventions

- **Edition**: 2024
- **Max line width**: 90
- **Imports granularity**: Crate-level
- **Format**: `make fmt` (nightly channel for unstable options). See `.rustfmt.toml` for full rules.
- **Lint**: `make lint` (clippy with `-D warnings`). See `.clippy.toml` for disallowed macros/methods.
- **Naming**: `snake_case` (fn/var), `CamelCase` (type), `SCREAMING_SNAKE_CASE` (const)
- **No `get_` prefix**: Use `fn name()` not `fn get_name()`
- **Conversions**: `as_` (cheap &), `to_` (expensive), `into_` (ownership)
- **Iterators**: `iter()` / `iter_mut()` / `into_iter()`
- **Newtypes**: Use `struct Email(String)` for domain semantics
- **Pre-allocate**: `Vec::with_capacity()`, `String::with_capacity()`

### Error Handling

- Use `thiserror` for all error types (library code)
- Return `Result<T, E>` for fallible operations; never `panic!` in library code
- Never use `unwrap()` in production code (use `expect()` with messages in dev only)
- Use `?` operator for error propagation, not `match` chains
- Each crate defines its own error enum implementing `ClassifiedError` from `arky-error`
- Error codes follow pattern: `CRATE_ERROR_NAME` (e.g., `PROVIDER_RATE_LIMITED`, `TOOL_TIMEOUT`)

### Async

- All I/O-bound operations are async, using `tokio` runtime
- `CancellationToken` from `tokio-util` for cooperative cancellation
- Never hold locks across `.await` points
- Use `JoinSet` for managing multiple concurrent tasks
- Sync for CPU-bound work; async is for I/O

### Traits

- Keep traits small and focused (4-6 methods max)
- Use dynamic dispatch (`Box<dyn Trait>`) for heterogeneous collections (tools, hooks)
- Static dispatch for monomorphic paths
- All public traits must be `Send + Sync`
- Use `#[async_trait]` for async trait methods

### Testing

- Use `pretty_assertions::assert_eq` instead of `std::assert_eq` (enforced by clippy)
- Name tests descriptively: `process_should_return_error_when_input_empty()`
- One assertion per test when possible
- Use `#[tokio::test]` for async tests
- Use doc tests (`///`) for public API examples

### Documentation

- `//!` for module-level docs
- `///` for public items
- `//` comments explain _why_ (safety, workarounds, design rationale)
- Every `TODO` needs a linked issue: `// TODO(#42): ...`

### Disallowed Patterns (enforced by .clippy.toml)

- **No `log` crate**: Use `tracing` instead
- **No `todo!()`, `dbg!()`, `unimplemented!()`**: Do not commit these
- **No `std::assert_eq`/`std::assert_ne`**: Use `pretty_assertions` versions
- **No `for_each`/`try_for_each`**: Use `for` loops for side-effects
- **No `map_or`/`map_or_else`**: Use `map(..).unwrap_or(..)` for legibility

## Commit & Pull Request Guidelines

- Use Conventional Commits: `feat: ...`, `fix: ...`, `build: ...`, `refactor: ...`, `test: ...`, `docs: ...`
- Before opening a PR: run `make verify` (fmt + lint + test)
- PRs should include: clear description and linked issue
- Do not rewrite unrelated files or reformat whole repo - limit diffs to your change

## Workspace Architecture

```
arky/
  Cargo.toml              (workspace root)
  crates/
    arky/              facade crate, re-exports everything
    arky-error/        shared error classification contracts
    arky-core/         agent loop and orchestration
    arky-provider/     Provider trait, ProviderRegistry
    arky-claude-code/  Claude Code CLI wrapper provider
    arky-codex/        Codex App Server wrapper provider
    arky-tools/        Tool trait, ToolRegistry, ToolResult
    arky-tools-macros/ #[tool] proc macro
    arky-mcp/          MCP client, server, bidirectional bridge
    arky-session/      SessionStore trait, InMemory, SQLite
    arky-hooks/        Hooks trait, HookChain, ShellCommandHook
    arky-config/       Configuration loading and validation
    arky-protocol/     Shared types (Message, AgentEvent, etc.)
    arky-server/       HTTP server for runtime exposure
```

### Crate Dependency Hierarchy (bottom-up)

1. **Leaf crates** (no internal deps): `arky-error`, `arky-protocol`, `arky-config`, `arky-tools-macros`
2. **Foundation**: `arky-tools`, `arky-hooks`, `arky-session`, `arky-provider`
3. **Integration**: `arky-mcp`
4. **Providers**: `arky-claude-code`, `arky-codex`
5. **Orchestration**: `arky-core`
6. **Server**: `arky-server`
7. **Facade**: `arky`

## Agent Skill Dispatch Protocol

Every agent MUST follow this protocol before writing code:

### Step 1: Identify Task Domain

- **Core/Agent**: agent loop, events, state -> `rust-engineer` + `rust-best-practices` + `rust-async-patterns`
- **Provider**: Provider trait, streaming, subprocess -> `rust-engineer` + `rust-async-patterns`
- **Tools**: Tool trait, registry, proc macro -> `rust-engineer` + `rust-coding-guidelines`
- **Hooks**: Hooks trait, lifecycle events -> `rust-engineer` + `rust-async-patterns`
- **Session**: SessionStore, persistence -> `rust-engineer` + `rust-best-practices`
- **MCP**: MCP client/server, bridge -> `rust-engineer` + `rust-async-patterns`
- **Config**: Configuration loading -> `rust-engineer` + `rust-coding-guidelines`
- **Protocol**: Shared types, serialization -> `rust-engineer` + `rust-coding-guidelines`
- **Server**: HTTP server, axum -> `rust-engineer` + `rust-async-patterns`
- **Bug fix**: any domain -> `systematic-debugging` + `no-workarounds` + domain skills
- **Tests**: any domain -> `test-anti-patterns` + domain skills
- **Ambiguous requirements / multi-crate scope** -> `requirements-clarity`
- **Test plan / regression scope** -> `qa-test-planner` + `test-anti-patterns`
- **Large or high-risk diff review** -> `adversarial-review`
- **External lib research** -> `sourcebot` + `deep-research`
- **Skill discovery / capability gaps** -> `find-skills`

### Step 2: Activate All Matching Skills

| Domain | Required Skills | Conditional Skills |
|--------|----------------|-------------------|
| Any Rust code | `rust-engineer` + `rust-best-practices` | + `rust-coding-guidelines` (style/naming) |
| Async code | `rust-async-patterns` | + `rust-engineer` |
| Bug fix | `systematic-debugging` + `no-workarounds` | + `test-anti-patterns` (test failures) |
| Writing tests | `test-anti-patterns` | + domain skill for code being tested |
| Ambiguous requirements | `requirements-clarity` | + domain skills after scope is clear |
| Test planning / regression design | `qa-test-planner` | + `test-anti-patterns` |
| High-risk change review | `adversarial-review` | + `receiving-code-review` (follow-up feedback) |
| External lib research | `sourcebot` | + `deep-research` (complex analysis) |
| Task completion | `verification-before-completion` | |
| Code review response | `receiving-code-review` | |
| Git rebase/conflicts | `git-rebase` | |
| Architecture audit | `architectural-analysis` | + `adversarial-review` (for risky structural changes) |
| Skill discovery / workflow extension | `find-skills` | |
| Parallel agent work | `dispatching-parallel-agents` | |

### Step 3: Verify Before Completion

Before any agent marks a task as complete:

1. Activate `verification-before-completion` skill
2. Run `make fmt && make lint && make test` - all three must pass at 100% with zero errors and zero warnings
3. Read and verify the full output - no skipping
4. Only then claim completion

## Anti-Patterns for Agents

**NEVER do these:**

1. **Skip skill activation** because "it's a small change" - every domain change requires its skill
2. **Activate only one skill** when the task touches multiple domains
3. **Forget `verification-before-completion`** before marking tasks done
4. **Use `sourcebot` for local code** - it's only for external libraries
5. **Write tests without `test-anti-patterns`** - leads to bad test patterns
6. **Fix bugs without `systematic-debugging`** - leads to symptom-patching
7. **Apply workarounds without `no-workarounds`** - type assertions, lint suppressions, error swallowing are all rejected
8. **Start implementation with unclear scope and skip `requirements-clarity`** - this creates avoidable rework
9. **Skip `qa-test-planner` when designing meaningful regression coverage** - this weakens validation quality
10. **Ship large or risky diffs without `adversarial-review`** - this misses obvious failure modes
11. **Complete tasks without running `make fmt && make lint && make test`** - all three must pass. Skipping any invalidates the task.
12. **Claim task is done when any check has warnings or errors** - zero warnings, zero errors, zero test failures. No exceptions.
13. **Use `unwrap()` in library code** - always use `?` or `expect()` with a message
14. **Use `log` crate** - use `tracing` instead (enforced by clippy)
15. **Commit `todo!()`, `dbg!()`, or `unimplemented!()`** - enforced by clippy

## Tech Spec Reference

The full technical specification is at `../compozy-code/tasks/prd-rust-providers/techspec.md`. ADR documents are at `../compozy-code/tasks/prd-rust-providers/adrs/`.
