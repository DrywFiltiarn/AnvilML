# Implementation Report: P18-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-A1                          |
| Phase         | 18 — HTTP/WebSocket Server Completion |
| Description   | anvilml-server: AppState gains hardware, env_report fields (final) |
| Implemented   | 2026-07-11T23:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Added the final two fields per `ANVILML_DESIGN.md §13.2` to `AppState`: `hardware: Arc<RwLock<HardwareInfo>>` and `env_report: Arc<RwLock<EnvReport>>`. Wired both fields to initial populate in `backend/main.rs` — `hardware` from the existing `detect_all_devices()` call (cloned before wrapping), `env_report` from a best-effort preflight check using `config.venv_path`. Updated all 6 existing test files that construct `AppState` directly to include the new fields with synthetic defaults. Added 3 new tests in `state_tests.rs` verifying construction and Arc-sharing semantics. Bumped `anvilml-server` patch version from `0.1.16` to `0.1.17`.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source         |
|--------|-------------------|------------------|----------------|
| crate  | tokio             | 1.47.0 (existing) | rust-docs MCP |
| crate  | anvilml-core      | 0.1.22 (workspace) | rust-docs MCP |

No new external dependencies. `tokio::sync::RwLock` is available via the existing `tokio` dependency with the `sync` feature. `HardwareInfo`, `EnvReport`, `ProvisioningState`, and `HostInfo` are re-exported from `anvilml-core` via `pub use types::*`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Added `hardware` and `env_report` fields to `AppState` struct with doc comments; added `use tokio::sync::RwLock` and `use anvilml_core::{EnvReport, HardwareInfo}` imports |
| Modify | `backend/src/main.rs` | Added imports for `EnvReport`, `ProvisioningState`, `tokio::sync::RwLock`; wrapped `hw_info` in `Arc<RwLock<HardwareInfo>>` after detection (cloned to preserve for `spawn_all`); constructed best-effort `EnvReport` at startup; updated `AppState` construction with both new fields |
| Modify | `crates/anvilml-server/tests/state_tests.rs` | Added imports for `EnvReport`, `HardwareInfo`, `ProvisioningState`, `tokio::sync::RwLock`; updated `make_full_state()` with both new fields; updated `test_app_state_constructs()` with both new fields; added 3 new tests |
| Modify | `crates/anvilml-server/tests/artifacts_tests.rs` | Added imports and synthetic `hardware`/`env_report` fields to `AppState` constructor |
| Modify | `crates/anvilml-server/tests/cors_tests.rs` | Added imports and synthetic `hardware`/`env_report` fields to `AppState` constructor |
| Modify | `crates/anvilml-server/tests/handler_tests.rs` | Added imports and synthetic `hardware`/`env_report` fields to `AppState` constructor |
| Modify | `crates/anvilml-server/tests/health_tests.rs` | Added imports and synthetic `hardware`/`env_report` fields to `AppState` constructor |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Added imports and synthetic `hardware`/`env_report` fields to `AppState` constructor |
| Modify | `crates/anvilml-server/tests/nodes_tests.rs` | Added imports and synthetic `hardware`/`env_report` fields to `AppState` constructor |
| Modify | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.16 → 0.1.17 |
| Modify | `docs/TESTS.md` | Added 3 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P18-A1_plan.md                  | 189 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 backend/src/main.rs                            |  34 +++++
 crates/anvilml-server/Cargo.toml               |   2 +-
 crates/anvilml-server/src/state.rs             |  17 ++-
 crates/anvilml-server/tests/artifacts_tests.rs |  22 ++-
 crates/anvilml-server/tests/cors_tests.rs      |  20 ++-
 crates/anvilml-server/tests/handler_tests.rs   |  22 ++-
 crates/anvilml-server/tests/health_tests.rs    |  20 ++-
 crates/anvilml-server/tests/jobs_tests.rs      |  23 ++-
 crates/anvilml-server/tests/nodes_tests.rs     |  22 ++-
 crates/anvilml-server/tests/state_tests.rs     | 125 +++++++++++++++-
 docs/TESTS.md                                  |  36 +++++
 15 files changed, 534 insertions(+), 19 deletions(-)
```

## Test Results

```
     Running tests/state_tests.rs (target/debug/deps/state_tests-dc6461877e8bd6cc)

running 12 tests
test test_app_state_artifact_store_constructs ... ok
test test_app_state_broadcaster_clone_shares ... ok
test test_app_state_hardware_env_report_clone_shares ... ok
test test_app_state_clone_preserves_all_fields ... ok
test test_app_state_clone_shares_node_registry ... ok
test test_app_state_with_new_fields ... ok
test test_app_state_artifact_store_clone_shares ... ok
test test_app_state_scheduler_arc_sharing ... ok
test test_app_state_hardware_field_constructs ... ok
test test_app_state_env_report_field_constructs ... ok
test test_app_state_broadcaster_constructs ... ok
test test_app_state_constructs ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 288 tests passed, 0 failed.

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.95s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.11s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.91s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.77s
```

All four cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

### Gate 2 — OpenAPI Drift
Not triggered — no handler signatures, `#[utoipa::path]` annotations, or response type changes.

### Gate 3 — Node Parity
Not triggered — no node type additions/renames.

### Gate 4 — Mock/Real Parity Markers
Not triggered — no `execute()`, `load()`, `sample()`, `decode()`, or `compute_latent_shape()` modifications.

## Public API Delta

```
+    pub hardware: Arc<RwLock<HardwareInfo>>,
+    pub env_report: Arc<RwLock<EnvReport>>,
```

Two new `pub` fields on `AppState` in module `anvilml_server::state`:
- `hardware: Arc<RwLock<HardwareInfo>>` — struct field
- `env_report: Arc<RwLock<EnvReport>>` — struct field

No new `pub fn`, `pub struct`, `pub enum`, `pub trait`, or `pub const` items.

## Deviations from Plan

1. **`hw_info` clone in `backend/main.rs`**: The plan stated wrapping `hw_info` directly in `Arc<RwLock<HardwareInfo>>`, but `hw_info` is used again later for `WorkerPool::spawn_all(&hw_info.gpus, ...)`. Resolved by cloning `hw_info` before wrapping: `Arc::new(RwLock::new(hw_info.clone()))`. This is a minimal, zero-cost change (the clone is a shallow copy of the `HardwareInfo` struct).

2. **Additional test files updated**: The plan only named `state_tests.rs`, but 5 additional test files (`artifacts_tests.rs`, `cors_tests.rs`, `handler_tests.rs`, `health_tests.rs`, `jobs_tests.rs`, `nodes_tests.rs`) construct `AppState` directly and required the same two fields. Updated all 6 test files with synthetic defaults matching the established pattern.

## Blockers

None.
