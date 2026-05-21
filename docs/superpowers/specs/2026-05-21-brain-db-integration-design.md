# Brain.db Integration — Sub-project #2 Design Spec

**Date:** 2026-05-21
**Status:** Approved by founder. Ready for implementation plan.
**Builds on:** [desktop-app-shell](2026-05-21-mantic-design.md) (sub-project #1, complete at `718aa07`)

---

## Goal

Mantic's desktop app embeds full brain.db functionality: reads via direct `rusqlite`, writes and complex queries via a bundled `brainctl-mcp` subprocess speaking MCP-over-stdio. Includes the foundational `State<T>` refactor flagged by the sub-project #1 final reviewer.

Two repos touched: **brainctl** (adds PyInstaller build target + CI) and **Mantic** (refactor + integration).

## Read/write split

- **Read-only table queries** (counts, listings, recent_events) → direct `rusqlite`, read-only connection. Fast, no subprocess hop.
- **Writes + complex reads with brainctl logic** (any write, plus `memory_search` with FTS boosting, `agent_orient`) → subprocess. Brainctl owns correctness; we just route.

## Mantic-side architecture

```
                       Tauri commands.rs
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
  State<KeyringStore>   State<BrainDb>    State<BrainctlClient>
  (refactor)            ┌──────────┐      ┌────────────────────┐
                        │ rusqlite │      │ MCP JSON-RPC stdio │
                        │ R/O pool │      │ child: brainctl-mcp│
                        └──────────┘      └────────────────────┘
```

### New Rust modules in `apps/desktop/src-tauri/src/`

- `state.rs` — `AppState { keyring, brain, brainctl }` constructed in `setup()`, registered via `app.manage()`.
- `brain_db.rs` — `BrainDb` over `rusqlite::Connection` in read-only mode. Methods: `count_memories()`, `count_events()`, `recent_events(limit)`, `recent_memories(limit)`.
- `brainctl_client.rs` — `BrainctlClient` owning subprocess + correlation table. Public typed methods: `memory_add(...)`, `event_add(...)`, `decision_add(...)`, `entity_create(...)`, `entity_observe(...)`, `agent_register(...)`, `agent_wrap_up(...)`, `agent_orient(...)`, `memory_search(...)`.
- `mcp_codec.rs` — newline-delimited JSON-RPC framing, request id correlation.
- `bundle.rs` — locates bundled `brainctl-mcp` via Tauri's `externalBin` resolver.
- `license.rs` refactored — `KeyringStore` struct stored in `State<T>`, replacing the static `OnceLock`. Same `save/load/clear` public API.

### Tauri commands exposed

**Read (rusqlite):**
- `brain_status() -> { memory_count: u64, event_count: u64 }`
- `recent_events(limit: u32) -> Vec<EventSummary>`
- `recent_memories(limit: u32) -> Vec<MemorySummary>`

**Subprocess (writes + complex reads):**
- `memory_add(content, category, scope?, tags?) -> MemoryRecord`
- `event_add(event_type, content, importance?) -> EventRecord`
- `decision_add(title, rationale, project?) -> DecisionRecord`
- `entity_create(name, entity_type, scope?) -> EntityRecord`
- `entity_observe(entity_id, observation) -> ObservationRecord`
- `agent_register(id, name, type?) -> AgentRecord`
- `agent_wrap_up(agent_id, summary, goal?, open_loops?, next_step?, project?) -> HandoffRecord`
- `agent_orient(agent_id, project?, query?) -> OrientSnapshot`
- `memory_search(query, limit?) -> Vec<MemoryHit>`

### Brain.db location

`app.path().app_data_dir() / "mantic" / "brain.db"`. On macOS: `~/Library/Application Support/org.brainctl.mantic/mantic/brain.db`. Created/migrated by `brainctl-mcp` on its first init call.

## brainctl-side architecture

- `brainctl/build/pyinstaller/brainctl_mcp.spec` — PyInstaller spec for `brainctl-mcp` entry point. Includes sqlite + any native extensions (`embed-populate` if needed).
- `scripts/build_mcp_bundle.sh` — builds the binary, outputs to `dist/brainctl-mcp-bundle-<platform>`.
- `.github/workflows/build-mcp-bundle.yml` — runs on tag push, **macOS arm64 only for v1**. Linux/Windows in a follow-up sub-project. Uploads binary as a release artifact.
- No code signing in v1. Run unsigned, accept Gatekeeper friction in dev; ad-hoc sign later.

## Binary distribution

Tauri 2's `externalBin` sidecar mechanism. `apps/desktop/src-tauri/tauri.conf.json` declares:

```json
"bundle": {
  "externalBin": ["binaries/brainctl-mcp"]
}
```

Developer (or CI step) downloads the latest `brainctl-mcp-bundle-<platform>` from the brainctl GitHub release page → drops at `apps/desktop/binaries/brainctl-mcp-<target-triple>` → `tauri build` includes it.

## Data flow — write/complex-read path

```
React invoke("memory_add", { content, category, scope })
  → commands::memory_add(state, ...)
  → state.brainctl.memory_add(...)
        ├── ensure subprocess is running (spawn if not, respawn if dead)
        ├── assign request_id, register oneshot in correlation table
        ├── write JSON-RPC line to stdin
        ├── reader task reads stdout lines, parses, matches by id
        ├── timeout: 5s default per request (configurable per method)
        └── delivers result through oneshot
  → returns MemoryRecord
```

## Data flow — read path

```
React invoke("brain_status")
  → commands::brain_status(state)
  → state.brain.count_memories() + count_events()
  → SELECT COUNT(*) FROM memories; SELECT COUNT(*) FROM events;
  → returns BrainStatus
```

Read connections are opened in **read-only mode** (`SQLite OPEN_READONLY`). Architecturally enforces "all writes go through brainctl."

## MCP framing

- Newline-delimited JSON, one message per line (matches brainctl-mcp's stdio transport).
- Concurrent requests: correlation by `id`. Internal pending map `HashMap<u64, oneshot::Sender<Value>>`.
- Server notifications (no `id`) logged and discarded — Mantic v1 doesn't subscribe.
- Method names match brainctl's MCP tool names exactly.

## New AppError variants

```rust
#[error("sqlite error: {0}")]
Sqlite(#[from] rusqlite::Error),

#[error("brainctl returned error {code}: {message}")]
Brainctl { code: i64, message: String },

#[error("brainctl unavailable: {reason}")]
BrainctlUnavailable { reason: String },
```

## Failure modes

| Failure | Detection | Recovery |
|---|---|---|
| Bundled binary missing | `spawn` returns ENOENT | Fatal on first write. Reads still work. Surface `BrainctlUnavailable`. |
| Subprocess crash | reader sees EOF | Mark dead; next call respawns with backoff 250ms / 1s / 4s. After 3 failures, surface `BrainctlUnavailable`. |
| Subprocess hangs | per-request `tokio::time::timeout` (5s default) | Drop channel, return `Brainctl { code: -32001, message: "timeout" }`. |
| `SQLITE_BUSY` | rusqlite return | busy_timeout=5s on pool init handles this transparently. |
| `SQLITE_CORRUPT` | rusqlite return | Surface error with recovery hint. No auto-recovery. |
| Schema version mismatch | brainctl-mcp init handshake | Fatal at first write; surface as `BrainctlUnavailable`. brainctl-mcp owns migrations. |

## Lifecycle

- **Startup** (`setup()`): build `KeyringStore` (eager load), `BrainDb` (eager — opens read pool), `BrainctlClient` (created but **NOT spawned** — lazy).
- **First write**: spawn subprocess, send init, send actual request.
- **Crash**: respawn with backoff on next call.
- **Quit**: Tauri `RunEvent::Exit` → `BrainctlClient::shutdown()` (SIGTERM, 2s grace, SIGKILL).

## Concurrency model

- `BrainDb`: `Arc<Mutex<rusqlite::Connection>>` wrapping a single read-only handle. Tauri commands are not high-volume; a connection pool is YAGNI for v1.
- `BrainctlClient`: `Arc<Mutex<ClientInner>>`. Writes serialized to stdin via the mutex; reader task is independent and dispatches via the pending map.

## Testing strategy

- **Rust unit tests:**
  - `mcp_codec.rs` — framing roundtrip, partial line handling, malformed JSON tolerance.
  - `brain_db.rs` — open in-memory SQLite with a minimal mantic schema fixture, verify count/listing methods. (For v1 we don't need to mirror brainctl's full schema; just create the tables we read from.)
  - `license.rs` — port the existing 3 keyring tests to the State<T> pattern. Keep the `Once`-gated mock init.
  - `brainctl_client.rs` — mock the subprocess via a fake `tokio::process::Child` that scripts stdin/stdout. Test request/response correlation, timeout, crash detection.

- **Integration test:** if the bundled `brainctl-mcp` binary is present at `apps/desktop/binaries/...`, spawn it for real, exercise `memory_add` end-to-end against a temp brain.db. Skipped (with a clear message) if binary is absent — so the test suite stays green for developers who haven't pulled the binary yet.

- **No new e2e changes** in this sub-project — the existing smoke test still validates the binary launches and stays alive 5s.

## Out of scope for this sub-project

- Cross-platform binary builds (macOS x86_64, Linux, Windows) — follow-up
- Code signing / notarization — pre-beta milestone
- Frontend UI for brain content — sub-project #11
- Federated brain sync — sub-project #7
- Reflexions / agent runtime — sub-project #4
- Tauri 1 → 2 migration of any kind — already on Tauri 2
- Brainctl's schema migration system — brainctl owns that internally

## Implementation plan decomposition

Single plan, this time — sub-project #2 fits in one plan (vs. sub-project #1 which decomposed further). Build order:

1. **State<T> refactor** — migrate keyring from static OnceLock to `State<KeyringStore>` via `app.manage()`. All existing keyring tests + license commands still pass.
2. **brainctl PyInstaller build** — add the spec + script + CI workflow. Local: produce a working macOS arm64 binary that responds to `--version`.
3. **MCP codec** — `mcp_codec.rs` with framing + correlation, unit-tested in isolation.
4. **BrainctlClient skeleton** — spawn/respawn lifecycle, no method calls yet. Verified via mock child.
5. **BrainctlClient methods** — `memory_add` first (proves the pipe), then `event_add`, `decision_add`, `entity_create`, `entity_observe`, `agent_register`, `agent_wrap_up`, `agent_orient`, `memory_search`.
6. **BrainDb (rusqlite reads)** — `brain_status`, `recent_events`, `recent_memories`.
7. **AppState assembly** — wire everything into `setup()`.
8. **Tauri commands** — expose the 12 commands listed above (3 reads + 9 subprocess).
9. **TS bridge updates** — typed wrappers for all 12 commands.
10. **Tauri externalBin config** — drop the binary into `apps/desktop/binaries/`, update `tauri.conf.json`, verify `tauri build --debug` finds it (without invoking the DMG bundler).
11. **Integration test** — gated on binary presence.
12. **README updates** — quickstart now mentions the brainctl-mcp bundle.

Estimated 20-25 implementation tasks. Comparable in scope to sub-project #1.
