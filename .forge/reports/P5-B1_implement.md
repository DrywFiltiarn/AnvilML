# Implementation Report: P5-B1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P5-B1                              |
| Phase         | 005 — SQLite Persistence           |
| Description   | backend: SqlitePool in AppState, real DB wired in main.rs |
| Implemented   | 2026-06-15T18:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Wired a real file-backed SQLite database into the AnvilML server lifecycle. Added `db: SqlitePool` as a new `pub` field on `AppState` so all HTTP handlers can access the persistent database. In `backend/src/main.rs`, replaced the in-memory placeholder pool with `anvilml_registry::open(&cfg.db_path)`, added the mandatory `tracing::info!(path = %cfg.db_path.display(), "database opened")` log line, and passed the real pool to both `detect_all_devices()` and `AppState::new_with_hardware()`. Updated `AppState::new_with_hardware` to accept a `SqlitePool` parameter. Updated all test files that construct `AppState` to pass a pool. Bumped `anvilml-server` patch version from `0.1.6` to `0.1.7`.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source        |
|--------|-------------------|------------------|---------------|
| crate  | sqlx              | 0.9.0            | Cargo.lock    |
| crate  | anvilml-registry  | 0.1.3            | Cargo.toml    |

Note: The `rust-docs` MCP tool was unavailable; versions confirmed from `Cargo.lock` and workspace `Cargo.toml`. The `SqlitePool::connect()` API and `anvilml_registry::open()` function are confirmed to exist in sqlx 0.9.0 from existing source code usage.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `pub db: sqlx::SqlitePool` field to `AppState`; update `new_with_hardware` to accept `db: sqlx::SqlitePool`; change `new()` to async to support async pool construction |
| MODIFY | `backend/src/main.rs` | Add `use anvilml_registry::open`; replace in-memory pool with `open(&cfg.db_path).await`; add `tracing::info!(path = %cfg.db_path.display(), "database opened")`; pass real pool to `detect_all_devices()` and `AppState::new_with_hardware()` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump version `0.1.6 → 0.1.7`; add `sqlx = { workspace = true }` to `[dependencies]`; add `anvilml-registry = { path = "../anvilml-registry" }` to `[dev-dependencies]` |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Add `use anvilml_registry::open_in_memory`; update `test_system_returns_200_with_hardware_info` to open in-memory pool and pass it to `new_with_hardware` |
| MODIFY | `crates/anvilml-server/tests/health_tests.rs` | Update `test_health_returns_200_with_status_key` to await `AppState::new()` |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Convert `#[test]` to `#[tokio::test]` and add `.await` to all `AppState::new()` calls (3 tests) |

## Commit Log

```
 .forge/reports/P5-B1_plan.md                | 128 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +--
 Cargo.lock                                  |   4 +-
 backend/src/main.rs                         |  30 ++++---
 crates/anvilml-server/Cargo.toml            |   4 +-
 crates/anvilml-server/src/state.rs          |  35 ++++++--
 crates/anvilml-server/tests/health_tests.rs |   2 +-
 crates/anvilml-server/tests/state_tests.rs  |  18 ++--
 crates/anvilml-server/tests/system_tests.rs |  10 ++-
 10 files changed, 210 insertions(+), 40 deletions(-)
```

## Test Results

```
     Running tests/health_tests.rs (target/debug/deps/health_tests-b48a8a267356dc62)

running 1 test
test test_health_returns_200_with_status_key ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/state_tests.rs (target/debug/deps/state_tests-50143da268107f6d)

running 3 tests
test test_app_state_clone ... ok
test test_app_state_version_from_env ... ok
test test_app_state_new ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/system_tests.rs (target/debug/deps/system_tests-3584858a30b51564)

running 2 tests
test test_system_env_returns_200_with_default_report ... ok
test test_system_returns_200_with_hardware_info ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/config_reference.rs (target/debug/deps/config_reference-971f19e0b138588f)

running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

(Full workspace: 101 tests passed, 0 failed)
```

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (exercises #[cfg(unix)] scaffold and mock paths)
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s

# 2. Mock-hardware Windows (exercises #[cfg(windows)] code paths)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.78s

# 3. Real-hardware Linux (exercises real Vulkan/sysfs paths, no mock)
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.85s

# 4. Real-hardware Windows (exercises real DXGI/NVML paths on Windows target)
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.92s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
    Running tests/config_reference.rs (target/debug/deps/config_reference-971f19e0b138588f)
    running 1 test
    test config_reference ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Gate 2 — OpenAPI Drift
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.40s
    (no diff — exit 0)
```

## Public API Delta

```
+    pub db: sqlx::SqlitePool,
+    pub async fn new(version: impl Into<String>) -> Self {
```

New pub items introduced:
- `AppState::db` — `pub field` of type `sqlx::SqlitePool` in `anvilml_server::state::AppState`
- `AppState::new` — `pub async fn` (signature changed from sync to async to support async pool construction)
- `AppState::new_with_hardware` — `pub fn` (signature updated: added `db: sqlx::SqlitePool` parameter)

## Deviations from Plan

- **`AppState::new()` changed from sync to async**: The approved plan did not specify changing `new()` to async. However, the `AppState` struct now requires a `SqlitePool` field, and `SqlitePool::connect()` is async. Synchronous pool creation is not possible with sqlx 0.9.0 without creating a nested tokio runtime, which panics when called from within an existing runtime. Making `new()` async is the cleanest solution and only affects test code (production code always uses `new_with_hardware`). All test files that call `AppState::new()` have been updated accordingly.
- **Added `sqlx` to `anvilml-server/Cargo.toml`**: The approved plan stated no Cargo.toml changes were needed for `anvilml-server`, but `sqlx` is required as a direct dependency because the `AppState` struct's `db` field uses `sqlx::SqlitePool` type directly. The transitive dependency through `anvilml-registry` would not make the type available in the `state.rs` module scope.
- **Added `anvilml-registry` to dev-dependencies**: Added to `anvilml-server/Cargo.toml` as a dev-dependency so that test code can use `open_in_memory()` for constructing test pools.

## Blockers

None.
