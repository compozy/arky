# Claude-Compatible Provider Profiles for Arky

## Summary
- Add first-class Claude-compatible providers to Arky for `claude-code`, `zai`, `openrouter`, `vercel`, `moonshot`, `minimax`, `bedrock`, `vertex`, and `ollama`, while keeping one shared Claude CLI harness.
- Expose distinct Rust wrapper types per provider, but delegate all execution to a single internal Claude-compatible runtime in `crates/arky-claude-code`.

## Key Changes
- Introduce one internal `ClaudeProviderProfile` layer in `crates/arky-claude-code` as the canonical source for supported Claude-compatible provider IDs, descriptor labels, selected-model rules, and provider-specific env shaping.
- Refactor the shared Claude engine so descriptor construction is profile-driven instead of hard-coding `ProviderId::new("claude-code")`.
- Keep `ClaudeCodeProvider` as the direct Anthropic/Claude path and add distinct wrapper types: `BedrockProvider`, `ZaiProvider`, `OpenRouterProvider`, `VercelProvider`, `MoonshotProvider`, `MinimaxProvider`, `VertexProvider`, and `OllamaProvider`.
- Add a shared base config, `ClaudeCompatibleProviderConfig`, for common Claude CLI settings, then layer typed wrapper configs over it.
- Preserve a raw escape hatch only in the shared base config via generic `env` and `extra_args`; wrapper-specific required fields remain typed and explicit.
- Define one env precedence rule in the shared engine and apply it to every wrapper.
- Keep runtime Claude alias and upstream selected model as separate concepts.
- Update `crates/arky-config/src/loader.rs` and related wiring so the already-recognized kinds map to real first-class provider identities.
- Re-export the new wrapper types and config types from `crates/arky/src/lib.rs` under the existing `claude-code` feature.
- Keep provider family behavior Claude-compatible for reasoning/tool support, but persist and surface the actual provider ID for registry, session, metadata, and usage.

## Public API / Interfaces
- New shared internal type: `ClaudeProviderProfile`
- New shared public config: `ClaudeCompatibleProviderConfig`
- New public wrapper types: `BedrockProvider`, `ZaiProvider`, `OpenRouterProvider`, `VercelProvider`, `MoonshotProvider`, `MinimaxProvider`, `VertexProvider`, `OllamaProvider`
- New public wrapper config types matching each wrapper
- Existing `ClaudeCodeProvider` remains supported and behavior-compatible
- `ProviderDescriptor.id` for Claude-backed wrappers must be their actual ID, while `ProviderDescriptor.family` remains `ProviderFamily::ClaudeCode`

## Test Plan
- Shared harness regression tests should prove the refactor preserves base `ClaudeCodeProvider` subprocess config, stream parsing, nested tools, warning behavior, and session resume semantics.
- Wrapper profile tests should table-drive all nine provider IDs and assert descriptor ID, family, env shaping, selected-model handling, and required typed fields.
- Config integration tests should verify each supported `kind` resolves to the correct concrete provider identity where exposed by the API.
- Facade tests should verify the new wrappers and config types are re-exported under the `claude-code` feature.
- Registry tests should verify multiple Claude-compatible provider instances can coexist and resolve by explicit provider ID without ambiguity.
- Session and metadata tests should verify persisted provider/session state uses the concrete provider ID rather than collapsing to `claude-code`.

## Implementation Order
- Step 1: Add the Claude-compatible profile module and supported provider ID constants in `arky-claude-code`.
- Step 2: Refactor the shared Claude engine to be profile-driven while keeping `ClaudeCodeProvider` behavior unchanged.
- Step 3: Add typed wrapper configs and wrapper provider structs for the full nine-provider set.
- Step 4: Add public mapping/helpers for supported provider kinds and update facade exports.
- Step 5: Add wrapper-focused tests and update docs/examples where needed.

## Assumptions and Boundaries
- This implementation is additive and phased, not a big-bang rewrite.
- No separate crates per derived provider.
- No provider-specific parser or event-pipeline forks unless a real upstream protocol divergence appears later.
- Model discovery changes are out of scope unless needed to avoid incorrect provider IDs in already-existing discovery surfaces.
