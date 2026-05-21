# Mantic Sub-project #4 Race Brief

Hand this to Windsurf Cascade, Devin, or Codex. Paste in full.

---

## Mission

Execute the implementation plan at `docs/superpowers/plans/2026-05-21-agent-runtime.md` end-to-end. Ship sub-project #4 (agent runtime) for Mantic. **22 tasks. TDD throughout. Each task is bite-sized with exact code, file paths, and commands.** Follow the plan task-by-task; don't skip or improvise.

## Race rules

You are racing three other agents. Each is on their own branch:
- `race/claude` — Claude Code (Anthropic Opus 4.7)
- `race/windsurf` — Windsurf Cascade
- `race/devin` — Devin
- `race/codex` — OpenAI Codex

**Your branch is the one named after you.** Do NOT touch other agents' branches.

Win conditions, in priority order:
1. **All tests pass** — Rust unit, frontend Vitest, Rust integration, Playwright smoke
2. **Plan fidelity** — followed the spec'd file structure, types, commands
3. **No regression in existing tests** — the 86 pre-existing tests must still pass
4. **Speed** — first to satisfy 1–3 wins
5. **Code quality** — tie-breaker; clean, idiomatic, no over-engineering

## Repo

```
URL:     https://github.com/TSchonleber/mantic
Clone:   git clone https://github.com/TSchonleber/mantic.git
Branch:  race/<your-agent-name>
```

You'll need access — Terrence is the owner (TSchonleber). If you don't have push access, fork and open a PR.

## Pre-flight

```bash
git checkout race/<your-agent-name>
node --version    # >= 20
pnpm --version    # >= 10
rustc --version   # >= 1.78
cargo --version
xcode-select -p   # macOS only
```

## Where the plan and spec live

- **Plan (your task list):** `docs/superpowers/plans/2026-05-21-agent-runtime.md`
- **Spec (the why):** `docs/superpowers/specs/2026-05-21-agent-runtime-design.md`
- **Prior sub-project specs** (read these for context): `docs/superpowers/specs/2026-05-21-mantic-design.md`, `2026-05-21-brain-db-integration-design.md`, `2026-05-21-wallet-session-signer-design.md`

## What already exists (don't redefine)

Sub-projects #1, #2, #3 are complete and merged to main. The plan imports from:

**Rust** (`apps/desktop/src-tauri/src/`):
- `account.rs` — `AccountInfo`, `decode_claims`
- `brain_db.rs` — `BrainDb` (rusqlite reads)
- `brainctl_client.rs` — `BrainctlClient` with typed methods: `memory_add`, `event_add`, `decision_add`, `entity_create`, `entity_observe`, `agent_register`, `agent_wrap_up`, `agent_orient`, `memory_search`
- `bundle.rs` — sidecar resolver
- `commands.rs`, `error.rs`, `heartbeat.rs`, `license.rs`, `mcp_codec.rs`, `pairing.rs`, `state.rs`
- `wallet/` — full Solana wallet integration

**Frontend** (`apps/desktop/src/`):
- `App.tsx` — 4-state routing (loading | unpaired | paired-no-wallet | paired-with-wallet)
- `lib/tauri-bridge.ts` — typed invoke wrappers
- `routes/Pair.tsx`, `Wallet.tsx`, `Account.tsx`

**Workspaces**:
- `apps/desktop` (Tauri 2 + React + TS + Tailwind 4 + React Router 7 + Vitest)
- `apps/wallet-bridge` (single-file React build for wallet connect)
- `services/mock-license` (Hono mock license server)
- `e2e` (Vitest binary smoke test)

## Critical corrections — bake these in from the start

These were learned the hard way in past sub-projects. The plan has them but you may forget:

1. **Tauri 2 lib.rs pattern.** `apps/desktop/src-tauri/src/main.rs` is a 6-line shim. The Tauri builder, `setup()` callback, and `invoke_handler!` macro all live in `lib.rs`. Module declarations and command registration go in `lib.rs`, NOT `main.rs`.

2. **DO NOT run `pnpm tauri build` or `pnpm build:desktop`.** macOS DMG bundler pops Finder windows during DMG layout rendering. The user is on stream — popping windows is bad. Use `cargo check` / `cargo test` / `pnpm --filter mantic-desktop test` for verification. For building the binary without bundling: `pnpm --filter mantic-desktop build && cd apps/desktop/src-tauri && cargo build --release`.

3. **DO NOT run `pnpm dev:desktop` casually.** It opens the Tauri window. Only use it if you genuinely need to inspect the live app. The Playwright smoke test uses `pnpm --filter mantic-desktop dev` (Vite only, no Tauri window).

4. **Tailwind 4 + Vite plugin.** The desktop app uses `@tailwindcss/vite` (NOT the PostCSS plugin — that was migrated away from due to a white-box rendering bug). New components use Tailwind classes (e.g., `bg-neutral-950`, `text-neutral-100`, `rounded-2xl`). Match existing route styles in `Pair.tsx`, `Wallet.tsx`, `Account.tsx`.

5. **brainctl-mcp MCP envelope + agent_id.** Direct method names won't work — wrap as `tools/call` with `{name, arguments}`. `agent_id` is the first positional on every brainctl tool. `BrainctlClient`'s typed methods already handle both internally — just call them.

6. **brainctl auto-migrate is in place.** The sidecar bootstraps brain.db schema on fresh DBs. No special handling.

7. **Real-browser smoke test (Task 21) is non-negotiable.** Vitest+jsdom does NOT render CSS. Sub-project #3 shipped with a Tailwind misconfiguration that all 88 tests missed because none rendered actual CSS. The Playwright test against Vite dev server is the canary.

8. **No emojis in code or commits unless explicitly asked.** Follow the existing style.

9. **No commits to main.** Stay on your `race/<agent>` branch. Push freely to your own branch — others can't see your work that way.

## Baseline test counts (don't regress)

| Suite | Count |
|---|---|
| mock-license | 6 |
| desktop frontend (Vitest) | 34 |
| desktop Rust lib (unit) | 44 |
| desktop Rust integration | 2 |
| e2e (binary smoke) | 2 |
| **Total** | **88** |

After your work, expect ~52 frontend + ~93 Rust unit + 3 Rust integration + 1 Playwright smoke (counts from plan).

## Reporting your finish

When done:
1. Push your branch
2. Open a PR against `main` titled `[race/<agent>] Sub-project #4 — agent runtime`
3. In the PR body, include: total tests passing, time-to-finish, any deviations from the plan
4. Tag Terrence (`@TSchonleber`)

## Sensitive context

- Terrence is **live-streaming** the race. Don't expose API keys, wallet addresses, or anything sensitive on-screen.
- Avoid opening browser windows, dialog boxes, or any GUI that takes focus on macOS.
- Anthropic API key for LLM testing — leave as BYO; do NOT hardcode a test key in any file.

## Tools you have access to

The plan itself enumerates all the libraries and tools needed:
- Rust: `tokio`, `reqwest`, `async-trait`, `serde`, `rusqlite`, `keyring`, `axum`, `ed25519-dalek` (all already in Cargo.toml)
- Frontend: React 19, TS 5, Vite 7, Tailwind 4 via `@tailwindcss/vite`, Vitest, React Router 7
- E2E: Playwright (need to add to a new or existing workspace per Task 21)

## Anything not in the plan?

If the plan tells you to do X and reality requires Y, follow reality but commit the deviation with a clear message (`fix(plan-correction): <what you did and why>`). Three past sub-projects have done this; it's expected.

---

**Race begins now. May the best agent win.**
