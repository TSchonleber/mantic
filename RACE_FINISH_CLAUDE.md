# Race Finish — Claude (race/claude)

Sub-project #4: Agent Runtime — paper-trading LLM decision loop, Fleet view,
LiveTape, AgentConfig modal, first-run seed, integration + Playwright tests.

## Summary

- **Total tasks completed:** 22 / 22
- **Plan:** `docs/superpowers/plans/2026-05-21-agent-runtime.md`
- **Branch:** `race/claude`
- **Final commit SHA (test sweep / README):** `396a7f27c12db2e51d7eba1f7b61b784fa8c13e3`
- **Final commit SHA (this report):** see `git log -1 race/claude` after this
  file lands.
- **Time-to-finish (rough):** ~12 hours wall (first commit on race/claude at
  2026-05-21 03:37 ET, final commits at 2026-05-21 15:42+ ET).

## Final test totals (all green)

| Suite                         | Tests                              | Status |
|-------------------------------|------------------------------------|--------|
| mock-license (vitest)         | 6                                  | pass   |
| desktop frontend (vitest)     | 52 (across 9 files)                | pass   |
| desktop Rust lib (cargo test) | 93                                 | pass   |
| Rust integration: brainctl    | 1 (skips cleanly if binary absent) | pass   |
| Rust integration: wallet      | 1                                  | pass   |
| Rust integration: agent loop  | 1 (gated on `test-helpers` feat.)  | pass   |
| Playwright smoke (Fleet)      | 1 (gated on chromium install)      | pass   |
| Desktop e2e binary smoke      | 2 (release build smoke)            | pass   |

Total runtime tests: **157** (6 + 52 + 93 + 3 integration + 1 Playwright + 2 e2e).

## How to reproduce locally

```bash
cd /Users/r4vager/Documents/Mantic
pnpm install
pnpm -r test                          # 6 + 52 + 1 (Playwright) + 2 (e2e)
cd apps/desktop/src-tauri
cargo test                            # 93 lib + 1 brainctl + 1 wallet
cargo test --features test-helpers --test agent_loop_integration   # +1
```

## Plan corrections made during execution

1. **Tauri 2 builder lives in `lib.rs`, not `main.rs`** (memory 2020). All
   tasks 5–14 that reference `main.rs` had to edit `lib.rs` instead.
2. **`tests/agent_loop_integration.rs` must be feature-gated.** The test uses
   `AgentRuntime::spawn_with_backend` and `AgentRuntime::executor()`, both
   under `#[cfg(any(test, feature = "test-helpers"))]`. Without a top-of-file
   `#![cfg(feature = "test-helpers")]` gate, default `cargo test` tries to
   compile the integration test and fails with E0599 on the private items.
   Task 22 added the gate; the file is now invisible to default `cargo test`
   and only compiles when the feature is on.
3. **Stray Finder duplicate `wallet-bridge 2.html`** in
   `apps/desktop/src-tauri/resources/` removed in Task 22.

## Known pre-existing issues (NOT in scope for this race)

- **`pnpm --filter mantic-desktop lint` is not clean.** `tsc --noEmit`
  surfaces 6 TS2345 errors in `apps/desktop/src/lib/tauri-bridge.ts` where
  typed input interfaces don't satisfy Tauri's `InvokeArgs = Record<string,
  unknown>` constraint. These errors exist at HEAD `af01153` (verified by
  git stash + re-run) and predate Task 22. Cheap fix: extend the input
  interfaces with `[k: string]: unknown` or pass the input through `as
  Record<string, unknown>` at the `invoke()` call site. Not done here
  because (a) it changes type ergonomics across all bridge wrappers and
  (b) the user's explicit test sweep for Task 22 did not include `lint`.
  Recommended for the next round to clean up.

## Recommendations for the next round (sub-project #5 onward)

1. **Fix the `tauri-bridge.ts` lint errors** as a prelude — five minutes of
   work, unblocks `pnpm lint` as a CI gate.
2. **Migrate `OnceLock<keyring::Entry>` in `license.rs` to Tauri `State<T>`**
   when SQLite connection pooling lands (memory 2021). The current static
   pattern is fine for one secret but won't compose with shared mutable
   resources.
3. **Validate the refreshed token in `heartbeat.rs::tick()` before
   persisting** (memory 2022). Mirror `pair_with_code`'s pattern: call
   `decode_claims(&new_token)?` before `license::save()`. Also: tick()
   short-circuits on `LicenseExpired` so a locally-expired token never
   reaches `/v1/refresh`; consider sending it anyway and letting the server
   reject it.
4. **`mantic-proxy` LlmBackend is a placeholder** that returns
   "Mantic proxy is not available yet". Wire it to a real proxy endpoint as
   sub-project #8 lands.
5. **Three dead-code warnings** in `mcp_codec.rs` (`RpcError.data`) and
   `wallet/bridge.rs` (`SharedState.auth_message`, `BridgeServer.state`).
   Either start using them or annotate `#[allow(dead_code)]`.
6. **No real signal source yet** — `MockSignalSource` + `Signal::new_test`
   are the only producers. Sub-project #5 (signals from JIN/Helius/Birdeye)
   plugs into `AgentRuntime::dispatch`.

## Brainctl footprint

- Agent id used by integration code: `mantic-desktop`.
- Per-agent identity entities are scoped `project:mantic` in brain.db.
- Every decision → `decision_add`. Every trade → `event_add(result)`.
  Every skip → `event_add(observation)`. Reasoning chains →
  `event_add(decision)`.
