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

## Tests

```bash
pnpm -r test                                            # 6 mock-license + 34 desktop frontend + 2 e2e
cd apps/desktop/src-tauri && cargo test                 # 44 Rust unit tests + 2 integration
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
