# Tasks: Phase 007 — WebSocket Event Stream

| Field | Value |
|-------|-------|
| Phase | 007 |
| Name | WebSocket Event Stream |
| Milestone group | Observable system state |
| Depends on phases | 1-6 |
| Task file | `.forge/tasks/tasks_phase007.json` |
| Tasks | 10 |

## Overview

Phase 7 adds the `EventBroadcaster`, the `GET /v1/events` WebSocket endpoint with 30s keepalive ping and lag-disconnect, and the 5-second `system.stats` tick. After this phase a client can subscribe to the live event stream and watch system statistics arrive every five seconds — the first real-time surface of the application.

Every task in this phase implements one module, one endpoint, or one infrastructure change plus its verification. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the Runnable Proof below passes. 

Group B locks in real-hardware lint coverage in CI, mirroring the real-hardware compile check added in P6-B2.

Group C eliminates the version-authority gap by introducing `[workspace.dependencies]` and upgrading all external dependencies to their current stable releases via MCP lookup.

Group D addresses production bugs discovered during manual testing: a first-run database creation failure in `anvilml-registry` (D1), silent hardware detector fallbacks in `anvilml-hardware` that mask the Vulkan extension misclassification responsible for DXGI being used instead of Vulkan on Windows AMD hardware (D2), and silent error discards in `anvilml-registry`'s scanner and the model HTTP handlers that make scan failures and database errors invisible at the server log level (D3).

Group E completes the major-version upgrade work deferred by P7-C1: E1 bumps thiserror (derive macro unused; zero code changes) and sha2 (local usage only); E2 migrates toml 0.8→1.x across the three call sites that break; E3 performs the coordinated axum 0.7→0.8 and tower 0.4→0.5 migration across all handler signatures in anvilml-server.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P7-A1 | `crates/anvilml-server/src/ws/broadcaster.rs` | anvilml-server: EventBroadcaster |
| P7-A2 | `crates/anvilml-server/src/ws/handler.rs` | anvilml-server: WebSocket /v1/events handler |
| P7-A3 | `crates/anvilml-server/src/ws/handler.rs` | anvilml-server: WS keepalive ping every 30s |
| P7-A4 | `crates/anvilml-server/src/ws/stats_tick.rs` | anvilml-server: system.stats tick task (5s broadcast) |
| P7-A5 | `backend/src/main.rs` | anvilml: start stats tick at startup; verify live WS stream |
| P7-B1 | `.github/workflows/ci.yml` | anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs |
| P7-C1 | `Cargo.toml`, all per-crate `Cargo.toml` files | anvilml: introduce [workspace.dependencies] and upgrade all external deps to current stable |
| P7-D1 | `crates/anvilml-registry/src/db.rs`, `crates/anvilml-server/tests/api_models.rs` | anvilml-registry: fix db::open to create missing database file |
| P7-D2 | `crates/anvilml-hardware/src/vulkan.rs`, `dxgi.rs`, `sysfs.rs`, `nvml.rs`, `lib.rs` | anvilml-hardware: explicit detector warnings + Vulkan extension fix |
| P7-D3 | `crates/anvilml-registry/src/scanner.rs`, `crates/anvilml-server/src/handlers/models.rs` | anvilml-registry + anvilml-server: silent error discard fixes |
| P7-E1 | `Cargo.toml` ([workspace.dependencies]) | anvilml: upgrade thiserror to 2.x and sha2 to 0.11.x |
| P7-E2 | `crates/anvilml-core/src/config_load.rs`, `config.rs`, `backend/tests/config_reference.rs` | anvilml-core: migrate toml dependency from 0.8.x to 1.x |
| P7-E3 | `crates/anvilml-server/src/handlers/*`, `ws/*`, `lib.rs`, `Cargo.toml` | anvilml-server: migrate axum from 0.7.x to 0.8.x (+ tower 0.4→0.5) |

## Task details

### Group A — WebSocket Event Stream

#### P7-A1: anvilml-server: EventBroadcaster

- **Prereqs:** P6-A7
- **Tags:** —

Create src/ws/broadcaster.rs: EventBroadcaster{sender: tokio::sync::broadcast::Sender<Arc<WsEvent>>}. new(capacity:usize). fn send(&self, event:WsEvent) wrapping in Arc and ignoring SendError (no subscribers is fine). fn subscribe(&self)->broadcast::Receiver<Arc<WsEvent>>. cargo test -p anvilml-server -- broadcaster exits 0: subscribe, send, receive equal event; send with no subscribers does not error.

**Acceptance criterion:** `cargo test -p anvilml-server -- broadcaster` exits 0.

---

#### P7-A2: anvilml-server: WebSocket /v1/events handler

- **Prereqs:** P7-A1
- **Tags:** reasoning

Add broadcaster: Arc<EventBroadcaster> to AppState (capacity from cfg.limits.ws_broadcast_capacity). Create src/ws/handler.rs: ws_events(WebSocketUpgrade, State)->on_upgrade. On connect subscribe; forward each Arc<WsEvent> as Message::Text(json). On RecvError::Lagged close with code 1008. No history replay. Wire GET /v1/events. Add tokio-tungstenite dev-dep — use mcp-rust-docs to resolve the current stable version compatible with axum 0.7 (tungstenite 0.23 series) before writing the version to Cargo.toml. cargo test -p anvilml-server --features mock-hardware -- ws exits 0: connect, broadcast a test event, assert received as JSON text.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0.

---

#### P7-A3: anvilml-server: WS keepalive ping every 30s

- **Prereqs:** P7-A2
- **Tags:** —

In ws/handler.rs add a ping task: tokio::time::interval(30s) sending Message::Ping(vec![]) to the socket; on send error end the connection. Run alongside the broadcast-forward task via tokio::select! so either ending closes the socket. cargo test -p anvilml-server --features mock-hardware -- ws still exits 0 (no regression).

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0 with no regressions.

---

#### P7-A4: anvilml-server: system.stats tick task (5s broadcast)

- **Prereqs:** P7-A3
- **Tags:** —

Create src/ws/stats_tick.rs: spawn_system_stats_tick(state)->JoinHandle. Every 5s build SystemStatsEvent: per-device vram from AppState.hardware (used 0 until worker reports exist), host ram via sysinfo; broadcaster.send(WsEvent::SystemStats). Call it from build_router setup or main startup. Verify via next task.

**Acceptance criterion:** `cargo build --features mock-hardware` exits 0 (runtime verification deferred to P7-A5).

---

#### P7-A5: anvilml: start stats tick at startup; verify live WS stream

- **Prereqs:** P7-A4
- **Tags:** —

In main.rs after AppState built, call spawn_system_stats_tick(state.clone()). Ensure broadcaster + tick are live for the bound server. Verify: cargo run --features mock-hardware, then in another shell `websocat ws://127.0.0.1:8488/v1/events` (or a browser WS console) shows a system.stats JSON frame arriving every ~5 seconds with event='system.stats' and a timestamp.

**Acceptance criterion:** `websocat ws://127.0.0.1:8488/v1/events` receives a `system.stats` JSON frame within 10 seconds of connecting.

---

### Group B — CI Hardening

#### P7-B1: anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs

- **Prereqs:** P7-A5
- **Tags:** —

The real-hardware code paths (`#[cfg(unix)]` and `#[cfg(windows)]` branches in `anvilml-hardware`) are never seen by `cargo clippy --workspace --features mock-hardware`. Warnings in those paths only surface at manual run time. P6-B2 added a real-hardware compile check to both CI jobs; this task adds the corresponding clippy pass immediately after it, closing the lint gap.

Both jobs receive a new step placed immediately after their existing `Real-hardware compile check` step:

```yaml
- name: Real-hardware lint
  run: cargo clippy --bin anvilml -- -D warnings
```

No `--features` flag on either. On `rust-linux` this lints `#[cfg(unix)]` paths natively. On `rust-windows` (native MSVC) it lints `#[cfg(windows)]` paths. All existing jobs and steps are preserved unchanged.

**Files to create or modify:**
- `.github/workflows/ci.yml` — add `Real-hardware lint` step to both `rust-linux` and `rust-windows` jobs, each placed immediately after their existing `Real-hardware compile check` step

**Key implementation notes:**
- Placement after `Real-hardware compile check` is mandatory — a compile failure in the preceding step halts the job before lint runs, which is correct ordering.
- Per `FORGE_AGENT_RULES §3.7`, CI workflow files may only be modified when explicitly listed in the task's Files Affected table.
- Do not add the step to any job other than `rust-linux` and `rust-windows`.

**Acceptance criterion:** `grep -c 'Real-hardware lint' .github/workflows/ci.yml` prints `2`.

---

### Group C — Dependency Governance

#### P7-C1: anvilml: introduce [workspace.dependencies] and upgrade all external deps to current stable

- **Prereqs:** P7-B1
- **Tags:** reasoning

The workspace root `Cargo.toml` has no `[workspace.dependencies]` table. Every crate carries its own version strings independently, scattered across nine `Cargo.toml` files with bare major-version constraints. This makes PLAN sessions the effective authority on dependency versions — a PLAN session can write any version it likes and ACT has historically deferred to it rather than the MCP lookup result. The consequence is stale or mismatched pins such as the `tokio-tungstenite`/axum 0.7 mismatch in P7-A2.

This task establishes a single authoritative location for all external dependency versions:

1. Add `[workspace.dependencies]` to root `Cargo.toml`, populated by querying `mcp-rust-docs` for the current stable version of every external dependency currently declared anywhere in the workspace. Use those current stable versions, not the versions currently in any per-crate `Cargo.toml`.
2. Update every per-crate `Cargo.toml` to reference shared external dependencies via `{ workspace = true }`, removing the now-redundant inline version strings. Features that are crate-specific must be declared at the crate level alongside `workspace = true` (e.g. `tokio = { workspace = true, features = ["macros"] }`).
3. Internal path dependencies (`anvilml-core`, `anvilml-hardware`, etc.) are unchanged.
4. Dev-dependencies that appear in more than one crate move to `[workspace.dependencies]`; those in only one crate may stay local.

If an MCP lookup returns a version that is semver-incompatible with existing code (e.g. a breaking major bump), pin the last compatible major version, document it under `## Blockers` with the incompatibility described, and stop. A follow-on retrofit leaf task will be authored to perform that specific migration.

**Files to create or modify:**
- `Cargo.toml` — add `[workspace.dependencies]` with current stable versions of all shared external deps
- `backend/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-core/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-hardware/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-registry/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-ipc/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-worker/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-scheduler/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-server/Cargo.toml` — migrate to `{ workspace = true }` references
- `crates/anvilml-openapi/Cargo.toml` — migrate to `{ workspace = true }` references

**Key implementation notes:**
- Query `mcp-rust-docs` for every external dependency before writing any version number. Record every lookup and its result in `## Resolved Dependencies`.
- `Cargo.lock` will be regenerated by cargo — commit the updated lock file.
- The `mock-hardware` feature flag chain is expressed via crate-level `[features]`, not `[workspace.dependencies]` — leave it unchanged.

**Acceptance criterion:** `cargo build --workspace --features mock-hardware` exits 0 AND `cargo test --workspace --features mock-hardware` exits 0 AND `grep -c 'workspace = true' Cargo.toml` is greater than 0.

---

### Group D — Bug Fixes

#### P7-D1: anvilml-registry: fix db::open to create missing database file

- **Prereqs:** P7-C1
- **Tags:** —

`db::open` passes a bare path string to `SqlitePoolOptions::connect`, which does not set `SQLITE_OPEN_CREATE`. On first run, when `anvilml.db` does not yet exist, SQLite returns error code 14 and the server panics before binding. The fix replaces the connect call with `SqliteConnectOptions::new().filename(path).create_if_missing(true)` passed to `connect_with`. The pre-creation workaround (`fs::File::create`) in `api_models.rs` exists solely because `open()` cannot create the file itself; it is removed in the same task.

**Files to create or modify:**
- `crates/anvilml-registry/src/db.rs` — replace `SqlitePoolOptions::connect(path_str)` with `SqliteConnectOptions::new().filename(path).create_if_missing(true)` via `connect_with`; add `SqliteConnectOptions` to the `sqlx::sqlite` import; add test `test_open_creates_file_if_missing` that calls `open()` on a path that does not yet exist and asserts the file is present afterwards
- `crates/anvilml-server/tests/api_models.rs` — remove the `fs::File::create(&db_path)` line from `setup_test_env`

**Key implementation notes:**
- The `filename()` builder accepts `&Path` directly and handles Windows backslash paths correctly. Do not construct a `sqlite://` URI manually.
- If P7-C1 upgraded sqlx to a version that changes the `SqliteConnectOptions` API, consult `mcp-rust-docs` for the correct import path before writing code.

**Acceptance criterion:** Delete `anvilml.db` if present, then `cargo run --features mock-hardware` starts without panicking at the database open step AND `cargo test --workspace --features mock-hardware` exits 0.

---

#### P7-D2: anvilml-hardware: explicit detector warnings + Vulkan extension fix

- **Prereqs:** P7-D1
- **Tags:** reasoning

Every silent `Ok(vec![])` early-return in the hardware detection crates discards the underlying error without any log entry, making it impossible at runtime to distinguish "no GPU present" from "detector failed for a fixable reason". The concrete consequence on Windows with an AMD GPU is that `VK_KHR_driver_properties` and `VK_EXT_memory_budget` are passed as instance-level extensions in `VkInstanceCreateInfo::ppEnabledExtensionNames`. Both are device extensions; the AMD ICD correctly rejects `vkCreateInstance` with `VK_ERROR_EXTENSION_NOT_PRESENT`, which the current code silently swallows, returning `Ok(vec![])` and falling through to the DXGI fallback. This task makes every discard visible and corrects the Vulkan extension misclassification so the primary detection path functions on AMD hardware.

**Files to create or modify:**
- `crates/anvilml-hardware/src/vulkan.rs` — remove `KHR_driver_properties` and `EXT_memory_budget` from the `extensions` vec passed to `create_instance`; pass an empty `enabled_extension_names` slice to `create_instance`; after enumerating physical devices, call `instance.enumerate_device_extension_properties(*pd, None)` per device to build the set of supported device extensions; gate the `PhysicalDeviceDriverProperties` pNext chain on `VK_KHR_driver_properties` membership in that set and the `PhysicalDeviceMemoryBudgetPropertiesEXT` pNext chain on `VK_EXT_memory_budget` membership; add `tracing::warn!(detector, error)` at every `Err(_) => return Ok(Vec::new())` site
- `crates/anvilml-hardware/src/dxgi.rs` — add `tracing::warn!(detector, error)` at every silent `Ok(vec![])` return including COM initialisation failure and per-adapter failure paths
- `crates/anvilml-hardware/src/sysfs.rs` — add `tracing::warn!(detector, error)` at every silent `Ok(vec![])` return
- `crates/anvilml-hardware/src/nvml.rs` — add `tracing::warn!(detector, error)` at every silent `Ok(vec![])` return
- `crates/anvilml-hardware/src/lib.rs` — add `tracing::warn!` at the `unwrap_or_default()` call sites in `enumerate_gpus` where a detector returning empty triggers fallback

**Key implementation notes:**
- Instance creation must use an empty extension name list. `VK_KHR_driver_properties` and `VK_EXT_memory_budget` do not appear in `vkEnumerateInstanceExtensionProperties` — passing them to `vkCreateInstance` is a Vulkan spec violation.
- `get_physical_device_properties2` requires only Vulkan 1.1 core (already requested via `api_version: 1.3.0`) and is called unconditionally. Only the pNext chain structs that require specific device extensions are conditionally included based on the per-device extension query result.
- All existing tests must continue to pass. `vulkan_detect_returns_ok` asserts `Ok` always — failures now warn before returning `Ok(vec![])`, which is unchanged behaviour from the public interface perspective.
- After this fix, startup on a Windows machine with a Vulkan-capable AMD GPU must log `enumeration_source=Vulkan` rather than `enumeration_source=Dxgi`.

**Acceptance criterion:** `cargo run` (without `--features mock-hardware`) logs `enumeration_source=Vulkan` for the detected GPU AND `cargo clippy --workspace -- -D warnings` exits 0 AND `cargo test --workspace --features mock-hardware` exits 0.

---

#### P7-D3: anvilml-registry + anvilml-server: silent error discard fixes

- **Prereqs:** P7-D2
- **Tags:** —

Two crates outside `anvilml-hardware` contain silent discards that make failures invisible at runtime. In `scanner.rs`, walkdir entry errors and metadata read failures are silently skipped with bare `continue`, and `canonicalize` failures silently fall back to the raw path — causing the affected model to receive an incorrect ID derived from the wrong path string, corrupting deduplication with no log entry. In `handlers/models.rs`, both the `list_models` and `get_model` handlers catch database errors into `Err(_e)` arms that discard `_e` and produce no log output.

**Files to create or modify:**
- `crates/anvilml-registry/src/scanner.rs` — in `scan_dirs`: replace `Err(_) => continue` on the walkdir iterator with `Err(e) => { tracing::warn!(path = %dir_config.path.display(), error = %e, "scanner: skipping unreadable entry"); continue; }`; replace `Err(_) => continue` on `entry.metadata()` with the same pattern naming the file path; replace `canonicalize().unwrap_or_else(|_| ...)` with an explicit `match` that warns on error before using the fallback path
- `crates/anvilml-server/src/handlers/models.rs` — in `list_models` `Err(_e)` arm: add `tracing::error!(error = %_e, "list_models: registry query failed")` and change the response body from `Json(vec![])` to `Json(serde_json::json!({"error":"internal_error","message":_e.to_string()}))`; in `get_model` `Err(_e)` arm: add `tracing::error!(error = %_e, "get_model: registry query failed")`

**Key implementation notes:**
- The `list_models` response type is `(StatusCode, Json<Vec<ModelMeta>>)`. Changing the error arm body requires changing the return type to `(StatusCode, Json<serde_json::Value>)` so both arms can return different JSON shapes. Update the function signature accordingly.
- The warn messages in `scanner.rs` must include the affected path so operators can identify which directory or file triggered the problem.
- All existing tests must continue to pass without modification — the changes only add log output and fix the error response body shape.

**Acceptance criterion:** `cargo clippy --workspace -- -D warnings` exits 0 AND `cargo test --workspace --features mock-hardware` exits 0.

---

### Group E — Major Version Upgrades (Deferred from P7-C1)

#### P7-E1: anvilml: upgrade thiserror to 2.x and sha2 to 0.11.x

- **Prereqs:** P7-C1
- **Tags:** —

Two of the four crates that P7-C1 pinned at their last compatible major have zero or near-zero migration cost and are bundled here. `thiserror` is declared as a workspace dependency but the derive macro is never invoked — `AnvilError` implements `Display`, `Error`, and `From` manually. The bump is a one-line version change. `sha2` usage is entirely local to `scanner.rs` with no `Digest` trait bounds in any public API; the only verification needed is that `hex 0.4.3`'s `hex::encode` still accepts `sha2 0.11`'s output type.

**Files to create or modify:**
- `Cargo.toml` — bump `thiserror` from `"1.0.69"` to `"2"` and `sha2` from `"0.10.8"` to `"0.11"` in `[workspace.dependencies]`

**Key implementation notes:**
- Confirm via `mcp-rust-docs` that the exact latest 2.x and 0.11.x patch versions are still what was resolved; P7-C1 may have already written the pinned versions.
- If `hex 0.4.3` does not compile against `sha2 0.11` output (a `GenericArray` type mismatch), bump `hex` to its current stable in the same commit rather than creating a separate task — it is a cosmetic fix within the same change scope.

**Acceptance criterion:** `cargo build --workspace --features mock-hardware` exits 0 AND `cargo test --workspace --features mock-hardware` exits 0.

---

#### P7-E2: anvilml-core: migrate toml dependency from 0.8.x to 1.x

- **Prereqs:** P7-E1
- **Tags:** reasoning

`toml 1.0` introduced three breaking changes affecting this codebase. First, `toml::de::Error` moved — `ConfigError::Toml(toml::de::Error)` and its `From` impl in `config_load.rs` must be updated to the new path. Second, `toml::to_string_pretty` was removed from the crate root and moved to `toml::ser::to_string_pretty`; two files call it. Third, the `toml::Value` API must be verified for compatibility with the usage in `config_reference.rs`. Consult `mcp-rust-docs` for the `toml` 1.x migration notes before modifying any file.

**Files to create or modify:**
- `Cargo.toml` — bump `toml` from `"0.8.23"` to `"1"` in `[workspace.dependencies]`
- `crates/anvilml-core/src/config_load.rs` — update `ConfigError::Toml` variant type and `From<toml::de::Error>` impl to the 1.x error path
- `crates/anvilml-core/src/config.rs` — update `toml::to_string_pretty` call to `toml::ser::to_string_pretty`
- `backend/tests/config_reference.rs` — update `toml::to_string_pretty` call; verify `toml::Value` and `toml::from_str` usage compiles unchanged

**Key implementation notes:**
- In `toml 1.x`, `toml::de::Error` is now `toml::de::Error` but accessed via `toml::Error` at the crate root for deserialization errors; check the exact type returned by `toml::from_str::<T>()` on failure against the 1.x docs.
- The drift guard test in `config_reference.rs` serializes `ServerConfig::default()` to TOML and parses it back — this must still pass after the migration.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0 AND `cargo clippy --workspace -- -D warnings` exits 0.

---

#### P7-E3: anvilml-server: migrate axum from 0.7.x to 0.8.x (+ tower 0.4→0.5)

- **Prereqs:** P7-E2
- **Tags:** reasoning

axum 0.8 and tower 0.5 must be upgraded together — axum 0.8 requires tower 0.5 and the two are incompatible if mismatched. The primary breaking change in axum 0.8 is extractor ordering: `State<T>` must be the first extractor in every handler signature. Every handler in `anvilml-server` must be audited. Additionally `Router::with_state` was changed and any `MethodRouter` usage must be verified. Consult `mcp-rust-docs` for the axum 0.8 migration guide and tower 0.5 changelog before modifying any file.

**Files to create or modify:**
- `Cargo.toml` — bump `axum` to `"0.8"` and `tower` to `"0.5"` in `[workspace.dependencies]`
- `crates/anvilml-server/src/handlers/health.rs` — verify/fix extractor ordering
- `crates/anvilml-server/src/handlers/system.rs` — verify/fix extractor ordering
- `crates/anvilml-server/src/handlers/models.rs` — verify/fix extractor ordering
- `crates/anvilml-server/src/ws/handler.rs` — verify/fix extractor ordering and WebSocketUpgrade API changes if any
- `crates/anvilml-server/src/lib.rs` — verify `build_router`, `Router::with_state`, and any `ServiceExt` usage in tests against 0.8 API

**Key implementation notes:**
- The test suite in `lib.rs` uses `tower::ServiceExt` for `oneshot` — verify `tower 0.5` still exposes this (it does, but confirm the import path).
- axum 0.8 removed `TypedHeader` — verify it is not used anywhere in the codebase before starting.
- Do not upgrade `tokio-tungstenite` as part of this task; the planning data confirms it is pinned at 0.24.x for axum 0.7 compatibility and must be re-evaluated after the axum upgrade is confirmed working.

**Acceptance criterion:** `cargo clippy --workspace -- -D warnings` exits 0 AND `cargo test --workspace --features mock-hardware` exits 0.

---

## Runnable Proof

Subscribe to the WebSocket and watch `system.stats` frames arrive.

```bash
cargo run --features mock-hardware
# another terminal:
websocat ws://127.0.0.1:8488/v1/events
```

Expected: roughly every 5 seconds a JSON text frame arrives with `"event":"system.stats"`, a `timestamp`, a `gpus` array, and `ram_used_mib`/`ram_total_mib`. The connection stays open (30s pings keep it alive). Phase done when a subscriber observes recurring `system.stats` frames, `cargo test -p anvilml-server --features mock-hardware` is green, `grep -c 'Real-hardware lint' .github/workflows/ci.yml` prints `2`, and `grep -c 'workspace = true' Cargo.toml` is greater than 0.

## Known Constraints and Gotchas

- P7-A2 must resolve `tokio-tungstenite` via `mcp-rust-docs` before pinning the version. The compatible version for axum 0.7 is the tungstenite 0.23 series (e.g. tokio-tungstenite 0.24). Do not copy the version from training data.
- P7-B1 modifies `.github/workflows/ci.yml`. Per `FORGE_AGENT_RULES §3.7` this is only permitted because the file is explicitly listed in that task's Files Affected table. The lint step must be placed after `Real-hardware compile check` — a compile failure in the preceding step halts the job before lint runs.
- P7-B1 must run after P7-A5 to avoid disrupting the in-progress WebSocket implementation chain.
- P7-C1 will regenerate `Cargo.lock` — the updated lock file must be staged and committed. Do not suppress lock file changes.
- P7-C1 uses `mcp-rust-docs` as the authoritative version source. Any version written into `[workspace.dependencies]` without a prior MCP lookup is a protocol violation. If the MCP server is unavailable, set `Status=BLOCKED` per `FORGE_AGENT_RULES §6.4`.
- After P7-C1, all future tasks that add a new external dependency MUST add it to `[workspace.dependencies]` first and reference it via `{ workspace = true }` in the per-crate `Cargo.toml`. Adding an inline version string to a per-crate `Cargo.toml` is a drift violation.
- If P7-C1 encounters a semver-incompatible major bump, it pins the last compatible major and stops. A manually authored follow-on retrofit leaf task (e.g. a new Group D task — not pre-authored; created only if needed) handles the migration. The retrofit task's `context` field must open with an explicit origin reference: `"P7-C1 pinned <crate> at <old version> due to semver incompatibility. Migrate to <new version>: ..."`.
- P7-D1 must run after P7-C1. The `SqliteConnectOptions` API surface depends on the sqlx version established by P7-C1; writing the fix against the pre-C1 version risks a second churn pass when C1 upgrades sqlx.
- P7-D2 changes the observable startup log on Windows with a Vulkan-capable AMD GPU: after the fix, the server logs `enumeration_source=Vulkan` instead of `enumeration_source=Dxgi`. The DXGI fallback path remains correct and is still reached when Vulkan genuinely produces an empty device list — it is no longer silently reached when Vulkan fails a rejectable `vkCreateInstance` call.
- P7-D3 changes the return type of `list_models` from `(StatusCode, Json<Vec<ModelMeta>>)` to `(StatusCode, Json<serde_json::Value>)`. The existing integration test in `api_models.rs` asserts the success-path response body as a JSON array — it must continue to pass because the success arm is unchanged and `serde_json::Value` can represent an array. No test changes are required, but The Forge must verify this explicitly after implementation.
- P7-E1 through P7-E3 are sequenced E1→E2→E3 to keep each upgrade atomic and independently revertable. They prereq P7-C1 (not each other's predecessors in the original chain) with the exception that E2 prereqs E1 and E3 prereqs E2, forming a linear sub-chain. Do not reorder.
- P7-E3 (axum 0.8) must not be attempted until P7-E2 is complete and the workspace builds clean. A partial upgrade of axum without tower 0.5 will fail to compile immediately.
- After P7-E3, `tokio-tungstenite` remains pinned at 0.24.x. The axum 0.8 + tower 0.5 environment may support a newer `tokio-tungstenite`; this should be evaluated as a separate follow-on leaf task in a later phase, not folded into P7-E3.