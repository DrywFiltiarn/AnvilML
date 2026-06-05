# Tasks: Phase 007 — WebSocket Event Stream

| Field | Value |
|-------|-------|
| Phase | 007 |
| Name | WebSocket Event Stream |
| Milestone group | Observable system state |
| Depends on phases | 1-6 |
| Task file | `.forge/tasks/tasks_phase007.json` |
| Tasks | 25 |

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
| P7-D4 | `backend/src/main.rs`, `crates/anvilml-hardware/src/lib.rs` | anvilml: fix OS field blank and stray colon in --print-hardware output |
| P7-E1 | `Cargo.toml` ([workspace.dependencies]) | anvilml: upgrade thiserror to 2.x and sha2 to 0.11.x |
| P7-E2 | `crates/anvilml-core/src/config_load.rs`, `config.rs`, `backend/tests/config_reference.rs` | anvilml-core: migrate toml dependency from 0.8.x to 1.x |
| P7-E3 | `crates/anvilml-server/src/handlers/*`, `ws/*`, `lib.rs`, `Cargo.toml` | anvilml-server: migrate axum from 0.7.x to 0.8.x (+ tower 0.4→0.5) |
| P7-F0 | `crates/anvilml-core/src/types/hardware.rs`, `crates/anvilml-hardware/src/lib.rs`, `backend/src/main.rs` | anvilml-core: extend InferenceCaps with fp32, fp8, fp4, nvfp4 fields |
| P7-F1 | `backend/migrations/004_device_capabilities.sql` | anvilml-registry: migration 004_device_capabilities.sql |
| P7-F2 | `crates/anvilml-registry/src/device_store.rs` | anvilml-registry: DeviceCapabilityStore upsert + get + seed |
| P7-F3 | `crates/anvilml-hardware/src/device_db.rs` | anvilml-hardware: SEED_ENTRIES from SUPPORTED_DEVICES_DB.md + resolve_caps_from_row |
| P7-F4 | `crates/anvilml-hardware/src/lib.rs`, `backend/src/main.rs` | anvilml-hardware: detect_all_devices seeds and queries device_capabilities |
| P7-G1 | `backend/seeds/devices.sql` | Create devices.sql seed file from SUPPORTED_DEVICES_DB.md |
| P7-G2a | `crates/anvilml-registry/src/seed_loader.rs` | seed_loader: tracking table bootstrap + SHA256 comparison |
| P7-G2b | `crates/anvilml-registry/src/seed_loader.rs` | seed_loader: replace_all and merge execution engine |
| P7-G3 | `crates/anvilml-hardware/src/lib.rs`, `crates/anvilml-hardware/src/device_db.rs` | Replace SEED_ENTRIES startup call with SeedLoader; remove const |
| P7-G4 | `crates/anvilml-hardware/src/device_db.rs`, `backend/src/main.rs` | Fix name resolution priority and --print-hardware display |

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

#### P7-D4: anvilml: fix OS field blank and stray colon in --print-hardware output

- **Prereqs:** P7-D3
- **Tags:** —

Two independent bugs conspire to produce the blank OS line with a stray colon. The first is in the table printer: the `println!` for the OS row passes `" ".repeat(50 - 8)` as the format argument instead of `hw.host.os`, so the field value is never interpolated. Additionally the format string contains a literal trailing `:` after the `{}` placeholder, producing the stray colon visible in the output. The second bug is in `populate_host_info`: `sysinfo 0.32` `System::name()` returns `Some("")` rather than `None` on this Windows configuration, so the `.unwrap_or_else(|| "Unknown".to_string())` fallback is never reached and the `HostInfo.os` field is stored as an empty string.

**Files to create or modify:**
- `backend/src/main.rs` — in `print_hardware_table`, replace the OS `println!` line:
```rust
  // Before (broken):
  println!("║ OS:          {}:", " ".repeat(50 - 8));
  // After (fixed):
  println!("║ OS:          {}", hw.host.os);
```
- `crates/anvilml-hardware/src/lib.rs` — in `populate_host_info`, replace the `os` assignment:
```rust
  // Before (broken on Windows):
  let os = sysinfo::System::name().unwrap_or_else(|| "Unknown".to_string());
  // After (fixed):
  let os = sysinfo::System::long_os_version()
      .or_else(|| sysinfo::System::name())
      .filter(|s| !s.is_empty())
      .unwrap_or_else(|| "Unknown".to_string());
```

**Key implementation notes:**
- `System::long_os_version()` in `sysinfo 0.32` returns the full product name on Windows (e.g. `"Windows 10 Pro"`, `"Windows Server 2019"`), making it the preferred source. `System::name()` is the fallback for Linux/macOS where `long_os_version` may return `None`. The `.filter(|s| !s.is_empty())` guard ensures that an `Some("")` result from either call is treated as absent rather than stored as an empty string.
- Note that `sysinfo` will be upgraded to 0.39.3 in P7-E1 or a subsequent task. In `sysinfo 0.39` the `System::long_os_version()` and `System::name()` API is unchanged in signature; this fix is forward-compatible and does not need revisiting after the upgrade.

**Acceptance criterion:** `cargo run -- --print-hardware` prints a non-empty OS string on the `OS:` line with no trailing stray colon.

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

## Group F — Device Capability DB Migration

### Why this group exists here

`InferenceCaps` in `anvilml-core` currently has three fields (`fp16`, `bf16`, `flash_attention`). Hardware detection on an AMD Radeon RX 9070 XT exposed two problems: the card was missing from `PCI_CAPABILITY_TABLE`, causing fallback with all-false capabilities, and the Vulkan driver string appeared as the device name because `resolve_caps` overwrote it on miss. Both are symptoms of the same root issue — capability data is baked into the binary at compile time, making it impossible to add a new card without a recompile.

Group F solves this in four sequential steps. First, `InferenceCaps` is extended to carry the full precision vocabulary needed for the worker phases (`fp32` via TF32, `fp8`, `fp4`, `nvfp4`), with the canonical field order `fp32 → fp16 → bf16 → fp8 → fp4 → nvfp4 → flash_attn` locked in across every surface: struct declaration, migration DDL, store row type, and CLI display. Second, a `device_capabilities` SQLite table and corresponding store are created. Third, `device_db.rs` is rewritten to export a `SEED_ENTRIES` const sourced verbatim from `docs/SUPPORTED_DEVICES_DB.md` — the 126-entry reference document that the Forge reads at PLAN time — replacing the old `PCI_CAPABILITY_TABLE`. Fourth, `detect_all_devices` is made async and wired to seed then query the store on every startup.

The group prereqs P7-E3 to avoid a second broad Cargo.toml churn pass, and begins with P7-F0 because adding fields to `InferenceCaps` is a workspace-wide breaking change: every `InferenceCaps` struct literal across six files fails to compile until the new fields are present.

---

### Task Descriptions

#### P7-F0: anvilml-core: extend InferenceCaps with fp32, fp8, fp4, nvfp4 fields

- **Prereqs:** P7-E3
- **Tags:** —

This is a workspace-wide breaking change. `InferenceCaps` struct literals are exhaustive in Rust — any site that names fields will fail to compile when new ones are added. All sites must be updated in the same commit.

**Files to create or modify:**

- `crates/anvilml-core/src/types/hardware.rs` — add four `bool` fields to `InferenceCaps` in canonical order: `fp32`, then `fp16` (existing), `bf16` (existing), `fp8`, `fp4`, `nvfp4`, `flash_attention` (existing). All new fields: `#[serde(default)]`. All fields: `pub`.
- `crates/anvilml-hardware/src/lib.rs` — extend `or_all_caps` to OR all seven fields. Update every `InferenceCaps` construction in tests.
- `crates/anvilml-hardware/src/cpu.rs` — update `GpuDevice` construction: new `InferenceCaps` fields default `false`.
- `crates/anvilml-hardware/src/mock.rs` — same.
- `crates/anvilml-hardware/src/device_db.rs` — any `InferenceCaps` literal constructions updated.
- `backend/src/main.rs` — update `print_hardware_table` to display all seven flags in canonical order. The existing display shows only `FP16 / BF16 / Flash Attention`; after this task it must show `FP32 / FP16 / BF16 / FP8 / FP4 / NVFP4 / FA`.

**Key implementation notes:**

The canonical field order (`fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attention`) must match the order in `SUPPORTED_DEVICES_DB.md`'s Migration DDL reference section and in the `DeviceCapabilityRow` struct that P7-F2 will create. Divergence here causes silent column misalignment in the SQLite store.

The `InferenceCaps` struct derives `Copy` and `Default`. Adding `bool` fields with `#[serde(default)]` is backward-compatible for JSON deserialization — old JSON without the new fields deserialises to `false` for each. The existing `inference_caps_backward_compat` test in `hardware.rs` must continue to pass without modification.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0 AND `cargo clippy --workspace -- -D warnings` exits 0.

---

#### P7-F1: anvilml-registry: migration 004_device_capabilities.sql

- **Prereqs:** P7-F0
- **Tags:** —

Schema-only task. No Rust code.

**Files to create or modify:**

- `backend/migrations/004_device_capabilities.sql` — DDL exactly as specified in `docs/SUPPORTED_DEVICES_DB.md` under "Migration DDL reference". Column order must match the canonical precision order: `vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`. All capability columns `INTEGER NOT NULL DEFAULT 0`. `model_name` and `arch` are `TEXT NOT NULL`. Primary key is composite `(vendor_id, device_id)`. One named unique index: `idx_device_capabilities_pci ON device_capabilities(vendor_id, device_id)`.
- `crates/anvilml-registry/tests/` — extend the existing `test_open_creates_tables` integration test to additionally assert that `device_capabilities` appears in `sqlite_master` after calling `db::open`.

**Key implementation notes:**

`sqlx::migrate!` discovers files by sorted filename. `004_` sorts correctly after `003_artifacts.sql`. Once this migration is applied to a database its filename checksum is recorded in `_sqlx_migrations` — never rename or edit the file after merging.

**Acceptance criterion:** `cargo test -p anvilml-registry` exits 0.

---

#### P7-F2: anvilml-registry: DeviceCapabilityStore upsert + get + seed

- **Prereqs:** P7-F1
- **Tags:** —

Follow the patterns established in `store.rs` exactly: tuple row type alias, `sqlx_error` helper, `sqlx::query_as` with manual column mapping.

**Files to create or modify:**

- `crates/anvilml-registry/src/device_store.rs` — `DeviceCapabilityRow` struct and `DeviceCapabilityStore` struct with three async methods.
- `crates/anvilml-registry/src/lib.rs` — re-export both types.
- `crates/anvilml-registry/tests/device_store.rs` — integration tests.

**Key implementation notes:**

`DeviceCapabilityRow` field order must match the migration column order exactly: `vendor_id: u16, device_id: u16, model_name: String, arch: String, fp32: bool, fp16: bool, bf16: bool, fp8: bool, fp4: bool, nvfp4: bool, flash_attn: bool`. The DB stores `vendor_id` and `device_id` as `INTEGER` (i64); cast to/from `u16` at the boundary. Boolean columns map via `field as i64` on write and `value != 0` on read.

`seed` runs all inserts in a single transaction using `INSERT OR REPLACE`. Re-seeding is idempotent — calling it twice with the same entries produces the same result. It returns the count of rows written (not affected), which equals `entries.len()` on a clean database.

Required tests (minimum 4): `upsert_then_get_roundtrip` verifying all 11 fields survive a round-trip; `get_miss_returns_none`; `seed_returns_correct_count` seeding 3 entries and asserting return value is 3; `bool_flags_roundtrip` verifying `fp32=true, fp16=false, fp8=true, nvfp4=false` survive serialisation.

**Acceptance criterion:** `cargo test -p anvilml-registry -- device_store` exits 0 with ≥4 tests passing.

---

#### P7-F3: anvilml-hardware: SEED_ENTRIES from SUPPORTED_DEVICES_DB.md + resolve_caps_from_row

- **Prereqs:** P7-F2
- **Tags:** reasoning

**Files to create or modify:**

- `crates/anvilml-hardware/src/device_db.rs` — complete rewrite.
- `crates/anvilml-hardware/Cargo.toml` — add `anvilml-registry = { workspace = true }`.

**Key implementation notes:**

At PLAN time, open `docs/SUPPORTED_DEVICES_DB.md` and locate the two Markdown device tables (NVIDIA and AMD). For each data row, construct one `DeviceCapabilityEntry` struct literal: `Y` maps to `true`, `N` maps to `false`. The resulting `pub const SEED_ENTRIES: &[DeviceCapabilityEntry]` must contain all 126 rows in table order. Do not generate, infer, or look up entries — copy the table values verbatim.

`DeviceCapabilityEntry` field order must match `DeviceCapabilityRow` exactly: `vendor_id: u16, device_id: u16, model_name: &'static str, arch: &'static str, fp32: bool, fp16: bool, bf16: bool, fp8: bool, fp4: bool, nvfp4: bool, flash_attn: bool`.

`resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceCapabilityRow>)`:

- **Hit:** set `dev.name = row.model_name.clone()`, `dev.arch = Some(row.arch.clone())`, populate all seven `InferenceCaps` fields from the row, set `dev.capabilities_source = CapabilitySource::DeviceTable`, `dev.enumeration_source = EnumerationSource::DeviceTable`.
- **Miss:** do **not** overwrite `dev.name` (the enumerator has already set a driver-supplied name); set `dev.caps = InferenceCaps::default()`, `dev.capabilities_source = CapabilitySource::Fallback`; emit `tracing::warn!(detector="DeviceDB", vendor_id=%format_args!("0x{:04X}", dev.pci_vendor_id), device_id=%format_args!("0x{:04X}", dev.pci_device_id), "unknown PCI ID — add to SUPPORTED_DEVICES_DB.md")`.

Remove `lookup()` and `resolve_caps()` entirely. Rewrite all existing tests to use `SEED_ENTRIES.iter().find(|e| e.vendor_id == v && e.device_id == d)` in place of `lookup()`. Add a `rx9070xt_entry_correct` test asserting `vendor_id=0x1002, device_id=0x7550` resolves to `model_name="AMD Radeon RX 9070 XT", arch="gfx1201", fp8=true, fp32=false`.

Before committing, verify no dependency cycle: run `cargo tree -p anvilml-hardware` and confirm `anvilml-registry` appears without `anvilml-hardware` in its subtree.

**Acceptance criterion:** `cargo test -p anvilml-hardware` exits 0.

---

#### P7-F4: anvilml-hardware: detect_all_devices seeds and queries device_capabilities

- **Prereqs:** P7-F3
- **Tags:** reasoning

This task has two call-site updates that must land in the same commit as the signature change, or the workspace will not compile.

**Files to create or modify:**

- `crates/anvilml-hardware/src/lib.rs` — make `detect_all_devices` async, add `pool: &SqlitePool` parameter.
- `backend/src/main.rs` — update the `detect_all_devices` call site to pass `&db` and `.await`.
- `crates/anvilml-registry/src/db.rs` — add `pub async fn open_in_memory() -> Result<SqlitePool, AnvilError>` if absent.

**Key implementation notes:**

At the top of `detect_all_devices`, before any device enumeration:

```rust
let store = DeviceCapabilityStore::new(pool.clone());
store.seed(&SEED_ENTRIES).await?;
```

For each detected device, replace the existing `device_db::resolve_caps(dev, ...)` call with:

```rust
let row = store.get(dev.pci_vendor_id, dev.pci_device_id).await?;
device_db::resolve_caps_from_row(&mut dev, row.as_ref());
```

This applies to all three code paths in `detect_all_devices`: the override branch, the mock branch, and the real enumeration branch.

All tests in `mod tests` that call `detect_all_devices` must be converted to `#[tokio::test]` and supply a pool:

```rust
let pool = anvilml_registry::db::open_in_memory().await.unwrap();
let info = detect_all_devices(&cfg, &pool).await.unwrap();
```

`open_in_memory` calls `open(Path::new(":memory:"))`, which runs all four migrations including `004_device_capabilities.sql`. This means the in-memory pool used in tests is fully migrated and the seed step in `detect_all_devices` will actually write 126 rows — this is correct and expected behaviour.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0.

---

## Known Constraints and Gotchas — append these entries

- **P7-F0 is a workspace-wide breaking change.** `InferenceCaps` struct literals are exhaustive. Adding four new fields breaks every construction site simultaneously. The Forge must update all six files (`hardware.rs`, `lib.rs`, `cpu.rs`, `mock.rs`, `device_db.rs`, `main.rs`) in a single commit. A partial update will not compile.

- **Canonical field order is a contract, not a preference.** `fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn` must be the field order in `InferenceCaps`, `DeviceCapabilityRow`, `DeviceCapabilityEntry`, and the migration DDL columns. Any divergence between the struct field order and the DDL column order causes silent data misalignment when `sqlx::query_as` maps a row tuple positionally.

- **`SUPPORTED_DEVICES_DB.md` is the seed source; do not regenerate.** P7-F3 must copy the table rows verbatim into `SEED_ENTRIES`. The Forge must not infer, supplement, or re-derive entries from training knowledge — the reference document was verified against hardware reports, ROCm docs, and kernel driver sources. Any row generated from memory will likely contain errors.

- **Migration `004_` checksum is immutable.** Once `004_device_capabilities.sql` is applied to any database, its content is recorded in `_sqlx_migrations`. Editing the file after merging will cause a checksum mismatch error on the next `db::open` call, breaking all existing databases. Author it correctly in P7-F1 and never modify it.

- **`detect_all_devices` signature change has exactly two call sites.** `backend/src/main.rs` and the `mod tests` block in `lib.rs`. Both must be updated in P7-F4's commit. The Forge must search for all occurrences before writing the plan.

- **`anvilml-hardware → anvilml-registry` is a new intra-workspace dependency introduced in P7-F3.** The permitted direction is `hardware → registry → core`. The direction `registry → hardware` or `core → hardware` is forbidden. Run `cargo tree -p anvilml-hardware` and verify no cycle before committing.

- **Seed is unconditional `INSERT OR REPLACE`.** Every startup overwrites all 126 rows with the compiled-in baseline. User-edited rows are overwritten on the next restart. This is the documented design decision; do not add a skip-if-exists guard.

---

## Group G — Seed Infrastructure and Name Resolution

### Why this group exists here

Group F completed the SQLite-backed capability store and wired it into startup, but left two unfinished concerns.

The first is that capability data is still compiled into the binary. `SEED_ENTRIES` in `device_db.rs` is a Rust const populated at compile time; `detect_all_devices` calls `store.seed(&SEED_ENTRIES)` to push this data into SQLite on every startup. This means adding or correcting a device entry requires a recompile and redeployment, which is exactly the problem the SQLite migration was meant to solve. Group G replaces this with a SQL seed file at `backend/seeds/devices.sql`, loaded by a generic `SeedLoader` in `anvilml-registry`. The loader bootstraps its own `seed_history` tracking table, computes SHA256 of the seed file content, and only re-applies the data when the file has changed. This is the first seed file; the infrastructure is designed generically so any future seed file in any crate can use the same loader.

The second concern is name resolution priority. After Group F, `resolve_caps_from_row` on a hit sets `dev.db_group_name` and preserves `dev.name`. But if the Vulkan enumerator provided no useful name (empty string, or a generic driver string), the display shows that rather than falling back to the database group label. The correct priority is: Vulkan-reported name (if non-empty and non-generic) → database group label → `"Unknown GPU (0x{vendor}:{device})"` as the last resort when neither is available.

Group G prereqs P7-F4, the final F task. All F tasks will have completed before any G task executes.

---

### Seed file header specification

Every file in `backend/seeds/` must begin with two directive comment lines before any SQL:

```sql
-- anvil:seed_table <table_name>
-- anvil:seed_strategy <replace_all|merge>
```

`seed_table` names the SQLite table the file targets. The `SeedLoader` uses this to scope the `DELETE` (for `replace_all`) and for `seed_history` keying by filename.

`seed_strategy` controls execution:
- `replace_all` — transaction wrapping `DELETE FROM <table>`, then all INSERTs, then `seed_history` update. The full table content is replaced. User-added rows are lost.
- `merge` — transaction wrapping `INSERT OR REPLACE` statements only, then `seed_history` update. Rows not present in the file are retained. User-added rows with unique keys are preserved.

A seed file missing the `seed_table` directive is a fatal error at startup. A missing `seed_strategy` defaults to `replace_all`.

---

### Task Descriptions

#### P7-G1: Create backend/seeds/devices.sql seed file

- **Prereqs:** P7-F4
- **Tags:** reasoning

**Files to create:**

- `backend/seeds/devices.sql`

**Key implementation notes:**

At PLAN time, open `docs/SUPPORTED_DEVICES_DB.md` and locate both device tables (NVIDIA and AMD). The file header must be exactly:

```sql
-- anvil:seed_table devices
-- anvil:seed_strategy replace_all
```

For every data row in both tables, emit one statement:

```sql
INSERT OR REPLACE INTO devices (vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn)
VALUES (0x10DE, 0x2684, 'NVIDIA GeForce RTX 4090', '8.9', 1, 1, 1, 1, 0, 0, 1);
```

Column order must exactly match the `004_device_capabilities.sql` DDL. `Y` in the table maps to `1`, `N` maps to `0`. `vendor_id` and `device_id` may be written as hex literals or decimal — hex is preferred for readability. `model_name` and `arch` are single-quoted strings.

Copy the rows verbatim from the reference document — do not generate, reorder, or supplement entries from training knowledge. The table is named `devices` (matching `anvil:seed_table devices`), not `device_capabilities` — note that the SQLite table created by migration `004_device_capabilities.sql` is named `device_capabilities`, while this seed file targets a view or synonym. **Verify the actual table name in the migration DDL before writing any INSERT statement.** If the migration table is named `device_capabilities`, the seed directive and INSERT target must both be `device_capabilities`, not `devices`.

**Acceptance criterion:** `backend/seeds/devices.sql` exists, begins with both directive comments, contains one INSERT per row from `SUPPORTED_DEVICES_DB.md`, and is syntactically valid SQL (verify by inspection — no Rust compilation required for this task).

---

#### P7-G2a: anvilml-registry: seed_loader — tracking table bootstrap + SHA256 comparison

- **Prereqs:** P7-G1
- **Tags:** —

**Files to create or modify:**

- `crates/anvilml-registry/src/seed_loader.rs` — new file.
- `crates/anvilml-registry/src/lib.rs` — re-export `run` or `SeedLoader`.

**Key implementation notes:**

The `seed_history` table is created by the loader itself, not by a migration. Issue this DDL unconditionally at the start of every `run` call:

```sql
CREATE TABLE IF NOT EXISTS seed_history (
    filename   TEXT    PRIMARY KEY,
    sha256     TEXT    NOT NULL,
    applied_at INTEGER NOT NULL
);
```

`applied_at` stores a Unix timestamp as `INTEGER` (seconds since epoch).

`run(pool: &SqlitePool, seeds_dir: &Path) -> Result<(), AnvilError>`:

1. Create `seed_history` table (idempotent `IF NOT EXISTS`).
2. Enumerate `.sql` files in `seeds_dir` in sorted filename order.
3. For each file: read bytes, parse header directives (first non-empty lines starting with `-- anvil:`). If `seed_table` directive is absent, return `Err(AnvilError::seed_missing_directive(filename))`.
4. Compute `sha256` of file bytes using the `sha2` crate (`Sha256::digest`).
5. Query `seed_history` for this filename. If the stored sha256 matches, skip. Otherwise proceed to execution (implemented in G2b).
6. After execution, upsert `seed_history`: `INSERT OR REPLACE INTO seed_history VALUES (filename, sha256, now)`.

Add `sha2` to `[workspace.dependencies]` and `crates/anvilml-registry/Cargo.toml`. Use `mcp-rust-docs` to resolve the correct version before pinning.

G2a covers steps 1–5 (scaffolding, parsing, comparison, skip logic) plus the `seed_history` upsert at step 6. Execution (the actual DELETE/INSERT) is stubbed as a no-op in G2a and implemented in G2b.

**Acceptance criterion:** `cargo test -p anvilml-registry -- seed` exits 0 (tests for: table bootstrap idempotent, directive parsing hit, directive parsing miss/error, sha256 skip on unchanged file).

---

#### P7-G2b: anvilml-registry: seed_loader — execution engine

- **Prereqs:** P7-G2a
- **Tags:** —

**Files to create or modify:**

- `crates/anvilml-registry/src/seed_loader.rs` — complete the execution stub from G2a.

**Key implementation notes:**

The SQL file body (everything after the `-- anvil:` header lines) contains `INSERT OR REPLACE` statements separated by semicolons. The parser must split on `;` and execute each non-empty statement individually within a single transaction.

`replace_all` strategy transaction sequence:

```sql
BEGIN;
DELETE FROM <seed_table>;
<INSERT statement 1>;
<INSERT statement 2>;
-- ...
UPDATE seed_history SET sha256=?, applied_at=? WHERE filename=?;
COMMIT;
```

`merge` strategy transaction sequence:

```sql
BEGIN;
<INSERT OR REPLACE statement 1>;
<INSERT OR REPLACE statement 2>;
-- ...
UPDATE seed_history SET sha256=?, applied_at=? WHERE filename=?;
COMMIT;
```

Both strategies are skipped entirely when the sha256 matches (enforced in G2a). The transaction must be rolled back on any statement error; the `seed_history` row must not be updated if the transaction fails.

Required tests (≥5): `sha256_skip_does_not_execute`, `replace_all_replaces_table_content`, `merge_preserves_unreferenced_rows`, `changed_sha256_reruns_seed`, `missing_seed_table_directive_returns_error`.

**Acceptance criterion:** `cargo test -p anvilml-registry -- seed` exits 0 with ≥5 tests passing.

---

#### P7-G3: anvilml-hardware: replace SEED_ENTRIES with SeedLoader; remove const

- **Prereqs:** P7-G2b
- **Tags:** reasoning

**Files to create or modify:**

- `crates/anvilml-hardware/src/lib.rs` — replace `store.seed(&SEED_ENTRIES).await?` with `anvilml_registry::seed_loader::run(pool, &cfg.seeds_path).await?`.
- `crates/anvilml-hardware/src/device_db.rs` — remove `pub const SEED_ENTRIES` and its entire entry list. Gate `DeviceCapabilityStore::seed()` in `device_store.rs` with `#[cfg(any(test, feature = "seed-util"))]`.
- `crates/anvilml-core/src/config.rs` (or wherever `ServerConfig` is defined) — add `seeds_path: PathBuf` with a default of `executable_dir().join("seeds")`. Add `ANVILML_SEEDS_PATH` env var override and `--seeds-path` CLI flag consistent with existing config resolution precedence.
- `backend/src/main.rs` — no change needed if `detect_all_devices` already receives `&cfg`.

**Key implementation notes:**

`executable_dir()` is `std::env::current_exe()?.parent()`. This means the `backend/seeds/` directory must be co-located with the compiled binary in development (`cargo run` places the binary in `target/debug/` or `target/release/`; seeds must be copied there or the path overridden via config). For development, the config default should fall back to the workspace root `backend/seeds/` if the exe-relative path does not exist. Add this fallback: try `exe_dir/seeds`, else try `CARGO_MANIFEST_DIR/../backend/seeds` (only when compiled with `debug_assertions`).

Update `mod tests` in `lib.rs` that previously called `store.seed(&SEED_ENTRIES)`: copy `backend/seeds/devices.sql` to a `tempfile::TempDir` and pass its path as `seeds_dir` to `detect_all_devices`.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0.

---

#### P7-G4: Fix name resolution priority and --print-hardware display

- **Prereqs:** P7-G3
- **Tags:** —

**Files to create or modify:**

- `crates/anvilml-hardware/src/device_db.rs` — update `resolve_caps_from_row` hit and miss paths.
- `backend/src/main.rs` — update `print_hardware_table` display logic.

**Key implementation notes:**

Define a helper to detect whether a driver-supplied name is generic (not SKU-specific). A name is considered generic if it is empty, equals `"AMD Radeon Graphics"`, equals `"AMD proprietary driver"`, or matches the pattern `"Device {hex}"` (common before `update-pciids`). This list can be extended; keep it as a small `fn is_generic_driver_name(s: &str) -> bool`.

`resolve_caps_from_row` hit path:

```
if dev.name is empty OR is_generic_driver_name(&dev.name):
    dev.name = row.model_name.clone()      // group label becomes primary name
    dev.db_group_name = None               // no redundant braces
else:
    // dev.name is a real Vulkan SKU name — keep it
    dev.db_group_name = Some(row.model_name.clone())
```

`resolve_caps_from_row` miss path:

```
if dev.name is empty OR is_generic_driver_name(&dev.name):
    dev.name = format!("Unknown GPU (0x{:04X}:0x{:04X})", dev.pci_vendor_id, dev.pci_device_id)
// else: keep whatever the enumerator set
```

`print_hardware_table` display:

```
let display_name = match &dev.db_group_name {
    Some(group) if group != &dev.name => format!("{} ({})", dev.name, group),
    _ => dev.name.clone(),
};
```

Add tests: `generic_name_replaced_by_group_label`, `specific_vulkan_name_preserved`, `miss_with_empty_name_shows_unknown`, `miss_with_specific_name_preserved`.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0.

---

## Interfaces and Contracts — append this row

| Contract document | Relevant to tasks | What must match |
|---|---|---|
| `SUPPORTED_DEVICES_DB.md` | P7-F1, P7-F2, P7-F3 | Migration DDL column order, `DeviceCapabilityRow` field order, `SEED_ENTRIES` const block — all must use the canonical order `fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn` |
| `SUPPORTED_DEVICES_DB.md` | P7-G1 | `devices.sql` INSERT column order must match `004_device_capabilities.sql` DDL: `vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn` |

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
- P7-D4 touches `populate_host_info` in `anvilml-hardware`. This function has no unit test covering the OS string value (it calls live sysinfo APIs). The acceptance criterion is verified manually via `--print-hardware`; no automated test addition is required for this task.
- P7-E1 through P7-E3 are sequenced E1→E2→E3 to keep each upgrade atomic and independently revertable. They prereq P7-C1 (not each other's predecessors in the original chain) with the exception that E2 prereqs E1 and E3 prereqs E2, forming a linear sub-chain. Do not reorder.
- P7-E3 (axum 0.8) must not be attempted until P7-E2 is complete and the workspace builds clean. A partial upgrade of axum without tower 0.5 will fail to compile immediately.
- After P7-E3, `tokio-tungstenite` remains pinned at 0.24.x. The axum 0.8 + tower 0.5 environment may support a newer `tokio-tungstenite`; this should be evaluated as a separate follow-on leaf task in a later phase, not folded into P7-E3.
- **G1 table name must match the migration DDL exactly.** The `anvil:seed_table` directive and every `INSERT INTO` target in `devices.sql` must name the same table that `004_device_capabilities.sql` creates. The migration uses `device_capabilities`; if `devices.sql` uses `devices` instead, all inserts will fail at runtime with a "no such table" error. The Forge must read the migration file to verify the table name before writing any INSERT statement.
- **`seed_history` is self-bootstrapped, not migration-managed.** The `SeedLoader` issues `CREATE TABLE IF NOT EXISTS seed_history` itself on every `run` call. Do not add a migration for this table. The tracking table is an implementation detail of the seed infrastructure and must not appear in `_sqlx_migrations`.
- **`sha2` crate must be added to `[workspace.dependencies]` via `mcp-rust-docs`.** Do not pin a version from training data. Version information changes; the MCP lookup is mandatory per `.clinerules §7.7`.
- **`SEED_ENTRIES` removal is a one-way change.** Once removed in G3, the const cannot be restored without re-authoring 126 entries. If the Forge encounters a compile error after removal, the fix is never to restore the const — it is to fix the call site.
- **Development path fallback for `seeds_path`.** In `cargo run` (debug builds), the binary is in `target/debug/` which does not contain a `seeds/` subdirectory. The config default must fall back to the workspace-relative `backend/seeds/` path when `debug_assertions` is enabled and the exe-relative path does not exist. Without this fallback, all local development runs will fail to find the seed file.
- **G4's `is_generic_driver_name` list is intentionally non-exhaustive.** It covers the known cases from hardware testing. New generic driver strings should be added when encountered; they do not require a new task. The miss-path fallback ensures the worst case is always `"Unknown GPU (0x{vendor}:{device})"`, never an empty name column.
- **`DeviceCapabilityStore::seed()` is gated, not removed.** After G3, the method is compiled only under `#[cfg(any(test, feature = "seed-util"))]`. Any test that previously called `store.seed(&SEED_ENTRIES)` must be updated to either use `SeedLoader` with a temp dir or construct `DeviceCapabilityRow` values inline. The Forge must search for all test call sites in G3 and update them.