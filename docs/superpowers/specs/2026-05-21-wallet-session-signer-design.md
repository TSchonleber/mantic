# Wallet + Session Signer — Sub-project #3 Design Spec

**Date:** 2026-05-21
**Status:** Approved by founder. Ready for implementation plan.
**Builds on:** sub-project #2 (brain.db integration, complete at main commit `1c850e1`)
**Scope:** Solana only, full real-wallet integration.

---

## Goal

Mantic can connect to a user's Phantom (or Solflare) wallet, generate a local Solana session keypair, capture a signed off-chain authorization, and use the session keypair to sign transactions autonomously. The session wallet is the trading wallet; the master wallet is treated as a vault.

## Signing model (Model B — Funded Session Wallet)

- Mantic generates an ed25519 **session keypair** locally.
- User signs an **off-chain authorization message** in Phantom acknowledging that:
  - This specific session pubkey is authorized to trade on their behalf
  - They will fund the session wallet themselves
  - Mantic has full control over the session wallet's contents
- User transfers SOL/USDC from master → session wallet at their discretion.
- The session wallet IS the trading wallet from sub-project #4 onward.
- **No on-chain delegation, no spending cap primitive.** The session wallet's balance is the security model. User wants to stop? Drain the session wallet back to master.
- Authorization signature is stored locally as an audit-trail record (and a proof artifact for future legal/disclosure UX), not as an on-chain credential.

## Connection mechanism — local web bridge

Desktop apps can't inject `window.solana` like a browser can. The pattern:

```
Mantic Desktop App                                         Browser
┌─────────────────────────┐                       ┌────────────────────────────┐
│ Wallet pane             │                       │ http://127.0.0.1:18421     │
│ [Connect Phantom]  ─────┼──── open URL ────────►│ (Mantic-spawned, one-shot) │
│                         │                       │ @solana/wallet-adapter UI  │
│                         │                       │ ↓ user picks Phantom       │
│                         │                       │ ↓ signMessage(auth_msg)    │
│                         │   POST /authorize     │ ↓                          │
│  ◄──────────────────────┼───────────────────────│ POST signed auth back      │
│                         │                       └────────────────────────────┘
│ Store in keychain:      │
│  - session_privkey      │
│  - master_pubkey        │
│  - authorization_blob   │
└─────────────────────────┘
```

The bridge is a one-shot localhost HTTP server bound to `127.0.0.1:18421` with a random nonce in the connection URL. Shuts down as soon as the authorization comes back.

The bridge page is a pre-built static HTML/JS bundle using `@solana/wallet-adapter-react` + Phantom/Solflare adapters, embedded in Mantic.app via Tauri's resource directory.

## Architecture

### New Rust modules (`apps/desktop/src-tauri/src/wallet/`)

- `mod.rs` — re-exports
- `session.rs` — `SessionKey` struct: ed25519 keypair, `generate()`, `pubkey() -> [u8; 32]`, `sign(message: &[u8]) -> [u8; 64]`. Serialization to/from a hex/base58 string for keychain storage.
- `authorization.rs` — `AuthorizationMessage` builder (constructs the human-readable text), `verify_signature(message, signature, master_pubkey)` using ed25519-dalek. Plus `StoredAuthorization` struct with `master_pubkey`, `session_pubkey`, `message_text`, `signature`, `signed_at`.
- `bridge.rs` — `BridgeServer` that spawns `axum` on `127.0.0.1:18421`, serves the static bridge HTML/JS from Tauri's resource dir, handles `POST /authorize`, fires a tokio oneshot when authorized, auto-shutdowns on receipt or timeout.
- `store.rs` — wraps the existing `KeyringStore` for wallet credentials. New keychain slots: `wallet-session-privkey`, `wallet-master-pubkey`, `wallet-authorization`. Save/load/clear methods, similar shape to the license storage.

### New static asset

- `apps/desktop/src-tauri/resources/wallet-bridge/index.html` — pre-built bundle from a tiny `apps/wallet-bridge/` workspace that compiles `@solana/wallet-adapter-react` + Phantom/Solflare adapters into a single-file HTML page. Bundled in `tauri.conf.json` `bundle.resources`.

### New Tauri commands

- `wallet_connect()` — starts the bridge, opens the browser, returns a oneshot future that resolves when authorization completes
- `wallet_status() -> WalletStatus | null` — returns the stored master pubkey + session pubkey + auth timestamp, or null if not connected
- `wallet_revoke()` — clears all wallet credentials from keychain
- `wallet_sign_message(message: Vec<u8>) -> [u8; 64]` — exposes the session key's signing for sub-project #4

### Frontend

- `apps/desktop/src/routes/Wallet.tsx` — new route showing connect button (if not connected), current wallet info (if connected), revoke button
- App.tsx routing extended: when paired AND no wallet, show Wallet route; when paired AND wallet connected, show the existing Account route (which we'll extend in sub-project #4 with the fleet UI)
- `tauri-bridge.ts` adds: `walletConnect()`, `walletStatus()`, `walletRevoke()`, `walletSignMessage(bytes)`

## Authorization message format

```
Mantic Session Authorization

I authorize Mantic to use the following session wallet for autonomous
trading on my behalf:

  Session wallet: <session-pubkey-base58>

I understand:
  - I will fund this wallet manually from my master wallet
  - Mantic has full control over the session wallet's contents
  - This authorization is recorded off-chain only

Master wallet: <master-pubkey-base58>
Issued: <ISO-8601 timestamp>
Nonce: <random 16-byte hex>
```

Signed by master via Phantom's `signMessage`. Stored locally as audit trail.

## Data flow — connect

1. User clicks "Connect Phantom" in Wallet pane → `invoke('wallet_connect')`
2. Rust: generate session keypair, build authorization message text, start bridge server with a random nonce, open default browser to `http://127.0.0.1:18421/connect?nonce=<random>`
3. Browser loads bundled wallet-adapter page
4. Page: user picks Phantom → wallet extension prompts for connect → page captures `publicKey`
5. Page calls `wallet.signMessage(authMessageText)` → Phantom shows the human-readable text → user signs
6. Page POSTs `{ pubkey, signature, message, nonce }` to `http://127.0.0.1:18421/authorize`
7. Rust: verify nonce matches, verify ed25519 signature, store credentials in keychain, send `wallet:connected` event, shut down bridge
8. React: refetches `walletStatus()`, transitions Wallet pane to connected state

## Data flow — sign transaction (sub-project #4 consumer)

`SessionKey::sign(transaction_bytes)` is plain ed25519 signing. The agent runtime (#4) calls `wallet_sign_message` from Rust (or via the bridge) when it has a serialized Solana transaction to sign. No human in the loop.

## Failure modes

| Failure | Detection | Recovery |
|---|---|---|
| Bridge port 18421 in use | `axum::Server::bind` returns EADDRINUSE | Try ports 18421..18430; report error if all are taken |
| User closes browser without signing | bridge times out after 5 minutes | shut down bridge, return `WalletError::ConnectionTimeout` |
| Signature verification fails | ed25519-dalek returns error | Reject the authorize POST, log it, don't store credentials |
| Nonce mismatch | bridge `/authorize` handler compares | Reject (replay attack defense) |
| Phantom not installed in browser | wallet-adapter throws | Page surfaces install link to phantom.com; user installs and retries |
| Keychain write fails | rusqlite-style error from keyring crate | Return error; user can retry — credentials NOT partially written |

## Concurrency model

- One connect-in-flight at a time. If user clicks Connect while a bridge is already running, return `WalletError::ConnectInProgress`.
- Once connected, `wallet_sign_message` is fully concurrent — `SessionKey` is `Arc<...>` + immutable ed25519 keypair.

## Testing

- **Rust unit tests:**
  - `session.rs` — keypair gen, sign/verify roundtrip, base58 serde
  - `authorization.rs` — message builder, signature verify against a known-good test vector
  - `bridge.rs` — axum handler logic with a mock signed payload (no real wallet)
  - `store.rs` — keychain save/load/revoke roundtrip (using the same mock pattern as `license.rs`)
- **Frontend tests:** Wallet.tsx component tests for connected/disconnected states
- **Integration test:** test the bridge HTTP surface against a curl-driven mock signed payload (real bridge spawn, fake wallet)
- **Manual:** real Phantom connect on user's machine before declaring done

## Out of scope

- Funds transfer UI from master to session (user does it manually in Phantom)
- Withdraw-to-master helper (post-v1 polish)
- Multi-wallet support (one master + one session in v1)
- Hardware wallet support (Ledger Live integration is post-v1)
- EVM chains (deferred to a separate sub-project)
- Solflare-specific UX (the wallet-adapter handles it generically)

## Implementation plan decomposition

Single plan (~15-18 tasks). Build order:

1. **Wallet workspace scaffolding** — `apps/wallet-bridge/` static-asset build
2. **Bridge HTML/JS bundle** — wallet-adapter React app, build to static index.html
3. **Solana deps in Rust** — add `solana-sdk`, `ed25519-dalek`, `bs58`, `axum`, `tower-http`
4. **`wallet::session` module + TDD** — keypair gen, sign/verify
5. **`wallet::authorization` module + TDD** — message builder, signature verify
6. **`wallet::store` module + TDD** — keychain wallet credential storage
7. **`wallet::bridge` module + TDD** — axum server handler logic (mock signed payload)
8. **`wallet::bridge` integration** — wire into Tauri resource dir to serve the bundled HTML
9. **Tauri commands** — `wallet_connect`, `wallet_status`, `wallet_revoke`, `wallet_sign_message`
10. **AppState extension** — `wallet: Arc<WalletStore>` field
11. **TS bridge** — `walletConnect`, `walletStatus`, `walletRevoke`, `walletSignMessage`
12. **Wallet.tsx route + tests**
13. **App.tsx routing** — wire Wallet route between Pair and Account
14. **End-to-end bridge test** — curl-driven mock signed payload
15. **README polish + manual Phantom test**

Estimated 5-7 hours via subagent-driven execution.
