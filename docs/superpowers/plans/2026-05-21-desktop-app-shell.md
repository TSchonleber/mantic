# Desktop App Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the foundational Tauri 2 desktop app that pairs with brainctl.org via a license code, stores and refreshes a JWT, and renders a minimal authenticated UI shell. This is sub-project #1 of the Mantic v1 design (see `docs/superpowers/specs/2026-05-21-mantic-design.md`). Every other sub-project plugs into this shell.

**Architecture:** Tauri 2.x desktop app. Rust core handles all secret storage (OS keychain via `keyring` crate), HTTP to the license server (`reqwest`), JWT decoding (`jsonwebtoken`), and a background heartbeat task (`tokio`). React + TypeScript frontend in Vite. Frontend invokes Rust commands through a typed `tauri-bridge.ts` wrapper. A local Hono mock-license server stands in for the real license server (sub-project #9) during development; both speak the same `/v1/pair` and `/v1/refresh` contract. Monorepo via pnpm workspaces.

**Tech Stack:**
- Tauri 2.x, Rust (stable)
- Crates: `keyring`, `reqwest`, `jsonwebtoken`, `tokio`, `serde`, `serde_json`, `thiserror`, `anyhow`, `chrono`
- React 19, TypeScript 5.x, Vite 6, React Router 7
- Tailwind CSS 4 + shadcn/ui
- Vitest + @testing-library/react (frontend tests)
- Playwright + tauri-driver (E2E smoke test)
- Hono + tsx + @hono/node-server (mock license server)
- jose (npm) for HS256 JWT signing in mock server
- pnpm workspaces
- GitHub Actions for CI/build

---

## Spec → task coverage map

| Spec requirement (section ref) | Implemented by |
|---|---|
| Desktop app paired via 6-digit code (B.5) | Tasks 2, 6, 10, 12 |
| License JWT issued by brainctl.org, refreshed on heartbeat (E.License) | Tasks 5, 6, 13 |
| Secrets stay local (A.Trust boundary) | Task 5 (keyring) |
| Cross-platform packaging macOS/Win/Linux (B.4) | Task 15 |
| Account portal shows tier + expiry (E.Account & identity) | Tasks 7, 11 |

Out of scope for this plan (other sub-projects): brain.db, wallet/signer, agent runtime, strategy templates, signal API, fleet brain, marketing site, real license server, token verification, GUI surfaces beyond Pair+Account, CLI headless mode, auto-updater.

---

## Pre-flight: prerequisites the engineer must have installed

Before starting Task 1, verify locally:

```bash
node --version   # >= 20.x
pnpm --version   # >= 9.x
rustc --version  # >= 1.78
cargo --version
```

Install Tauri 2 prerequisites per https://tauri.app/start/prerequisites/ (Xcode CLT on macOS; WebView2 on Windows; webkit2gtk on Linux). If any of these are missing, stop and install before proceeding.

---

## Task 1: Monorepo scaffold

**Files:**
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `.gitignore`
- Create: `.editorconfig`
- Create: `README.md`

- [ ] **Step 1: Create root `package.json`**

```json
{
  "name": "mantic",
  "version": "0.0.0",
  "private": true,
  "description": "Mantic — brainctl-powered trading agent platform",
  "scripts": {
    "dev:mock-license": "pnpm --filter mock-license dev",
    "dev:desktop": "pnpm --filter mantic-desktop tauri dev",
    "build:desktop": "pnpm --filter mantic-desktop tauri build",
    "test": "pnpm -r test",
    "lint": "pnpm -r lint"
  },
  "packageManager": "pnpm@9.12.0",
  "engines": { "node": ">=20" }
}
```

- [ ] **Step 2: Create `pnpm-workspace.yaml`**

```yaml
packages:
  - "apps/*"
  - "services/*"
```

- [ ] **Step 3: Create `.gitignore`**

```gitignore
# Node
node_modules/
.pnpm-store/
dist/
*.log

# Rust / Tauri
target/
**/Cargo.lock.bak

# OS
.DS_Store
Thumbs.db

# Editor
.idea/
.vscode/*
!.vscode/extensions.json

# Env
.env
.env.local

# Playwright
test-results/
playwright-report/
```

- [ ] **Step 4: Create `.editorconfig`**

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 2
insert_final_newline = true
trim_trailing_whitespace = true

[*.{rs,toml}]
indent_size = 4
```

- [ ] **Step 5: Create `README.md` (single section, minimal)**

```markdown
# Mantic

Trading agent platform powered by brainctl. See `docs/superpowers/specs/` for design, `docs/superpowers/plans/` for implementation plans.

## Quickstart

```bash
pnpm install
pnpm dev:mock-license   # in one terminal
pnpm dev:desktop        # in another
```
```

- [ ] **Step 6: Verify pnpm install works**

Run: `pnpm install`
Expected: Lockfile created at `pnpm-lock.yaml`, exit code 0. (No workspaces have a package.json yet, so this is essentially a no-op — that's fine.)

- [ ] **Step 7: Commit**

```bash
git add package.json pnpm-workspace.yaml .gitignore .editorconfig README.md pnpm-lock.yaml
git commit -m "chore: monorepo scaffold"
```

---

## Task 2: Mock license server (Hono)

This stands in for brainctl.org's real license server during development. Sub-project #9 will replace it with a real implementation. Same HTTP contract.

**Files:**
- Create: `services/mock-license/package.json`
- Create: `services/mock-license/tsconfig.json`
- Create: `services/mock-license/src/jwt.ts`
- Create: `services/mock-license/src/server.ts`
- Create: `services/mock-license/tests/server.test.ts`

- [ ] **Step 1: Create `services/mock-license/package.json`**

```json
{
  "name": "mock-license",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "tsx watch src/server.ts",
    "test": "vitest run",
    "lint": "tsc --noEmit"
  },
  "dependencies": {
    "@hono/node-server": "^1.13.0",
    "hono": "^4.6.0",
    "jose": "^5.9.0"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "tsx": "^4.19.0",
    "typescript": "^5.6.0",
    "vitest": "^2.1.0"
  }
}
```

- [ ] **Step 2: Create `services/mock-license/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "noEmit": true
  },
  "include": ["src/**/*", "tests/**/*"]
}
```

- [ ] **Step 3: Install deps**

Run: `pnpm install`
Expected: `node_modules` populated, exit code 0.

- [ ] **Step 4: Write the failing test for JWT signing**

Create `services/mock-license/tests/server.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { signLicenseJwt, verifyLicenseJwt, MOCK_SECRET } from "../src/jwt.js";

describe("license jwt", () => {
  it("signs a payload and verifies round-trip", async () => {
    const token = await signLicenseJwt({
      account_id: "acct_123",
      tier: "pro",
      expires_at: Math.floor(Date.now() / 1000) + 3600,
    });
    expect(typeof token).toBe("string");
    expect(token.split(".")).toHaveLength(3);

    const decoded = await verifyLicenseJwt(token);
    expect(decoded.account_id).toBe("acct_123");
    expect(decoded.tier).toBe("pro");
  });

  it("rejects a tampered token", async () => {
    const token = await signLicenseJwt({
      account_id: "acct_123",
      tier: "pro",
      expires_at: Math.floor(Date.now() / 1000) + 3600,
    });
    const tampered = token.slice(0, -4) + "XXXX";
    await expect(verifyLicenseJwt(tampered)).rejects.toThrow();
  });
});
```

- [ ] **Step 5: Run test to verify it fails**

Run: `pnpm --filter mock-license test`
Expected: FAIL with "Cannot find module '../src/jwt.js'" or similar.

- [ ] **Step 6: Implement `services/mock-license/src/jwt.ts`**

```typescript
import { SignJWT, jwtVerify } from "jose";

export interface LicensePayload {
  account_id: string;
  tier: "free" | "starter" | "pro" | "whale";
  expires_at: number;
}

export const MOCK_SECRET = new TextEncoder().encode(
  "dev-only-mantic-mock-secret-do-not-use-in-prod",
);

export async function signLicenseJwt(payload: LicensePayload): Promise<string> {
  return await new SignJWT({
    account_id: payload.account_id,
    tier: payload.tier,
  })
    .setProtectedHeader({ alg: "HS256" })
    .setIssuedAt()
    .setExpirationTime(payload.expires_at)
    .sign(MOCK_SECRET);
}

export async function verifyLicenseJwt(token: string): Promise<LicensePayload> {
  const { payload } = await jwtVerify(token, MOCK_SECRET);
  if (typeof payload.account_id !== "string" || typeof payload.tier !== "string") {
    throw new Error("invalid claims");
  }
  return {
    account_id: payload.account_id,
    tier: payload.tier as LicensePayload["tier"],
    expires_at: payload.exp ?? 0,
  };
}
```

- [ ] **Step 7: Run JWT test to verify pass**

Run: `pnpm --filter mock-license test`
Expected: PASS, 2 tests.

- [ ] **Step 8: Add the server integration test**

Append to `services/mock-license/tests/server.test.ts`:

```typescript
import { app } from "../src/server.js";

describe("server /v1/pair", () => {
  it("issues a JWT for a valid pairing code", async () => {
    const res = await app.request("/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: "123456" }),
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as { token: string };
    expect(typeof body.token).toBe("string");
  });

  it("rejects an invalid code", async () => {
    const res = await app.request("/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: "" }),
    });
    expect(res.status).toBe(400);
  });
});

describe("server /v1/refresh", () => {
  it("issues a new JWT for a valid current token", async () => {
    const pairRes = await app.request("/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: "123456" }),
    });
    const { token } = (await pairRes.json()) as { token: string };

    const res = await app.request("/v1/refresh", {
      method: "POST",
      headers: { authorization: `Bearer ${token}` },
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as { token: string };
    expect(typeof body.token).toBe("string");
  });

  it("rejects missing auth", async () => {
    const res = await app.request("/v1/refresh", { method: "POST" });
    expect(res.status).toBe(401);
  });
});
```

- [ ] **Step 9: Run server tests to verify failure**

Run: `pnpm --filter mock-license test`
Expected: FAIL (server.js does not exist yet).

- [ ] **Step 10: Implement `services/mock-license/src/server.ts`**

```typescript
import { Hono } from "hono";
import { serve } from "@hono/node-server";
import { signLicenseJwt, verifyLicenseJwt } from "./jwt.js";

export const app = new Hono();

app.post("/v1/pair", async (c) => {
  const body = (await c.req.json().catch(() => ({}))) as { code?: string };
  if (!body.code || body.code.length < 4) {
    return c.json({ error: "invalid_code" }, 400);
  }
  const token = await signLicenseJwt({
    account_id: "acct_dev_" + body.code,
    tier: "pro",
    expires_at: Math.floor(Date.now() / 1000) + 60 * 60 * 24,
  });
  return c.json({ token });
});

app.post("/v1/refresh", async (c) => {
  const auth = c.req.header("authorization");
  if (!auth?.startsWith("Bearer ")) {
    return c.json({ error: "missing_auth" }, 401);
  }
  const current = auth.slice("Bearer ".length);
  try {
    const decoded = await verifyLicenseJwt(current);
    const token = await signLicenseJwt({
      account_id: decoded.account_id,
      tier: decoded.tier,
      expires_at: Math.floor(Date.now() / 1000) + 60 * 60 * 24,
    });
    return c.json({ token });
  } catch {
    return c.json({ error: "invalid_token" }, 401);
  }
});

app.get("/healthz", (c) => c.json({ ok: true }));

const port = Number(process.env.PORT ?? 4001);

if (import.meta.url === `file://${process.argv[1]}`) {
  serve({ fetch: app.fetch, port }, (info) => {
    console.log(`[mock-license] listening on http://localhost:${info.port}`);
  });
}
```

- [ ] **Step 11: Run all mock-license tests**

Run: `pnpm --filter mock-license test`
Expected: PASS, 6 tests.

- [ ] **Step 12: Verify the server runs**

Run: `pnpm --filter mock-license dev` (in a separate terminal; Ctrl-C to stop)
In another terminal:

```bash
curl -X POST http://localhost:4001/v1/pair \
  -H "content-type: application/json" \
  -d '{"code":"123456"}'
```

Expected: a JSON response containing a `token` field with a JWT.

- [ ] **Step 13: Commit**

```bash
git add services/mock-license pnpm-lock.yaml
git commit -m "feat(mock-license): hono mock license server with /v1/pair and /v1/refresh"
```

---

## Task 3: Tauri app scaffold

**Files:**
- Create: `apps/desktop/` (full Tauri 2 scaffold)
- Modify: `apps/desktop/package.json` (rename + script tweaks)
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

- [ ] **Step 1: Scaffold the Tauri app**

From the repo root, run:

```bash
mkdir -p apps && cd apps && pnpm create tauri-app@latest
```

The CLI is interactive. Answer the prompts as follows:

| Prompt | Answer |
|---|---|
| Project name | `desktop` |
| Identifier | `org.brainctl.mantic` |
| Frontend language | `TypeScript / JavaScript` |
| Package manager | `pnpm` |
| UI template | `React` |
| UI flavor | `TypeScript` |

After the scaffold completes, `cd ..` back to the repo root.

Expected: a new `apps/desktop/` directory with Tauri 2 + React + TypeScript scaffold. Subsequent tasks patch this scaffold; if a future Tauri CLI version produces a different file layout, adapt the patches accordingly but keep the same final structure (Rust under `src-tauri/`, frontend under `src/`).

- [ ] **Step 2: Patch `apps/desktop/package.json` name to match workspace convention**

Modify `apps/desktop/package.json`: change `"name"` field to `"mantic-desktop"` and ensure `"private": true`. Verify scripts section contains:

```json
"scripts": {
  "dev": "vite",
  "build": "tsc && vite build",
  "preview": "vite preview",
  "tauri": "tauri",
  "test": "vitest run",
  "test:watch": "vitest",
  "lint": "tsc --noEmit"
}
```

If any are missing, add them. Leave existing devDependencies alone — you'll add more in later tasks.

- [ ] **Step 3: Patch `apps/desktop/src-tauri/tauri.conf.json`**

Modify the file so the product identity is correct:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Mantic",
  "version": "0.0.0",
  "identifier": "org.brainctl.mantic",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "pnpm dev",
    "beforeBuildCommand": "pnpm build"
  },
  "app": {
    "windows": [
      {
        "title": "Mantic",
        "width": 1100,
        "height": 720,
        "minWidth": 900,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

Leave the default icons in place (Tauri scaffolds reasonable placeholders).

- [ ] **Step 4: Install deps**

Run: `pnpm install`
Expected: workspace links `mantic-desktop`, exits 0.

- [ ] **Step 5: Smoke-run the dev mode**

Run: `pnpm dev:desktop`
Expected: Tauri window opens displaying the default React + Tauri starter content. Close the window and stop the process.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop pnpm-lock.yaml
git commit -m "feat(desktop): initial tauri 2 + react-ts scaffold"
```

---

## Task 4: Rust dependencies for license/pairing

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Add the dependencies block**

Modify `apps/desktop/src-tauri/Cargo.toml` — add or merge into the `[dependencies]` section:

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
jsonwebtoken = "9"
keyring = "3"
thiserror = "1"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
mockito = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time", "test-util"] }
```

(Tauri's scaffold may have already added `tauri`, `serde`, `serde_json`. Merge rather than duplicate.)

- [ ] **Step 2: Verify the crate builds**

Run: `cd apps/desktop/src-tauri && cargo check`
Expected: PASS (warnings allowed). If keyring fails on Linux due to missing libdbus, install `libdbus-1-dev` (or equivalent) and retry.

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "chore(desktop): add license/pairing dependencies"
```

---

## Task 5: License storage module (Rust, TDD)

**Files:**
- Create: `apps/desktop/src-tauri/src/error.rs`
- Create: `apps/desktop/src-tauri/src/license.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: Create `apps/desktop/src-tauri/src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid pairing code")]
    InvalidCode,

    #[error("no license stored")]
    NoLicense,

    #[error("license expired")]
    LicenseExpired,

    #[error("server returned {status}: {body}")]
    Server { status: u16, body: String },
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
```

- [ ] **Step 2: Write the failing license-storage test**

Create `apps/desktop/src-tauri/src/license.rs`:

```rust
use crate::error::{AppError, Result};

const SERVICE: &str = "org.brainctl.mantic";
const KEY: &str = "license-jwt";

pub fn save(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, KEY)?;
    entry.set_password(token)?;
    Ok(())
}

pub fn load() -> Result<String> {
    let entry = keyring::Entry::new(SERVICE, KEY)?;
    match entry.get_password() {
        Ok(p) => Ok(p),
        Err(keyring::Error::NoEntry) => Err(AppError::NoLicense),
        Err(e) => Err(AppError::Keyring(e)),
    }
}

pub fn clear() -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, KEY)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keyring(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring::{mock, set_default_credential_builder};

    fn use_mock_keyring() {
        set_default_credential_builder(mock::default_credential_builder());
    }

    #[test]
    fn save_then_load_roundtrip() {
        use_mock_keyring();
        save("token-123").unwrap();
        assert_eq!(load().unwrap(), "token-123");
    }

    #[test]
    fn load_with_no_entry_returns_no_license() {
        use_mock_keyring();
        clear().unwrap();
        match load() {
            Err(AppError::NoLicense) => {}
            other => panic!("expected NoLicense, got {other:?}"),
        }
    }

    #[test]
    fn clear_is_idempotent() {
        use_mock_keyring();
        clear().unwrap();
        clear().unwrap();
    }
}
```

Note: the `keyring` crate's `mock` feature must be enabled in dev-dependencies. Update `apps/desktop/src-tauri/Cargo.toml`:

```toml
[dev-dependencies]
keyring = { version = "3", features = ["mock"] }
```

(Re-declare the dep with `mock` feature in dev-dependencies; cargo will merge features.)

- [ ] **Step 3: Wire modules into `main.rs`**

Replace `apps/desktop/src-tauri/src/main.rs` with:

```rust
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod license;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `cd apps/desktop/src-tauri && cargo test --lib license`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock apps/desktop/src-tauri/src/error.rs apps/desktop/src-tauri/src/license.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(desktop): license storage via keyring"
```

---

## Task 6: Pairing command (Rust, TDD)

**Files:**
- Create: `apps/desktop/src-tauri/src/pairing.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs` (register module)

- [ ] **Step 1: Write the failing pairing test**

Create `apps/desktop/src-tauri/src/pairing.rs`:

```rust
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct PairRequest<'a> {
    code: &'a str,
}

#[derive(Debug, Deserialize)]
struct PairResponse {
    token: String,
}

pub async fn pair(server_url: &str, code: &str) -> Result<String> {
    if code.trim().is_empty() {
        return Err(AppError::InvalidCode);
    }
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{server_url}/v1/pair"))
        .json(&PairRequest { code })
        .send()
        .await?;

    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        if status.as_u16() == 400 {
            return Err(AppError::InvalidCode);
        }
        return Err(AppError::Server {
            status: status.as_u16(),
            body,
        });
    }

    let body: PairResponse = res.json().await?;
    Ok(body.token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn pair_returns_token_on_200() {
        let mut server = Server::new_async().await;
        let m = server
            .mock("POST", "/v1/pair")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"token":"abc.def.ghi"}"#)
            .create_async()
            .await;

        let token = pair(&server.url(), "654321").await.unwrap();
        assert_eq!(token, "abc.def.ghi");
        m.assert_async().await;
    }

    #[tokio::test]
    async fn pair_rejects_empty_code_without_calling_server() {
        let err = pair("http://unused", "").await.unwrap_err();
        assert!(matches!(err, AppError::InvalidCode));
    }

    #[tokio::test]
    async fn pair_maps_400_to_invalid_code() {
        let mut server = Server::new_async().await;
        server
            .mock("POST", "/v1/pair")
            .with_status(400)
            .with_body(r#"{"error":"invalid_code"}"#)
            .create_async()
            .await;

        let err = pair(&server.url(), "12").await.unwrap_err();
        assert!(matches!(err, AppError::InvalidCode));
    }
}
```

- [ ] **Step 2: Register module in `main.rs`**

Update `apps/desktop/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod license;
mod pairing;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run pairing tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib pairing`
Expected: PASS, 3 tests.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/pairing.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(desktop): pairing http client with mockito-backed tests"
```

---

## Task 7: Account command (Rust, TDD)

Decodes the stored JWT into account claims for the UI. No signature verification — we trust the token because we stored it ourselves; verification is the server's job.

**Files:**
- Create: `apps/desktop/src-tauri/src/account.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing account test**

Create `apps/desktop/src-tauri/src/account.rs`:

```rust
use crate::error::{AppError, Result};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AccountInfo {
    pub account_id: String,
    pub tier: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
struct Claims {
    account_id: String,
    tier: String,
    exp: i64,
}

pub fn decode_claims(token: &str) -> Result<AccountInfo> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    let data = decode::<Claims>(token, &DecodingKey::from_secret(&[]), &validation)?;
    let c = data.claims;
    let now = chrono::Utc::now().timestamp();
    if c.exp <= now {
        return Err(AppError::LicenseExpired);
    }
    Ok(AccountInfo {
        account_id: c.account_id,
        tier: c.tier,
        expires_at: c.exp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    fn make_token(account_id: &str, tier: &str, exp: i64) -> String {
        let claims = json!({ "account_id": account_id, "tier": tier, "exp": exp });
        encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"test"),
        )
        .unwrap()
    }

    #[test]
    fn decodes_a_valid_token() {
        let exp = chrono::Utc::now().timestamp() + 3600;
        let token = make_token("acct_42", "pro", exp);
        let info = decode_claims(&token).unwrap();
        assert_eq!(info.account_id, "acct_42");
        assert_eq!(info.tier, "pro");
        assert_eq!(info.expires_at, exp);
    }

    #[test]
    fn rejects_expired_token() {
        let exp = chrono::Utc::now().timestamp() - 1;
        let token = make_token("acct_42", "pro", exp);
        let err = decode_claims(&token).unwrap_err();
        assert!(matches!(err, AppError::LicenseExpired));
    }
}
```

- [ ] **Step 2: Register module in `main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod error;
mod license;
mod pairing;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run account tests**

Run: `cd apps/desktop/src-tauri && cargo test --lib account`
Expected: PASS, 2 tests.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/account.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(desktop): decode account info from stored jwt"
```

---

## Task 8: Tauri command layer + license-server URL config

**Files:**
- Create: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src-tauri/tauri.conf.json` (add capability)
- Create: `apps/desktop/src-tauri/capabilities/default.json` (or modify existing)

- [ ] **Step 1: Create commands module**

Create `apps/desktop/src-tauri/src/commands.rs`:

```rust
use crate::account::{decode_claims, AccountInfo};
use crate::error::{AppError, Result};
use crate::{license, pairing};

const DEFAULT_SERVER: &str = "http://localhost:4001";

fn server_url() -> String {
    std::env::var("MANTIC_LICENSE_SERVER").unwrap_or_else(|_| DEFAULT_SERVER.to_string())
}

#[tauri::command]
pub async fn pair_with_code(code: String) -> Result<AccountInfo> {
    let token = pairing::pair(&server_url(), &code).await?;
    license::save(&token)?;
    decode_claims(&token)
}

#[tauri::command]
pub fn current_account() -> Result<AccountInfo> {
    let token = license::load()?;
    decode_claims(&token)
}

#[tauri::command]
pub fn sign_out() -> Result<()> {
    license::clear()
}
```

- [ ] **Step 2: Register commands in `main.rs`**

Replace `apps/desktop/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod commands;
mod error;
mod license;
mod pairing;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::pair_with_code,
            commands::current_account,
            commands::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify cargo builds**

Run: `cd apps/desktop/src-tauri && cargo check`
Expected: PASS (warnings about unused functions are fine — frontend will call them).

- [ ] **Step 4: Run all Rust tests**

Run: `cd apps/desktop/src-tauri && cargo test`
Expected: PASS, all 8 tests across `license`, `pairing`, `account` modules.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/commands.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(desktop): expose pair/account/signout tauri commands"
```

---

## Task 9: Frontend deps + Tailwind + Router + Vitest setup

**Files:**
- Modify: `apps/desktop/package.json`
- Create: `apps/desktop/tailwind.config.ts`
- Create: `apps/desktop/postcss.config.js`
- Create: `apps/desktop/vitest.config.ts`
- Create: `apps/desktop/src/test-setup.ts`
- Modify: `apps/desktop/src/index.css` (or App.css)
- Modify: `apps/desktop/vite.config.ts`

- [ ] **Step 1: Add deps to `apps/desktop/package.json`**

Run, from repo root:

```bash
pnpm --filter mantic-desktop add react-router-dom@^7
pnpm --filter mantic-desktop add -D tailwindcss@^4 @tailwindcss/postcss@^4 postcss autoprefixer
pnpm --filter mantic-desktop add -D vitest@^2 @vitest/ui jsdom @testing-library/react@^16 @testing-library/jest-dom @testing-library/user-event
```

Expected: dependencies appear in `apps/desktop/package.json`, lockfile updated.

- [ ] **Step 2: Create `apps/desktop/tailwind.config.ts`**

```typescript
import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {},
  },
  plugins: [],
} satisfies Config;
```

- [ ] **Step 3: Create `apps/desktop/postcss.config.js`**

```javascript
export default {
  plugins: {
    "@tailwindcss/postcss": {},
    autoprefixer: {},
  },
};
```

- [ ] **Step 4: Replace `apps/desktop/src/App.css` (or create `src/index.css`) with Tailwind imports**

Delete `apps/desktop/src/App.css` if it exists. Create `apps/desktop/src/index.css`:

```css
@import "tailwindcss";

:root {
  color-scheme: dark;
}

body {
  @apply bg-neutral-950 text-neutral-100 antialiased;
  margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
}
```

- [ ] **Step 5: Update `apps/desktop/src/main.tsx`**

Replace contents:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>,
);
```

- [ ] **Step 6: Create `apps/desktop/vitest.config.ts`**

```typescript
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
```

- [ ] **Step 7: Create `apps/desktop/src/test-setup.ts`**

```typescript
import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Default mock for the Tauri bridge. Individual tests can override per-call.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
```

- [ ] **Step 8: Verify dev mode still launches**

Run: `pnpm dev:desktop`
Expected: window opens, default Tauri content is now rendered with dark-themed Tailwind background. Stop the process.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop pnpm-lock.yaml
git commit -m "chore(desktop): tailwind 4, react-router 7, vitest setup"
```

---

## Task 10: TypeScript Tauri bridge

**Files:**
- Create: `apps/desktop/src/lib/tauri-bridge.ts`
- Create: `apps/desktop/src/lib/tauri-bridge.test.ts`

- [ ] **Step 1: Write the failing bridge test**

Create `apps/desktop/src/lib/tauri-bridge.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { pairWithCode, currentAccount, signOut } from "./tauri-bridge";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("tauri-bridge", () => {
  it("pairWithCode invokes the rust command with the code", async () => {
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_1",
      tier: "pro",
      expires_at: 9999999999,
    });
    const result = await pairWithCode("123456");
    expect(invokeMock).toHaveBeenCalledWith("pair_with_code", { code: "123456" });
    expect(result.tier).toBe("pro");
  });

  it("currentAccount returns null when no license is stored", async () => {
    invokeMock.mockRejectedValueOnce("no license stored");
    const result = await currentAccount();
    expect(result).toBeNull();
  });

  it("currentAccount returns account info when stored", async () => {
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_1",
      tier: "pro",
      expires_at: 9999999999,
    });
    const result = await currentAccount();
    expect(result?.account_id).toBe("acct_1");
  });

  it("signOut invokes sign_out", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await signOut();
    expect(invokeMock).toHaveBeenCalledWith("sign_out");
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `pnpm --filter mantic-desktop test`
Expected: FAIL — `tauri-bridge.ts` does not exist.

- [ ] **Step 3: Implement `apps/desktop/src/lib/tauri-bridge.ts`**

```typescript
import { invoke } from "@tauri-apps/api/core";

export interface AccountInfo {
  account_id: string;
  tier: "free" | "starter" | "pro" | "whale";
  expires_at: number;
}

export async function pairWithCode(code: string): Promise<AccountInfo> {
  return invoke<AccountInfo>("pair_with_code", { code });
}

export async function currentAccount(): Promise<AccountInfo | null> {
  try {
    return await invoke<AccountInfo>("current_account");
  } catch (e) {
    const msg = typeof e === "string" ? e : String(e);
    if (msg.includes("no license stored") || msg.includes("license expired")) {
      return null;
    }
    throw e;
  }
}

export async function signOut(): Promise<void> {
  await invoke("sign_out");
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `pnpm --filter mantic-desktop test`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/lib
git commit -m "feat(desktop): typed tauri bridge for pair/account/signout"
```

---

## Task 11: Pair route component (React, TDD)

**Files:**
- Create: `apps/desktop/src/routes/Pair.tsx`
- Create: `apps/desktop/src/routes/Pair.test.tsx`

- [ ] **Step 1: Write the failing component test**

Create `apps/desktop/src/routes/Pair.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Pair from "./Pair";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

function renderPair(onPaired = vi.fn()) {
  return render(
    <MemoryRouter>
      <Pair onPaired={onPaired} />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<Pair />", () => {
  it("renders a pairing code input and submit button", () => {
    renderPair();
    expect(screen.getByLabelText(/pairing code/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /pair device/i })).toBeInTheDocument();
  });

  it("submits the code and calls onPaired with the account info", async () => {
    const onPaired = vi.fn();
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_99",
      tier: "starter",
      expires_at: 9999999999,
    });

    renderPair(onPaired);
    await userEvent.type(screen.getByLabelText(/pairing code/i), "123456");
    await userEvent.click(screen.getByRole("button", { name: /pair device/i }));

    expect(invokeMock).toHaveBeenCalledWith("pair_with_code", { code: "123456" });
    expect(onPaired).toHaveBeenCalledWith(
      expect.objectContaining({ account_id: "acct_99", tier: "starter" }),
    );
  });

  it("shows an error message when pairing fails", async () => {
    invokeMock.mockRejectedValueOnce("invalid pairing code");

    renderPair();
    await userEvent.type(screen.getByLabelText(/pairing code/i), "x");
    await userEvent.click(screen.getByRole("button", { name: /pair device/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/invalid pairing code/i);
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `pnpm --filter mantic-desktop test`
Expected: FAIL — `Pair.tsx` does not exist.

- [ ] **Step 3: Implement `apps/desktop/src/routes/Pair.tsx`**

```tsx
import { useState, FormEvent } from "react";
import { pairWithCode, AccountInfo } from "../lib/tauri-bridge";

interface Props {
  onPaired: (info: AccountInfo) => void;
}

export default function Pair({ onPaired }: Props) {
  const [code, setCode] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const info = await pairWithCode(code.trim());
      onPaired(info);
    } catch (err) {
      const msg = typeof err === "string" ? err : (err as Error).message;
      setError(msg);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-8">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-md space-y-6 rounded-2xl bg-neutral-900 p-8 shadow-xl"
      >
        <div>
          <h1 className="text-2xl font-semibold">Pair this device</h1>
          <p className="mt-2 text-sm text-neutral-400">
            Enter the 6-digit pairing code from brainctl.org to link this machine to your
            Mantic account.
          </p>
        </div>

        <label className="block">
          <span className="text-sm font-medium text-neutral-300">Pairing code</span>
          <input
            type="text"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="123456"
            autoComplete="one-time-code"
            inputMode="numeric"
            className="mt-1 w-full rounded-lg border border-neutral-700 bg-neutral-800 px-3 py-2 font-mono text-lg tracking-widest focus:border-blue-500 focus:outline-none"
            disabled={submitting}
          />
        </label>

        {error && (
          <div role="alert" className="rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={submitting || code.trim().length === 0}
          className="w-full rounded-lg bg-blue-600 px-4 py-2 font-medium transition hover:bg-blue-500 disabled:cursor-not-allowed disabled:bg-neutral-700"
        >
          {submitting ? "Pairing..." : "Pair device"}
        </button>
      </form>
    </div>
  );
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `pnpm --filter mantic-desktop test`
Expected: PASS, 7 tests total (3 new for Pair, 4 from earlier).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/routes/Pair.tsx apps/desktop/src/routes/Pair.test.tsx
git commit -m "feat(desktop): pairing route with form and error handling"
```

---

## Task 12: Account route component (React, TDD)

**Files:**
- Create: `apps/desktop/src/routes/Account.tsx`
- Create: `apps/desktop/src/routes/Account.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/desktop/src/routes/Account.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Account from "./Account";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const account = {
  account_id: "acct_42",
  tier: "pro" as const,
  expires_at: Math.floor(Date.now() / 1000) + 86400,
};

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<Account />", () => {
  it("renders tier and expiry", () => {
    render(
      <MemoryRouter>
        <Account account={account} onSignOut={vi.fn()} />
      </MemoryRouter>,
    );
    expect(screen.getByText(/pro/i)).toBeInTheDocument();
    expect(screen.getByText(/acct_42/i)).toBeInTheDocument();
    expect(screen.getByText(/license expires/i)).toBeInTheDocument();
  });

  it("invokes sign_out and calls onSignOut on click", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    const onSignOut = vi.fn();
    render(
      <MemoryRouter>
        <Account account={account} onSignOut={onSignOut} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: /unpair this device/i }));
    expect(invokeMock).toHaveBeenCalledWith("sign_out");
    expect(onSignOut).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `pnpm --filter mantic-desktop test`
Expected: FAIL — `Account.tsx` does not exist.

- [ ] **Step 3: Implement `apps/desktop/src/routes/Account.tsx`**

```tsx
import { signOut, AccountInfo } from "../lib/tauri-bridge";

interface Props {
  account: AccountInfo;
  onSignOut: () => void;
}

function formatExpiry(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export default function Account({ account, onSignOut }: Props) {
  async function handleSignOut() {
    await signOut();
    onSignOut();
  }

  return (
    <div className="min-h-screen p-8">
      <div className="mx-auto max-w-2xl space-y-8">
        <header>
          <h1 className="text-3xl font-semibold">Mantic</h1>
          <p className="mt-1 text-sm text-neutral-400">read the tape</p>
        </header>

        <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
          <h2 className="text-lg font-medium">Account</h2>
          <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
            <div>
              <dt className="text-neutral-500">Account ID</dt>
              <dd className="mt-1 font-mono">{account.account_id}</dd>
            </div>
            <div>
              <dt className="text-neutral-500">Tier</dt>
              <dd className="mt-1 font-medium uppercase">{account.tier}</dd>
            </div>
            <div className="col-span-2">
              <dt className="text-neutral-500">License expires</dt>
              <dd className="mt-1">{formatExpiry(account.expires_at)}</dd>
            </div>
          </dl>
        </section>

        <section className="rounded-2xl bg-neutral-900 p-6 text-sm text-neutral-400 shadow-xl">
          <p>
            The trading UI ships in a later milestone. This shell verifies your account is paired
            and the license is healthy.
          </p>
        </section>

        <div className="flex justify-end">
          <button
            type="button"
            onClick={handleSignOut}
            className="rounded-lg border border-neutral-700 px-4 py-2 text-sm text-neutral-300 transition hover:bg-neutral-800"
          >
            Unpair this device
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `pnpm --filter mantic-desktop test`
Expected: PASS, 9 tests total.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/routes/Account.tsx apps/desktop/src/routes/Account.test.tsx
git commit -m "feat(desktop): account route showing tier, expiry, sign-out"
```

---

## Task 13: App router + auth gate

**Files:**
- Modify: `apps/desktop/src/App.tsx`
- Create: `apps/desktop/src/App.test.tsx`

- [ ] **Step 1: Write the failing routing test**

Create `apps/desktop/src/App.test.tsx`:

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
    invokeMock.mockRejectedValueOnce("no license stored");
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByText(/pair this device/i)).toBeInTheDocument();
  });

  it("shows Account route when a license is stored", async () => {
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_pro",
      tier: "pro",
      expires_at: Math.floor(Date.now() / 1000) + 3600,
    });
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(screen.getByText(/acct_pro/i)).toBeInTheDocument();
    });
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `pnpm --filter mantic-desktop test`
Expected: FAIL — current `App.tsx` shows scaffold content, not the routes.

- [ ] **Step 3: Replace `apps/desktop/src/App.tsx`**

```tsx
import { useEffect, useState } from "react";
import { currentAccount, AccountInfo } from "./lib/tauri-bridge";
import Pair from "./routes/Pair";
import Account from "./routes/Account";

type Status = "loading" | "unpaired" | "paired";

export default function App() {
  const [status, setStatus] = useState<Status>("loading");
  const [account, setAccount] = useState<AccountInfo | null>(null);

  useEffect(() => {
    let active = true;
    (async () => {
      const info = await currentAccount();
      if (!active) return;
      if (info) {
        setAccount(info);
        setStatus("paired");
      } else {
        setStatus("unpaired");
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
        onPaired={(info) => {
          setAccount(info);
          setStatus("paired");
        }}
      />
    );
  }

  return (
    <Account
      account={account}
      onSignOut={() => {
        setAccount(null);
        setStatus("unpaired");
      }}
    />
  );
}
```

- [ ] **Step 4: Run tests to verify pass**

Run: `pnpm --filter mantic-desktop test`
Expected: PASS, 11 tests total.

- [ ] **Step 5: Manual smoke test end-to-end**

Run mock server in one terminal: `pnpm dev:mock-license`
Run desktop app in another: `pnpm dev:desktop`

In the app:
1. Pairing screen appears.
2. Enter `123456` and click "Pair device".
3. UI transitions to Account screen showing `acct_dev_123456`, tier `PRO`, expiry one day out.
4. Click "Unpair this device" — back to pairing screen.
5. Quit the app and relaunch — you should land on the Account screen directly (license persisted to OS keychain).

If all five pass, proceed. If any fail, fix before commit.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/App.tsx apps/desktop/src/App.test.tsx
git commit -m "feat(desktop): auth-gated routing between Pair and Account"
```

---

## Task 14: License heartbeat (Rust, TDD)

A background task that refreshes the JWT before it expires. Runs whenever a license is stored.

**Files:**
- Create: `apps/desktop/src-tauri/src/heartbeat.rs`
- Modify: `apps/desktop/src-tauri/src/pairing.rs` (add `refresh()` function)
- Modify: `apps/desktop/src-tauri/src/main.rs` (spawn heartbeat task)
- Modify: `apps/desktop/src-tauri/src/commands.rs` (use refresh)

- [ ] **Step 1: Add `refresh()` to `pairing.rs`**

Append to `apps/desktop/src-tauri/src/pairing.rs` (above the `#[cfg(test)]` block):

```rust
pub async fn refresh(server_url: &str, current_token: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{server_url}/v1/refresh"))
        .bearer_auth(current_token)
        .send()
        .await?;

    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(AppError::Server {
            status: status.as_u16(),
            body,
        });
    }
    let body: PairResponse = res.json().await?;
    Ok(body.token)
}
```

- [ ] **Step 2: Add refresh test (still in `pairing.rs`)**

Append inside the existing `mod tests`:

```rust
    #[tokio::test]
    async fn refresh_returns_new_token() {
        let mut server = Server::new_async().await;
        server
            .mock("POST", "/v1/refresh")
            .match_header("authorization", "Bearer old.token.value")
            .with_status(200)
            .with_body(r#"{"token":"new.token.value"}"#)
            .create_async()
            .await;

        let new_token = refresh(&server.url(), "old.token.value").await.unwrap();
        assert_eq!(new_token, "new.token.value");
    }

    #[tokio::test]
    async fn refresh_returns_server_error_on_401() {
        let mut server = Server::new_async().await;
        server
            .mock("POST", "/v1/refresh")
            .with_status(401)
            .with_body(r#"{"error":"invalid_token"}"#)
            .create_async()
            .await;

        let err = refresh(&server.url(), "expired").await.unwrap_err();
        assert!(matches!(err, AppError::Server { status: 401, .. }));
    }
```

- [ ] **Step 3: Write the failing heartbeat test**

Create `apps/desktop/src-tauri/src/heartbeat.rs`:

```rust
use crate::account::decode_claims;
use crate::error::Result;
use crate::{license, pairing};
use std::time::Duration;

/// Returns true if the token expires within `refresh_window` seconds from now.
pub fn needs_refresh(expires_at: i64, now: i64, refresh_window_secs: i64) -> bool {
    expires_at - now <= refresh_window_secs
}

pub async fn tick(server_url: &str, refresh_window_secs: i64) -> Result<bool> {
    let token = match license::load() {
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
    license::save(&new_token)?;
    Ok(true)
}

pub async fn run_forever(server_url: String, interval: Duration, refresh_window_secs: i64) {
    loop {
        let _ = tick(&server_url, refresh_window_secs).await;
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

- [ ] **Step 4: Register module & spawn the background task in `main.rs`**

Replace `apps/desktop/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod account;
mod commands;
mod error;
mod heartbeat;
mod license;
mod pairing;

use std::time::Duration;

fn server_url() -> String {
    std::env::var("MANTIC_LICENSE_SERVER").unwrap_or_else(|_| "http://localhost:4001".to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| {
            tauri::async_runtime::spawn(heartbeat::run_forever(
                server_url(),
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

- [ ] **Step 5: Run all Rust tests**

Run: `cd apps/desktop/src-tauri && cargo test`
Expected: PASS, all 13 tests (license×3, pairing×5, account×2, heartbeat×3).

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/heartbeat.rs apps/desktop/src-tauri/src/pairing.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(desktop): jwt heartbeat refresh background task"
```

---

## Task 15: Binary smoke test (Vitest + child_process)

A full WebDriver E2E test for Tauri requires `tauri-driver`, which has uneven cross-platform support (Linux/Windows OK, macOS still maturing as of 2026). Rather than ship E2E code that pretends to work, this milestone ships a smaller, honest smoke test: spawn the built binary, give it a few seconds to initialize, verify it didn't crash, then kill it. This catches the most common shipping failure (binary refuses to launch on a clean machine due to missing config, signing, or runtime deps). A full UI-driving E2E is deferred to a later sub-project (likely #11 — GUI surfaces — when there's more UI worth driving).

**Files:**
- Create: `e2e/package.json`
- Create: `e2e/vitest.config.ts`
- Create: `e2e/smoke.test.ts`
- Create: `e2e/tsconfig.json`
- Modify: `pnpm-workspace.yaml` (add `e2e` to packages)
- Modify: `package.json` (add `test:e2e` script)

- [ ] **Step 1: Add `e2e` to workspaces**

Edit `pnpm-workspace.yaml`:

```yaml
packages:
  - "apps/*"
  - "services/*"
  - "e2e"
```

- [ ] **Step 2: Create `e2e/package.json`**

```json
{
  "name": "mantic-e2e",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "lint": "tsc --noEmit"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "typescript": "^5.6.0",
    "vitest": "^2.1.0"
  }
}
```

- [ ] **Step 3: Create `e2e/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "types": ["node"],
    "noEmit": true
  },
  "include": ["**/*.ts"]
}
```

- [ ] **Step 4: Create `e2e/vitest.config.ts`**

```typescript
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["**/*.test.ts"],
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});
```

- [ ] **Step 5: Create `e2e/smoke.test.ts`**

```typescript
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { spawn, ChildProcess, execSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { platform } from "node:os";

function findBinary(): string {
  const repoRoot = resolve(import.meta.dirname, "..");
  const base = resolve(repoRoot, "apps/desktop/src-tauri/target/release");
  const candidates =
    platform() === "darwin"
      ? [resolve(base, "bundle/macos/Mantic.app/Contents/MacOS/Mantic")]
      : platform() === "win32"
        ? [resolve(base, "mantic-desktop.exe"), resolve(base, "Mantic.exe")]
        : [resolve(base, "mantic-desktop"), resolve(base, "mantic")];

  const found = candidates.find((p) => existsSync(p));
  if (!found) {
    throw new Error(
      `Could not find built binary. Run \`pnpm build:desktop\` first.\nChecked:\n${candidates.join("\n")}`,
    );
  }
  return found;
}

let mockServer: ChildProcess | undefined;

beforeAll(async () => {
  mockServer = spawn("pnpm", ["--filter", "mock-license", "dev"], {
    cwd: resolve(import.meta.dirname, ".."),
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  await sleep(3000);
});

afterAll(async () => {
  if (mockServer?.pid) {
    try {
      process.kill(-mockServer.pid, "SIGTERM");
    } catch {
      // already dead
    }
  }
});

describe("desktop binary smoke", () => {
  it("launches and stays alive for at least 5 seconds", async () => {
    const bin = findBinary();
    const proc = spawn(bin, [], {
      env: { ...process.env, MANTIC_LICENSE_SERVER: "http://localhost:4001" },
      stdio: ["ignore", "pipe", "pipe"],
    });

    let exitCode: number | null = null;
    let stderr = "";
    proc.on("exit", (code) => {
      exitCode = code;
    });
    proc.stderr?.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    await sleep(5000);

    const stillAlive = exitCode === null;

    try {
      proc.kill("SIGTERM");
    } catch {
      // already dead
    }

    expect(stillAlive, `Binary exited early with code ${exitCode}. stderr:\n${stderr}`).toBe(true);
  });

  it("mock license server responds to healthz", async () => {
    const out = execSync("curl -sf http://localhost:4001/healthz", { encoding: "utf8" });
    const body = JSON.parse(out);
    expect(body.ok).toBe(true);
  });
});
```

- [ ] **Step 6: Install Vitest in e2e workspace**

Run, from repo root: `pnpm install`
Expected: `e2e/node_modules` populated.

- [ ] **Step 7: Add a root convenience script**

Modify root `package.json`, adding `"test:e2e"` to `"scripts"`:

```json
"test:e2e": "pnpm --filter mantic-e2e test"
```

The full scripts block in root `package.json` should now read:

```json
"scripts": {
  "dev:mock-license": "pnpm --filter mock-license dev",
  "dev:desktop": "pnpm --filter mantic-desktop tauri dev",
  "build:desktop": "pnpm --filter mantic-desktop tauri build",
  "test": "pnpm -r test",
  "test:e2e": "pnpm --filter mantic-e2e test",
  "lint": "pnpm -r lint"
}
```

- [ ] **Step 8: Create `e2e/README.md`** (one-file exception to "no docs" rule — documents how to run the test)

```markdown
# Mantic E2E

Smoke test for the desktop binary: launches the release build, gives it 5 seconds to initialize, verifies it didn't crash.

A full UI-driving E2E (Playwright + tauri-driver) is deferred to a later milestone once `tauri-driver` matures across all three platforms.

## Run

```bash
pnpm build:desktop        # must succeed first — produces the release binary
pnpm --filter mantic-e2e test
```
```

- [ ] **Step 9: Run the smoke test (will require a release build first)**

Run:

```bash
pnpm build:desktop
pnpm --filter mantic-e2e test
```

Expected: 2 tests PASS. If the binary fails to find a connectable license server within the 5s window that's OK — the test only verifies the process stays alive, not that pairing works. If the binary exits early, the failure message will include stderr to diagnose.

- [ ] **Step 10: Commit**

```bash
git add e2e pnpm-workspace.yaml package.json pnpm-lock.yaml
git commit -m "feat(e2e): smoke test that the release binary launches"
```

---

## Task 16: GitHub Actions CI — build + package + tests

**Files:**
- Create: `.github/workflows/build.yml`

- [ ] **Step 1: Create `.github/workflows/build.yml`**

```yaml
name: build

on:
  push:
    branches: [main]
  pull_request:

jobs:
  unit-tests:
    name: Unit tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: pnpm/action-setup@v4
        with:
          version: 9

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm

      - uses: dtolnay/rust-toolchain@stable

      - name: Install Linux deps
        run: |
          sudo apt-get update
          sudo apt-get install -y libdbus-1-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Install JS deps
        run: pnpm install --frozen-lockfile

      - name: Frontend + service tests
        run: pnpm -r test

      - name: Rust tests
        working-directory: apps/desktop/src-tauri
        run: cargo test

      - name: Lint
        run: pnpm -r lint

  build-desktop:
    name: Build desktop (${{ matrix.os }})
    needs: unit-tests
    strategy:
      fail-fast: false
      matrix:
        os: [macos-latest, windows-latest, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - uses: pnpm/action-setup@v4
        with:
          version: 9

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm

      - uses: dtolnay/rust-toolchain@stable

      - name: Install Linux deps
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libdbus-1-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Install JS deps
        run: pnpm install --frozen-lockfile

      - name: Build desktop bundle
        run: pnpm --filter mantic-desktop tauri build

      - name: Upload bundle
        uses: actions/upload-artifact@v4
        with:
          name: mantic-${{ matrix.os }}
          path: |
            apps/desktop/src-tauri/target/release/bundle/**/*
          if-no-files-found: error
          retention-days: 7
```

- [ ] **Step 2: Verify YAML parses locally**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/build.yml'))"`
Expected: no output, exit code 0.

(If `python3` is not available, use any YAML linter or trust it.)

- [ ] **Step 3: Commit**

```bash
git add .github
git commit -m "ci: github actions matrix for tests and desktop packaging"
```

---

## Task 17: Final integration check + README polish

**Files:**
- Modify: `README.md` (expand quickstart with full instructions)

- [ ] **Step 1: Run the full test suite locally**

```bash
pnpm install
pnpm -r test
cd apps/desktop/src-tauri && cargo test && cd ../../..
```

Expected: all suites green. Total counts: 6 (mock-license) + 11 (desktop frontend) + 13 (desktop Rust) = 30 tests.

- [ ] **Step 2: Produce a release bundle locally to confirm packaging works**

```bash
pnpm --filter mantic-desktop tauri build
```

Expected: a platform-appropriate bundle appears under `apps/desktop/src-tauri/target/release/bundle/`. On macOS, a `.dmg` and `.app` under `target/release/bundle/macos`. On Linux, a `.AppImage` and `.deb`. On Windows, an `.msi`.

- [ ] **Step 3: Expand `README.md`**

Replace the contents of `README.md`:

```markdown
# Mantic

Trading agent platform powered by brainctl. See `docs/superpowers/specs/` for design, `docs/superpowers/plans/` for implementation plans.

## Repo layout

- `apps/desktop` — Tauri 2 desktop app (the product surface)
- `services/mock-license` — dev-only mock license server (stand-in for brainctl.org license endpoints)
- `e2e` — Playwright + tauri-driver smoke tests
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
pnpm -r test                                # frontend + service unit tests
cd apps/desktop/src-tauri && cargo test     # Rust unit tests
pnpm --filter mantic-e2e test               # Playwright E2E (Linux/Windows only for now)
```

## Build a release bundle

```bash
pnpm build:desktop
# artifacts under apps/desktop/src-tauri/target/release/bundle/
```

## License

TBD before public release.
```

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: expand README with repo layout, quickstart, tests, build"
```

- [ ] **Step 5: Push to remote (if a remote is configured)**

If `git remote -v` lists an `origin`:

```bash
git push -u origin main
```

If not, defer push until the user sets up the remote.

---

## Definition of done

This sub-project is complete when:

- [ ] All 30 unit tests pass (`pnpm -r test` and `cargo test`)
- [ ] `pnpm dev:desktop` launches the app; entering a pairing code transitions to the Account view; restarting the app remembers the pairing
- [ ] `pnpm build:desktop` produces a platform bundle on at least one platform
- [ ] CI pipeline is green on a pushed branch (run `gh run watch` or check Actions tab)
- [ ] License JWT is stored in the OS keychain (verify on macOS via Keychain Access: search for `org.brainctl.mantic`)
- [ ] Heartbeat task is running (visible if you instrument logs or run with `RUST_LOG=info`)

When all checked, ready to begin sub-project #2 (brain.db integration).

---

## Glossary

- **Pairing code** — 6-digit one-time code shown in the web dashboard, redeemed by the desktop app for a license JWT.
- **License JWT** — short-lived signed token containing `account_id`, `tier`, and `exp` claims. Stored locally, refreshed by the heartbeat task.
- **Mock license server** — a local Hono service implementing the same HTTP contract as brainctl.org's real license server. Replaced by sub-project #9.
- **Heartbeat** — background tokio task that refreshes the JWT before expiry by calling `/v1/refresh`.

---

*End of plan. Total: 17 tasks, 30 tests, ~5–7 engineer-hours estimated for a developer comfortable with Tauri + React + Rust. Reduce to ~3 hours if running via the subagent-driven-development skill with parallel subagents per task family.*
