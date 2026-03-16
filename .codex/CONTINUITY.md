Goal (incl. success criteria):
- Update `/Users/pedronauck/Dev/compozy/arky` so the latest stable Rust release is enforced by default and the pinned nightly used for formatting is updated where needed.
- Success: stable and nightly pins updated in the repo’s enforcement/configuration files, and `make fmt && make lint && make test` all pass on the updated configuration.

Constraints/Assumptions:
- Do not use destructive git commands.
- Any manual file edits must use `apply_patch`.
- External version information must be verified from current upstream sources, not memory.
- Keep changes scoped to version enforcement/configuration surfaces unless verification forces a code fix.

Key decisions:
- Treat `rust-toolchain.toml`, workspace `rust-version`, and Makefile nightly pin as the repo’s effective Rust enforcement surface.
- Use stable `1.94.0` based on the official Rust release notes and use the latest available nightly reported by `rustup check`, validating that the exact nightly installs successfully before pinning it.
- Re-run the repo’s mandated verification commands after changing the pins.

State:
- Concluido.

Done:
- Verified official Rust release notes page lists `Version 1.94.0 (2026-03-05)` as the latest stable release.
- Confirmed the repo previously pinned stable `1.85.0` in `rust-toolchain.toml` and workspace `rust-version`, and pinned formatting nightly `nightly-2026-02-20` in `Makefile`.
- Confirmed the workspace already builds, lints, and tests cleanly on stable `1.94.0`.
- Validated and installed `nightly-2026-03-15` for formatting.
- Updated `rust-toolchain.toml` to stable `1.94.0`, updated workspace `rust-version` to `1.94.0`, and updated `Makefile` nightly pin to `nightly-2026-03-15`.
- Ran `make fmt`, `make lint`, and `make test`, all passing on the updated configuration.

Now:
- Nenhum trabalho ativo.

Next:
- Aguardar a próxima instrução do usuário.

Open questions (UNCONFIRMED if needed):
- None at the moment.

Working set (files/ids/commands):
- `/Users/pedronauck/Dev/compozy/arky/.codex/CONTINUITY.md`
- `/Users/pedronauck/Dev/compozy/arky/rust-toolchain.toml`
- `/Users/pedronauck/Dev/compozy/arky/Cargo.toml`
- `/Users/pedronauck/Dev/compozy/arky/Makefile`
- `/Users/pedronauck/Dev/compozy/arky/.rustfmt.toml`
- `/Users/pedronauck/Dev/compozy/arky/.clippy.toml`
- `rustc -Vv`
- `rustup show active-toolchain`
- `rustup check`
- `rustup toolchain install 1.94.0 --profile minimal -c clippy -c rustfmt`
- `cargo +1.94.0 check --all-targets --all-features`
- `cargo +1.94.0 clippy --all-targets --all-features -- -D warnings`
- `cargo +1.94.0 test --all-features`
- `make fmt`
- `make lint`
- `make test`
- `curl -Ls -o /dev/null -w '%{url_effective}\n' https://blog.rust-lang.org/releases/latest/`
- `https://doc.rust-lang.org/stable/releases.html`
