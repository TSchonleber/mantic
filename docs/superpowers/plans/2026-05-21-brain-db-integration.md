# Brain.db Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Embed brain.db into Mantic — direct `rusqlite` reads + bundled `brainctl-mcp` subprocess for writes/complex reads, plus the `State<T>` foundation refactor for the existing license storage.

**Architecture:** Mantic-side: refactor keyring into `State<KeyringStore>`, add `State<BrainDb>` (rusqlite read-only pool), add `State<BrainctlClient>` (lazy-spawned subprocess speaking newline-delimited JSON-RPC over stdio). brainctl-side: PyInstaller build target produces a standalone `brainctl-mcp-bundle` binary; Tauri's `externalBin` sidecar mechanism embeds it in `Mantic.app`. See spec: `docs/superpowers/specs/2026-05-21-brain-db-integration-design.md`.

**Tech Stack:** Rust (tokio, rusqlite, serde, serde_json, async-trait), Tauri 2 sidecar, PyInstaller (Python), GitHub Actions.

---

## Pre-flight

The plan touches TWO repos:
- `/Users/r4vager/Documents/Mantic` (this) on branch `build/brain-db-integration` (you'll create this in Task 1)
- `/Users/r4vager/work/brainctl` — for the PyInstaller bundle (Tasks 2-3)

Verify prereqs:

```bash
node --version  # >= 20
pnpm --version  # >= 10
rustc --version # >= 1.78
cargo --version
python3 --version  # >= 3.11 for brainctl
ls /Users/r4vager/work/brainctl/pyproject.toml   # must exist
```

DO NOT run `pnpm tauri build` or `pnpm build:desktop` — macOS DMG bundler pops Finder. Use `cargo check`, `cargo test`, and `cargo build --release` (no bundler) for verification.

---

## File Structure

### Mantic (`/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/`)

New files:
- `state.rs` — `AppState { keyring, brain, brainctl }` registered via `app.manage()`
- `brain_db.rs` — read-only rusqlite wrapper
- `brainctl_client.rs` — MCP subprocess client
- `mcp_codec.rs` — newline-delimited JSON-RPC framing + correlation
- `bundle.rs` — locates `brainctl-mcp` sidecar via Tauri APIs

Modified files:
- `license.rs` — refactor to `KeyringStore` struct stored in `State<T>`
- `commands.rs` — pass `State<AppState>` to existing commands; add 12 new commands
- `lib.rs` — wire setup with new state
- `Cargo.toml` — add `rusqlite`, `async-trait`
- `tauri.conf.json` — declare `externalBin` for the sidecar
- `error.rs` — add 3 new error variants

### Mantic frontend

- `apps/desktop/src/lib/tauri-bridge.ts` — 12 new typed wrapper functions

### Sidecar binary location

- `apps/desktop/src-tauri/binaries/brainctl-mcp-aarch64-apple-darwin` — downloaded/copied from brainctl release

### brainctl (`/Users/r4vager/work/brainctl/`)

New files:
- `build/pyinstaller/brainctl_mcp.spec` — PyInstaller spec
- `scripts/build_mcp_bundle.sh` — build script
- `.github/workflows/build-mcp-bundle.yml` — release-tag-triggered build

---

## Task 1: Branch + State<KeyringStore> refactor

**Files:**
- Modify: `apps/desktop/src-tauri/src/license.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/heartbeat.rs`

- [ ] **Step 1: Create feature branch**

```bash
cd /Users/r4vager/Documents/Mantic
git checkout main 2>/dev/null || git checkout build/desktop-app-shell
git checkout -b build/brain-db-integration
```

- [ ] **Step 2: Read current license.rs**

Take a moment to read `apps/desktop/src-tauri/src/license.rs` so the refactor is informed. Key fact: currently there's a static `OnceLock<Result<keyring::Entry, keyring::Error>>` plus a `clone_keyring_error` adapter. We're going to wrap that lazy-init logic in a `KeyringStore` struct that can be passed into `State<T>`.

- [ ] **Step 3: Rewrite license.rs to KeyringStore struct**

Replace the entire contents of `apps/desktop/src-tauri/src/license.rs` with:

```rust
use crate::error::{AppError, Result};
use std::sync::OnceLock;

const SERVICE: &str = "org.brainctl.mantic";
const KEY: &str = "license-jwt";

pub struct KeyringStore {
    entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            entry: OnceLock::new(),
        }
    }

    fn entry(&self) -> Result<&keyring::Entry> {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            });
        }
        let cell = self.entry.get_or_init(|| keyring::Entry::new(SERVICE, KEY));
        cell.as_ref()
            .map_err(|e| AppError::Keyring(clone_keyring_error(e)))
    }

    pub fn save(&self, token: &str) -> Result<()> {
        self.entry()?.set_password(token)?;
        Ok(())
    }

    pub fn load(&self) -> Result<String> {
        match self.entry()?.get_password() {
            Ok(p) => Ok(p),
            Err(keyring::Error::NoEntry) => Err(AppError::NoLicense),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }

    pub fn clear(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Keyring(e)),
        }
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

fn clone_keyring_error(e: &keyring::Error) -> keyring::Error {
    keyring::Error::PlatformFailure(std::io::Error::other(format!("{e}")).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset(store: &KeyringStore) {
        let _ = store.clear();
    }

    #[test]
    fn save_then_load_roundtrip() {
        let store = KeyringStore::new();
        reset(&store);
        store.save("token-123").unwrap();
        assert_eq!(store.load().unwrap(), "token-123");
    }

    #[test]
    fn load_with_no_entry_returns_no_license() {
        let store = KeyringStore::new();
        reset(&store);
        match store.load() {
            Err(AppError::NoLicense) => {}
            other => panic!("expected NoLicense, got {other:?}"),
        }
    }

    #[test]
    fn clear_is_idempotent() {
        let store = KeyringStore::new();
        reset(&store);
        store.clear().unwrap();
        store.clear().unwrap();
    }
}
```

Important: the `OnceLock` is now per-instance (lives inside the struct). The `#[cfg(test)]` mock-init still runs once per process via the static `TEST_INIT: Once`.

- [ ] **Step 4: Update commands.rs to take KeyringStore via State**

Read `apps/desktop/src-tauri/src/commands.rs`. Replace its contents with:

```rust
use crate::account::{decode_claims, AccountInfo};
use crate::error::Result;
use crate::license::KeyringStore;
use crate::pairing;
use tauri::State;

const DEFAULT_SERVER: &str = "http://localhost:4001";

pub fn server_url() -> String {
    std::env::var("MANTIC_LICENSE_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_string())
}

#[tauri::command]
pub async fn pair_with_code(
    code: String,
    keyring: State<'_, KeyringStore>,
) -> Result<AccountInfo> {
    let token = pairing::pair(&server_url(), &code).await?;
    let info = decode_claims(&token)?;
    keyring.save(&token)?;
    Ok(info)
}

#[tauri::command]
pub fn current_account(keyring: State<'_, KeyringStore>) -> Result<AccountInfo> {
    let token = keyring.load()?;
    decode_claims(&token)
}

#[tauri::command]
pub fn sign_out(keyring: State<'_, KeyringStore>) -> Result<()> {
    keyring.clear()
}
```

Note: `State<'_, KeyringStore>` is how Tauri passes the managed state. Tauri allows mixing State params with serializable params; the State is auto-injected.

- [ ] **Step 5: Update heartbeat.rs to take KeyringStore**

Read `apps/desktop/src-tauri/src/heartbeat.rs`. The `tick()` function currently uses `license::save/load`. Refactor to take a `&KeyringStore`:

```rust
use crate::account::decode_claims;
use crate::error::Result;
use crate::license::KeyringStore;
use crate::pairing;
use std::time::Duration;

pub fn needs_refresh(expires_at: i64, now: i64, refresh_window_secs: i64) -> bool {
    expires_at - now <= refresh_window_secs
}

pub async fn tick(
    keyring: &KeyringStore,
    server_url: &str,
    refresh_window_secs: i64,
) -> Result<bool> {
    let token = match keyring.load() {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let info = match decode_claims(&token) {
        Ok(i) => i,
        Err(_) => return Ok(false),
    };
    let now = chrono::Utc::now().timestamp();
    if !needs_refresh(info.expires_at, now, refresh_window_secs) {
        return Ok(false);
    }
    let new_token = pairing::refresh(server_url, &token).await?;
    decode_claims(&new_token)?;
    keyring.save(&new_token)?;
    Ok(true)
}

pub async fn run_forever(
    keyring: std::sync::Arc<KeyringStore>,
    server_url: String,
    interval: Duration,
    refresh_window_secs: i64,
) {
    loop {
        let _ = tick(&keyring, &server_url, refresh_window_secs).await;
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_refresh_true_when_within_window() {
        assert!(needs_refresh(1000, 950, 60));
    }

    #[test]
    fn needs_refresh_false_when_outside_window() {
        assert!(!needs_refresh(1000, 800, 60));
    }

    #[test]
    fn needs_refresh_true_when_already_expired() {
        assert!(needs_refresh(800, 1000, 60));
    }
}
```

- [ ] **Step 6: Update lib.rs to manage KeyringStore as state**

Replace the contents of `apps/desktop/src-tauri/src/lib.rs` with:

```rust
mod account;
mod commands;
mod error;
mod heartbeat;
mod license;
mod pairing;

use license::KeyringStore;
use std::sync::Arc;
use std::time::Duration;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let keyring = Arc::new(KeyringStore::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(keyring.clone())
        .setup(move |_app| {
            let keyring_for_task = keyring.clone();
            tauri::async_runtime::spawn(heartbeat::run_forever(
                keyring_for_task,
                commands::server_url(),
                Duration::from_secs(60 * 5),
                60 * 60 * 2,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pair_with_code,
            commands::current_account,
            commands::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Notes: We use `Arc<KeyringStore>` because both the Tauri state and the heartbeat task need to reference it. `.manage(keyring.clone())` registers `Arc<KeyringStore>` so commands take `State<'_, Arc<KeyringStore>>`. Adjust the commands accordingly — see Step 7.

- [ ] **Step 7: Fix State<T> type to Arc<KeyringStore> in commands.rs**

Update the State parameter type in `commands.rs`:

```rust
use std::sync::Arc;

#[tauri::command]
pub async fn pair_with_code(
    code: String,
    keyring: State<'_, Arc<KeyringStore>>,
) -> Result<AccountInfo> {
    let token = pairing::pair(&server_url(), &code).await?;
    let info = decode_claims(&token)?;
    keyring.save(&token)?;
    Ok(info)
}

#[tauri::command]
pub fn current_account(keyring: State<'_, Arc<KeyringStore>>) -> Result<AccountInfo> {
    let token = keyring.load()?;
    decode_claims(&token)
}

#[tauri::command]
pub fn sign_out(keyring: State<'_, Arc<KeyringStore>>) -> Result<()> {
    keyring.clear()
}
```

`Arc<KeyringStore>` derefs to `KeyringStore`, so `.save()`/`.load()`/`.clear()` work via auto-deref.

- [ ] **Step 8: Verify everything compiles + tests pass**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib
```

Expected: 13 tests pass (same count as before; just refactored).

- [ ] **Step 9: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/
git commit -m "refactor(desktop): migrate keyring from static OnceLock to State<Arc<KeyringStore>>"
```

---

## Task 2: brainctl PyInstaller build script

**Files (in `/Users/r4vager/work/brainctl/`):**
- Create: `build/pyinstaller/brainctl_mcp.spec`
- Create: `scripts/build_mcp_bundle.sh`

- [ ] **Step 1: Inspect brainctl-mcp entry point**

```bash
cd /Users/r4vager/work/brainctl
grep -r "brainctl-mcp" pyproject.toml
```

Expected: `pyproject.toml` declares an entry point like `brainctl-mcp = "agentmemory.mcp_server:main"` (or similar). Note the actual module path — you'll need it in the PyInstaller spec.

If unsure, run:
```bash
which brainctl-mcp 2>/dev/null && python3 -c "import importlib.metadata; print(importlib.metadata.entry_points(group='console_scripts')['brainctl-mcp'].value)"
```

- [ ] **Step 2: Create the PyInstaller spec**

Create `/Users/r4vager/work/brainctl/build/pyinstaller/brainctl_mcp.spec`:

```python
# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec for brainctl-mcp standalone binary.
# Build: `pyinstaller build/pyinstaller/brainctl_mcp.spec`

import sys
from pathlib import Path

block_cipher = None

# Repo root: spec is at build/pyinstaller/, so go up two levels.
project_root = Path(SPECPATH).parent.parent

a = Analysis(
    [str(project_root / "src" / "agentmemory" / "mcp_server.py")],
    pathex=[str(project_root / "src")],
    binaries=[],
    datas=[],
    hiddenimports=[
        "sqlite3",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name="brainctl-mcp",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
```

**Note:** If brainctl's entry point module differs from `agentmemory.mcp_server`, adjust the path in `Analysis(...)`. Verify via Step 1's grep.

- [ ] **Step 3: Create the build script**

Create `/Users/r4vager/work/brainctl/scripts/build_mcp_bundle.sh`:

```bash
#!/usr/bin/env bash
# Build a standalone brainctl-mcp binary via PyInstaller.
# Output: dist/brainctl-mcp on the current platform.

set -euo pipefail

cd "$(dirname "$0")/.."

# Detect target triple (used in artifact naming)
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) TARGET="aarch64-apple-darwin" ;;
    Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
    Linux-x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
    MINGW*|MSYS*|CYGWIN*) TARGET="x86_64-pc-windows-msvc" ;;
    *) echo "Unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

echo "Building brainctl-mcp bundle for $TARGET..."

# Set up venv if not present
if [ ! -d .venv-bundle ]; then
    python3 -m venv .venv-bundle
fi
source .venv-bundle/bin/activate

# Install brainctl + PyInstaller
pip install -e ".[mcp]" >/dev/null
pip install "pyinstaller>=6.0" >/dev/null

# Build
pyinstaller --clean --noconfirm build/pyinstaller/brainctl_mcp.spec

# Verify the binary runs
echo "Verifying binary..."
./dist/brainctl-mcp --help 2>&1 | head -5 || echo "(no --help flag; binary built but help not available)"

# Stamp with target triple
mv dist/brainctl-mcp "dist/brainctl-mcp-$TARGET"
echo "Built: dist/brainctl-mcp-$TARGET"
```

Mark it executable:

```bash
chmod +x /Users/r4vager/work/brainctl/scripts/build_mcp_bundle.sh
```

- [ ] **Step 4: Run the build script (local proof of concept)**

```bash
cd /Users/r4vager/work/brainctl
./scripts/build_mcp_bundle.sh
```

Expected: produces `dist/brainctl-mcp-aarch64-apple-darwin` (on the user's M-series Mac). First build can take 1-3 minutes (PyInstaller dep resolution + bundling).

If it fails: the most common failures are missing `hiddenimports` (PyInstaller can't detect dynamic imports). Capture the error and add the missing module name(s) to the `hiddenimports` list in the spec. Re-run.

- [ ] **Step 5: Verify the binary speaks MCP stdio**

```bash
cd /Users/r4vager/work/brainctl
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"mantic-test","version":"0.0.0"}}}' | ./dist/brainctl-mcp-aarch64-apple-darwin | head -1
```

Expected: a JSON-RPC response line back with `"id":1` and an `initialize` result. If the binary hangs or errors, PyInstaller missed something — check stderr.

- [ ] **Step 6: Commit (in brainctl repo)**

```bash
cd /Users/r4vager/work/brainctl
git add build/pyinstaller scripts/build_mcp_bundle.sh
git commit -m "feat(build): PyInstaller spec + script for standalone brainctl-mcp binary"
```

- [ ] **Step 7: Copy the binary into Mantic for sidecar use**

```bash
mkdir -p /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/binaries
cp /Users/r4vager/work/brainctl/dist/brainctl-mcp-aarch64-apple-darwin \
   /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/binaries/brainctl-mcp-aarch64-apple-darwin
```

The Tauri sidecar mechanism (configured in Task 11) requires the binary to be named with the target-triple suffix and live in `src-tauri/binaries/`.

- [ ] **Step 8: Add binaries to .gitignore in Mantic**

Add to `/Users/r4vager/Documents/Mantic/.gitignore`:

```gitignore
# Bundled sidecar binaries — built artifacts, not source
apps/desktop/src-tauri/binaries/
```

We don't commit the binary (large, platform-specific). CI and devs each pull/build their own.

Commit just the gitignore change for now (the binary is staged outside of git):

```bash
cd /Users/r4vager/Documents/Mantic
git add .gitignore
git commit -m "chore: ignore bundled sidecar binaries"
```

---

## Task 3: brainctl CI workflow for bundle releases

**Files (in `/Users/r4vager/work/brainctl/`):**
- Create: `.github/workflows/build-mcp-bundle.yml`

- [ ] **Step 1: Create the workflow**

Create `/Users/r4vager/work/brainctl/.github/workflows/build-mcp-bundle.yml`:

```yaml
name: build-mcp-bundle

on:
  push:
    tags: ["v*"]
  workflow_dispatch:

permissions:
  contents: write

jobs:
  build:
    name: Build (${{ matrix.target }})
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"

      - name: Build bundle
        run: ./scripts/build_mcp_bundle.sh

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: brainctl-mcp-${{ matrix.target }}
          path: dist/brainctl-mcp-${{ matrix.target }}
          if-no-files-found: error

      - name: Attach to release
        if: startsWith(github.ref, 'refs/tags/')
        uses: softprops/action-gh-release@v2
        with:
          files: dist/brainctl-mcp-${{ matrix.target }}
```

v1 only ships `aarch64-apple-darwin`. Add `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc` matrix entries when we cross-platform-ship.

- [ ] **Step 2: Validate YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('/Users/r4vager/work/brainctl/.github/workflows/build-mcp-bundle.yml'))"
```

Expected: exit 0, no output.

- [ ] **Step 3: Commit (in brainctl)**

```bash
cd /Users/r4vager/work/brainctl
git add .github/workflows/build-mcp-bundle.yml
git commit -m "ci(build): release workflow for brainctl-mcp bundle (macOS arm64)"
```

---

## Task 4: rusqlite dependency in Mantic

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Add rusqlite + async-trait**

Append to `[dependencies]` block in `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/Cargo.toml`:

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
async-trait = "0.1"
```

`bundled` features statically links sqlite — no system dep needed, works in CI without extra installs.

- [ ] **Step 2: Verify cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0. First-time rusqlite compile takes ~30-60s.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "chore(desktop): add rusqlite (bundled) and async-trait deps"
```

---

## Task 5: MCP codec (framing + correlation), TDD

**Files:**
- Create: `apps/desktop/src-tauri/src/mcp_codec.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (add `mod mcp_codec;`)

- [ ] **Step 1: Write the failing tests**

Create `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/mcp_codec.rs`:

```rust
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Request {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }

    pub fn encode(&self) -> String {
        let mut s = serde_json::to_string(self).unwrap();
        s.push('\n');
        s
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Response {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

impl Response {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

pub struct IdGen {
    next: AtomicU64,
}

impl IdGen {
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    pub fn next(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for IdGen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_encode_includes_newline() {
        let req = Request::new(1, "memory_add", json!({"content": "hi"}));
        let encoded = req.encode();
        assert!(encoded.ends_with('\n'));
        let parsed: Value = serde_json::from_str(encoded.trim()).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "memory_add");
        assert_eq!(parsed["params"]["content"], "hi");
    }

    #[test]
    fn response_parse_success() {
        let line = r#"{"jsonrpc":"2.0","id":7,"result":{"memory_id":42}}"#;
        let resp = Response::parse(line).unwrap();
        assert_eq!(resp.id, Some(7));
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["memory_id"], 42);
    }

    #[test]
    fn response_parse_error() {
        let line = r#"{"jsonrpc":"2.0","id":8,"error":{"code":-32602,"message":"bad params"}}"#;
        let resp = Response::parse(line).unwrap();
        assert_eq!(resp.id, Some(8));
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32602);
        assert_eq!(err.message, "bad params");
    }

    #[test]
    fn response_parse_notification_has_no_id() {
        let line = r#"{"jsonrpc":"2.0","method":"some/notification","params":{}}"#;
        let resp = Response::parse(line).unwrap();
        assert!(resp.id.is_none());
    }

    #[test]
    fn id_gen_is_monotonic() {
        let g = IdGen::new();
        assert_eq!(g.next(), 1);
        assert_eq!(g.next(), 2);
        assert_eq!(g.next(), 3);
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

Add `mod mcp_codec;` alphabetically to `apps/desktop/src-tauri/src/lib.rs`:

```rust
mod account;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib mcp_codec
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/mcp_codec.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): mcp newline-delimited json-rpc codec + correlation id generator"
```

---

## Task 6: Error variants + bundle module

**Files:**
- Modify: `apps/desktop/src-tauri/src/error.rs`
- Create: `apps/desktop/src-tauri/src/bundle.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Add new error variants to error.rs**

Append to the `AppError` enum (before the closing `}` of the enum, but inside it):

```rust
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("brainctl returned error {code}: {message}")]
    Brainctl { code: i64, message: String },

    #[error("brainctl unavailable: {reason}")]
    BrainctlUnavailable { reason: String },
```

Full file should now have 11 variants.

- [ ] **Step 2: Create bundle.rs**

Create `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/bundle.rs`:

```rust
use crate::error::{AppError, Result};
use std::path::PathBuf;
use tauri::Manager;

/// Returns the path to the bundled brainctl-mcp sidecar binary.
///
/// In dev (`tauri dev` / `cargo run`), Tauri resolves this to
/// `src-tauri/binaries/brainctl-mcp-<target-triple>`.
/// In production, it's inside the app bundle's resources.
pub fn brainctl_mcp_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf> {
    let resolver = app.path();
    let target_triple = target_triple();
    let candidate = resolver
        .resolve(
            format!("binaries/brainctl-mcp-{target_triple}"),
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(|e| AppError::BrainctlUnavailable {
            reason: format!("could not resolve sidecar path: {e}"),
        })?;

    if !candidate.exists() {
        return Err(AppError::BrainctlUnavailable {
            reason: format!("bundled brainctl-mcp not found at {}", candidate.display()),
        });
    }
    Ok(candidate)
}

fn target_triple() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else {
        "unknown"
    }
}
```

- [ ] **Step 3: Register module in lib.rs**

Add `mod bundle;` alphabetically:

```rust
mod account;
mod brain_db;     // we'll create this in Task 7 — leave the line, it'll resolve then
mod brainctl_client;  // same, Task 8
mod bundle;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
mod state;        // Task 9
```

Wait — these later modules don't exist yet. Comment out the missing ones for now and add them back as their tasks land. **Replace the lines above with just**:

```rust
mod account;
mod bundle;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
```

Future tasks (7, 8, 9) will add `brain_db`, `brainctl_client`, `state`.

- [ ] **Step 4: cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0. Warnings about unused `Sqlite`, `Brainctl`, `BrainctlUnavailable` variants are fine — Tasks 7-9 consume them.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/src/bundle.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): add Sqlite/Brainctl error variants and bundle sidecar path resolver"
```

---

## Task 7: BrainDb (rusqlite read-only), TDD

**Files:**
- Create: `apps/desktop/src-tauri/src/brain_db.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/brain_db.rs`:

```rust
use crate::error::Result;
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct BrainStatus {
    pub memory_count: u64,
    pub event_count: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct EventSummary {
    pub id: i64,
    pub event_type: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct MemorySummary {
    pub id: i64,
    pub category: String,
    pub content: String,
    pub created_at: i64,
}

pub struct BrainDb {
    conn: Mutex<Connection>,
}

impl BrainDb {
    /// Open the brain.db file at the given path, read-only.
    /// File must already exist (brainctl-mcp handles creation).
    pub fn open_read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn status(&self) -> Result<BrainStatus> {
        let conn = self.conn.lock().unwrap();
        let memory_count: u64 = conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?;
        let event_count: u64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?;
        Ok(BrainStatus {
            memory_count,
            event_count,
        })
    }

    pub fn recent_events(&self, limit: u32) -> Result<Vec<EventSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, event_type, content, created_at FROM events ORDER BY created_at DESC LIMIT ?",
        )?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok(EventSummary {
                    id: r.get(0)?,
                    event_type: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn recent_memories(&self, limit: u32) -> Result<Vec<MemorySummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, category, content, created_at FROM memories ORDER BY created_at DESC LIMIT ?",
        )?;
        let rows = stmt
            .query_map(params![limit], |r| {
                Ok(MemorySummary {
                    id: r.get(0)?,
                    category: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    /// Build a minimal fixture brain.db with the columns BrainDb reads.
    /// We don't mirror brainctl's full schema — just what the read methods touch.
    fn fixture_db() -> NamedTempFile {
        let f = NamedTempFile::new().unwrap();
        let path = f.path();
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE memories (
                id INTEGER PRIMARY KEY,
                category TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE events (
                id INTEGER PRIMARY KEY,
                event_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            INSERT INTO memories (category, content, created_at) VALUES
                ('lesson', 'first', 100),
                ('decision', 'second', 200),
                ('preference', 'third', 300);
            INSERT INTO events (event_type, content, created_at) VALUES
                ('observation', 'ev1', 1000),
                ('result', 'ev2', 1100);
            "#,
        )
        .unwrap();
        f
    }

    #[test]
    fn status_returns_counts() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let s = db.status().unwrap();
        assert_eq!(s.memory_count, 3);
        assert_eq!(s.event_count, 2);
    }

    #[test]
    fn recent_events_orders_by_created_at_desc() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let events = db.recent_events(10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].content, "ev2");
        assert_eq!(events[1].content, "ev1");
    }

    #[test]
    fn recent_memories_respects_limit() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let mems = db.recent_memories(2).unwrap();
        assert_eq!(mems.len(), 2);
        assert_eq!(mems[0].content, "third");
        assert_eq!(mems[1].content, "second");
    }

    #[test]
    fn open_nonexistent_file_returns_error() {
        let r = BrainDb::open_read_only(Path::new("/nonexistent/brain.db"));
        assert!(r.is_err());
    }

    #[test]
    fn read_only_mode_blocks_writes() {
        let f = fixture_db();
        let db = BrainDb::open_read_only(f.path()).unwrap();
        let conn = db.conn.lock().unwrap();
        let r = conn.execute("INSERT INTO memories (category, content, created_at) VALUES ('x', 'y', 0)", []);
        assert!(r.is_err(), "expected write to be rejected on read-only connection");
    }
}
```

Note: tests use `tempfile`. Add it as a dev-dep in the next step.

- [ ] **Step 2: Add tempfile as dev-dep**

Edit `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/Cargo.toml` `[dev-dependencies]` section. Append:

```toml
tempfile = "3"
```

- [ ] **Step 3: Register module in lib.rs**

Add `mod brain_db;` alphabetically to `lib.rs`:

```rust
mod account;
mod brain_db;
mod bundle;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
```

- [ ] **Step 4: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib brain_db
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/brain_db.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "feat(desktop): BrainDb read-only rusqlite wrapper with status/recent_events/recent_memories"
```

---

## Task 8: BrainctlClient skeleton (spawn/respawn lifecycle), TDD

**Files:**
- Create: `apps/desktop/src-tauri/src/brainctl_client.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing tests + skeleton**

Create `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/brainctl_client.rs`:

```rust
use crate::error::{AppError, Result};
use crate::mcp_codec::{IdGen, Request, Response};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

pub struct BrainctlClient {
    inner: Arc<Mutex<Option<ClientState>>>,
    binary_path: PathBuf,
    db_path: PathBuf,
    ids: IdGen,
    request_timeout: Duration,
}

struct ClientState {
    child: Child,
    stdin: ChildStdin,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<std::result::Result<Value, (i64, String)>>>>>,
    _reader_task: JoinHandle<()>,
}

impl BrainctlClient {
    pub fn new(binary_path: PathBuf, db_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            binary_path,
            db_path,
            ids: IdGen::new(),
            request_timeout: Duration::from_secs(5),
        }
    }

    /// Send a JSON-RPC request and await its response.
    /// Spawns the subprocess on first call. On crash, respawns up to 3 times.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let mut attempts = 0;
        let mut last_err: Option<AppError> = None;
        while attempts < 3 {
            attempts += 1;
            match self.try_call(method, params.clone()).await {
                Ok(v) => return Ok(v),
                Err(AppError::BrainctlUnavailable { reason }) => {
                    last_err = Some(AppError::BrainctlUnavailable { reason });
                    // tear down any dead client + backoff
                    let mut guard = self.inner.lock().await;
                    *guard = None;
                    drop(guard);
                    tokio::time::sleep(Duration::from_millis(250 * (1 << (attempts - 1)))).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_err.unwrap_or(AppError::BrainctlUnavailable {
            reason: "exhausted retries".into(),
        }))
    }

    async fn try_call(&self, method: &str, params: Value) -> Result<Value> {
        self.ensure_spawned().await?;
        let id = self.ids.next();
        let req = Request::new(id, method, params);
        let (tx, rx) = oneshot::channel();

        let mut guard = self.inner.lock().await;
        let state = guard
            .as_mut()
            .ok_or_else(|| AppError::BrainctlUnavailable {
                reason: "subprocess not running".into(),
            })?;
        state.pending.lock().await.insert(id, tx);
        let encoded = req.encode();
        state.stdin.write_all(encoded.as_bytes()).await.map_err(|e| {
            AppError::BrainctlUnavailable {
                reason: format!("stdin write failed: {e}"),
            }
        })?;
        state.stdin.flush().await.ok();
        drop(guard);

        match tokio::time::timeout(self.request_timeout, rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err((code, message)))) => Err(AppError::Brainctl { code, message }),
            Ok(Err(_)) => Err(AppError::BrainctlUnavailable {
                reason: "response channel closed (subprocess crash?)".into(),
            }),
            Err(_) => Err(AppError::Brainctl {
                code: -32001,
                message: "request timeout".into(),
            }),
        }
    }

    async fn ensure_spawned(&self) -> Result<()> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        let mut cmd = Command::new(&self.binary_path);
        cmd.env("BRAINCTL_DB", &self.db_path);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| AppError::BrainctlUnavailable {
            reason: format!("failed to spawn brainctl-mcp ({}): {e}", self.binary_path.display()),
        })?;

        let stdin = child.stdin.take().ok_or_else(|| AppError::BrainctlUnavailable {
            reason: "failed to capture child stdin".into(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| AppError::BrainctlUnavailable {
            reason: "failed to capture child stdout".into(),
        })?;

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<std::result::Result<Value, (i64, String)>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_for_reader = pending.clone();

        let reader_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                let resp = match Response::parse(&line) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let Some(id) = resp.id else {
                    continue; // server notification, ignore
                };
                let mut p = pending_for_reader.lock().await;
                if let Some(tx) = p.remove(&id) {
                    let payload = if let Some(err) = resp.error {
                        Err((err.code, err.message))
                    } else {
                        Ok(resp.result.unwrap_or(Value::Null))
                    };
                    let _ = tx.send(payload);
                }
            }
        });

        // Send MCP initialize handshake — best-effort, ignore response.
        let init_req = Request::new(
            0,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "mantic-desktop", "version": "0.0.0" }
            }),
        );

        let mut stdin = stdin;
        stdin.write_all(init_req.encode().as_bytes()).await.ok();
        stdin.flush().await.ok();

        *guard = Some(ClientState {
            child,
            stdin,
            pending,
            _reader_task: reader_task,
        });
        Ok(())
    }

    /// Send SIGTERM, wait briefly, then SIGKILL.
    pub async fn shutdown(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(mut state) = guard.take() {
            let _ = state.child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), state.child.wait()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn call_against_missing_binary_returns_unavailable() {
        let client = BrainctlClient::new(
            PathBuf::from("/definitely/not/here/brainctl-mcp"),
            PathBuf::from("/tmp/test-brain.db"),
        );
        let r = client.call("memory_add", json!({"content": "x", "category": "lesson"})).await;
        assert!(matches!(r, Err(AppError::BrainctlUnavailable { .. })));
    }

    #[tokio::test]
    async fn call_against_echo_binary_round_trips() {
        // Use a tiny shell echo as the "subprocess" — it reads stdin, writes a canned response.
        // We script it to read one line and emit a valid JSON-RPC response for it.
        let script = r#"#!/usr/bin/env bash
read -r line
echo '{"jsonrpc":"2.0","id":1,"result":{"ok":true}}'
sleep 1
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), script).unwrap();
        let mut perms = std::fs::metadata(tmp.path()).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        std::fs::set_permissions(tmp.path(), perms).unwrap();

        let client = BrainctlClient::new(
            tmp.path().to_path_buf(),
            PathBuf::from("/tmp/test-brain.db"),
        );
        // The echo emits id=1 regardless; ensure our IdGen starts at 1 too (it does).
        let r = client.call("memory_add", json!({"content": "x"})).await;
        assert!(r.is_ok(), "expected ok, got {r:?}");
        assert_eq!(r.unwrap()["ok"], true);
        client.shutdown().await;
    }
}
```

The second test exercises the real spawn/stdio path against a tiny shell script that responds with a canned JSON-RPC response. It catches MCP framing bugs without needing the real brainctl-mcp binary.

- [ ] **Step 2: Register module in lib.rs**

Add `mod brainctl_client;` alphabetically:

```rust
mod account;
mod brain_db;
mod brainctl_client;
mod bundle;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib brainctl_client
```

Expected: 2 tests pass. (The second test is Unix-only via the bash script — that's fine for macOS dev. CI on Linux will also pass; Windows CI would skip with `cfg(unix)` if you wanted, but for v1 macOS-only is the target.)

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/brainctl_client.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): BrainctlClient with subprocess lifecycle, MCP correlation, timeout"
```

---

## Task 9: BrainctlClient typed methods (writes + complex reads)

**Files:**
- Modify: `apps/desktop/src-tauri/src/brainctl_client.rs`

- [ ] **Step 1: Add typed methods**

Append to `impl BrainctlClient` (after `shutdown`):

```rust
    pub async fn memory_add(
        &self,
        content: &str,
        category: &str,
        scope: Option<&str>,
        tags: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({ "content": content, "category": category });
        if let Some(s) = scope {
            params["scope"] = json!(s);
        }
        if let Some(t) = tags {
            params["tags"] = json!(t);
        }
        self.call("memory_add", params).await
    }

    pub async fn event_add(
        &self,
        event_type: &str,
        content: &str,
        importance: Option<f64>,
    ) -> Result<Value> {
        let mut params = json!({ "event_type": event_type, "content": content });
        if let Some(i) = importance {
            params["importance"] = json!(i);
        }
        self.call("event_add", params).await
    }

    pub async fn decision_add(
        &self,
        title: &str,
        rationale: &str,
        project: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({ "title": title, "rationale": rationale });
        if let Some(p) = project {
            params["project"] = json!(p);
        }
        self.call("decision_add", params).await
    }

    pub async fn entity_create(
        &self,
        name: &str,
        entity_type: &str,
        scope: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({ "name": name, "entity_type": entity_type });
        if let Some(s) = scope {
            params["scope"] = json!(s);
        }
        self.call("entity_create", params).await
    }

    pub async fn entity_observe(&self, entity_id: i64, observation: &str) -> Result<Value> {
        self.call(
            "entity_observe",
            json!({ "entity_id": entity_id, "observation": observation }),
        )
        .await
    }

    pub async fn agent_register(
        &self,
        id: &str,
        name: &str,
        agent_type: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({ "id": id, "name": name });
        if let Some(t) = agent_type {
            params["type"] = json!(t);
        }
        self.call("agent_register", params).await
    }

    pub async fn agent_wrap_up(
        &self,
        agent_id: &str,
        summary: &str,
        goal: Option<&str>,
        open_loops: Option<&str>,
        next_step: Option<&str>,
        project: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({ "agent_id": agent_id, "summary": summary });
        if let Some(g) = goal {
            params["goal"] = json!(g);
        }
        if let Some(o) = open_loops {
            params["open_loops"] = json!(o);
        }
        if let Some(n) = next_step {
            params["next_step"] = json!(n);
        }
        if let Some(p) = project {
            params["project"] = json!(p);
        }
        self.call("agent_wrap_up", params).await
    }

    pub async fn agent_orient(
        &self,
        agent_id: &str,
        project: Option<&str>,
        query: Option<&str>,
    ) -> Result<Value> {
        let mut params = json!({ "agent_id": agent_id });
        if let Some(p) = project {
            params["project"] = json!(p);
        }
        if let Some(q) = query {
            params["query"] = json!(q);
        }
        self.call("agent_orient", params).await
    }

    pub async fn memory_search(&self, query: &str, limit: Option<u32>) -> Result<Value> {
        let mut params = json!({ "query": query });
        if let Some(l) = limit {
            params["limit"] = json!(l);
        }
        self.call("memory_search", params).await
    }
```

- [ ] **Step 2: cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0.

- [ ] **Step 3: cargo test (existing tests still pass)**

```bash
cargo test --lib
```

Expected: 23 tests pass (13 existing + 5 mcp_codec + 5 brain_db + 2 brainctl_client = wait, that's 25. Actually: 3 license + 5 pairing + 2 account + 3 heartbeat = 13 from sub-project 1. Plus 5 mcp_codec + 5 brain_db + 2 brainctl_client = 12 new. Total: 25.). Verify the actual count matches.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/brainctl_client.rs
git commit -m "feat(desktop): typed BrainctlClient methods for 9 brainctl operations"
```

---

## Task 10: AppState assembly + lifecycle wiring

**Files:**
- Create: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create state.rs**

Create `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/state.rs`:

```rust
use crate::brain_db::BrainDb;
use crate::brainctl_client::BrainctlClient;
use crate::license::KeyringStore;
use crate::error::{AppError, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Holds all the long-lived components of the app.
pub struct AppState {
    pub keyring: Arc<KeyringStore>,
    pub brain: Arc<BrainDb>,
    pub brainctl: Arc<BrainctlClient>,
}

impl AppState {
    pub fn new(brain_db_path: PathBuf, brainctl_binary: PathBuf) -> Result<Self> {
        let keyring = Arc::new(KeyringStore::new());
        // brainctl-mcp owns brain.db creation on first init. For now, if the file
        // doesn't exist yet, surface a clear error from BrainDb construction so
        // the user knows to launch brainctl-mcp once. (Tauri setup will hit this
        // path on first launch; we tolerate the failure and defer to lazy retry
        // via try_open_brain).
        let brain = Arc::new(BrainDb::open_read_only(&brain_db_path).or_else(|_| {
            // Bootstrap an empty brain.db with minimal schema so reads return
            // sensible defaults even before brainctl-mcp ever runs.
            bootstrap_empty_brain(&brain_db_path)?;
            BrainDb::open_read_only(&brain_db_path)
        })?);
        let brainctl = Arc::new(BrainctlClient::new(brainctl_binary, brain_db_path));
        Ok(Self {
            keyring,
            brain,
            brainctl,
        })
    }
}

fn bootstrap_empty_brain(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::BrainctlUnavailable {
            reason: format!("failed to create brain.db parent: {e}"),
        })?;
    }
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memories (
            id INTEGER PRIMARY KEY,
            category TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY,
            event_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        "#,
    )?;
    Ok(())
}
```

Note: this bootstrap is a minimal stand-in. When the real `brainctl-mcp` runs, it will issue full migrations against the same file. Our minimal schema doesn't conflict — brainctl uses `CREATE TABLE IF NOT EXISTS` style migrations.

- [ ] **Step 2: Wire AppState into lib.rs**

Replace the contents of `apps/desktop/src-tauri/src/lib.rs`:

```rust
mod account;
mod brain_db;
mod brainctl_client;
mod bundle;
mod commands;
mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
mod state;

use state::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let app_data = app.path().app_data_dir().expect("app data dir");
            let brain_path = app_data.join("mantic").join("brain.db");

            let brainctl_binary = bundle::brainctl_mcp_path(&app.handle())
                .unwrap_or_else(|_| PathBuf::from("brainctl-mcp"));

            let state = AppState::new(brain_path, brainctl_binary)
                .expect("failed to build app state");

            // Heartbeat task
            let keyring_for_task = state.keyring.clone();
            tauri::async_runtime::spawn(heartbeat::run_forever(
                keyring_for_task,
                commands::server_url(),
                Duration::from_secs(60 * 5),
                60 * 60 * 2,
            ));

            // Register individual Arcs so commands can take State<'_, Arc<KeyringStore>>, etc.
            app.manage(state.keyring.clone());
            app.manage(state.brain.clone());
            app.manage(state.brainctl.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pair_with_code,
            commands::current_account,
            commands::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0. May warn about unused bundle::brainctl_mcp_path in some configs — fine.

- [ ] **Step 4: Run all tests**

```bash
cargo test --lib
```

Expected: 25 tests pass (no new tests yet for state.rs — it's lifecycle code best tested via Task 14's integration test).

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/state.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): AppState assembly with brain.db bootstrap and sidecar resolution"
```

---

## Task 11: Tauri commands for brain operations

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Append new commands to commands.rs**

Append to `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/commands.rs`:

```rust
use crate::brain_db::{BrainDb, BrainStatus, EventSummary, MemorySummary};
use crate::brainctl_client::BrainctlClient;
use serde_json::Value;

// --- Read commands (rusqlite direct) ---

#[tauri::command]
pub fn brain_status(brain: State<'_, Arc<BrainDb>>) -> Result<BrainStatus> {
    brain.status()
}

#[tauri::command]
pub fn recent_events(limit: u32, brain: State<'_, Arc<BrainDb>>) -> Result<Vec<EventSummary>> {
    brain.recent_events(limit)
}

#[tauri::command]
pub fn recent_memories(limit: u32, brain: State<'_, Arc<BrainDb>>) -> Result<Vec<MemorySummary>> {
    brain.recent_memories(limit)
}

// --- Subprocess commands (write + complex reads) ---

#[tauri::command]
pub async fn memory_add(
    content: String,
    category: String,
    scope: Option<String>,
    tags: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .memory_add(&content, &category, scope.as_deref(), tags.as_deref())
        .await
}

#[tauri::command]
pub async fn event_add(
    event_type: String,
    content: String,
    importance: Option<f64>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.event_add(&event_type, &content, importance).await
}

#[tauri::command]
pub async fn decision_add(
    title: String,
    rationale: String,
    project: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .decision_add(&title, &rationale, project.as_deref())
        .await
}

#[tauri::command]
pub async fn entity_create(
    name: String,
    entity_type: String,
    scope: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .entity_create(&name, &entity_type, scope.as_deref())
        .await
}

#[tauri::command]
pub async fn entity_observe(
    entity_id: i64,
    observation: String,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.entity_observe(entity_id, &observation).await
}

#[tauri::command]
pub async fn agent_register(
    id: String,
    name: String,
    agent_type: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_register(&id, &name, agent_type.as_deref())
        .await
}

#[tauri::command]
pub async fn agent_wrap_up(
    agent_id: String,
    summary: String,
    goal: Option<String>,
    open_loops: Option<String>,
    next_step: Option<String>,
    project: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_wrap_up(
            &agent_id,
            &summary,
            goal.as_deref(),
            open_loops.as_deref(),
            next_step.as_deref(),
            project.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn agent_orient(
    agent_id: String,
    project: Option<String>,
    query: Option<String>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl
        .agent_orient(&agent_id, project.as_deref(), query.as_deref())
        .await
}

#[tauri::command]
pub async fn memory_search(
    query: String,
    limit: Option<u32>,
    brainctl: State<'_, Arc<BrainctlClient>>,
) -> Result<Value> {
    brainctl.memory_search(&query, limit).await
}
```

- [ ] **Step 2: Register commands in lib.rs invoke_handler**

Replace the `.invoke_handler(...)` call in lib.rs with:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::pair_with_code,
            commands::current_account,
            commands::sign_out,
            commands::brain_status,
            commands::recent_events,
            commands::recent_memories,
            commands::memory_add,
            commands::event_add,
            commands::decision_add,
            commands::entity_create,
            commands::entity_observe,
            commands::agent_register,
            commands::agent_wrap_up,
            commands::agent_orient,
            commands::memory_search,
        ])
```

- [ ] **Step 3: Verify cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0.

- [ ] **Step 4: Run all tests**

```bash
cargo test --lib
```

Expected: 25 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): expose 12 brain operations as Tauri commands"
```

---

## Task 12: TypeScript bridge for brain commands, TDD

**Files:**
- Modify: `apps/desktop/src/lib/tauri-bridge.ts`
- Modify: `apps/desktop/src/lib/tauri-bridge.test.ts`

- [ ] **Step 1: Add tests for new bridge functions**

Append to `/Users/r4vager/Documents/Mantic/apps/desktop/src/lib/tauri-bridge.test.ts`:

```typescript
import {
  brainStatus,
  recentEvents,
  recentMemories,
  memoryAdd,
  eventAdd,
  decisionAdd,
  entityCreate,
  entityObserve,
  agentRegister,
  agentWrapUp,
  agentOrient,
  memorySearch,
} from "./tauri-bridge";

describe("brain bridge — reads", () => {
  it("brainStatus invokes the rust command", async () => {
    invokeMock.mockResolvedValueOnce({ memory_count: 5, event_count: 12 });
    const r = await brainStatus();
    expect(invokeMock).toHaveBeenCalledWith("brain_status");
    expect(r.memory_count).toBe(5);
    expect(r.event_count).toBe(12);
  });

  it("recentEvents passes the limit", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await recentEvents(10);
    expect(invokeMock).toHaveBeenCalledWith("recent_events", { limit: 10 });
  });

  it("recentMemories passes the limit", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await recentMemories(7);
    expect(invokeMock).toHaveBeenCalledWith("recent_memories", { limit: 7 });
  });
});

describe("brain bridge — writes", () => {
  it("memoryAdd passes content + category", async () => {
    invokeMock.mockResolvedValueOnce({ memory_id: 1 });
    await memoryAdd({ content: "hi", category: "lesson" });
    expect(invokeMock).toHaveBeenCalledWith("memory_add", {
      content: "hi",
      category: "lesson",
      scope: undefined,
      tags: undefined,
    });
  });

  it("memoryAdd passes optional scope and tags", async () => {
    invokeMock.mockResolvedValueOnce({});
    await memoryAdd({
      content: "hi",
      category: "lesson",
      scope: "project:mantic",
      tags: "trading,llm",
    });
    expect(invokeMock).toHaveBeenCalledWith("memory_add", {
      content: "hi",
      category: "lesson",
      scope: "project:mantic",
      tags: "trading,llm",
    });
  });

  it("decisionAdd passes title + rationale", async () => {
    invokeMock.mockResolvedValueOnce({ decision_id: 9 });
    await decisionAdd({ title: "x", rationale: "y" });
    expect(invokeMock).toHaveBeenCalledWith("decision_add", {
      title: "x",
      rationale: "y",
      project: undefined,
    });
  });

  it("agentRegister passes id + name", async () => {
    invokeMock.mockResolvedValueOnce({ ok: true });
    await agentRegister({ id: "a", name: "A" });
    expect(invokeMock).toHaveBeenCalledWith("agent_register", {
      id: "a",
      name: "A",
      agentType: undefined,
    });
  });
});

describe("brain bridge — complex reads", () => {
  it("memorySearch passes the query", async () => {
    invokeMock.mockResolvedValueOnce({ hits: [] });
    await memorySearch({ query: "trade", limit: 5 });
    expect(invokeMock).toHaveBeenCalledWith("memory_search", { query: "trade", limit: 5 });
  });

  it("agentOrient invokes correctly", async () => {
    invokeMock.mockResolvedValueOnce({ memories: [], events: [] });
    await agentOrient({ agentId: "claude-code-test", project: "mantic" });
    expect(invokeMock).toHaveBeenCalledWith("agent_orient", {
      agentId: "claude-code-test",
      project: "mantic",
      query: undefined,
    });
  });
});

describe("eventAdd and remaining bridges", () => {
  it("eventAdd passes type/content/importance", async () => {
    invokeMock.mockResolvedValueOnce({});
    await eventAdd({ eventType: "result", content: "x", importance: 0.8 });
    expect(invokeMock).toHaveBeenCalledWith("event_add", {
      eventType: "result",
      content: "x",
      importance: 0.8,
    });
  });

  it("entityCreate passes name/type/scope", async () => {
    invokeMock.mockResolvedValueOnce({ entity_id: 1 });
    await entityCreate({ name: "BONK", entityType: "token", scope: "project:mantic" });
    expect(invokeMock).toHaveBeenCalledWith("entity_create", {
      name: "BONK",
      entityType: "token",
      scope: "project:mantic",
    });
  });

  it("entityObserve passes id and text", async () => {
    invokeMock.mockResolvedValueOnce({});
    await entityObserve({ entityId: 42, observation: "rugged" });
    expect(invokeMock).toHaveBeenCalledWith("entity_observe", {
      entityId: 42,
      observation: "rugged",
    });
  });

  it("agentWrapUp passes required + optional fields", async () => {
    invokeMock.mockResolvedValueOnce({});
    await agentWrapUp({
      agentId: "a",
      summary: "done",
      goal: "ship",
      openLoops: "none",
      nextStep: "next",
      project: "mantic",
    });
    expect(invokeMock).toHaveBeenCalledWith("agent_wrap_up", {
      agentId: "a",
      summary: "done",
      goal: "ship",
      openLoops: "none",
      nextStep: "next",
      project: "mantic",
    });
  });
});
```

- [ ] **Step 2: Run test — must FAIL**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm --filter mantic-desktop test
```

Expected: FAIL — the new functions don't exist yet.

- [ ] **Step 3: Implement the bridge additions**

Append to `/Users/r4vager/Documents/Mantic/apps/desktop/src/lib/tauri-bridge.ts`:

```typescript
// ---- Brain types ----

export interface BrainStatus {
  memory_count: number;
  event_count: number;
}

export interface EventSummary {
  id: number;
  event_type: string;
  content: string;
  created_at: number;
}

export interface MemorySummary {
  id: number;
  category: string;
  content: string;
  created_at: number;
}

// ---- Reads ----

export function brainStatus(): Promise<BrainStatus> {
  return invoke<BrainStatus>("brain_status");
}

export function recentEvents(limit: number): Promise<EventSummary[]> {
  return invoke<EventSummary[]>("recent_events", { limit });
}

export function recentMemories(limit: number): Promise<MemorySummary[]> {
  return invoke<MemorySummary[]>("recent_memories", { limit });
}

// ---- Writes ----

export interface MemoryAddInput {
  content: string;
  category: string;
  scope?: string;
  tags?: string;
}

export function memoryAdd(input: MemoryAddInput): Promise<unknown> {
  return invoke("memory_add", {
    content: input.content,
    category: input.category,
    scope: input.scope,
    tags: input.tags,
  });
}

export interface EventAddInput {
  eventType: string;
  content: string;
  importance?: number;
}

export function eventAdd(input: EventAddInput): Promise<unknown> {
  return invoke("event_add", input);
}

export interface DecisionAddInput {
  title: string;
  rationale: string;
  project?: string;
}

export function decisionAdd(input: DecisionAddInput): Promise<unknown> {
  return invoke("decision_add", {
    title: input.title,
    rationale: input.rationale,
    project: input.project,
  });
}

export interface EntityCreateInput {
  name: string;
  entityType: string;
  scope?: string;
}

export function entityCreate(input: EntityCreateInput): Promise<unknown> {
  return invoke("entity_create", input);
}

export interface EntityObserveInput {
  entityId: number;
  observation: string;
}

export function entityObserve(input: EntityObserveInput): Promise<unknown> {
  return invoke("entity_observe", input);
}

export interface AgentRegisterInput {
  id: string;
  name: string;
  agentType?: string;
}

export function agentRegister(input: AgentRegisterInput): Promise<unknown> {
  return invoke("agent_register", {
    id: input.id,
    name: input.name,
    agentType: input.agentType,
  });
}

export interface AgentWrapUpInput {
  agentId: string;
  summary: string;
  goal?: string;
  openLoops?: string;
  nextStep?: string;
  project?: string;
}

export function agentWrapUp(input: AgentWrapUpInput): Promise<unknown> {
  return invoke("agent_wrap_up", input);
}

// ---- Complex reads ----

export interface AgentOrientInput {
  agentId: string;
  project?: string;
  query?: string;
}

export function agentOrient(input: AgentOrientInput): Promise<unknown> {
  return invoke("agent_orient", input);
}

export interface MemorySearchInput {
  query: string;
  limit?: number;
}

export function memorySearch(input: MemorySearchInput): Promise<unknown> {
  return invoke("memory_search", input);
}
```

- [ ] **Step 4: Run tests — must PASS**

```bash
pnpm --filter mantic-desktop test
```

Expected: PASS. Count: 4 (existing bridge) + 11 (new brain bridge) + 3 (Pair) + 2 (Account) + 2 (App) = 22 frontend tests.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src/lib/tauri-bridge.ts apps/desktop/src/lib/tauri-bridge.test.ts
git commit -m "feat(desktop): typed TypeScript bridge for 12 brain commands"
```

---

## Task 13: Tauri externalBin sidecar config

**Files:**
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

- [ ] **Step 1: Add externalBin to tauri.conf.json**

Modify `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/tauri.conf.json` `bundle` section to add `externalBin`:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/brainctl-mcp"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
```

Tauri appends the target-triple suffix automatically when looking for the file (`binaries/brainctl-mcp-aarch64-apple-darwin` on macOS arm64).

- [ ] **Step 2: Verify the binary is present**

```bash
ls /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/binaries/
```

Expected: `brainctl-mcp-aarch64-apple-darwin` is listed. If not, re-run Task 2 Step 7 to copy the binary in place.

- [ ] **Step 3: cargo check (sanity)**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0. (cargo check doesn't validate tauri.conf.json sidecar paths — that happens at build time. We're not running tauri build in this task.)

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/tauri.conf.json
git commit -m "feat(desktop): declare brainctl-mcp as bundled externalBin sidecar"
```

---

## Task 14: Integration test (gated on binary presence)

**Files:**
- Create: `apps/desktop/src-tauri/tests/brainctl_integration.rs`

- [ ] **Step 1: Write the integration test**

Create `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/tests/brainctl_integration.rs`:

```rust
//! Integration test against the real brainctl-mcp sidecar binary.
//! Skipped automatically if the binary isn't present at the expected path.

use desktop_lib::brainctl_client::BrainctlClient;
use serde_json::json;
use std::path::{Path, PathBuf};

fn find_sidecar() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("brainctl-mcp-aarch64-apple-darwin");
    if p.exists() { Some(p) } else { None }
}

#[tokio::test]
async fn memory_add_round_trip_against_real_binary() {
    let Some(bin) = find_sidecar() else {
        eprintln!("[skip] brainctl-mcp sidecar not present at binaries/brainctl-mcp-aarch64-apple-darwin");
        return;
    };

    let tmp = tempfile::NamedTempFile::new().unwrap();
    let db_path = tmp.path().to_path_buf();
    drop(tmp); // we want only the path, brainctl creates the file itself

    let client = BrainctlClient::new(bin, db_path.clone());
    let result = client
        .memory_add(
            "integration test entry",
            "lesson",
            Some("project:mantic-test"),
            None,
        )
        .await;
    client.shutdown().await;

    // We expect either Ok with a memory_id field OR a deterministic brainctl error.
    // The point of this test is to confirm spawn + handshake + request/response works
    // end-to-end. The exact response shape may evolve; we just check it didn't blow up.
    match result {
        Ok(v) => {
            assert!(
                v.is_object() || v.is_null(),
                "expected JSON object/null, got: {v}"
            );
        }
        Err(e) => panic!("brainctl-mcp memory_add failed: {e}"),
    }

    // brainctl-mcp should have created the db file
    assert!(Path::new(&db_path).exists(), "brain.db not created at {db_path:?}");

    // cleanup
    let _ = std::fs::remove_file(&db_path);
}
```

The integration test is in `tests/` (not `src/`) so it runs as a separate binary linked against `desktop_lib`. The `desktop_lib::brainctl_client::BrainctlClient` reference requires that `brainctl_client` is `pub` or that we re-export it. Already public.

- [ ] **Step 2: Make sure brainctl_client is reachable via the library crate**

Open `/Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/lib.rs`. Confirm `mod brainctl_client;` is there (it is from Task 8). For integration tests to access it as `desktop_lib::brainctl_client`, the module needs to be `pub`. Change `mod brainctl_client;` to `pub mod brainctl_client;`. Do the same for any other modules the test references.

Check what the test imports:
- `desktop_lib::brainctl_client::BrainctlClient` — make `brainctl_client` public.

Apply: change to `pub mod brainctl_client;` in `lib.rs`.

- [ ] **Step 3: Run integration test**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test --test brainctl_integration -- --nocapture
```

Expected:
- If the binary exists at `binaries/brainctl-mcp-aarch64-apple-darwin`: test runs and passes (or fails informatively if brainctl-mcp errors). The test self-cleans the temp db.
- If the binary is absent: test prints `[skip]` and returns Ok — no failure.

The test acts as a real smoke for the integration. If you've done Task 2 (which copied the binary) it'll execute for real.

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/tests/brainctl_integration.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "test(desktop): integration test for brainctl-mcp sidecar (skips if binary absent)"
```

---

## Task 15: README polish + final integration check

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Run all tests**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm install
pnpm -r test
cd apps/desktop/src-tauri && cargo test
```

Expected: all green. Test counts: 6 (mock-license) + 22 (desktop frontend) + 25 (desktop Rust unit) + 1 integration (or skipped) = ~54 unit tests passing.

- [ ] **Step 2: Update README**

Replace `/Users/r4vager/Documents/Mantic/README.md` with:

```markdown
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
```

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add README.md
git commit -m "docs: README for sub-project #2 (brain integration + sidecar)"
```

---

## Definition of done

- [ ] All 25 Rust unit tests pass (3 license + 5 pairing + 2 account + 3 heartbeat + 5 mcp_codec + 5 brain_db + 2 brainctl_client)
- [ ] All 22 frontend unit tests pass (4 bridge + 11 brain bridge + 3 Pair + 2 Account + 2 App)
- [ ] All 6 mock-license tests pass
- [ ] Integration test passes (or skips cleanly) — verifies real brainctl-mcp spawn + memory_add
- [ ] brainctl repo has PyInstaller spec, build script, CI workflow committed
- [ ] Sidecar binary at `apps/desktop/src-tauri/binaries/brainctl-mcp-aarch64-apple-darwin` (gitignored)
- [ ] `pnpm dev:desktop` launches the app, all existing license flows still work, no regressions
- [ ] All 12 new Tauri commands callable from frontend via the typed TS bridge

When all checked, the desktop app embeds brainctl, ready for sub-project #4 (agent runtime) to start logging real trade decisions to the brain.

---

*Plan total: 15 tasks. Significant cross-repo work in Tasks 2-3 (brainctl side). Estimated 4-6 hours via subagent-driven execution.*
