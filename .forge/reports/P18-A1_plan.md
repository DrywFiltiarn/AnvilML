# Plan Report: P18-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-A1                                      |
| Phase       | 18 — HTTP/WebSocket Server Completion       |
| Description | anvilml-server: AppState gains hardware, env_report fields (final) |
| Depends on  | P17-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-11T19:46:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `AppState`'s incremental field build-up (started in Phase 11, continued through Phases 14, 15, and 16) by adding its final two fields per `ANVILML_DESIGN.md §13.2`: `hardware: Arc<RwLock<HardwareInfo>>` and `env_report: Arc<RwLock<EnvReport>>`. Wire both fields to their initial populate in `backend/main.rs` — `hardware` from `detect_all_devices()` (already called at startup), `env_report` from a best-effort preflight check at startup. After this task, `AppState` holds every field `§13.2` specifies, and the acceptance tests confirm construction, Arc-sharing semantics, and correct initial values for both new fields.

## Scope

### In Scope
- Add `hardware: Arc<RwLock<HardwareInfo>>` and `env_report: Arc<RwLock<EnvReport>>` fields to `AppState` in `crates/anvilml-server/src/state.rs`.
- Update `make_full_state()` test helper in `crates/anvilml-server/tests/state_tests.rs` to construct both new fields.
- Add `test_app_state_hardware_field_constructs()` — verifies `hardware` is present, contains at least one device, and the `Arc<RwLock<...>>` pointer is valid.
- Add `test_app_state_env_report_field_constructs()` — verifies `env_report` is present, `preflight_ok` is `false` (best-effort, no torch), and the `Arc<RwLock<...>>` pointer is valid.
- Add `test_app_state_hardware_env_report_clone_shares()` — verifies both new fields' `Arc` pointers are shared between original and cloned `AppState`.
- Update `backend/main.rs` to populate both fields at startup: `hardware` from the existing `detect_all_devices()` call, `env_report` from a best-effort preflight check (interpreter path from config, python version check, torch import attempt).
- Bump `anvilml-server` crate version from `0.1.16` to `0.1.17`.

### Out of Scope
- No scope is deferred. `defers_to (from JSON): absent` — this task implements its full scope.
- The `/v1/system` and `/v1/system/env` HTTP handlers that *read* these fields are handled by P18-B1.
- Ongoing VRAM refresh during dispatch (separate concern from the initial populate).
- Full preflight subsystem (this task wires best-effort initial populate only).
- Worker lifecycle management changes.

## Existing Codebase Assessment

**What already exists:** `AppState` currently has 8 fields (`config`, `node_registry`, `start_time`, `scheduler`, `workers`, `db`, `artifact_store`, `broadcaster`) in `crates/anvilml-server/src/state.rs`. `HardwareInfo` and `EnvReport` already exist in `anvilml-core` (types/hardware.rs and types/worker.rs respectively) and are re-exported via `pub use types::*`. `detect_all_devices(&ServerConfig)` is already called in `backend/main.rs` at line 174 to populate `hw_info`, which is logged but not stored in `AppState`. The `anvilml-server` crate already depends on `anvilml-hardware` (Cargo.toml line 14) and has `tokio` with the `sync` feature enabled (line 17), so `tokio::sync::RwLock` is available without new dependencies. The `Arc<RwLock<T>>` pattern is established in the codebase — `WorkerHandle::status` uses `Arc<RwLock<WorkerStatus>>` in `anvilml-worker/src/managed.rs`.

**Established patterns:** All `AppState` fields are `pub` and documented with `///` doc comments. Tests use `make_full_state()` as the construction helper, and `Arc::as_ptr()` pointer comparison to verify sharing. The `state_tests.rs` file uses `#[tokio::test]` for async tests and a sync helper `create_test_pool_sync()` for synchronous pool creation.

**Gap between design doc and current source:** The design doc §13.2 shows `AppState` with 10 fields including `registry: Arc<ModelRegistry>`, but the current codebase has `node_registry: Arc<NodeTypeRegistry>` instead (which is the correct runtime-populated variant). The `start_time` field exists in the current code but not in the design doc §13.2 snippet. This gap does not affect this task — we add `hardware` and `env_report` alongside the existing fields, matching the actual current shape.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | tokio             | 1.47.0          | rust-docs MCP| sync (already present in Cargo.toml) |
| crate  | anvilml-core      | 0.1.x (workspace) | rust-docs MCP | n/a (path dependency) |
| crate  | anvilml-hardware  | 0.1.x (workspace) | rust-docs MCP | n/a (path dependency, mock-hardware forwarded) |

No new external dependencies are introduced. `tokio::sync::RwLock` is available via the existing `tokio` dependency with the `sync` feature. `HardwareInfo` and `EnvReport` are re-exported from `anvilml-core` via `pub use types::*`. `detect_all_devices()` signature is confirmed as `pub async fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` (only takes `&ServerConfig`, not `&SqlitePool` as the design doc §6.4 suggests — the actual code is authoritative).

## Approach

1. **Modify `crates/anvilml-server/src/state.rs`:**
   - Add `use tokio::sync::RwLock;` import.
   - Add `use anvilml_core::{EnvReport, HardwareInfo};` import.
   - Add two new fields to `AppState` after `broadcaster`:
     ```rust
     /// Snapshot of the host machine's hardware (GPU/CPU devices and capabilities).
     ///
     /// Populated once at server startup via `detect_all_devices()`. The `RwLock`
     /// allows the scheduler to read the snapshot during dispatch while a future
     /// VRAM-refresh path can update it without reconstructing the entire struct.
     pub hardware: Arc<RwLock<HardwareInfo>>,

     /// Python environment health report collected at startup preflight.
     ///
     /// Contains the interpreter path, Python version, torch availability, and
     /// provisioning status. Best-effort initial populate at startup — a full
     /// preflight subsystem is a later concern.
     pub env_report: Arc<RwLock<EnvReport>>,
     ```
   - These are the last two fields `§13.2` specifies. After this change, `AppState` has 10 fields.

2. **Modify `backend/main.rs`:**
   - After the existing `detect_all_devices()` call (line 174) produces `hw_info`, wrap it in `Arc<RwLock<HardwareInfo>>` and store it in a local variable:
     ```rust
     let hardware = Arc::new(RwLock::new(hw_info));
     ```
   - Add a best-effort `EnvReport` construction before `AppState` construction. Use `ServerConfig.venv_path` for `python_path`, attempt to extract Python version from the interpreter, set `torch_version: None` (no torch import in Rust), `preflight_ok: false` (best-effort, no full preflight), `provisioning: ProvisioningState::NotStarted`, `reason: None`, `node_types: Vec::new()`:
     ```rust
     // Best-effort initial EnvReport at startup.
     // A full preflight subsystem is a later concern — this just captures
     // the interpreter path and a conservative preflight status.
     // torch_version is None because Rust cannot import Python modules;
     // the Python worker will populate this on its Ready event later.
     let env_report = Arc::new(RwLock::new(EnvReport {
         python_path: Some(config.venv_path.join("bin/python3").to_string_lossy().into_owned()),
         python_version: None, // Will be filled by worker Ready event
         torch_version: None,
         provisioning: ProvisioningState::NotStarted,
         preflight_ok: false,
         reason: None,
         node_types: Vec::new(),
     }));
     ```
   - Update the `AppState` construction (line 294) to include both new fields:
     ```rust
     let app_state = AppState {
         config: Arc::new(config),
         node_registry,
         start_time,
         scheduler,
         workers: Arc::clone(&workers),
         db: pool,
         artifact_store,
         broadcaster: Arc::clone(&broadcaster),
         hardware,
         env_report,
     };
     ```

3. **Modify `crates/anvilml-server/tests/state_tests.rs`:**
   - Add imports: `use anvilml_core::{EnvReport, HardwareInfo, ProvisioningState};` and `use tokio::sync::RwLock;`.
   - Update `make_full_state()` to accept and include the two new fields. The helper already constructs all 8 existing fields; add the two new ones:
     ```rust
     hardware: Arc::new(RwLock::new(HardwareInfo {
         host: anvilml_core::HostInfo {
             hostname: "test-host".to_string(),
             os: "Linux".to_string(),
         },
         gpus: vec![],
         inference_caps: anvilml_core::InferenceCaps::default(),
     })),
     env_report: Arc::new(RwLock::new(EnvReport {
         python_path: Some("./worker/.venv/bin/python3".to_string()),
         python_version: None,
         torch_version: None,
         provisioning: ProvisioningState::NotStarted,
         preflight_ok: false,
         reason: None,
         node_types: Vec::new(),
     })),
     ```
   - Add three new tests (see Tests section below).

4. **Bump `anvilml-server` crate version** from `0.1.16` to `0.1.17` in `crates/anvilml-server/Cargo.toml`.

5. **Verify:** Run `cargo test -p anvilml-server --test state_tests` and `cargo build -p anvilml` — both must exit 0.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| Field | `anvilml_server::AppState::hardware` | `pub hardware: Arc<RwLock<HardwareInfo>>` — new field on existing struct |
| Field | `anvilml_server::AppState::env_report` | `pub env_report: Arc<RwLock<EnvReport>>` — new field on existing struct |

No new `pub fn`, `pub struct`, `pub enum`, or `pub trait` items are introduced. This task only adds fields to an existing struct.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `hardware` and `env_report` fields to `AppState` |
| Modify | `backend/src/main.rs` | Populate both fields at startup (hardware from detect_all_devices, env_report from best-effort preflight) |
| Modify | `crates/anvilml-server/tests/state_tests.rs` | Update `make_full_state()` helper; add 3 new tests for both fields |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.16 → 0.1.17 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_hardware_field_constructs()` | `hardware` field is present, contains a valid `HardwareInfo` with at least one host entry, `Arc<RwLock<...>>` pointer is valid | `cargo test -p anvilml-server --test state_tests -- hardware_field_constructs` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_env_report_field_constructs()` | `env_report` field is present, `preflight_ok` is `false` (best-effort), `Arc<RwLock<...>>` pointer is valid | `cargo test -p anvilml-server --test state_tests -- env_report_field_constructs` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_hardware_env_report_clone_shares()` | Both `hardware` and `env_report` `Arc` pointers are identical between original and cloned `AppState` (pointer comparison via `std::ptr::eq`) | `cargo test -p anvilml-server --test state_tests -- clone_shares` exits 0 |

## CI Impact

No CI changes required. The task only adds fields to `AppState` and updates the test helper — no new file types, no new gates, and no changes to CI workflow files. The existing `rust-linux` and `rust-windows` CI jobs will pick up the new tests automatically via `cargo test --workspace --features mock-hardware`. The config-drift gate (Gate 1) does not trigger because `AppState` is not a config struct.

## Platform Considerations

None identified. The `Arc<RwLock<T>>` pattern is platform-neutral. `tokio::sync::RwLock` uses the same implementation on Linux and Windows. The `EnvReport` construction uses `PathBuf::to_string_lossy()` which handles platform-specific path separators correctly. The Windows cross-check in `ENVIRONMENT.md §7` (check 4: `cargo check --bin anvilml --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `detect_all_devices()` signature differs from design doc (§6.4 claims `&SqlitePool` param, actual code takes only `&ServerConfig`) | Low | Medium | The actual code in `anvilml-hardware/src/detect.rs` line 83 is authoritative: `pub async fn detect_all_devices(cfg: &ServerConfig)`. The plan uses the actual signature. If the actual signature changes before ACT, the plan will fail to compile and the ACT agent will discover this immediately. |
| `make_full_state()` test helper in `state_tests.rs` is used by 6 existing tests — modifying it could break those tests | Medium | Medium | The helper already constructs all 8 existing fields; adding 2 more with synthetic defaults that match the existing pattern (e.g. empty `gpus` vec, default `InferenceCaps`) is additive and does not change existing behavior. All existing tests will pass as long as the new fields compile and accept the synthetic values. |
| `EnvReport` construction in `backend/main.rs` needs `HostInfo` for `HardwareInfo` — but `EnvReport` has no `host` field, only `HardwareInfo` does | Low | Low | `HardwareInfo` requires `HostInfo` (which has `hostname` and `os` fields). The test helper synthesises these; `backend/main.rs` uses `sysinfo::System::new_all()` or `hostname::get()` at runtime for the real value. The design doc §5.5 defines `HostInfo` with `hostname: String, os: String`. The ACT agent will use a simple `sysinfo` call (already a dependency of `anvilml-server`) to get the hostname. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test state_tests` exits 0 (all 11 tests: 8 existing + 3 new)
- [ ] `cargo build -p anvilml` exits 0 (full workspace build with new fields)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
