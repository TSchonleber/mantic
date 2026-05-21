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

## Agent Runtime

Mantic includes a paper-trading agent runtime that drives an LLM decision loop.
On first launch, the desktop app seeds a `Default Paper Agent`. To exercise the
loop end to end:

1. Open Fleet after pairing and wallet connect.
2. Create an agent with a name, max position size, behavior notes, and an
   Anthropic API key.
3. Arm the agent.
4. Fire a test signal.
5. Watch the live tape for decision and trade events.

### LLM backends

- `anthropic-direct` uses a bring-your-own Anthropic API key stored in the OS
  keychain at `agent-llm-anthropic-key-<agent-id>`.
- `mantic-proxy` is reserved for the hosted proxy and currently returns a
  not-available error.

### Brain persistence

The runtime logs agent registration and config snapshots through brainctl.
Decisions are recorded with `decision_add`, trades with
`event_add(event_type="result")`, skips with
`event_add(event_type="observation")`, and reasoning chains with
`event_add(event_type="decision")`.

## Tests

```bash
pnpm -r test                                            # workspace unit tests
cd apps/desktop/src-tauri && cargo test                 # Rust unit + integration tests
pnpm --filter mantic-desktop lint                       # desktop TypeScript typecheck
pnpm test:smoke                                         # Vite-only Playwright smoke
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
