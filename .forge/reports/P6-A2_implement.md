# Implementation Report: P6-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P6-A2                              |
| Phase         | 006 — Model Registry               |
| Description   | anvilml-registry: ModelStore SQLite CRUD |
| Implemented   | 2026-06-15T20:45:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented `ModelStore` in `crates/anvilml-registry/src/store.rs` with four CRUD methods (`upsert`, `get`, `list`, `delete`) backed by SQLite. Added `sqlx::FromRow` derive and `Display`/`FromStr` impls to the model enums in `anvilml-core`. Created 7 integration tests covering the full model lifecycle. Fixed a pre-existing SQLite in-memory database per-connection issue in `open_in_memory()` by switching to `PoolOptions::new().max_connections(1)`. Bumped `anvilml-registry` version from 0.1.4 to 0.1.5.

## Resolved Dependencies

| Type   | Name         | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| crate  | sqlx        | 0.9.0            | Cargo.lock     |
| crate  | chrono      | 0.4.45           | Cargo.lock     |

**Notes:**
- `sqlx` already declared in workspace. Added `chrono` feature to `anvilml-core` and `anvilml-registry` Cargo.toml to enable `DateTime<Utc>` support in `FromRow` derive.
- `chrono` already declared in workspace. No new dependency introduced.
- `Display` and `FromStr` impls added to `ModelKind`, `ModelDtype`, `ModelFormat` in `anvilml-core` to enable string serialization/deserialization for SQLite TEXT columns.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/store.rs` | ModelStore struct with upsert/get/list/delete CRUD methods |
| CREATE | `crates/anvilml-registry/tests/store_tests.rs` | 7 integration tests for ModelStore |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod store;` and `pub use store::ModelStore;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bumped version 0.1.4 → 0.1.5; added `chrono` feature to sqlx |
| MODIFY | `crates/anvilml-core/src/types/model.rs` | Added `sqlx::FromRow` derive to ModelMeta; added Display/FromStr impls to enums |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `chrono` feature to sqlx in [dependencies] and [dev-dependencies] |
| MODIFY | `crates/anvilml-registry/src/db.rs` | Fixed `open_in_memory()` to use `PoolOptions::new().max_connections(1)` for SQLite in-memory DB isolation |
| MODIFY | `docs/TESTS.md` | Added 7 test entries for store_tests.rs |

## Commit Log

```
 .forge/reports/P6-A2_plan.md                 | 153 ++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   6 +-
 crates/anvilml-core/Cargo.toml               |   4 +-
 crates/anvilml-core/src/types/model.rs       | 100 ++++++++++-
 crates/anvilml-registry/Cargo.toml           |   4 +-
 crates/anvilml-registry/src/db.rs            |  13 +-
 crates/anvilml-registry/src/lib.rs           |   2 +
 crates/anvilml-registry/src/store.rs         | 244 +++++++++++++++++++++++++
 crates/anvilml-registry/tests/store_tests.rs | 256 +++++++++++++++++++++++++++
 docs/TESTS.md                                |  63 +++++++
 12 files changed, 845 insertions(+), 19 deletions(-)
```

## Test Results

```
     Running tests/store_tests.rs (target/debug/deps/store_tests-be705d0695584b1c)

running 7 tests
test test_delete_not_found ... ok
test test_get_not_found ... ok
test test_delete_existing ... ok
test test_list_all ... ok
test test_upsert_and_get ... ok
test test_upsert_overwrites ... ok
test test_list_filter_by_kind ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

Full workspace test suite: 120 tests passed, 0 failed.

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (exercises #[cfg(unix)] scaffold and mock paths)
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.51s

# 2. Mock-hardware Windows (exercises #[cfg(windows)] code paths)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.07s

# 3. Real-hardware Linux (exercises real Vulkan/sysfs paths, no mock)
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s

# 4. Real-hardware Windows (exercises real DXGI/NVML paths on Windows target)
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.60s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync

```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes.

## Public API Delta

```
+pub mod store;
+pub use store::ModelStore;
```

New pub items:
- `pub mod store` — module path: `anvilml_registry::store`
- `pub use store::ModelStore` — re-export at crate root
- `pub struct ModelStore` — module path: `anvilml_registry::store::ModelStore`
- `pub async fn new(pool: SqlitePool) -> Self` — module path: `anvilml_registry::store::ModelStore`
- `pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>` — module path: `anvilml_registry::store::ModelStore`
- `pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` — module path: `anvilml_registry::store::ModelStore`
- `pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>` — module path: `anvilml_registry::store::ModelStore`
- `pub async fn delete(&self, id: &str) -> Result<bool, AnvilError>` — module path: `anvilml_registry::store::ModelStore`

Additionally in `anvilml-core`:
- `impl fmt::Display for ModelKind` — new impl (not a pub item per se, but enables `to_string()` on the enum)
- `impl FromStr for ModelKind` — new impl
- `impl fmt::Display for ModelDtype` — new impl
- `impl FromStr for ModelDtype` — new impl
- `impl fmt::Display for ModelFormat` — new impl
- `impl FromStr for ModelFormat` — new impl

## Deviations from Plan

1. **`query_as!` replaced with `query()` for upsert**: The plan specified using `sqlx::query_as!` for the upsert method. However, `query_as!` requires `DATABASE_URL` to be set for online mode (to validate SQL at compile time). Since the project does not use `DATABASE_URL`, I used `sqlx::query()` with manual binding instead. This is a compatible substitution — both approaches execute the same SQL.

2. **Enum fields read as `String` then parsed via `FromStr`**: The plan specified using `row.get()` directly with `ModelKind`, `ModelDtype`, and `ModelFormat`. However, these enums do not implement `sqlx::Type<Sqlite>` or `sqlx::Decode<'_, Sqlite>`, so direct `row.get()` fails. I added `FromStr` impls to each enum and read the fields as `String` then parsed them. This is a necessary adaptation to the actual type system.

3. **`Display` and `FromStr` impls added to enums**: The plan only specified adding `FromRow` to `ModelMeta`. However, to support string serialization/deserialization for SQLite TEXT columns, I added `Display` and `FromStr` impls to `ModelKind`, `ModelDtype`, and `ModelFormat` in `anvilml-core`. This is a minimal extension of the plan.

4. **`open_in_memory()` fixed to use `max_connections(1)`**: The original `open_in_memory()` used `SqlitePool::connect("sqlite::memory:")` which creates a separate in-memory database per connection. With a pool of multiple connections, operations on different connections would see different databases. I changed it to `PoolOptions::new().max_connections(1).connect("sqlite::memory:")` to ensure a single connection is shared. This is a fix for a pre-existing bug that would have caused the store tests to fail.

5. **Transactions added to `upsert` and `delete`**: To ensure SQLite writes are committed before the connection is returned to the pool, both `upsert` and `delete` use explicit transactions (`begin()` → execute → `commit()`). This prevents a race condition where the connection might be returned to the pool before the implicit transaction commits.

## Blockers

None.
