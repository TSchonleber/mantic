# Mantic

Trading agent platform powered by brainctl. See `docs/superpowers/specs/` for design, `docs/superpowers/plans/` for implementation plans.

## Repo layout

- `apps/desktop` — Tauri 2 desktop app (the product surface)
- `services/mock-license` — dev-only mock license server
- `e2e` — smoke test for the desktop binary
- `docs/superpowers` — design specs and implementation plans

## Prerequisites

- Node 20+, pnpm 10+
- Rust stable (1.78+)
- Xcode CLT (macOS) / WebView2 (Windows) / webkit2gtk (Linux)
- Python 3.11+ (only if you want to rebuild the brainctl-mcp sidecar locally)

## Sidecar binary

The desktop app embeds `brainctl-mcp` as a sidecar. Build it once locally:

```bash
cd /Users/r4vager/work/brainctl
./scripts/build_mcp_bundle.sh
# produces dist/brainctl-mcp-<target-triple>

cp dist/brainctl-mcp-aarch64-apple-darwin \
   /path/to/Mantic/apps/desktop/src-tauri/binaries/
```

Or pull a prebuilt binary from a brainctl GitHub release once the build workflow has run.

## Quickstart (development)

```bash
pnpm install

# terminal 1
pnpm dev:mock-license

# terminal 2
pnpm dev:desktop
```

Use any 4+ character pairing code on first launch.

## Wallet connect (Solana)

Mantic uses a localhost web bridge to connect to Phantom or Solflare. The bridge ships
as a single-file HTML asset built from `apps/wallet-bridge/`. Build it locally:

```bash
pnpm --filter wallet-bridge build
```

The Rust build step (`build.rs`) copies the result into Tauri resources at compile
time. If `tauri dev` warns about a missing wallet-bridge bundle, run the build
command above and re-run.

### Wallet model

Mantic generates a Solana ed25519 session keypair locally. You authorize it by
signing an off-chain message in Phantom (or Solflare). The session wallet is the
trading wallet — you fund it manually from your master wallet, and Mantic has
full control over its contents. To stop trading, just drain the session wallet
back to your master.

## Agent Runtime (sub-project #4)

Mantic ships a paper-trading agent runtime that drives an LLM-based decision
loop. On first launch a "Default Paper Agent" is auto-created. To exercise
the loop end-to-end:

1. Open Fleet (post-wallet landing screen).
2. Click "Create Agent" — set a name, max position, NL overlay, and paste
   an Anthropic API key. Click Create.
3. Click "Arm" on the agent row.
4. Click "Fire test signal" — a synthetic signal for `BONK` is dispatched.
5. Watch the live tape: decision event + (if buy/sell) trade record appear
   within 1–3 seconds.

### LLM backends

- `anthropic-direct` (BYO key) — works today. Key is stored in the OS
  keychain at `agent-llm-anthropic-key-<agent-id>`.
- `mantic-proxy` — returns "Mantic proxy is not available yet" until
  sub-project #8 lands.

### Brain persistence

Every decision is logged via `decision_add`; every trade via
`event_add(event_type="result")`; every skip via `event_add(event_type="observation")`;
every reasoning chain via `event_add(event_type="decision")`. The Mantic
client uses agent-id `mantic-desktop`; per-agent identity lives as a
brain.db entity in scope `project:mantic`.

## Tests

```bash
pnpm -r test                                            # 6 mock-license + 52 desktop frontend + 2 e2e
cd apps/desktop/src-tauri && cargo test                 # ~93 Rust unit tests + 3 integration (brainctl + wallet + agent loop)
pnpm --filter mantic-desktop-smoke test                 # Playwright real-browser smoke for Fleet (requires `playwright install chromium`)
pnpm --filter mantic-e2e test                           # binary smoke (requires release build)
```

## Build a release binary

```bash
# No bundler — produces just the executable, no DMG
pnpm --filter mantic-desktop build
cd apps/desktop/src-tauri && cargo build --release

# Full bundle (DMG/exe/AppImage)
pnpm build:desktop
```

## License

Apache License 2.0. See [LICENSE](./LICENSE).
