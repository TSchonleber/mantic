# Mantic

Trading agent platform powered by brainctl. See `docs/superpowers/specs/` for design, `docs/superpowers/plans/` for implementation plans.

## Repo layout

- `apps/desktop` — Tauri 2 desktop app (the product surface)
- `services/mock-license` — dev-only mock license server (stand-in for brainctl.org license endpoints)
- `e2e` — smoke test for the desktop binary
- `docs/superpowers` — design specs and implementation plans

## Quickstart (development)

```bash
pnpm install

# terminal 1
pnpm dev:mock-license

# terminal 2
pnpm dev:desktop
```

Use pairing code `123456` (or any non-empty value) to pair the dev build against the mock server.

## Tests

```bash
pnpm -r test                                # frontend + service unit tests (17 total)
cd apps/desktop/src-tauri && cargo test     # Rust unit tests (13 total)
pnpm --filter mantic-e2e test               # binary smoke test (requires release build first)
```

## Build a release binary

Two paths:

**No bundler (recommended for local dev — no DMG/installer):**
```bash
pnpm --filter mantic-desktop build      # vite builds frontend
cd apps/desktop/src-tauri && cargo build --release
# binary at apps/desktop/src-tauri/target/release/desktop
```

**Full bundle (DMG/exe/AppImage — pops Finder on macOS during DMG layout):**
```bash
pnpm build:desktop
# artifacts under apps/desktop/src-tauri/target/release/bundle/
```

## License

TBD before public release.
