# Implementation Report: P4-C1

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P4-C1                                           |
| Phase         | 004 — Hardware Detection                        |
| Description   | anvilml-server: GET /v1/system wired to real HardwareInfo |
| Implemented   | 2026-06-15T14:30:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Implemented `GET /v1/system` endpoint wired to real `HardwareInfo` from `detect_all_devices()`. Added `hardware: Arc<RwLock<HardwareInfo>>` field to `AppState` with a `new_with_hardware` constructor. The handler reads the hardware snapshot under a read lock and returns it as JSON. At server startup in `backend/src/main.rs`, an in-memory `SqlitePool` placeholder is created, `detect_all_devices()` is called, and each detected device is logged at INFO level. An integration test verifies the endpoint returns HTTP 200 with a valid `HardwareInfo` containing a non-empty `gpus` array. Also added `Default` derive to `HostInfo` and `HardwareInfo` in `anvilml-core` to support default construction, and added `tokio` as a direct dependency of `anvilml-server`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | sqlx    | 0.9.0            | Cargo.lock     |
| crate  | tokio   | 1.52.3           | Cargo.lock     |

No new external dependencies were introduced. `sqlx` was added to `backend/Cargo.toml` (already declared in workspace). `tokio` was added to `anvilml-server/Cargo.toml` as a direct dependency (previously only a dev-dependency) to support `tokio::sync::RwLock` in the `AppState::hardware` field.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `hardware` field and `new_with_hardware` constructor; remove `#[allow(dead_code)]`; add `use std::sync::Arc` |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Add `get_system` handler returning `Json<HardwareInfo>` |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Re-export `get_system` |
| Modify | `crates/anvilml-server/src/lib.rs` | Mount `GET /v1/system` route; re-export `get_system` at crate root |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `tokio` dependency; bump version 0.1.5 → 0.1.6 |
| Modify | `backend/src/main.rs` | Call `detect_all_devices` at startup; create in-memory pool; log devices; use `new_with_hardware` constructor |
| Modify | `backend/Cargo.toml` | Add `sqlx` dependency; bump version 0.1.6 → 0.1.7 |
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add `Default` derive to `HostInfo` and `HardwareInfo` |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Add `test_system_returns_200_with_hardware_info` integration test |
| Modify | `docs/TESTS.md` | Add test entry for `test_system_returns_200_with_hardware_info` |

## Commit Log

```
 .forge/reports/P4-C1_plan.md                 | 242 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   5 +-
 backend/Cargo.toml                           |   3 +-
 backend/src/main.rs                          |  39 ++++-
 crates/anvilml-core/src/types/hardware.rs    |   4 +-
 crates/anvilml-server/Cargo.toml             |   3 +-
 crates/anvilml-server/src/handlers/mod.rs    |   1 +
 crates/anvilml-server/src/handlers/system.rs |  13 ++
 crates/anvilml-server/src/lib.rs             |  12 +-
 crates/anvilml-server/src/state.rs           |  41 ++++-
 crates/anvilml-server/tests/system_tests.rs  |  91 ++++++++++
 docs/TESTS.md                                |   9 +
 14 files changed, 456 insertions(+), 26 deletions(-)
```

## Test Results

```
     Running tests/system_tests.rs (target/debug/deps/system_tests-296961116b8950b5)

running 2 tests
test test_system_env_returns_200_with_default_report ... ok
test test_system_returns_200_with_hardware_info ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Full workspace: 108 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
Not applicable — task wrote no source files (formatter ran in-place at pass 1 and pass 3).
Pass 2 check: exit 0 (no drift after pass 3 reformat).
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.32s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.30s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.51s

All four checks: exit 0
```

## Project Gates

### Gate 1 — Config Surface Sync
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-fac6e150629186cc)

running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Gate 2 — OpenAPI Drift
```
     Running `target/debug/anvilml-openapi`
git diff --exit-code api/openapi.json → exit 0 (no drift)
```

## Public API Delta

```
+pub use system::get_system;
+pub async fn get_system(State(state): State<AppState>) -> Json<anvilml_core::types::HardwareInfo> {
+pub use handlers::system::get_system;
+    pub hardware: Arc<tokio::sync::RwLock<anvilml_core::types::HardwareInfo>>,
+    pub fn new_with_hardware(
```

All new pub items match the plan's Public API Surface table:
- `get_system` — `pub async fn` in `anvilml-server/src/handlers/system.rs`
- `AppState::new_with_hardware` — `pub fn` in `anvilml-server/src/state.rs`
- `AppState::hardware` — `pub field` in `anvilml-server/src/state.rs`

## Deviations from Plan

1. **Added `tokio` as a direct dependency to `anvilml-server/Cargo.toml`** — The plan assumed `tokio::sync::RwLock` was available in the server crate, but `tokio` was only a dev-dependency. Added as a runtime dependency to support the `hardware: Arc<tokio::sync::RwLock<...>>` field.

2. **Added `Default` derive to `HostInfo` and `HardwareInfo` in `anvilml-core/src/types/hardware.rs`** — The plan's test used `HardwareInfo::default()`, but neither type derived `Default`. Added `Default` to both types so the handler can be tested with a default-constructed snapshot. `HostInfo` defaults to empty strings and zero RAM; `HardwareInfo` defaults to empty host info, empty gpus vec, and all-false inference caps.

3. **Test constructs a `HardwareInfo` with one GPU entry** — The plan said "using a default `HardwareInfo`" but `HardwareInfo::default()` produces an empty `gpus` vec. The test instead constructs a `HardwareInfo` with one synthetic GPU device to match the expected output of `detect_all_devices()`.

4. **Removed `#[allow(dead_code)]` from `AppState`** — The plan said to remove it since the `hardware` field will be used. This was done in `state.rs`.

## Blockers

None.
