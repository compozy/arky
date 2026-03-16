## markdown

## status: pending

<task_context>
<domain>infra/ci</domain>
<type>implementation</type>
<scope>configuration</scope>
<complexity>medium</complexity>
<dependencies>task1,task2,task3,task4,task5,task6,task7,task8,task9,task10,task11,task12,task13,task14,task15</dependencies>
</task_context>

# Task 16.0: CI/CD, Hardening & Documentation

## Overview

Set up CI/CD pipeline, add hardening measures (fixture corpus, benchmarks, dependency graph enforcement), and create developer documentation. This is the final task that ensures the SDK is production-ready with automated quality gates, performance baselines, and comprehensive documentation. Runnable examples are handled by Task 15.0 — this task focuses on CI infrastructure, regression fixtures, benchmarks, and docs.

<critical>
- **ALWAYS READ** @AGENTS.md before start - **MANDATORY SKILLS** must be checked for your domain
- **ALWAYS READ** the technical docs from this PRD before start (techspec.md)
- **YOU CAN ONLY** finish when `cargo fmt && cargo clippy -D warnings && cargo test` pass
- **IF YOU DON'T CHECK SKILLS** your task will be invalid
</critical>

<requirements>
- Set up CI pipeline: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo test --all-features`
- Create provider fixture corpus for protocol regression tests (Claude CLI and Codex JSON-RPC fixtures)
- Implement crate dependency graph enforcement in CI (verify leaf crates stay leaf crates, no cycles)
- Add benchmarks: event throughput, spawn latency, replay overhead
- Create documentation: crate-level docs, architecture overview, getting-started guide
- Ensure all public types have documentation comments
- Set up `cargo doc --no-deps` generation in CI
</requirements>

## Subtasks

- [ ] 16.1 Set up CI configuration (GitHub Actions or equivalent) with fmt, clippy, test, doc jobs
- [ ] 16.2 Create Claude CLI fixture corpus (recorded CLI output for protocol regression tests)
- [ ] 16.3 Create Codex JSON-RPC fixture corpus (recorded JSON-RPC exchanges)
- [ ] 16.4 Implement crate dependency graph validation script (enforce acyclic leaf crate invariant)
- [ ] 16.5 Add benchmarks for event throughput (events per second through stream processing)
- [ ] 16.6 Add benchmarks for provider spawn latency
- [ ] 16.7 Add benchmarks for session replay overhead
- [ ] 16.8 Write architecture overview documentation (`docs/architecture.md`)
- [ ] 16.9 Write getting-started guide (`docs/getting-started.md`)
- [ ] 16.10 Audit all public types for documentation comments, add missing ones
- [ ] 16.11 Set up `cargo doc --no-deps` in CI pipeline
- [ ] 16.12 Verify examples compile in CI (`cargo build --examples` — examples from Task 15.0)

## Implementation Details

### Relevant Files

- `~/dev/compozy/arky/.github/workflows/ci.yml` (or equivalent CI config)
- `~/dev/compozy/arky/docs/architecture.md`
- `~/dev/compozy/arky/docs/getting-started.md`
- `~/dev/compozy/arky/benches/event_throughput.rs`
- `~/dev/compozy/arky/benches/spawn_latency.rs`
- `~/dev/compozy/arky/benches/replay_overhead.rs`
- `~/dev/compozy/arky/scripts/check-deps.sh` (dependency graph validation)
- `~/dev/compozy/arky/crates/arky-claude-code/tests/fixtures/`
- `~/dev/compozy/arky/crates/arky-codex/tests/fixtures/`

### Dependent Files

- All workspace crates (documentation references them)
- `~/dev/compozy/arky/examples/` — Examples from Task 15.0 (verified in CI)
- `tasks/prd-rust-providers/techspec.md` — Sections: Testing Approach, Monitoring & Observability, Standards Compliance

## Deliverables

- CI/CD pipeline configuration with all quality gates
- Provider fixture corpus for regression testing
- Dependency graph enforcement script
- Benchmarks with baseline measurements
- Architecture and getting-started documentation
- All public types documented

## Tests

### Unit Tests (Required)

- [ ] Fixture corpus: each fixture file parses without errors
- [ ] Dependency graph script: detects intentionally introduced cycle, passes on clean graph

### Integration Tests (Required)

- [ ] CI pipeline: full pipeline runs locally (simulated or dry-run)
- [ ] Examples from Task 15.0 compile in CI (`cargo build --examples`)
- [ ] Benchmarks: each benchmark runs without error (verified by `cargo bench --no-run`)

### Regression and Anti-Pattern Guards

- [ ] CI fails on any clippy warning
- [ ] CI fails on any formatting diff
- [ ] CI fails on test failures
- [ ] Documentation generation succeeds without warnings

### Verification Commands

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo test --workspace --all-features`
- [ ] `cargo doc --no-deps --workspace`
- [ ] `cargo build --examples`
- [ ] `cargo bench --no-run`

## Success Criteria

- CI pipeline catches formatting, lint, and test failures
- Fixture corpus covers major protocol scenarios for both providers
- Dependency graph enforcement prevents accidental cycles
- Benchmarks establish performance baselines
- Documentation is comprehensive
- All public types are documented
- All verification commands pass

---

## Notes

- Save executable task files as `_task_<number>.md` (example: `_task_16.md`).
- `scripts/markdown/check.go` in `prd-tasks` mode discovers only files matching `^_task_\d+\.md$`.
- Keep `## status:` and `<task_context>` fields intact so parser metadata is available in execution prompts.
