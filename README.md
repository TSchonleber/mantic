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

## Tests

```bash
pnpm -r test                                            # 28 frontend + service tests
cd apps/desktop/src-tauri && cargo test                 # 25 Rust unit tests + 1 integration
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

TBD before public release.
