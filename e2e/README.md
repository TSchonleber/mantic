# Mantic E2E

Smoke test for the desktop binary: launches the release build, gives it 5 seconds to initialize, verifies it didn't crash.

A full UI-driving E2E (Playwright + tauri-driver) is deferred to a later milestone once `tauri-driver` matures across all three platforms.

## Run

```bash
# 1. Build the binary (NO bundler — avoids macOS Finder pop)
pnpm --filter mantic-desktop build
cd apps/desktop/src-tauri && cargo build --release && cd ../../..

# 2. Run the smoke test
pnpm --filter mantic-e2e test
```
