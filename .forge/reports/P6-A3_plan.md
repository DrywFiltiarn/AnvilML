# Plan Report: P6-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A3                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-registry: DeviceCapabilityStore backed by seed table |
| Depends on  | P6-A1, P6-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T18:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `DeviceCapabilityStore` in `crates/anvilml-registry/src/device_store.rs`, a SQLite-backed store that reads device capability rows from the `device_capabilities` table (populated by `SeedLoader` from `database/seeds/devices.sql`). The store exposes `pub async fn get(&self, vendor_id: u16, device_id: u16) -> Result<Option<DeviceRow>, AnvilError>`. `DeviceRow` mirrors `InferenceCaps` with the addition of `vendor_id`, `device_id`, `name`, and `arch`. Boolean columns stored as `INTEGER 0/1` in SQLite are mapped to Rust `bool` at the store boundary via `value != 0`. The acceptance criterion is that `cargo test -p anvilml-registry -- device_store` exits 0.

## Scope

### In Scope
- **CREATE** `crates/anvilml-registry/src/device_store.rs` — `DeviceRow` struct and `DeviceCapabilityStore` with `new()` and `get()` methods.
- **MODIFY** `crates/anvilml-registry/src/lib.rs` — add `pub mod device_store;` and `pub use device_store::{DeviceCapabilityStore, DeviceRow};`.
- **CREATE** `crates/anvilml-registry/tests/device_store_tests.rs` — integration tests for `DeviceCapabilityStore`.
- **MODIFY** `crates/anvilml-registry/Cargo.toml` — bump patch version from `0.1.5` to `0.1.6`.
- No migration changes — the `device_capabilities` table already exists in `001_initial.sql`.
- No seed file changes — `devices.sql` already populates the table.

### Out of Scope
- `list()` or `list_by_arch()` methods on `DeviceCapabilityStore` (future task).
- Any HTTP handler or REST endpoint for device capabilities.
- Any modification to `SeedLoader` or `SeedLoader` tests.
- Any changes to `anvilml-core` types.
- Any changes to `anvilml-hardware` device detection.

## Existing Codebase Assessment

The `anvilml-registry` crate already has `ModelStore` (in `store.rs`) as the reference pattern for SQLite-backed stores: it wraps a `SqlitePool`, uses raw `sqlx::query()` (not `query_as!`), applies `#[tracing::instrument]` annotations, and converts SQL rows into domain structs using `row.get::<T, _>("column_name")`. The `device_capabilities` table already exists in the `001_initial.sql` migration with the exact schema needed: `vendor_id INTEGER`, `device_id INTEGER`, `name TEXT`, `arch TEXT`, `fp32 INTEGER`, `fp16 INTEGER`, `bf16 INTEGER`, `fp8 INTEGER`, `fp4 INTEGER`, `flash_attention INTEGER`. Seed data is populated via `SeedLoader` from `database/seeds/devices.sql`. Tests in `tests/store_tests.rs` use `open_in_memory()` from `db.rs`, construct helper types, and assert field-by-field equality. The `lib.rs` currently declares `pub mod db`, `scanner`, `seed_loader`, `store` — it already mentions `device_store` in its crate-level doc comment but does not yet declare the module.

## Resolved Dependencies

No new external dependencies are introduced. This task uses only existing workspace dependencies: `sqlx` (already in `Cargo.toml` with `runtime-tokio`, `sqlite`, `json` features), `anvilml-core` (path dependency), and `tracing` (workspace). The `chrono` dev-dependency is already present for test timestamp assertions.

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | sqlx        | 0.9.0           | workspace      | runtime-tokio, sqlite, json |
| crate  | tracing     | 0.1.44          | workspace      | std, attributes         |

## Approach

1. **Create `crates/anvilml-registry/src/device_store.rs`** with:
   - `DeviceRow` struct: `pub vendor_id: u16`, `pub device_id: u16`, `pub name: String`, `pub arch: String`, `pub fp32: bool`, `pub fp16: bool`, `pub bf16: bool`, `pub fp8: bool`, `pub fp4: bool`, `pub flash_attention: bool`. The boolean fields are mapped from `INTEGER 0/1` at the store boundary via `row.get::<i64, _>("col") != 0`.
   - `DeviceCapabilityStore` struct: `pool: SqlitePool` (same pattern as `ModelStore`).
   - `impl DeviceCapabilityStore`:
     - `pub async fn new(pool: SqlitePool) -> Self` — same signature as `ModelStore::new()`. Performs no I/O.
     - `pub async fn get(&self, vendor_id: u16, device_id: u16) -> Result<Option<DeviceRow>, AnvilError>` — queries `device_capabilities` by primary key `(vendor_id, device_id)`. Returns `Ok(None)` when no row matches (not an error). Uses raw `sqlx::query()` with `.fetch_optional(&self.pool).await?`. Map `INTEGER` columns to `bool` via `!= 0`. Includes `#[tracing::instrument]` on the method.
   - All public items get `///` doc comments following the project convention (describes what it does, arguments, return value).
   - The `get()` method uses `#[tracing::instrument(skip(self), fields(vendor_id, device_id))]` — same pattern as `ModelStore::delete()`.

2. **Modify `crates/anvilml-registry/src/lib.rs`**:
   - Add `pub mod device_store;` after the existing `pub mod` declarations.
   - Add `pub use device_store::{DeviceCapabilityStore, DeviceRow};` to the existing `pub use` block.
   - The crate-level doc comment already mentions `device_store` — no change needed there.

3. **Create `crates/anvilml-registry/tests/device_store_tests.rs`** with at least 4 tests:
   - `test_get_existing_device`: Insert a row via raw SQL (seed data may or may not cover the specific PCI pair), then call `get()` and assert all fields match.
   - `test_get_not_found`: Call `get()` with a vendor/device pair that has no row; assert `Ok(None)`.
   - `test_get_with_all_caps_true`: Insert a row with all boolean flags set to 1, verify they map to `true`.
   - `test_get_with_all_caps_false`: Insert a row with all boolean flags set to 0, verify they map to `false`.
   - Each test creates its own `open_in_memory()` pool for isolation.
   - Tests insert seed data using raw `sqlx::query()` INSERT statements (same pattern used in `seed_loader_tests.rs` for verifying device_capabilities).

4. **Bump `crates/anvilml-registry/Cargo.toml`** version from `0.1.5` to `0.1.6` (patch version only).

## Public API Surface

```rust
// crates/anvilml-registry/src/device_store.rs

/// A single row from the `device_capabilities` table.
///
/// Mirrors `InferenceCaps` from `anvilml-core` with the addition of
/// PCI vendor/device identifiers and architecture string. Boolean
/// fields are mapped from SQLite `INTEGER 0/1` at the store boundary.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub vendor_id: u16,
    pub device_id: u16,
    pub name: String,
    pub arch: String,
    pub fp32: bool,
    pub fp16: bool,
    pub bf16: bool,
    pub fp8: bool,
    pub fp4: bool,
    pub flash_attention: bool,
}

/// Persistent storage for device capability rows backed by SQLite.
///
/// Wraps a `SqlitePool` and provides lookup by PCI vendor/device ID
/// pair. The underlying `device_capabilities` table is populated by
/// `SeedLoader` from `database/seeds/devices.sql`.
pub struct DeviceCapabilityStore {
    pool: SqlitePool,
}

impl DeviceCapabilityStore {
    /// Create a new `DeviceCapabilityStore` backed by the given SQLite connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already been configured with WAL mode
    ///   and has the `device_capabilities` table created (via migrations).
    ///
    /// # Returns
    ///
    /// A new `DeviceCapabilityStore` instance. This constructor performs no I/O.
    pub async fn new(pool: SqlitePool) -> Self;

    /// Look up a device capability row by PCI vendor and device ID.
    ///
    /// # Arguments
    ///
    /// * `vendor_id` — PCI vendor ID (e.g. `0x10de` = 4318 for NVIDIA).
    /// * `device_id` — PCI device ID (e.g. `8994` for H100-SXM5-80GB).
    ///
    /// # Returns
    ///
    /// `Some(DeviceRow)` if a matching row exists, `None` if no row matches
    /// the given vendor/device pair. Returns `AnvilError::Db` only on
    /// query failure (connection lost, schema mismatch, etc.).
    #[tracing::instrument(skip(self), fields(vendor_id, device_id))]
    pub async fn get(&self, vendor_id: u16, device_id: u16) -> Result<Option<DeviceRow>, AnvilError>;
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/device_store.rs` | `DeviceRow` struct and `DeviceCapabilityStore` with `new()` and `get()` |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod device_store;` and `pub use device_store::{...};` |
| CREATE | `crates/anvilml-registry/tests/device_store_tests.rs` | Integration tests for `DeviceCapabilityStore` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/device_store_tests.rs` | `test_get_existing_device` | `get()` returns `Some(DeviceRow)` with correct fields for a known PCI pair | Row exists in `device_capabilities` (inserted via raw SQL) | `vendor_id=4318, device_id=8994` (H100) | `Some(DeviceRow { vendor_id: 4318, device_id: 8994, name: "NVIDIA H100-SXM5-80GB", arch: "9.0", fp32: true, fp16: true, bf16: true, fp8: true, fp4: false, flash_attention: true })` | `cargo test -p anvilml-registry -- device_store` exits 0 |
| `crates/anvilml-registry/tests/device_store_tests.rs` | `test_get_not_found` | `get()` returns `Ok(None)` for a non-existent PCI pair | No row exists for the given PCI pair | `vendor_id=9999, device_id=9999` | `Ok(None)` | same |
| `crates/anvilml-registry/tests/device_store_tests.rs` | `test_get_all_caps_true` | Boolean flags stored as `1` map to `true` | Row with all caps = 1 inserted | `vendor_id=4318, device_id=8994` (H100, all caps true) | All bool fields are `true` | same |
| `crates/anvilml-registry/tests/device_store_tests.rs` | `test_get_all_caps_false` | Boolean flags stored as `0` map to `false` | Row with all caps = 0 inserted | `vendor_id=4318, device_id=6912` (TITAN X Pascal, all caps false) | All bool fields are `false` | same |

## CI Impact

No CI changes required. The new test file follows the existing convention (`crates/{name}/tests/`) and is automatically picked up by `cargo test --workspace --features mock-hardware`. No CI workflow files are modified.

## Platform Considerations

None identified. The SQLite queries and `INTEGER` → `bool` mapping are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::query()` with `.fetch_optional()` may not return a row when the pool has multiple connections and the insert was done on a different connection (SQLite in-memory per-connection isolation). | Medium | High | Use `PoolOptions::new().max_connections(1)` for test pools, same pattern as `store_tests.rs::test_delete_existing`. This ensures all operations in the test see the same in-memory database. |
| Seed data in `devices.sql` may not contain the exact PCI pair used in `test_get_existing_device`, causing a false-negative test. | Medium | Medium | Use raw SQL `INSERT` in the test to guarantee the row exists, rather than relying on seed data. The test should insert a known row and verify the roundtrip. |
| `i64` vs `i32` for SQLite `INTEGER` columns: `row.get::<i64, _>("fp32")` may fail if sqlx resolves the column as `i32`. | Low | Medium | Use `row.get::<i64, _>("fp32")` — sqlx 0.9.0 maps SQLite `INTEGER` to Rust `i64` by default. If clippy or compiler rejects it, fall back to `row.get::<i32, _>("fp32")` and cast to `i64` before comparison. |
| The `device_store.rs` file may grow beyond 400 lines if excessive test helper code is included inline. | Low | Low | Keep test helpers in the test file only. The production code is minimal (~80 lines). The review threshold is not a concern. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- device_store` exits 0
- [ ] `cargo test -p anvilml-registry` exits 0 (full crate test suite)
- [ ] `cargo clippy -p anvilml-registry -- -D warnings` exits 0 (no warnings in the crate)
- [ ] `head -1 .forge/reports/P6-A3_plan.md` prints `# Plan Report: P6-A3`
- [ ] `grep "^## " .forge/reports/P6-A3_plan.md` shows 11 section headings
- [ ] `wc -l .forge/reports/P6-A3_plan.md` reports > 40 lines
