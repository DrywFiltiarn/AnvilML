# Plan Report: P4-C1

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P4-C1                                          |
| Phase       | 004 — Hardware Detection                       |
| Description | anvilml-server: GET /v1/system wired to real HardwareInfo |
| Depends on  | P4-A1, P4-A2, P4-A3, P4-A4, P4-A5, P4-B1      |
| Project     | anvilml                                        |
| Planned at  | 2026-06-15T12:00:00Z                           |
| Attempt     | 1                                               |

## Objective

Wire `GET /v1/system` to return a full `HardwareInfo` snapshot populated by calling
`detect_all_devices()` at server startup. The endpoint returns HTTP 200 with a JSON body
containing the host info, detected GPU devices, and union of inference capabilities. With
the `mock-hardware` feature and `ANVILML_MOCK_DEVICE_TYPE=cuda`, the response contains
at least one GPU entry. The observable proof is:

```bash
ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
```

## Scope

### In Scope
- **`crates/anvilml-server/src/state.rs`** — add `hardware: Arc<RwLock<HardwareInfo>>` field
  to `AppState`, update `new()` constructor (initialise with default `HardwareInfo`).
- **`crates/anvilml-server/src/handlers/system.rs`** — add `get_system` handler returning
  `Json<HardwareInfo>` from `AppState.hardware`.
- **`crates/anvilml-server/src/handlers/mod.rs`** — re-export `get_system`.
- **`crates/anvilml-server/src/lib.rs`** — mount `GET /v1/system` route in `build_router`.
- **`backend/src/main.rs`** — create an in-memory `SqlitePool` placeholder, call
  `detect_all_devices(cfg, &pool)` at startup, populate `AppState.hardware`, log each
  detected device at INFO.
- **`crates/anvilml-server/tests/system_tests.rs`** — add integration test for `/v1/system`.

### Out of Scope
- The `anvilml-registry` `open_in_memory()` function (deferred to Phase 005).
- WebSocket event broadcasting of hardware changes.
- Any changes to `anvilml-hardware` detection logic.
- OpenAPI regeneration (handled separately by Gate 2).

## Existing Codebase Assessment

The codebase has a clean separation between domain types (`anvilml-core`), hardware
detection (`anvilml-hardware`), and HTTP serving (`anvilml-server`). The `HardwareInfo`
type already exists in `crates/anvilml-core/src/types/hardware.rs` with `host`, `gpus`,
and `inference_caps` fields, all derive `Serialize`, `Deserialize`, and `ToSchema`.

The `detect_all_devices()` function in `anvilml-hardware` is fully implemented (Phase 004
Groups A/B completed). It takes `&ServerConfig` and `&SqlitePool`, returning
`Result<HardwareInfo, AnvilError>`. It is already instrumented with
`#[instrument(name = "detect_all_devices", ...)]`. It never panics and always returns at
least one CPU device.

The `AppState` struct currently has `start_time`, `version`, and `env_report` fields.
The `build_router()` function mounts `/health` and `/v1/system/env` routes. Handler
patterns use `axum::extract::State<AppState>` and return `Json<T>`.

The test pattern uses `Router::oneshot` with `tower::util::ServiceExt` to exercise the
full handler pipeline without binding a live TCP listener. Tests are in
`crates/anvilml-server/tests/`.

## Resolved Dependencies

| Type   | Name               | Version verified | MCP source     | Feature flags confirmed |
|--------|--------------------|-----------------|----------------|------------------------|
| crate  | sqlx               | 0.9.0           | Cargo.lock     | runtime-tokio, sqlite, json |
| crate  | tokio              | 1.52.3          | Cargo.lock     | full                   |
| crate  | axum               | 0.8.9           | Cargo.lock     | json, http1, tokio, ws |
| crate  | utoipa             | 5.5.0           | Cargo.lock     | macros, chrono, uuid   |

No new external dependencies are introduced. The task uses `sqlx::SqlitePool::connect`
directly (in-memory pool placeholder), `tokio::sync::RwLock`, `anvilml_hardware::detect_all_devices`,
and `anvilml_core::HardwareInfo` — all already declared in existing manifests.

## Approach

1. **Add `hardware` field to `AppState`** (`crates/anvilml-server/src/state.rs`).
   Add `pub hardware: Arc<RwLock<HardwareInfo>>` to the struct. In `new()`, initialise
   it with `Arc::new(tokio::sync::RwLock::new(HardwareInfo::default()))`. Remove the
   `#[allow(dead_code)]` attribute since the field will now be used by the new handler.
   *Rationale:* `Arc<RwLock<>>` allows the hardware snapshot to be shared across handlers
   and updated independently of request handling, following the established pattern used
   for `env_report` in the design doc.

2. **Add `get_system` handler** (`crates/anvilml-server/src/handlers/system.rs`).
   Implement:
   ```rust
   pub async fn get_system(State(state): State<AppState>) -> Json<HardwareInfo> {
       Json(state.hardware.read().await.clone())
   }
   ```
   This reads the hardware snapshot under a read lock, clones it (cheap — all fields are
   `Clone`), and returns it as JSON. No logging is needed here — the handler is a simple
   data pass-through. The `#[tracing::instrument]` attribute is not applied because this
   is a trivial getter; the mandatory logging occurs at startup in `main.rs`.

3. **Re-export `get_system`** (`crates/anvilml-server/src/handlers/mod.rs`).
   Add `pub use system::get_system;` alongside the existing `pub use system::get_env;`.

4. **Mount the route** (`crates/anvilml-server/src/lib.rs`).
   Add `.route("/v1/system", get(get_system))` to the router chain, between the health
   route and the system/env route. Also add `pub use handlers::system::get_system;` to
   the crate root re-exports.

5. **Call `detect_all_devices` at startup** (`backend/src/main.rs`).
   After loading config (line 47, after `tracing::info!(host = ..., port = ..., "config loaded")`),
   create an in-memory `SqlitePool` placeholder and call `detect_all_devices`:
   ```rust
   // Create an in-memory SQLite pool as a placeholder for the real database
   // connection (Phase 005 will wire the actual pool). This is sufficient
   // because detect_all_devices only needs a pool reference — it does not
   // execute SQL against it in Phase 004.
   let pool = sqlx::SqlitePool::connect("sqlite::memory:")
       .await
       .expect("failed to create in-memory pool for hardware detection");

   // Detect all hardware devices at startup. The pool is a placeholder
   // (in-memory) until Phase 005 wires the real database connection.
   // detect_all_devices never panics and always returns at least one device.
   let hardware_info = detect_all_devices(&cfg, &pool)
       .await
       .expect("hardware detection failed");

   // Log each detected device at INFO level (mandatory log point per
   // ENVIRONMENT.md §9 — Hardware subsystem, "each detected device" event).
   for dev in &hardware_info.gpus {
       tracing::info!(
           index = dev.index,
           name = %dev.name,
           device_type = ?dev.device_type,
           vram_total_mib = dev.vram_total_mib,
           fp8 = dev.caps.fp8,
           "hardware detected"
       );
   }
   ```
   Add the import: `use anvilml_hardware::detect_all_devices;` at the top of main.rs.

6. **Populate `AppState.hardware`** (`backend/src/main.rs`).
   Replace the current `let state = AppState::new(...)` line with:
   ```rust
   let state = AppState::new_with_hardware(
       env!("CARGO_PKG_VERSION"),
       Arc::new(tokio::sync::RwLock::new(hardware_info)),
   );
   ```
   This requires adding a new constructor `new_with_hardware` to `AppState` that accepts
   the hardware `Arc<RwLock<HardwareInfo>>` parameter.

7. **Add `new_with_hardware` constructor** (`crates/anvilml-server/src/state.rs`).
   Add:
   ```rust
   /// Create a new AppState with hardware detection results.
   ///
   /// This constructor is used at server startup after `detect_all_devices()`
   /// has populated the hardware snapshot. The version and hardware data are
   /// stored directly; `env_report` is initialised with default values.
   pub fn new_with_hardware(
       version: impl Into<String>,
       hardware: Arc<tokio::sync::RwLock<HardwareInfo>>,
   ) -> Self {
       Self {
           start_time: std::time::Instant::now(),
           version: version.into(),
           env_report: anvilml_core::types::EnvReport::default(),
           hardware,
       }
   }
   ```
   Add `use std::sync::Arc;` to the imports.

8. **Add integration test** (`crates/anvilml-server/tests/system_tests.rs`).
   Add a new test `test_system_returns_200_with_hardware_info` that:
   - Creates an `AppState` with `new_with_hardware` using a default `HardwareInfo`.
   - Builds the router via `build_router`.
   - Dispatches a GET request to `/v1/system`.
   - Asserts HTTP 200 status.
   - Parses the JSON body and asserts the `gpus` array exists and has at least one entry.

## Public API Surface

| Item | Type | Crate/Module Path | Description |
|------|------|-------------------|-------------|
| `get_system` | `pub async fn` | `anvilml-server/src/handlers/system.rs` | Handler for `GET /v1/system` returning `Json<HardwareInfo>` |
| `AppState::new_with_hardware` | `pub fn` | `anvilml-server/src/state.rs` | Constructor accepting pre-detect `Arc<RwLock<HardwareInfo>>` |
| `AppState::hardware` | `pub field` | `anvilml-server/src/state.rs` | `Arc<RwLock<HardwareInfo>>` — shared hardware snapshot |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `hardware` field and `new_with_hardware` constructor |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Add `get_system` handler |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Re-export `get_system` |
| Modify | `crates/anvilml-server/src/lib.rs` | Mount `GET /v1/system` route |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6 |
| Modify | `backend/src/main.rs` | Call `detect_all_devices` at startup, populate `AppState.hardware`, log devices |
| Modify | `backend/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Add integration test for `/v1/system` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/system_tests.rs` | `test_system_returns_200_with_hardware_info` | GET /v1/system returns 200 with valid HardwareInfo JSON containing a non-empty `gpus` array | AppState constructed with `new_with_hardware` using default HardwareInfo | GET /v1/system request via Router::oneshot | HTTP 200, JSON body with `gpus` array length >= 1 | `cargo test -p anvilml-server --features mock-hardware -- test_system_returns_200_with_hardware_info` exits 0 |

## CI Impact

No CI job behaviour changes. The new handler is a simple data pass-through returning
existing types. The `mock-hardware` feature is already used in all CI builds, so the
integration test runs under the same conditions as existing tests. No new CI jobs are
required.

## Platform Considerations

None identified. The `HardwareInfo` type is platform-agnostic (all fields are serialisable
primitives or `Option<String>`). The `detect_all_devices` function already handles platform
differences internally (Vulkan primary, DXGI fallback on Windows, sysfs fallback on Unix,
CPU always synthesised). The in-memory SQLite pool is platform-neutral.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `detect_all_devices` panics on systems without Vulkan drivers or missing sysinfo data | Low | High | The function is designed to never panic — it returns `Err` for detection failures and the CPU fallback always produces at least one device. The `.expect()` in main.rs is acceptable because a panicking detection is a bug in `anvilml-hardware`, not a recoverable condition at the server level. |
| `AppState::hardware` clone overhead on each request | Low | Low | `HardwareInfo` contains `Vec<GpuDevice>` which clones all device structs. For typical systems with 1-4 GPUs, this is negligible (< 1KB). If this becomes a concern, the handler could return a reference via `Json` of a borrowed value, but that would require changing the handler signature. |
| In-memory pool placeholder leaks connection resources during long-running dev sessions | Low | Low | The pool is created once at startup and dropped when the process exits. For development, this is acceptable. Phase 005 will replace it with the real database pool. |
| `#[tracing::instrument]` on `detect_all_devices` already logs at DEBUG level; adding INFO per-device logging in main.rs may create duplicate log output for the same detection event | Low | Low | The `#[instrument]` macro logs at DEBUG level by default (span creation). The per-device INFO logs in main.rs are a separate, mandatory log point per ENVIRONMENT.md §9. They serve different purposes: the span log shows the overall detection operation, the per-device logs are indexable by operators for hardware inventory. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware -- test_system_returns_200_with_hardware_info` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware &` starts without error, `curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"` succeeds (kills server after test)
