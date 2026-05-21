# Wallet + Session Signer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mantic generates a Solana ed25519 session keypair, captures a Phantom-signed off-chain authorization via a localhost web bridge, and stores everything in the OS keychain so sub-project #4 can sign trades autonomously.

**Architecture:** Rust side adds a `wallet/` module (session keypair, authorization message, signature verify, keychain storage, axum-based localhost bridge). The bridge serves a pre-built static HTML/JS bundle that uses `@solana/wallet-adapter` to talk to Phantom in the user's default browser. Authorization comes back via HTTP POST to the bridge, which verifies the ed25519 signature and persists credentials. Sub-project #4 will call `wallet_sign_message` for actual trade transactions.

**Tech Stack:** Rust (ed25519-dalek, bs58, axum, tower-http, tokio), Tauri 2 (resource bundling), React + Vite + `@solana/wallet-adapter-react` + `@solana/wallet-adapter-phantom` (the wallet-bridge static page), Vitest, mockito-style ed25519 fixtures.

---

## Pre-flight

Working dir: `/Users/r4vager/Documents/Mantic`. Current `main` HEAD: `4f5e046` (the spec commit). Create a new feature branch in Task 1.

Verify prereqs:
```bash
node --version  # >= 20
pnpm --version  # >= 10
rustc --version # >= 1.78
```

DO NOT run `pnpm tauri build` or `pnpm build:desktop` — macOS DMG bundler pops Finder.

---

## File Structure

### New: `apps/wallet-bridge/` (a standalone Vite workspace producing a single-file HTML asset)

- `apps/wallet-bridge/package.json`
- `apps/wallet-bridge/tsconfig.json`
- `apps/wallet-bridge/vite.config.ts`
- `apps/wallet-bridge/index.html`
- `apps/wallet-bridge/src/main.tsx`
- `apps/wallet-bridge/src/App.tsx`

Build output: `apps/wallet-bridge/dist/index.html` (single file with inline JS/CSS, copied into Tauri resources during Mantic's build).

### New Rust modules: `apps/desktop/src-tauri/src/wallet/`

- `mod.rs` — re-exports
- `session.rs` — `SessionKey` ed25519 keypair
- `authorization.rs` — `AuthorizationMessage` builder + signature verify
- `store.rs` — `WalletStore` wrapping keychain entries for session credentials
- `bridge.rs` — `BridgeServer` axum localhost server

### Modified Rust files

- `apps/desktop/src-tauri/Cargo.toml` — add ed25519-dalek, bs58, axum, tower-http
- `apps/desktop/src-tauri/src/lib.rs` — register `mod wallet;`, wire `WalletStore` into AppState
- `apps/desktop/src-tauri/src/state.rs` — add `wallet: Arc<WalletStore>` field
- `apps/desktop/src-tauri/src/commands.rs` — add 4 wallet commands
- `apps/desktop/src-tauri/src/error.rs` — add `Wallet*` error variants
- `apps/desktop/src-tauri/tauri.conf.json` — register `wallet-bridge/dist/index.html` in `bundle.resources`

### Tauri resources

- Copy `apps/wallet-bridge/dist/index.html` to `apps/desktop/src-tauri/resources/wallet-bridge.html` at build time

### Modified frontend

- `apps/desktop/src/lib/tauri-bridge.ts` — add 4 wallet wrappers + tests
- `apps/desktop/src/routes/Wallet.tsx` (new) + `Wallet.test.tsx` (new)
- `apps/desktop/src/App.tsx` — route between Pair / Wallet / Account based on state

### Other

- `pnpm-workspace.yaml` — add `apps/wallet-bridge` to packages
- `package.json` (root) — add `build:wallet-bridge` script

---

## Task 1: Branch + workspace scaffolding

**Files:**
- Modify: `pnpm-workspace.yaml`
- Modify: `package.json` (root)

- [ ] **Step 1: Create feature branch**

```bash
cd /Users/r4vager/Documents/Mantic
git checkout main
git pull origin main 2>/dev/null  # noop, no upstream changes since push
git checkout -b build/wallet-session-signer
```

- [ ] **Step 2: Add wallet-bridge to workspaces**

Edit `pnpm-workspace.yaml`:

```yaml
packages:
  - "apps/*"
  - "services/*"
  - "e2e"
```

`apps/*` already includes `apps/wallet-bridge` once it's created in Task 2 — no change needed if that glob is already present. Verify the existing content matches. If `apps/*` is present, leave it. If only specific subdirs are listed, change to the glob.

- [ ] **Step 3: Add build:wallet-bridge script**

Edit root `package.json` `scripts`:

```json
"scripts": {
  "dev:mock-license": "pnpm --filter mock-license dev",
  "dev:desktop": "pnpm --filter mantic-desktop tauri dev",
  "build:desktop": "pnpm --filter mantic-desktop tauri build",
  "build:wallet-bridge": "pnpm --filter wallet-bridge build",
  "test": "pnpm -r test",
  "test:e2e": "pnpm --filter mantic-e2e test",
  "lint": "pnpm -r lint"
}
```

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add pnpm-workspace.yaml package.json
git commit -m "chore: add wallet-bridge workspace + build script"
```

---

## Task 2: Wallet-bridge Vite scaffold

**Files:**
- Create: `apps/wallet-bridge/package.json`
- Create: `apps/wallet-bridge/tsconfig.json`
- Create: `apps/wallet-bridge/vite.config.ts`
- Create: `apps/wallet-bridge/index.html`
- Create: `apps/wallet-bridge/src/main.tsx`
- Create: `apps/wallet-bridge/src/App.tsx`

- [ ] **Step 1: Create `apps/wallet-bridge/package.json`**

```json
{
  "name": "wallet-bridge",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "@solana/wallet-adapter-base": "^0.9.23",
    "@solana/wallet-adapter-react": "^0.15.35",
    "@solana/wallet-adapter-react-ui": "^0.9.35",
    "@solana/wallet-adapter-phantom": "^0.9.24",
    "@solana/wallet-adapter-solflare": "^0.6.28",
    "@solana/web3.js": "^1.95.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "typescript": "^5.6.0",
    "vite": "^7.0.0",
    "vite-plugin-singlefile": "^2.0.0"
  }
}
```

- [ ] **Step 2: Create `apps/wallet-bridge/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "noEmit": true
  },
  "include": ["src/**/*", "index.html"]
}
```

- [ ] **Step 3: Create `apps/wallet-bridge/vite.config.ts`**

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";

export default defineConfig({
  plugins: [react(), viteSingleFile()],
  build: {
    target: "es2020",
    cssCodeSplit: false,
    assetsInlineLimit: 100_000_000,
    rollupOptions: {
      output: { inlineDynamicImports: true },
    },
  },
});
```

- [ ] **Step 4: Create `apps/wallet-bridge/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Mantic — Connect Wallet</title>
    <style>
      :root { color-scheme: dark; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
      body { background: #0a0a0a; color: #e5e5e5; margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center; }
      .card { background: #171717; border: 1px solid #2a2a2a; border-radius: 16px; padding: 32px; max-width: 480px; }
      h1 { margin: 0 0 12px; font-size: 22px; }
      p { color: #a3a3a3; line-height: 1.5; }
      pre { background: #0a0a0a; border: 1px solid #2a2a2a; border-radius: 8px; padding: 12px; white-space: pre-wrap; font-size: 12px; }
      button { background: #2563eb; color: white; border: none; padding: 10px 16px; border-radius: 8px; cursor: pointer; font-weight: 500; }
      button:disabled { background: #525252; cursor: not-allowed; }
      .err { background: #450a0a; color: #fecaca; padding: 10px; border-radius: 8px; margin-top: 12px; }
      .ok { background: #052e16; color: #bbf7d0; padding: 10px; border-radius: 8px; margin-top: 12px; }
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Create `apps/wallet-bridge/src/main.tsx`**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { ConnectionProvider, WalletProvider } from "@solana/wallet-adapter-react";
import { WalletModalProvider } from "@solana/wallet-adapter-react-ui";
import { PhantomWalletAdapter } from "@solana/wallet-adapter-phantom";
import { SolflareWalletAdapter } from "@solana/wallet-adapter-solflare";
import "@solana/wallet-adapter-react-ui/styles.css";
import App from "./App";

const endpoint = "https://api.mainnet-beta.solana.com";
const wallets = [new PhantomWalletAdapter(), new SolflareWalletAdapter()];

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ConnectionProvider endpoint={endpoint}>
      <WalletProvider wallets={wallets} autoConnect={false}>
        <WalletModalProvider>
          <App />
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  </React.StrictMode>,
);
```

- [ ] **Step 6: Create `apps/wallet-bridge/src/App.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";
import bs58 from "bs58";

type Status = "idle" | "signing" | "submitting" | "done" | "error";

export default function App() {
  const { publicKey, signMessage, connected, wallet } = useWallet();
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [authMessage, setAuthMessage] = useState<string | null>(null);
  const [nonce, setNonce] = useState<string | null>(null);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    setNonce(params.get("nonce"));
    fetch(`/auth-message${window.location.search}`)
      .then((r) => r.text())
      .then(setAuthMessage)
      .catch((e) => setError(`Failed to fetch auth message: ${e}`));
  }, []);

  async function authorize() {
    if (!publicKey || !signMessage || !authMessage || !nonce) return;
    setStatus("signing");
    setError(null);
    try {
      const messageBytes = new TextEncoder().encode(authMessage);
      const signature = await signMessage(messageBytes);
      setStatus("submitting");
      const res = await fetch("/authorize", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          pubkey: publicKey.toBase58(),
          signature: bs58.encode(signature),
          message: authMessage,
          nonce,
        }),
      });
      if (!res.ok) {
        const body = await res.text();
        throw new Error(`Authorize failed (${res.status}): ${body}`);
      }
      setStatus("done");
    } catch (e) {
      setStatus("error");
      setError((e as Error).message);
    }
  }

  return (
    <div className="card">
      <h1>Connect wallet to Mantic</h1>
      <p>
        Mantic will create a session wallet on your machine. You'll authorize it by signing a
        message in your Solana wallet, then fund it manually when you're ready to trade.
      </p>

      {!connected && (
        <div style={{ marginTop: 20 }}>
          <WalletMultiButton />
        </div>
      )}

      {connected && authMessage && (
        <>
          <p style={{ marginTop: 20 }}>Connected to {wallet?.adapter.name}. Sign this to authorize:</p>
          <pre>{authMessage}</pre>
          <button
            disabled={status === "signing" || status === "submitting" || status === "done"}
            onClick={authorize}
          >
            {status === "signing" && "Sign in your wallet..."}
            {status === "submitting" && "Authorizing..."}
            {status === "done" && "Done — you can close this tab"}
            {status === "idle" && "Authorize"}
            {status === "error" && "Retry"}
          </button>
          {status === "done" && <div className="ok">Authorization complete. Return to Mantic.</div>}
          {error && <div className="err">{error}</div>}
        </>
      )}
    </div>
  );
}
```

- [ ] **Step 7: Install deps**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm install
```

Expected: all packages resolve, ~30-60s for first install with Solana SDK. If `bs58` is missing from `dependencies`, add it:

```bash
pnpm --filter wallet-bridge add bs58
```

(It's used in `App.tsx` for signature encoding.)

- [ ] **Step 8: Build the bridge**

```bash
pnpm --filter wallet-bridge build
```

Expected: `apps/wallet-bridge/dist/index.html` produced as a single file with inline JS+CSS (thanks to vite-plugin-singlefile).

```bash
ls -la apps/wallet-bridge/dist/
wc -c apps/wallet-bridge/dist/index.html
```

Expected: a single index.html, probably 500KB–2MB depending on how aggressively the wallet-adapter tree-shakes.

- [ ] **Step 9: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/wallet-bridge pnpm-lock.yaml
git commit -m "feat(wallet-bridge): vite singlefile bundle for Phantom/Solflare connect UI"
```

(The `dist/` output is gitignored via the root `.gitignore`'s `dist/` rule from sub-project #1. The build artifact is regenerated by CI / dev builds, not committed.)

---

## Task 3: Solana Rust dependencies

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Append to `[dependencies]`**

```toml
ed25519-dalek = { version = "2", features = ["std", "rand_core"] }
bs58 = "0.5"
axum = { version = "0.7", default-features = false, features = ["http1", "json", "tokio"] }
tower-http = { version = "0.5", features = ["fs"] }
rand = "0.8"
hex = "0.4"
```

Notes:
- We deliberately do NOT add `solana-sdk` here. It's heavyweight and only needed for transaction signing (sub-project #4). Ed25519 + bs58 cover everything wallet-bridge needs.
- `axum` is minimal-feature to keep compile time down.
- `rand 0.8` matches `ed25519-dalek 2.x`'s expected RNG version.

- [ ] **Step 2: cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0. ~30-60s first-time compile of axum + ed25519-dalek.

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "chore(desktop): add ed25519-dalek, bs58, axum, tower-http, rand, hex deps"
```

---

## Task 4: Wallet error variants

**Files:**
- Modify: `apps/desktop/src-tauri/src/error.rs`

- [ ] **Step 1: Add new variants**

Inside the `AppError` enum, append:

```rust
    #[error("wallet not connected")]
    WalletNotConnected,

    #[error("wallet connect already in progress")]
    WalletConnectInProgress,

    #[error("wallet connection timeout")]
    WalletConnectionTimeout,

    #[error("wallet signature verification failed")]
    WalletInvalidSignature,

    #[error("wallet nonce mismatch")]
    WalletNonceMismatch,

    #[error("wallet bridge error: {0}")]
    WalletBridge(String),

    #[error("bs58 decode error: {0}")]
    Bs58(#[from] bs58::decode::Error),
```

- [ ] **Step 2: cargo check**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0 (warnings about unused variants are fine — Tasks 5-9 consume them).

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/error.rs
git commit -m "feat(desktop): add wallet error variants"
```

---

## Task 5: SessionKey module (ed25519 keypair), TDD

**Files:**
- Create: `apps/desktop/src-tauri/src/wallet/mod.rs`
- Create: `apps/desktop/src-tauri/src/wallet/session.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Create the module dir + mod.rs**

```bash
mkdir -p /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/src/wallet
```

Create `apps/desktop/src-tauri/src/wallet/mod.rs`:

```rust
pub mod authorization;
pub mod bridge;
pub mod session;
pub mod store;

pub use session::SessionKey;
pub use store::{WalletStore, WalletCredentials};
```

(Some referenced submodules don't exist yet — that's fine, they land in Tasks 5-8. We declare them now so subsequent tasks just create the file.)

Wait — Rust requires referenced modules to exist or it won't compile. We'll add them progressively. For now write only `session;` here:

```rust
pub mod session;

pub use session::SessionKey;
```

Tasks 6-8 will add the other declarations as they're written.

- [ ] **Step 2: Write failing tests + skeleton**

Create `apps/desktop/src-tauri/src/wallet/session.rs`:

```rust
use crate::error::{AppError, Result};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, SECRET_KEY_LENGTH, SIGNATURE_LENGTH};
use rand::rngs::OsRng;

/// A locally-generated ed25519 keypair used as the Solana session wallet.
pub struct SessionKey {
    signing: SigningKey,
}

impl SessionKey {
    /// Generate a new random session keypair using the OS RNG.
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self { signing }
    }

    /// Returns the public key (Solana address) as a 32-byte array.
    pub fn pubkey(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Returns the public key in Solana's standard base58 representation.
    pub fn pubkey_base58(&self) -> String {
        bs58::encode(self.pubkey()).into_string()
    }

    /// Sign an arbitrary byte slice. Returns a 64-byte ed25519 signature.
    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LENGTH] {
        self.signing.sign(message).to_bytes()
    }

    /// Serialize the 32-byte secret key as hex for storage.
    pub fn to_hex(&self) -> String {
        hex::encode(self.signing.to_bytes())
    }

    /// Reconstruct a SessionKey from its hex-encoded secret.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s).map_err(|e| AppError::WalletBridge(format!("hex decode: {e}")))?;
        if bytes.len() != SECRET_KEY_LENGTH {
            return Err(AppError::WalletBridge(format!(
                "expected {SECRET_KEY_LENGTH} secret bytes, got {}",
                bytes.len()
            )));
        }
        let mut arr = [0u8; SECRET_KEY_LENGTH];
        arr.copy_from_slice(&bytes);
        let signing = SigningKey::from_bytes(&arr);
        Ok(Self { signing })
    }
}

/// Free-standing verify for any ed25519 signature against a pubkey + message.
/// Used by `authorization::verify_signature` for the master wallet's signature.
pub fn verify_signature(pubkey: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<()> {
    let verifying = VerifyingKey::from_bytes(pubkey)
        .map_err(|_| AppError::WalletInvalidSignature)?;
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    verifying
        .verify_strict(message, &sig)
        .map_err(|_| AppError::WalletInvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_then_pubkey_is_32_bytes() {
        let key = SessionKey::generate();
        let pk = key.pubkey();
        assert_eq!(pk.len(), 32);
    }

    #[test]
    fn sign_and_self_verify_roundtrip() {
        let key = SessionKey::generate();
        let msg = b"hello mantic";
        let sig = key.sign(msg);
        verify_signature(&key.pubkey(), msg, &sig).unwrap();
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let key = SessionKey::generate();
        let sig = key.sign(b"original");
        let err = verify_signature(&key.pubkey(), b"tampered", &sig).unwrap_err();
        assert!(matches!(err, AppError::WalletInvalidSignature));
    }

    #[test]
    fn verify_rejects_wrong_pubkey() {
        let a = SessionKey::generate();
        let b = SessionKey::generate();
        let msg = b"hello";
        let sig = a.sign(msg);
        let err = verify_signature(&b.pubkey(), msg, &sig).unwrap_err();
        assert!(matches!(err, AppError::WalletInvalidSignature));
    }

    #[test]
    fn hex_roundtrip_preserves_signing_capability() {
        let key = SessionKey::generate();
        let hex = key.to_hex();
        let key2 = SessionKey::from_hex(&hex).unwrap();
        let msg = b"persistence test";
        let sig = key2.sign(msg);
        verify_signature(&key.pubkey(), msg, &sig).unwrap();
    }

    #[test]
    fn pubkey_base58_is_valid_solana_format() {
        let key = SessionKey::generate();
        let bs58_str = key.pubkey_base58();
        // Solana pubkeys are typically 43-44 base58 chars
        assert!(bs58_str.len() >= 32 && bs58_str.len() <= 44);
        let decoded = bs58::decode(&bs58_str).into_vec().unwrap();
        assert_eq!(decoded, key.pubkey().to_vec());
    }
}
```

- [ ] **Step 3: Register `mod wallet;` in lib.rs**

Edit `apps/desktop/src-tauri/src/lib.rs`. The current mod block (after sub-project #2):

```rust
mod account;
mod brain_db;
pub mod brainctl_client;
mod bundle;
mod commands;
pub mod error;
mod heartbeat;
mod license;
mod mcp_codec;
mod pairing;
mod state;
```

Add `mod wallet;` alphabetically (after `state`):

```rust
mod state;
mod wallet;
```

- [ ] **Step 4: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib wallet::session
```

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/wallet apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): wallet::session ed25519 keypair with sign/verify + hex roundtrip"
```

---

## Task 6: AuthorizationMessage builder + verify, TDD

**Files:**
- Create: `apps/desktop/src-tauri/src/wallet/authorization.rs`
- Modify: `apps/desktop/src-tauri/src/wallet/mod.rs`

- [ ] **Step 1: Write the file**

Create `apps/desktop/src-tauri/src/wallet/authorization.rs`:

```rust
use crate::error::{AppError, Result};
use crate::wallet::session::verify_signature;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Builds the human-readable authorization message that Phantom shows to the user.
///
/// The format is fixed text with five substituted values: session pubkey, master pubkey,
/// ISO-8601 issued time, and a 16-byte random nonce.
pub fn build_message(
    session_pubkey_b58: &str,
    master_pubkey_b58: &str,
    issued_at: DateTime<Utc>,
    nonce_hex: &str,
) -> String {
    format!(
        "Mantic Session Authorization\n\
         \n\
         I authorize Mantic to use the following session wallet for autonomous\n\
         trading on my behalf:\n\
         \n\
           Session wallet: {session_pubkey_b58}\n\
         \n\
         I understand:\n\
           - I will fund this wallet manually from my master wallet\n\
           - Mantic has full control over the session wallet's contents\n\
           - This authorization is recorded off-chain only\n\
         \n\
         Master wallet: {master_pubkey_b58}\n\
         Issued: {issued_at}\n\
         Nonce: {nonce_hex}",
        issued_at = issued_at.to_rfc3339(),
    )
}

/// Generate a fresh 16-byte random nonce as a hex string.
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// What the bridge HTTP handler receives from the browser when the user signs.
#[derive(Debug, Deserialize)]
pub struct AuthorizePayload {
    pub pubkey: String,    // base58
    pub signature: String, // base58
    pub message: String,
    pub nonce: String,
}

/// What we persist locally after successful verification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredAuthorization {
    pub master_pubkey_b58: String,
    pub session_pubkey_b58: String,
    pub message: String,
    pub signature_b58: String,
    pub signed_at: DateTime<Utc>,
}

/// Verify the user's signature and return a StoredAuthorization.
/// Caller is responsible for matching the nonce against the expected value.
pub fn verify_payload(
    payload: &AuthorizePayload,
    expected_nonce: &str,
    expected_session_pubkey_b58: &str,
) -> Result<StoredAuthorization> {
    if payload.nonce != expected_nonce {
        return Err(AppError::WalletNonceMismatch);
    }
    if !payload.message.contains(expected_session_pubkey_b58) {
        return Err(AppError::WalletBridge(
            "session pubkey not present in signed message".into(),
        ));
    }
    let pubkey_bytes = bs58::decode(&payload.pubkey).into_vec()?;
    if pubkey_bytes.len() != 32 {
        return Err(AppError::WalletBridge(format!(
            "pubkey is {} bytes, expected 32",
            pubkey_bytes.len()
        )));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pubkey_bytes);

    let sig_bytes = bs58::decode(&payload.signature).into_vec()?;
    if sig_bytes.len() != 64 {
        return Err(AppError::WalletBridge(format!(
            "signature is {} bytes, expected 64",
            sig_bytes.len()
        )));
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&sig_bytes);

    verify_signature(&pk, payload.message.as_bytes(), &sig)?;

    Ok(StoredAuthorization {
        master_pubkey_b58: payload.pubkey.clone(),
        session_pubkey_b58: expected_session_pubkey_b58.to_string(),
        message: payload.message.clone(),
        signature_b58: payload.signature.clone(),
        signed_at: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::session::SessionKey;

    #[test]
    fn build_message_contains_all_fields() {
        let session = "SessSesssessSessAddr".to_string();
        let master = "MASTERmasterMasterAddr".to_string();
        let issued = "2026-05-21T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let nonce = "deadbeef".to_string();
        let msg = build_message(&session, &master, issued, &nonce);
        assert!(msg.contains(&session));
        assert!(msg.contains(&master));
        assert!(msg.contains("2026-05-21"));
        assert!(msg.contains(&nonce));
        assert!(msg.starts_with("Mantic Session Authorization"));
    }

    #[test]
    fn generate_nonce_is_32_hex_chars() {
        let n = generate_nonce();
        assert_eq!(n.len(), 32);
        assert!(hex::decode(&n).is_ok());
    }

    #[test]
    fn verify_payload_happy_path() {
        // Use a SessionKey as a stand-in for the master wallet (also ed25519).
        let master = SessionKey::generate();
        let session_pubkey_b58 = "SessionFakePubkey1234567890";

        let issued = Utc::now();
        let nonce = generate_nonce();
        let msg = build_message(
            session_pubkey_b58,
            &master.pubkey_base58(),
            issued,
            &nonce,
        );

        let sig_bytes = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig_bytes).into_string(),
            message: msg.clone(),
            nonce: nonce.clone(),
        };

        let stored = verify_payload(&payload, &nonce, session_pubkey_b58).unwrap();
        assert_eq!(stored.master_pubkey_b58, master.pubkey_base58());
        assert_eq!(stored.session_pubkey_b58, session_pubkey_b58);
        assert_eq!(stored.message, msg);
    }

    #[test]
    fn verify_payload_rejects_nonce_mismatch() {
        let master = SessionKey::generate();
        let msg = "any text".to_string();
        let sig = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg,
            nonce: "actual".into(),
        };
        let err = verify_payload(&payload, "expected", "any-session").unwrap_err();
        assert!(matches!(err, AppError::WalletNonceMismatch));
    }

    #[test]
    fn verify_payload_rejects_session_pubkey_not_in_message() {
        let master = SessionKey::generate();
        let nonce = "n0nce".to_string();
        // Build a message that does NOT contain the expected session pubkey
        let msg = "this message lacks the session pubkey".to_string();
        let sig = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg,
            nonce: nonce.clone(),
        };
        let err = verify_payload(&payload, &nonce, "ExpectedSessionPubkey").unwrap_err();
        match err {
            AppError::WalletBridge(m) => assert!(m.contains("session pubkey")),
            other => panic!("expected WalletBridge, got {other:?}"),
        }
    }

    #[test]
    fn verify_payload_rejects_bad_signature() {
        let master = SessionKey::generate();
        let other = SessionKey::generate();
        let session_pubkey_b58 = "SessionFakePubkey1234567890";
        let issued = Utc::now();
        let nonce = generate_nonce();
        let msg = build_message(
            session_pubkey_b58,
            &master.pubkey_base58(),
            issued,
            &nonce,
        );
        // Sign with `other`, claim it's from `master`
        let sig = other.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg,
            nonce: nonce.clone(),
        };
        let err = verify_payload(&payload, &nonce, session_pubkey_b58).unwrap_err();
        assert!(matches!(err, AppError::WalletInvalidSignature));
    }
}
```

- [ ] **Step 2: Update wallet/mod.rs**

```rust
pub mod authorization;
pub mod session;

pub use authorization::{build_message, generate_nonce, AuthorizePayload, StoredAuthorization};
pub use session::SessionKey;
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib wallet
```

Expected: 12 tests pass (6 session + 6 authorization).

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/wallet
git commit -m "feat(desktop): wallet::authorization with message builder + signature verify"
```

---

## Task 7: WalletStore (keychain), TDD

**Files:**
- Create: `apps/desktop/src-tauri/src/wallet/store.rs`
- Modify: `apps/desktop/src-tauri/src/wallet/mod.rs`

- [ ] **Step 1: Create store.rs**

The WalletStore mirrors KeyringStore's pattern (struct-with-OnceLock-Entry, `#[cfg(test)]` mock init) for three credential slots:

```rust
use crate::error::{AppError, Result};
use crate::wallet::authorization::StoredAuthorization;
use crate::wallet::session::SessionKey;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const SERVICE: &str = "org.brainctl.mantic";
const KEY_SESSION_PRIV: &str = "wallet-session-privkey";
const KEY_MASTER_PUB: &str = "wallet-master-pubkey";
const KEY_AUTHORIZATION: &str = "wallet-authorization";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletCredentials {
    pub session_pubkey_b58: String,
    pub master_pubkey_b58: String,
    pub authorization: StoredAuthorization,
}

pub struct WalletStore {
    session_entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
    master_entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
    auth_entry: OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
}

impl WalletStore {
    pub fn new() -> Self {
        Self {
            session_entry: OnceLock::new(),
            master_entry: OnceLock::new(),
            auth_entry: OnceLock::new(),
        }
    }

    fn entry(
        &self,
        cell: &OnceLock<std::result::Result<keyring::Entry, keyring::Error>>,
        key: &str,
    ) -> Result<&keyring::Entry> {
        #[cfg(test)]
        {
            static TEST_INIT: std::sync::Once = std::sync::Once::new();
            TEST_INIT.call_once(|| {
                keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
            });
        }
        let c = cell.get_or_init(|| keyring::Entry::new(SERVICE, key));
        c.as_ref().map_err(|e| AppError::Keyring(clone_keyring_error(e)))
    }

    pub fn save(
        &self,
        session: &SessionKey,
        master_pubkey_b58: &str,
        authorization: &StoredAuthorization,
    ) -> Result<()> {
        self.entry(&self.session_entry, KEY_SESSION_PRIV)?
            .set_password(&session.to_hex())?;
        self.entry(&self.master_entry, KEY_MASTER_PUB)?
            .set_password(master_pubkey_b58)?;
        let auth_json = serde_json::to_string(authorization)?;
        self.entry(&self.auth_entry, KEY_AUTHORIZATION)?
            .set_password(&auth_json)?;
        Ok(())
    }

    pub fn load(&self) -> Result<(SessionKey, WalletCredentials)> {
        let priv_hex = self.entry(&self.session_entry, KEY_SESSION_PRIV)?.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => AppError::WalletNotConnected,
                other => AppError::Keyring(other),
            })?;
        let session = SessionKey::from_hex(&priv_hex)?;
        let master_pubkey_b58 = self.entry(&self.master_entry, KEY_MASTER_PUB)?.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => AppError::WalletNotConnected,
                other => AppError::Keyring(other),
            })?;
        let auth_json = self.entry(&self.auth_entry, KEY_AUTHORIZATION)?.get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => AppError::WalletNotConnected,
                other => AppError::Keyring(other),
            })?;
        let authorization: StoredAuthorization = serde_json::from_str(&auth_json)?;
        let creds = WalletCredentials {
            session_pubkey_b58: session.pubkey_base58(),
            master_pubkey_b58,
            authorization,
        };
        Ok((session, creds))
    }

    pub fn status(&self) -> Result<Option<WalletCredentials>> {
        match self.load() {
            Ok((_, c)) => Ok(Some(c)),
            Err(AppError::WalletNotConnected) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn clear(&self) -> Result<()> {
        for (cell, key) in [
            (&self.session_entry, KEY_SESSION_PRIV),
            (&self.master_entry, KEY_MASTER_PUB),
            (&self.auth_entry, KEY_AUTHORIZATION),
        ] {
            let entry = self.entry(cell, key)?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(AppError::Keyring(e)),
            }
        }
        Ok(())
    }
}

impl Default for WalletStore {
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
    use crate::wallet::authorization::{build_message, generate_nonce};
    use chrono::Utc;

    fn fixture() -> (SessionKey, String, StoredAuthorization) {
        let session = SessionKey::generate();
        let master = SessionKey::generate();
        let nonce = generate_nonce();
        let issued = Utc::now();
        let msg = build_message(&session.pubkey_base58(), &master.pubkey_base58(), issued, &nonce);
        let sig = master.sign(msg.as_bytes());
        let auth = StoredAuthorization {
            master_pubkey_b58: master.pubkey_base58(),
            session_pubkey_b58: session.pubkey_base58(),
            message: msg,
            signature_b58: bs58::encode(sig).into_string(),
            signed_at: Utc::now(),
        };
        (session, master.pubkey_base58(), auth)
    }

    fn reset(store: &WalletStore) {
        let _ = store.clear();
    }

    #[test]
    fn save_then_load_roundtrip() {
        let store = WalletStore::new();
        reset(&store);
        let (session, master_pub, auth) = fixture();
        let session_pub_b58 = session.pubkey_base58();
        store.save(&session, &master_pub, &auth).unwrap();

        let (loaded_session, loaded_creds) = store.load().unwrap();
        assert_eq!(loaded_session.pubkey_base58(), session_pub_b58);
        assert_eq!(loaded_creds.master_pubkey_b58, master_pub);
        assert_eq!(loaded_creds.authorization.signature_b58, auth.signature_b58);
    }

    #[test]
    fn status_returns_none_when_empty() {
        let store = WalletStore::new();
        reset(&store);
        assert!(store.status().unwrap().is_none());
    }

    #[test]
    fn clear_then_load_returns_wallet_not_connected() {
        let store = WalletStore::new();
        reset(&store);
        let (session, master_pub, auth) = fixture();
        store.save(&session, &master_pub, &auth).unwrap();
        store.clear().unwrap();
        let err = store.load().unwrap_err();
        assert!(matches!(err, AppError::WalletNotConnected));
    }

    #[test]
    fn clear_is_idempotent() {
        let store = WalletStore::new();
        reset(&store);
        store.clear().unwrap();
        store.clear().unwrap();
    }
}
```

- [ ] **Step 2: Update wallet/mod.rs**

```rust
pub mod authorization;
pub mod session;
pub mod store;

pub use authorization::{build_message, generate_nonce, AuthorizePayload, StoredAuthorization};
pub use session::SessionKey;
pub use store::{WalletCredentials, WalletStore};
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib wallet
```

Expected: 16 tests pass (6 session + 6 authorization + 4 store).

- [ ] **Step 4: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/wallet
git commit -m "feat(desktop): wallet::store with three keychain slots and atomic save/load/clear"
```

---

## Task 8: BridgeServer (axum localhost), TDD

This task implements the local web bridge as an axum server. We test the handler logic against synthesized payloads — full network spawn is exercised in Task 14's end-to-end test.

**Files:**
- Create: `apps/desktop/src-tauri/src/wallet/bridge.rs`
- Modify: `apps/desktop/src-tauri/src/wallet/mod.rs`

- [ ] **Step 1: Create bridge.rs**

```rust
use crate::error::{AppError, Result};
use crate::wallet::authorization::{
    build_message, generate_nonce, verify_payload, AuthorizePayload, StoredAuthorization,
};
use crate::wallet::session::SessionKey;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
const PORT_RANGE: std::ops::Range<u16> = 18421..18431;
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Deserialize)]
struct NonceQuery {
    nonce: String,
}

struct SharedState {
    expected_nonce: String,
    session_pubkey_b58: String,
    auth_message: String,
    result_tx: Mutex<Option<oneshot::Sender<Result<StoredAuthorization>>>>,
    bridge_html: String,
}

/// The result of a successful bridge round trip: the authorized credentials.
pub struct BridgeOutcome {
    pub session_key: SessionKey,
    pub stored: StoredAuthorization,
}

/// One-shot bridge: spawns an axum server, opens the browser, waits for authorize POST,
/// shuts down. Caller is expected to await the returned future.
pub struct BridgeServer {
    state: Arc<SharedState>,
    bind_addr: SocketAddr,
    result_rx: oneshot::Receiver<Result<StoredAuthorization>>,
    server_task: JoinHandle<()>,
    session_key: Option<SessionKey>,
}

impl BridgeServer {
    /// Start the bridge. Returns the URL the user should be sent to, plus a handle
    /// that resolves to the authorized credentials when the user completes the flow.
    pub async fn start(bridge_html: String) -> Result<(String, Self)> {
        let session = SessionKey::generate();
        let session_pubkey_b58 = session.pubkey_base58();
        let nonce = generate_nonce();
        let issued = Utc::now();
        // The master_pubkey_b58 isn't known yet — the user picks it in the bridge.
        // We render the message with a placeholder master, then on POST we re-build
        // and verify the message the user actually signed (which the bridge JS will
        // re-fetch via /auth-message after wallet-connect).
        // To keep things simple, we serve TWO copies of the message: the placeholder
        // version in the HTML (informational) and the canonical version computed
        // server-side once the user POSTs back. But Phantom signs what we tell it
        // to sign, so we MUST pre-commit to the exact message string before POST.
        // Approach: render the message with the master pubkey set to the literal
        // string "<pending>" (NOT a real pubkey). After the user picks their wallet
        // we re-render with the actual master pubkey and have the page sign that.
        // The bridge HTML fetches the auth message via /auth-message?master=<pubkey>
        // once the user has connected.
        let placeholder_message = build_message(
            &session_pubkey_b58,
            "<pending>",
            issued,
            &nonce,
        );

        let (result_tx, result_rx) = oneshot::channel::<Result<StoredAuthorization>>();
        let state = Arc::new(SharedState {
            expected_nonce: nonce.clone(),
            session_pubkey_b58: session_pubkey_b58.clone(),
            auth_message: placeholder_message,
            result_tx: Mutex::new(Some(result_tx)),
            bridge_html,
        });

        let (listener, bind_addr) = bind_one_of(PORT_RANGE).await?;
        let app = Router::new()
            .route("/", get(serve_html))
            .route("/connect", get(serve_html))
            .route("/auth-message", get(auth_message_handler))
            .route("/authorize", post(authorize_handler))
            .with_state(state.clone());

        let server_task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let url = format!(
            "http://{}:{}/connect?nonce={}",
            bind_addr.ip(),
            bind_addr.port(),
            nonce
        );
        Ok((
            url,
            Self {
                state,
                bind_addr,
                result_rx,
                server_task,
                session_key: Some(session),
            },
        ))
    }

    /// Await the user's authorization. Returns when:
    /// - the user successfully authorizes (Ok)
    /// - the bridge times out after 5 minutes (Err)
    pub async fn await_authorization(mut self) -> Result<BridgeOutcome> {
        let session = self.session_key.take().expect("session_key consumed twice");
        let result = tokio::select! {
            r = &mut self.result_rx => r.map_err(|_| AppError::WalletBridge("bridge closed unexpectedly".into()))?,
            _ = tokio::time::sleep(BRIDGE_TIMEOUT) => Err(AppError::WalletConnectionTimeout),
        };
        self.server_task.abort();
        let stored = result?;
        Ok(BridgeOutcome { session_key: session, stored })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }
}

async fn bind_one_of(range: std::ops::Range<u16>) -> Result<(tokio::net::TcpListener, SocketAddr)> {
    for port in range {
        let addr = SocketAddr::new(HOST, port);
        if let Ok(l) = tokio::net::TcpListener::bind(addr).await {
            return Ok((l, addr));
        }
    }
    Err(AppError::WalletBridge(
        "no localhost port in 18421..18430 is available".into(),
    ))
}

async fn serve_html(State(state): State<Arc<SharedState>>) -> Html<String> {
    Html(state.bridge_html.clone())
}

#[derive(Debug, Deserialize)]
struct AuthMessageQuery {
    master: Option<String>,
    nonce: Option<String>,
}

async fn auth_message_handler(
    State(state): State<Arc<SharedState>>,
    Query(q): Query<AuthMessageQuery>,
) -> impl IntoResponse {
    let nonce_match = q.nonce.as_deref() == Some(&state.expected_nonce);
    if !nonce_match {
        return (StatusCode::BAD_REQUEST, "nonce mismatch".to_string());
    }
    // If the client passes ?master=<pubkey>, render the canonical message they
    // will sign. Otherwise return the placeholder.
    let master = q.master.unwrap_or_else(|| "<pending>".into());
    let issued = Utc::now();
    let msg = build_message(
        &state.session_pubkey_b58,
        &master,
        issued,
        &state.expected_nonce,
    );
    (StatusCode::OK, msg)
}

async fn authorize_handler(
    State(state): State<Arc<SharedState>>,
    Query(NonceQuery { nonce }): Query<NonceQuery>,
    Json(payload): Json<AuthorizePayload>,
) -> impl IntoResponse {
    let _ = nonce; // accepted but ignored; the nonce is in the payload and re-verified
    let outcome = verify_payload(&payload, &state.expected_nonce, &state.session_pubkey_b58);
    let mut tx = state.result_tx.lock().await;
    if let Some(sender) = tx.take() {
        let send_result = match outcome {
            Ok(s) => {
                let _ = sender.send(Ok(s));
                (StatusCode::OK, "ok".to_string())
            }
            Err(e) => {
                let msg = format!("{e}");
                let _ = sender.send(Err(e));
                (StatusCode::BAD_REQUEST, msg)
            }
        };
        send_result
    } else {
        (StatusCode::CONFLICT, "already authorized".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::session::SessionKey;

    fn fixture_html() -> String {
        "<html>test</html>".to_string()
    }

    #[tokio::test]
    async fn start_returns_url_with_nonce() {
        let (url, server) = BridgeServer::start(fixture_html()).await.unwrap();
        assert!(url.starts_with("http://127.0.0.1:"));
        assert!(url.contains("/connect?nonce="));
        let _ = server.await_authorization_with_short_timeout().await;
    }

    #[tokio::test]
    async fn timeout_returns_connection_timeout_error() {
        let (_url, server) = BridgeServer::start(fixture_html()).await.unwrap();
        let err = server.await_authorization_with_short_timeout().await.unwrap_err();
        assert!(matches!(err, AppError::WalletConnectionTimeout));
    }

    #[tokio::test]
    async fn full_round_trip_succeeds_with_valid_signature() {
        let (url, server) = BridgeServer::start(fixture_html()).await.unwrap();
        // Parse the bind addr from the URL
        let port: u16 = url.split(':').nth(2).unwrap().split('/').next().unwrap().parse().unwrap();
        let nonce = url.split("nonce=").nth(1).unwrap().to_string();
        // We need the session pubkey to build the message the master "signs"
        // The bridge generated the session keypair internally; we can't see it
        // directly, but the /auth-message endpoint will tell us what message
        // would have been signed.
        let master = SessionKey::generate();
        let auth_msg_url = format!(
            "http://127.0.0.1:{port}/auth-message?nonce={nonce}&master={}",
            master.pubkey_base58()
        );
        let msg = reqwest::get(&auth_msg_url).await.unwrap().text().await.unwrap();

        let sig = master.sign(msg.as_bytes());
        let payload = AuthorizePayload {
            pubkey: master.pubkey_base58(),
            signature: bs58::encode(sig).into_string(),
            message: msg.clone(),
            nonce: nonce.clone(),
        };

        let client = reqwest::Client::new();
        let res = client
            .post(format!("http://127.0.0.1:{port}/authorize?nonce={nonce}"))
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 200);

        let outcome = server.await_authorization().await.unwrap();
        assert_eq!(outcome.stored.master_pubkey_b58, master.pubkey_base58());
    }

    impl BridgeServer {
        async fn await_authorization_with_short_timeout(mut self) -> Result<BridgeOutcome> {
            let session = self.session_key.take().expect("session_key consumed twice");
            let result = tokio::select! {
                r = &mut self.result_rx => r.map_err(|_| AppError::WalletBridge("bridge closed unexpectedly".into()))?,
                _ = tokio::time::sleep(Duration::from_millis(200)) => Err(AppError::WalletConnectionTimeout),
            };
            self.server_task.abort();
            let stored = result?;
            Ok(BridgeOutcome { session_key: session, stored })
        }
    }
}
```

This is the biggest module in the plan. Implementation notes:
- `BridgeServer::start` binds a port in 18421..18430, mints a session key, generates a nonce, returns the URL the user should open
- `serve_html` returns the embedded bridge HTML (Tauri passes the loaded file content as a String)
- `auth_message_handler` renders the canonical message to be signed once the wallet-bridge JS knows the master pubkey
- `authorize_handler` parses + verifies the POSTed signature, fires the oneshot to release `await_authorization`
- A `#[cfg(test)] impl` block adds a short-timeout variant for tests so they don't have to wait 5 minutes

- [ ] **Step 2: Add reqwest to dev-deps** (used by the round-trip test)

Edit `apps/desktop/src-tauri/Cargo.toml` `[dev-dependencies]`:

```toml
[dev-dependencies]
mockito = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time", "test-util"] }
tempfile = "3"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

Reqwest is already a production dep; redeclaring it in dev-deps with the same features just makes the intent explicit.

- [ ] **Step 3: Update wallet/mod.rs**

```rust
pub mod authorization;
pub mod bridge;
pub mod session;
pub mod store;

pub use authorization::{build_message, generate_nonce, AuthorizePayload, StoredAuthorization};
pub use bridge::{BridgeOutcome, BridgeServer};
pub use session::SessionKey;
pub use store::{WalletCredentials, WalletStore};
```

- [ ] **Step 4: Run tests**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo test --lib wallet
```

Expected: 19 tests pass (6 session + 6 authorization + 4 store + 3 bridge).

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/wallet apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "feat(desktop): wallet::bridge axum localhost server with full round-trip test"
```

---

## Task 9: AppState extension + Tauri commands

**Files:**
- Modify: `apps/desktop/src-tauri/src/state.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Extend AppState**

Edit `apps/desktop/src-tauri/src/state.rs`:

- Add `use crate::wallet::WalletStore;` at the top
- Add `pub wallet: Arc<WalletStore>,` to the `AppState` struct
- In `AppState::new`, after the `brainctl` line:
  ```rust
  let wallet = Arc::new(WalletStore::new());
  ```
- Return field:
  ```rust
  Ok(Self {
      keyring,
      brain,
      brainctl,
      wallet,
  })
  ```

- [ ] **Step 2: Add Tauri commands**

Edit `apps/desktop/src-tauri/src/commands.rs`. Add imports:

```rust
use crate::wallet::{WalletCredentials, WalletStore};
use serde::Serialize;
use std::sync::Mutex as StdMutex;
use tauri::{AppHandle, Emitter};
```

Add an in-progress flag — only one connect can run at a time. We'll store a `Mutex<bool>` in state. Actually, to keep state.rs cleaner, we'll bury the flag inside a static for v1:

```rust
static CONNECT_IN_PROGRESS: StdMutex<bool> = StdMutex::new(false);
```

Add `pub` to that variable name? Not necessary if it lives in commands.rs.

Add the commands at the bottom of commands.rs:

```rust
#[derive(Debug, Serialize)]
pub struct WalletConnectStarted {
    pub url: String,
}

#[tauri::command]
pub async fn wallet_connect(
    app: AppHandle,
    wallet: State<'_, Arc<WalletStore>>,
) -> Result<WalletConnectStarted> {
    {
        let mut guard = CONNECT_IN_PROGRESS.lock().unwrap();
        if *guard {
            return Err(crate::error::AppError::WalletConnectInProgress);
        }
        *guard = true;
    }

    let bridge_html = load_bridge_html(&app)?;
    let (url, server) = crate::wallet::BridgeServer::start(bridge_html).await?;

    // Spawn the authorize-wait in a background task so wallet_connect can return immediately
    // with the URL the frontend should open.
    let wallet = wallet.inner().clone();
    let app_handle = app.clone();
    let url_for_open = url.clone();
    tauri::async_runtime::spawn(async move {
        let result = server.await_authorization().await;
        {
            let mut guard = CONNECT_IN_PROGRESS.lock().unwrap();
            *guard = false;
        }
        match result {
            Ok(outcome) => {
                if let Err(e) = wallet.save(
                    &outcome.session_key,
                    &outcome.stored.master_pubkey_b58,
                    &outcome.stored,
                ) {
                    let _ = app_handle.emit("wallet:error", e.to_string());
                    return;
                }
                let creds = WalletCredentials {
                    session_pubkey_b58: outcome.session_key.pubkey_base58(),
                    master_pubkey_b58: outcome.stored.master_pubkey_b58.clone(),
                    authorization: outcome.stored,
                };
                let _ = app_handle.emit("wallet:connected", creds);
            }
            Err(e) => {
                let _ = app_handle.emit("wallet:error", e.to_string());
            }
        }
    });

    // Open the user's default browser to the bridge URL
    open_browser(&url_for_open);
    Ok(WalletConnectStarted { url })
}

#[tauri::command]
pub fn wallet_status(wallet: State<'_, Arc<WalletStore>>) -> Result<Option<WalletCredentials>> {
    wallet.status()
}

#[tauri::command]
pub fn wallet_revoke(wallet: State<'_, Arc<WalletStore>>) -> Result<()> {
    wallet.clear()
}

#[tauri::command]
pub async fn wallet_sign_message(
    message: Vec<u8>,
    wallet: State<'_, Arc<WalletStore>>,
) -> Result<String> {
    let (session, _creds) = wallet.load()?;
    let sig = session.sign(&message);
    Ok(bs58::encode(sig).into_string())
}

fn load_bridge_html(app: &AppHandle) -> Result<String> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve("resources/wallet-bridge.html", tauri::path::BaseDirectory::Resource)
        .map_err(|e| crate::error::AppError::WalletBridge(format!("resolve resource: {e}")))?;
    std::fs::read_to_string(&path)
        .map_err(|e| crate::error::AppError::WalletBridge(format!("read {}: {e}", path.display())))
}

fn open_browser(url: &str) {
    // Best-effort: use the OS default URL handler. Failures are surfaced as an
    // event because wallet_connect has already returned.
    let _ = std::process::Command::new(if cfg!(target_os = "macos") { "open" } else if cfg!(target_os = "windows") { "cmd" } else { "xdg-open" })
        .args(if cfg!(target_os = "windows") { vec!["/C", "start", "", url] } else { vec![url] })
        .spawn();
}
```

- [ ] **Step 3: Register commands in lib.rs**

Edit `apps/desktop/src-tauri/src/lib.rs`:

- Add `app.manage(state.wallet.clone());` next to the other `.manage` calls in setup
- Add the 4 wallet commands to `tauri::generate_handler!`:

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
    commands::wallet_connect,
    commands::wallet_status,
    commands::wallet_revoke,
    commands::wallet_sign_message,
])
```

- [ ] **Step 4: Verify**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri && cargo check
```

Expected: exit 0. Some warnings about unused `WalletConnectStarted` (consumed by frontend) and other minor things are fine.

```bash
cargo test --lib
```

Expected: 44 tests pass (25 existing + 19 wallet).

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/src/state.rs apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "feat(desktop): wallet AppState + 4 wallet Tauri commands"
```

---

## Task 10: Copy wallet-bridge dist into Tauri resources at build time

**Files:**
- Modify: `apps/desktop/src-tauri/build.rs` (or create if missing)
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/resources/.gitkeep`

The Tauri scaffold has a `build.rs` that calls `tauri_build::build()`. We extend it to copy the wallet-bridge bundle into `resources/wallet-bridge.html` so `BaseDirectory::Resource` can resolve it at runtime.

- [ ] **Step 1: Read current build.rs**

```bash
cat /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/build.rs
```

Expected: a one-line file like `fn main() { tauri_build::build() }`.

- [ ] **Step 2: Update build.rs**

```rust
use std::path::{Path, PathBuf};

fn main() {
    copy_wallet_bridge();
    tauri_build::build()
}

fn copy_wallet_bridge() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir
        .parent()
        .unwrap()
        .join("wallet-bridge")
        .join("dist")
        .join("index.html");
    let dst_dir = manifest_dir.join("resources");
    let dst = dst_dir.join("wallet-bridge.html");

    println!("cargo:rerun-if-changed={}", src.display());

    if !src.exists() {
        println!(
            "cargo:warning=wallet-bridge bundle not found at {}. Run `pnpm build:wallet-bridge` before tauri build.",
            src.display()
        );
        // Write an empty placeholder so cargo doesn't fail
        std::fs::create_dir_all(&dst_dir).ok();
        let _ = std::fs::write(&dst, b"<html><body>wallet-bridge not built</body></html>");
        return;
    }

    std::fs::create_dir_all(&dst_dir).expect("create resources dir");
    std::fs::copy(&src, &dst).expect("copy wallet-bridge.html");
}
```

This:
- Re-runs when wallet-bridge/dist/index.html changes
- Falls back to a placeholder if the bridge hasn't been built (won't break the build)
- Emits a cargo warning if the placeholder is used, telling the developer how to fix it

- [ ] **Step 3: Update tauri.conf.json**

Add `resources` under `bundle`:

```json
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/brainctl-mcp"],
    "resources": ["resources/wallet-bridge.html"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
```

- [ ] **Step 4: Create resources/.gitkeep**

```bash
mkdir -p /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/resources
touch /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri/resources/.gitkeep
```

Add `.gitignore` entry to ignore the generated `wallet-bridge.html` (we don't commit build artifacts):

Edit root `.gitignore` to add (if not already covered by `dist/`):

```
# Generated by build.rs from wallet-bridge dist
apps/desktop/src-tauri/resources/wallet-bridge.html
```

- [ ] **Step 5: Build wallet-bridge then verify the copy**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm --filter wallet-bridge build
cd apps/desktop/src-tauri && cargo check
ls -la resources/
```

Expected: `resources/wallet-bridge.html` exists and is the same size as `apps/wallet-bridge/dist/index.html`.

- [ ] **Step 6: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/build.rs apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/resources/.gitkeep .gitignore
git commit -m "build(desktop): copy wallet-bridge dist into Tauri resources at build time"
```

---

## Task 11: TypeScript bridge for wallet commands, TDD

**Files:**
- Modify: `apps/desktop/src/lib/tauri-bridge.ts`
- Modify: `apps/desktop/src/lib/tauri-bridge.test.ts`

- [ ] **Step 1: Write failing tests**

Append to `apps/desktop/src/lib/tauri-bridge.test.ts`:

```typescript
import {
  walletConnect,
  walletStatus,
  walletRevoke,
  walletSignMessage,
} from "./tauri-bridge";

describe("wallet bridge", () => {
  it("walletConnect invokes wallet_connect and returns the bridge URL", async () => {
    invokeMock.mockResolvedValueOnce({ url: "http://127.0.0.1:18421/connect?nonce=abc" });
    const r = await walletConnect();
    expect(invokeMock).toHaveBeenCalledWith("wallet_connect");
    expect(r.url).toContain("127.0.0.1");
  });

  it("walletStatus returns null when not connected", async () => {
    invokeMock.mockResolvedValueOnce(null);
    const r = await walletStatus();
    expect(invokeMock).toHaveBeenCalledWith("wallet_status");
    expect(r).toBeNull();
  });

  it("walletStatus returns credentials when connected", async () => {
    invokeMock.mockResolvedValueOnce({
      session_pubkey_b58: "Sess111",
      master_pubkey_b58: "Master222",
      authorization: { message: "x", signature_b58: "y", signed_at: "2026-05-21T12:00:00Z", master_pubkey_b58: "Master222", session_pubkey_b58: "Sess111" },
    });
    const r = await walletStatus();
    expect(r?.session_pubkey_b58).toBe("Sess111");
  });

  it("walletRevoke invokes wallet_revoke", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await walletRevoke();
    expect(invokeMock).toHaveBeenCalledWith("wallet_revoke");
  });

  it("walletSignMessage passes message bytes and returns signature", async () => {
    invokeMock.mockResolvedValueOnce("signatureBase58Result");
    const sig = await walletSignMessage(new Uint8Array([1, 2, 3]));
    expect(invokeMock).toHaveBeenCalledWith("wallet_sign_message", { message: [1, 2, 3] });
    expect(sig).toBe("signatureBase58Result");
  });
});
```

- [ ] **Step 2: Run — must FAIL**

```bash
cd /Users/r4vager/Documents/Mantic && pnpm --filter mantic-desktop test
```

Expected: FAIL — functions don't exist.

- [ ] **Step 3: Implement the bridge additions**

Append to `apps/desktop/src/lib/tauri-bridge.ts`:

```typescript
// ---- Wallet ----

export interface WalletConnectStarted {
  url: string;
}

export interface StoredAuthorization {
  master_pubkey_b58: string;
  session_pubkey_b58: string;
  message: string;
  signature_b58: string;
  signed_at: string;
}

export interface WalletCredentials {
  session_pubkey_b58: string;
  master_pubkey_b58: string;
  authorization: StoredAuthorization;
}

export function walletConnect(): Promise<WalletConnectStarted> {
  return invoke<WalletConnectStarted>("wallet_connect");
}

export function walletStatus(): Promise<WalletCredentials | null> {
  return invoke<WalletCredentials | null>("wallet_status");
}

export function walletRevoke(): Promise<void> {
  return invoke<void>("wallet_revoke");
}

export function walletSignMessage(message: Uint8Array): Promise<string> {
  return invoke<string>("wallet_sign_message", { message: Array.from(message) });
}
```

- [ ] **Step 4: Run — must PASS**

```bash
pnpm --filter mantic-desktop test
```

Expected: PASS, with 5 new tests added to the existing tests for a total of 29.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src/lib/tauri-bridge.ts apps/desktop/src/lib/tauri-bridge.test.ts
git commit -m "feat(desktop): typed TS bridge for 4 wallet commands"
```

---

## Task 12: Wallet route component, TDD

**Files:**
- Create: `apps/desktop/src/routes/Wallet.tsx`
- Create: `apps/desktop/src/routes/Wallet.test.tsx`

- [ ] **Step 1: Write failing tests**

Create `apps/desktop/src/routes/Wallet.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Wallet from "./Wallet";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

function renderWallet(onConnected = vi.fn()) {
  return render(
    <MemoryRouter>
      <Wallet onConnected={onConnected} initialStatus={null} />
    </MemoryRouter>,
  );
}

describe("<Wallet />", () => {
  it("shows Connect button when no wallet is connected", () => {
    renderWallet();
    expect(screen.getByRole("button", { name: /connect/i })).toBeInTheDocument();
  });

  it("invokes wallet_connect when the button is clicked", async () => {
    invokeMock.mockResolvedValueOnce({ url: "http://127.0.0.1:18421/connect?nonce=abc" });
    renderWallet();
    await userEvent.click(screen.getByRole("button", { name: /connect/i }));
    expect(invokeMock).toHaveBeenCalledWith("wallet_connect");
  });

  it("shows connected state when initialStatus is provided", () => {
    render(
      <MemoryRouter>
        <Wallet
          onConnected={vi.fn()}
          initialStatus={{
            session_pubkey_b58: "SessionPubkey",
            master_pubkey_b58: "MasterPubkey",
            authorization: {
              master_pubkey_b58: "MasterPubkey",
              session_pubkey_b58: "SessionPubkey",
              message: "x",
              signature_b58: "y",
              signed_at: "2026-05-21T12:00:00Z",
            },
          }}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText(/SessionPubkey/i)).toBeInTheDocument();
    expect(screen.getByText(/MasterPubkey/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /revoke/i })).toBeInTheDocument();
  });

  it("invokes wallet_revoke when Revoke is clicked", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    render(
      <MemoryRouter>
        <Wallet
          onConnected={vi.fn()}
          initialStatus={{
            session_pubkey_b58: "S",
            master_pubkey_b58: "M",
            authorization: {
              master_pubkey_b58: "M",
              session_pubkey_b58: "S",
              message: "x",
              signature_b58: "y",
              signed_at: "2026-05-21T12:00:00Z",
            },
          }}
        />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: /revoke/i }));
    expect(invokeMock).toHaveBeenCalledWith("wallet_revoke");
  });
});
```

- [ ] **Step 2: Run — must FAIL**

```bash
cd /Users/r4vager/Documents/Mantic && pnpm --filter mantic-desktop test
```

Expected: FAIL — `./Wallet` doesn't exist.

- [ ] **Step 3: Implement Wallet.tsx**

Create `apps/desktop/src/routes/Wallet.tsx`:

```tsx
import { useState } from "react";
import {
  WalletCredentials,
  walletConnect,
  walletRevoke,
} from "../lib/tauri-bridge";

interface Props {
  initialStatus: WalletCredentials | null;
  onConnected: (creds: WalletCredentials) => void;
}

export default function Wallet({ initialStatus, onConnected: _onConnected }: Props) {
  const [status, setStatus] = useState<WalletCredentials | null>(initialStatus);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bridgeUrl, setBridgeUrl] = useState<string | null>(null);

  async function startConnect() {
    setError(null);
    setConnecting(true);
    try {
      const { url } = await walletConnect();
      setBridgeUrl(url);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message;
      setError(msg);
      setConnecting(false);
    }
  }

  async function revoke() {
    try {
      await walletRevoke();
      setStatus(null);
      setBridgeUrl(null);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message;
      setError(msg);
    }
  }

  if (status) {
    return (
      <div className="min-h-screen p-8">
        <div className="mx-auto max-w-2xl space-y-6">
          <header>
            <h1 className="text-3xl font-semibold">Wallet</h1>
            <p className="mt-1 text-sm text-neutral-400">Solana session wallet</p>
          </header>

          <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
            <h2 className="text-lg font-medium">Connected</h2>
            <dl className="mt-4 space-y-3 text-sm">
              <div>
                <dt className="text-neutral-500">Session wallet</dt>
                <dd className="mt-1 break-all font-mono">{status.session_pubkey_b58}</dd>
              </div>
              <div>
                <dt className="text-neutral-500">Master wallet</dt>
                <dd className="mt-1 break-all font-mono">{status.master_pubkey_b58}</dd>
              </div>
              <div>
                <dt className="text-neutral-500">Authorized at</dt>
                <dd className="mt-1">{new Date(status.authorization.signed_at).toLocaleString()}</dd>
              </div>
            </dl>
          </section>

          {error && <div role="alert" className="rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300">{error}</div>}

          <div className="flex justify-end">
            <button
              type="button"
              onClick={revoke}
              className="rounded-lg border border-red-800 px-4 py-2 text-sm text-red-300 transition hover:bg-red-950"
            >
              Revoke wallet
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-8">
      <div className="w-full max-w-md space-y-6 rounded-2xl bg-neutral-900 p-8 shadow-xl">
        <div>
          <h1 className="text-2xl font-semibold">Connect a Solana wallet</h1>
          <p className="mt-2 text-sm text-neutral-400">
            Mantic will generate a session wallet on your machine. You'll authorize it by
            signing a message in your master wallet, then fund it manually when you're
            ready to trade.
          </p>
        </div>

        <button
          type="button"
          disabled={connecting}
          onClick={startConnect}
          className="w-full rounded-lg bg-blue-600 px-4 py-2 font-medium transition hover:bg-blue-500 disabled:cursor-not-allowed disabled:bg-neutral-700"
        >
          {connecting ? "Waiting for browser..." : "Connect Phantom or Solflare"}
        </button>

        {bridgeUrl && (
          <div className="rounded-lg bg-neutral-950 p-3 text-xs text-neutral-400">
            Your browser should have opened. If not:{" "}
            <a href={bridgeUrl} className="text-blue-400 underline">{bridgeUrl}</a>
          </div>
        )}

        {error && (
          <div role="alert" className="rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300">{error}</div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run — must PASS**

```bash
pnpm --filter mantic-desktop test
```

Expected: 33 frontend tests pass (29 existing + 4 Wallet).

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src/routes/Wallet.tsx apps/desktop/src/routes/Wallet.test.tsx
git commit -m "feat(desktop): Wallet route with connect button and connected state"
```

---

## Task 13: App.tsx routing — Pair → Wallet → Account, TDD

**Files:**
- Modify: `apps/desktop/src/App.tsx`
- Modify: `apps/desktop/src/App.test.tsx`

The state machine becomes: `loading → unpaired (Pair) → paired-no-wallet (Wallet) → paired-with-wallet (Account)`.

- [ ] **Step 1: Update App.test.tsx**

Replace `apps/desktop/src/App.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<App />", () => {
  it("shows Pair route when no license is stored", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "current_account") throw "no license stored";
      throw new Error(`unexpected: ${cmd}`);
    });
    render(<MemoryRouter><App /></MemoryRouter>);
    expect(await screen.findByText(/pair this device/i)).toBeInTheDocument();
  });

  it("shows Wallet route when paired but no wallet", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "current_account") {
        return { account_id: "acct_x", tier: "pro", expires_at: Math.floor(Date.now() / 1000) + 3600 };
      }
      if (cmd === "wallet_status") return null;
      throw new Error(`unexpected: ${cmd}`);
    });
    render(<MemoryRouter><App /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText(/connect a solana wallet/i)).toBeInTheDocument();
    });
  });

  it("shows Account route when paired AND wallet connected", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "current_account") {
        return { account_id: "acct_x", tier: "pro", expires_at: Math.floor(Date.now() / 1000) + 3600 };
      }
      if (cmd === "wallet_status") {
        return {
          session_pubkey_b58: "Sess",
          master_pubkey_b58: "Mast",
          authorization: {
            master_pubkey_b58: "Mast",
            session_pubkey_b58: "Sess",
            message: "x",
            signature_b58: "y",
            signed_at: "2026-05-21T12:00:00Z",
          },
        };
      }
      throw new Error(`unexpected: ${cmd}`);
    });
    render(<MemoryRouter><App /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText(/acct_x/i)).toBeInTheDocument();
    });
  });
});
```

- [ ] **Step 2: Run — must FAIL** (App.tsx still only routes Pair vs Account)

```bash
cd /Users/r4vager/Documents/Mantic && pnpm --filter mantic-desktop test
```

Expected: FAIL on the new "shows Wallet route" test.

- [ ] **Step 3: Update App.tsx**

Replace `apps/desktop/src/App.tsx`:

```tsx
import { useEffect, useState } from "react";
import {
  AccountInfo,
  WalletCredentials,
  currentAccount,
  walletStatus,
} from "./lib/tauri-bridge";
import Pair from "./routes/Pair";
import Wallet from "./routes/Wallet";
import Account from "./routes/Account";

type Status = "loading" | "unpaired" | "paired-no-wallet" | "paired-with-wallet";

export default function App() {
  const [status, setStatus] = useState<Status>("loading");
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [wallet, setWallet] = useState<WalletCredentials | null>(null);

  useEffect(() => {
    let active = true;
    (async () => {
      const acct = await currentAccount();
      if (!active) return;
      if (!acct) {
        setStatus("unpaired");
        return;
      }
      setAccount(acct);
      const w = await walletStatus();
      if (!active) return;
      if (w) {
        setWallet(w);
        setStatus("paired-with-wallet");
      } else {
        setStatus("paired-no-wallet");
      }
    })().catch(() => {
      if (active) setStatus("unpaired");
    });
    return () => {
      active = false;
    };
  }, []);

  if (status === "loading") {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-neutral-500">Loading…</p>
      </div>
    );
  }

  if (status === "unpaired" || !account) {
    return (
      <Pair
        onPaired={async (info) => {
          setAccount(info);
          const w = await walletStatus();
          if (w) {
            setWallet(w);
            setStatus("paired-with-wallet");
          } else {
            setStatus("paired-no-wallet");
          }
        }}
      />
    );
  }

  if (status === "paired-no-wallet" || !wallet) {
    return (
      <Wallet
        initialStatus={wallet}
        onConnected={(creds) => {
          setWallet(creds);
          setStatus("paired-with-wallet");
        }}
      />
    );
  }

  return (
    <Account
      account={account}
      onSignOut={() => {
        setAccount(null);
        setWallet(null);
        setStatus("unpaired");
      }}
    />
  );
}
```

- [ ] **Step 4: Run — must PASS**

```bash
pnpm --filter mantic-desktop test
```

Expected: 33 frontend tests pass (the 3 App tests + 4 Wallet + 11 brain-bridge + 4 license-bridge + 3 Pair + 2 Account + 6 misc = adjust if your count varies; the important thing is all green).

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src/App.tsx apps/desktop/src/App.test.tsx
git commit -m "feat(desktop): three-state routing (Pair → Wallet → Account)"
```

---

## Task 14: End-to-end bridge HTTP test

We already test the bridge's handler logic in Task 8. This task adds a tighter integration test that spawns a Tauri-less bridge with a known HTML payload and drives it via reqwest, including the `/auth-message` and `/authorize` flow.

This is essentially the Task 8 `full_round_trip_succeeds_with_valid_signature` test but moved to a dedicated integration test file under `tests/` to exercise the lib-crate boundary.

**Files:**
- Create: `apps/desktop/src-tauri/tests/wallet_bridge_integration.rs`

- [ ] **Step 1: Make wallet module public**

In `apps/desktop/src-tauri/src/lib.rs`, change `mod wallet;` to `pub mod wallet;`.

- [ ] **Step 2: Create the integration test**

```rust
//! End-to-end test of the wallet bridge: spawn the axum server, drive it via reqwest
//! as if we were the wallet-bridge JS page, verify a complete signed authorization
//! round-trips back into a StoredAuthorization.

use desktop_lib::wallet::session::SessionKey;
use desktop_lib::wallet::bridge::BridgeServer;
use desktop_lib::wallet::authorization::AuthorizePayload;

#[tokio::test]
async fn full_bridge_round_trip() {
    let html = "<html>test bridge</html>".to_string();
    let (url, server) = BridgeServer::start(html).await.unwrap();

    // Parse port + nonce from the URL
    let port: u16 = url
        .strip_prefix("http://127.0.0.1:").unwrap()
        .split('/').next().unwrap()
        .parse().unwrap();
    let nonce = url.split("nonce=").nth(1).unwrap().to_string();

    // Pretend we're the browser-side JS: pick a "master" wallet
    let master = SessionKey::generate();
    let auth_msg_url = format!(
        "http://127.0.0.1:{port}/auth-message?nonce={nonce}&master={}",
        master.pubkey_base58()
    );
    let msg = reqwest::get(&auth_msg_url).await.unwrap().text().await.unwrap();

    // Sign the message with the "master" wallet
    let sig = master.sign(msg.as_bytes());
    let payload = AuthorizePayload {
        pubkey: master.pubkey_base58(),
        signature: bs58::encode(sig).into_string(),
        message: msg.clone(),
        nonce: nonce.clone(),
    };

    // POST it back
    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://127.0.0.1:{port}/authorize?nonce={nonce}"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);

    // Server completes the await with the verified authorization
    let outcome = server.await_authorization().await.unwrap();
    assert_eq!(outcome.stored.master_pubkey_b58, master.pubkey_base58());
    assert!(outcome.stored.message.contains(&outcome.stored.master_pubkey_b58));
    assert!(outcome.stored.message.contains(&outcome.session_key.pubkey_base58()));
}
```

- [ ] **Step 3: Add bs58 to dev-deps (if not already there)**

bs58 is in main deps (Task 3). Integration tests can use main deps, but let's confirm by running. If linking fails, add to `[dev-dependencies]` explicitly.

- [ ] **Step 4: Run the integration test**

```bash
cd /Users/r4vager/Documents/Mantic/apps/desktop/src-tauri
cargo test --test wallet_bridge_integration -- --nocapture
```

Expected: 1 test passes.

Then run the whole test surface:

```bash
cargo test
```

Expected: lib tests (44) + brainctl_integration (1) + wallet_bridge_integration (1) = 46 total.

- [ ] **Step 5: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add apps/desktop/src-tauri/tests/wallet_bridge_integration.rs apps/desktop/src-tauri/src/lib.rs
git commit -m "test(desktop): end-to-end wallet bridge HTTP round-trip integration test"
```

---

## Task 15: README polish + final test sweep

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Full test sweep**

```bash
cd /Users/r4vager/Documents/Mantic
pnpm install
pnpm -r test
cd apps/desktop/src-tauri && cargo test
```

Expected counts (approximate):
- mock-license: 6
- desktop frontend: 33
- e2e smoke: 2
- desktop Rust lib: 44
- integration tests: 2 (brainctl + wallet_bridge)

All green.

- [ ] **Step 2: Update README**

In `/Users/r4vager/Documents/Mantic/README.md`, add a Wallet section under "Quickstart". Find the existing Quickstart block and append:

```markdown
## Wallet connect (Solana)

Mantic uses a localhost web bridge to connect to Phantom or Solflare. The bridge ships
as a single-file HTML asset built from `apps/wallet-bridge/`. To build it locally:

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
```

- [ ] **Step 3: Commit**

```bash
cd /Users/r4vager/Documents/Mantic
git add README.md
git commit -m "docs: README wallet section for sub-project #3"
```

---

## Definition of done

- [ ] All 44 Rust lib tests pass on `build/wallet-session-signer`
- [ ] Both integration tests pass (brainctl + wallet_bridge)
- [ ] All 33 frontend tests pass
- [ ] All 6 mock-license tests pass
- [ ] All 2 e2e smoke tests pass
- [ ] `pnpm dev:desktop` launches the app; an unpaired+no-wallet path lands you on Pair; pair, and you land on Wallet; click Connect and the browser opens to a real Phantom-compatible bridge page
- [ ] `pnpm --filter wallet-bridge build` produces a single-file HTML asset
- [ ] `cargo build --release` from `apps/desktop/src-tauri` succeeds (NO `tauri build` — Finder pop)

After all checked, the desktop daemon can connect to Phantom, has a session keypair stored in the keychain, and sub-project #4 (agent runtime) can call `wallet_sign_message` to sign trades.

---

*Plan total: 15 tasks. Comparable in scope to sub-project #2. Estimated 5-7 hours via subagent-driven execution.*
