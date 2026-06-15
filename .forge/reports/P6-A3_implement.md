# Implementation Report: P6-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P6-A3                              |
| Phase         | 006 — Model Registry               |
| Description   | anvilml-registry: DeviceCapabilityStore backed by seed table |
| Implemented   | 2026-06-15T21:15:00+0200           |
| Status        | COMPLETE                           |

## Summary

Implemented `DeviceCapabilityStore` in `crates/anvilml-registry/src/device_store.rs` — a SQLite-backed read-only store for GPU capability metadata. Created `DeviceRow` struct mirroring the `device_capabilities` table schema with 10 fields, and `DeviceCapabilityStore` with `new()` and `get()` methods. Modified `lib.rs` to export the new module and types. Created 4 integration tests in `tests/device_store_tests.rs` covering existing device lookup, not-found case, and boolean flag mapping for all-true and all-false cases. Bumped crate version from 0.1.5 to 0.1.6. Updated `docs/TESTS.md` with entries for all 4 new tests.

## Resolved Dependencies

None. This task uses only existing workspace dependencies: `sqlx` (0.9.0), `tracing`, and `anvilml-core`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/device_store.rs` | `DeviceRow` struct and `DeviceCapabilityStore` with `new()` and `get()` |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod device_store;` and `pub use device_store::{DeviceCapabilityStore, DeviceRow};` |
| CREATE | `crates/anvilml-registry/tests/device_store_tests.rs` | 4 integration tests for `DeviceCapabilityStore` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6 |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for device_store tests |

## Commit Log

```
 .forge/reports/P6-A3_plan.md                       | 178 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   2 +-
 crates/anvilml-registry/Cargo.toml                 |   2 +-
 crates/anvilml-registry/src/device_store.rs        | 133 +++++++++++++++
 crates/anvilml-registry/src/lib.rs                 |   2 +
 crates/anvilml-registry/tests/device_store_tests.rs| 149 +++++++++++++++++
 docs/TESTS.md                                      |  36 +++++
 9 files changed, 510 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/device_store_tests.rs (target/debug/deps/device_store_tests-555f089786880868)

running 4 tests
test test_get_all_caps_false ... ok
test test_get_all_caps_true ... ok
test test_get_existing_device ... ok
test test_get_not_found ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

Full workspace test suite: 86 tests passed, 0 failed across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Checking anvilml-registry v0.1.6 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.7 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.7 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.02s

# 2. Mock-hardware Windows
Checking anvilml-registry v0.1.6 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.7 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.7 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.10s

# 3. Real-hardware Linux
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.7 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.7 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.42s

# 4. Real-hardware Windows
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.7 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml v0.1.7 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s
```

All four checks passed with zero errors.

## Project Gates

### Gate 1 — Config Surface Sync
```
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
```
exit=0 (no diff — openapi.json is up to date)
```

### Gate 3 — Node Parity
Not applicable — task does not touch node types or `node_registry.rs`.

### Python Tests
```
============================= test session starts
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
collected 1 item
worker/tests/test_placeholder.py::test_placeholder PASSED                [100%]
============================= 1 passed in 0.01s
```

## Public API Delta

From `crates/anvilml-registry/src/lib.rs`:
```
+pub mod device_store;
+pub use device_store::{DeviceCapabilityStore, DeviceRow};
```

From `crates/anvilml-registry/src/device_store.rs`:
```
pub struct DeviceRow { ... }
pub struct DeviceCapabilityStore { ... }
impl DeviceCapabilityStore {
    pub async fn new(pool: SqlitePool) -> Self
    pub async fn get(&self, vendor_id: u16, device_id: u16) -> Result<Option<DeviceRow>, AnvilError>
}
```

All new pub items match the plan's Public API Surface table exactly.

## Deviations from Plan

- **Pool cloning in tests**: The plan specified using `open_in_memory()` for each test, but the raw SQL INSERT in the test requires access to the pool after passing it to `DeviceCapabilityStore::new()`. Since `new()` takes ownership of the pool, I cloned the pool before construction (same pattern used in `store_tests.rs::test_delete_existing`). This ensures all operations see the same in-memory database via `max_connections(1)`.
- **No deviations from the plan's In Scope, Files Affected, or Public API Surface sections.**

## Blockers

None.
