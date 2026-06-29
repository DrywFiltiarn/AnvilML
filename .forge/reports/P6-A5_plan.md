# Plan Report: P6-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A5                                       |
| Phase       | 006 — Model Registry & Artifacts            |
| Description | anvilml-registry: DeviceCapabilityStore PCI-ID lookup |
| Depends on  | P6-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T17:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-registry/src/device_store.rs` implementing `DeviceCapabilityStore`, a read-only SQLite-backed query layer over the `device_capabilities` table. The single method `lookup(vendor_id, device_id)` queries the composite PK, maps INTEGER 0/1 columns to `InferenceCaps::bool` fields, and returns `None` (not `Err`) for unknown PCI-ID pairs. This enables `anvilml-hardware`'s future detection orchestration to query pre-spawn capability hints from the same `SqlitePool` used by every other registry operation.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/device_store.rs` with `DeviceCapabilityStore { pool: SqlitePool }` and `lookup(&self, vendor_id: u16, device_id: u16) -> Result<Option<InferenceCaps>, AnvilError>`.
- Map `device_capabilities` table columns (`vendor_id`, `device_id`, `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) to `InferenceCaps` struct fields; boolean columns (INTEGER 0/1) map via `value != 0`.
- Return `Ok(None)` for unknown PCI-ID pairs; never return `Err` for a missing row.
- Return `Err(AnvilError::Db)` only for genuine database errors (connection failure, malformed query).
- Update `crates/anvilml-registry/src/lib.rs` to declare `pub mod device_store;` and `pub use device_store::DeviceCapabilityStore;`.
- Create `crates/anvilml-registry/tests/device_store_tests.rs` with ≥4 integration tests using in-memory SQLite.
- Bump `anvilml-registry` patch version from 0.1.3 to 0.1.4.

### Out of Scope
- No write methods (insert/update/delete) — the table is populated once by the seed loader (P6-A6/P6-A7).
- No fallback row invention — if no PCI-ID match, return `None`; the caller falls through to `CapabilitySource::Fallback`.
- No hardware detection logic — this is a pure query layer; the caller wires it into `detect_all_devices()`.
- No seed data or SQL seed file — that is P6-A8's scope.
- No integration with `anvilml-hardware`'s `DeviceDetector` trait — that is a future phase task.

defers_to (from JSON): [] — this task may not defer any scope.

## Existing Codebase Assessment

**What already exists:** The `device_capabilities` table is already defined in `database/migrations/001_initial.sql` (lines 30–42), with a composite PK `(vendor_id, device_id)`, a unique index `idx_device_capabilities_pci`, and boolean columns stored as `INTEGER NOT NULL DEFAULT 0`. The `InferenceCaps` struct exists in `anvilml-core/src/types/hardware.rs` with six `bool` fields (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) that map one-to-one to the table's boolean columns. `AnvilError::Db` already converts from `sqlx::Error` via `#[from]`, enabling `?` propagation. The `SqlitePool` creation and migration runner exist in `db.rs`. The test helper pattern for in-memory databases with unique UUID cache names is established in `tests/store_tests.rs` (the `make_pool()` function).

**Established patterns:**
- Structs holding a `SqlitePool` are created via a `new(pool: SqlitePool)` constructor (see `ModelStore`).
- `sqlx::query_as!` is used with a private `FromRow`-derived helper struct to map SQL rows to domain types, since sqlx cannot natively map `PathBuf`/`DateTime<Utc>` (not relevant here — all columns are simple types).
- `fetch_optional` returns `Ok(None)` for missing rows — the established pattern for "not found" queries.
- Tests use `#[tokio::test]` async functions with per-test in-memory pools, no `#[serial]` needed.
- `#[tracing::instrument]` is applied to public methods; `tracing::debug!` logs at key decision points.
- All `pub` items have `///` doc comments describing purpose, arguments, and error variants.

**Gap between design doc and current source:** The `device_store.rs` file does not exist yet (confirmed by directory listing). The `lib.rs` currently exports only `db`, `scanner`, and `store` modules. No existing tests for device capabilities queries. The migration and domain types are fully in place and ready to be queried.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sqlx    | 0.9.0           | rust-docs MCP  | sqlite, runtime-tokio, migrate, chrono |

The `sqlx` version 0.9.0 is confirmed as the latest stable release via `rust-docs_get_crate_version`. The features used in this task (`sqlite`, `runtime-tokio`, `migrate`, `chrono`) are already declared in the crate's `Cargo.toml`. The `fetch_optional`, `query_as`, and `FromRow` APIs are standard sqlx 0.9 — confirmed via `rust-docs_get_crate_docs`.

No new dependencies are introduced. This task only reads from the existing `SqlitePool`.

## Approach

### Step 1: Create `crates/anvilml-registry/src/device_store.rs`

Implement the `DeviceCapabilityStore` struct and its `lookup` method.

**Struct definition:**
```rust
pub struct DeviceCapabilityStore {
    pool: SqlitePool,
}
```

**Constructor:**
```rust
impl DeviceCapabilityStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
```

**Helper row struct:** A private `DeviceCapsRow` struct with `sqlx::FromRow` derive to map the SQL columns. This follows the same pattern as `ModelMetaRow` in `store.rs` — a lightweight struct with field names matching the SQL column names:
```rust
#[derive(sqlx::FromRow)]
struct DeviceCapsRow {
    vendor_id: i64,
    device_id: i64,
    fp32: i64,
    fp16: i64,
    bf16: i64,
    fp8: i64,
    fp4: i64,
    flash_attention: i64,
}
```

**Lookup method:**
```rust
pub async fn lookup(
    &self,
    vendor_id: u16,
    device_id: u16,
) -> Result<Option<InferenceCaps>, AnvilError> {
    let row = sqlx::query_as::<_, DeviceCapsRow>(
        "SELECT vendor_id, device_id, fp32, fp16, bf16, fp8, fp4, flash_attention \
         FROM device_capabilities WHERE vendor_id = ? AND device_id = ?",
    )
    .bind(vendor_id as i64)
    .bind(device_id as i64)
    .fetch_optional(&self.pool)
    .await?;

    match row {
        Some(r) => Ok(Some(self.row_to_caps(r))),
        None => Ok(None),
    }
}
```

The `?` operator propagates `sqlx::Error` to `AnvilError::Db` via the existing `#[from]` impl. `fetch_optional` returns `Ok(None)` when no row matches — this is the correct "not found" semantics per the task spec.

**Row-to-struct conversion:**
```rust
fn row_to_caps(&self, row: DeviceCapsRow) -> InferenceCaps {
    InferenceCaps {
        fp32: row.fp32 != 0,
        fp16: row.fp16 != 0,
        bf16: row.bf16 != 0,
        fp8: row.fp8 != 0,
        fp4: row.fp4 != 0,
        flash_attention: row.flash_attention != 0,
    }
}
```

Each boolean column is mapped via `value != 0`, matching the SQLite convention documented in `001_initial.sql` ("All boolean columns use INTEGER 0/1").

**Doc comments and logging:** Every `pub` item gets a `///` doc comment (constructor, lookup). The `lookup` method uses `#[tracing::instrument(fields(vendor_id = %vendor_id, device_id = %device_id), skip(self))]`. A `tracing::debug!` call is included when a row is found.

### Step 2: Update `crates/anvilml-registry/src/lib.rs`

Add two lines after the existing module declarations and re-exports:
```rust
pub mod device_store;
pub use device_store::DeviceCapabilityStore;
```

The file currently has 9 lines. After adding these 2 lines, it will be 11 lines — well within the 80-line hard cap.

### Step 3: Bump crate version

Update `crates/anvilml-registry/Cargo.toml` version from `0.1.3` to `0.1.4`.

### Step 4: Create `crates/anvilml-registry/tests/device_store_tests.rs`

Create the integration test file with ≥4 tests, following the established pattern from `store_tests.rs`:
- Use `make_pool()` helper to create an in-memory SQLite pool with migrations applied.
- Use `DeviceCapabilityStore::new(pool)` to construct the store.
- Manually INSERT device_capabilities rows via `sqlx::query` before calling `lookup`.
- Each test gets its own isolated pool (UUID-based cache name).

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `DeviceCapabilityStore` | `anvilml_registry::device_store` | `pub struct DeviceCapabilityStore { pool: SqlitePool }` |
| `DeviceCapabilityStore::new` | `anvilml_registry::device_store` | `pub fn new(pool: SqlitePool) -> Self` |
| `DeviceCapabilityStore::lookup` | `anvilml_registry::device_store` | `pub async fn lookup(&self, vendor_id: u16, device_id: u16) -> Result<Option<InferenceCaps>, AnvilError>` |

Re-export in `lib.rs`:
```rust
pub use device_store::DeviceCapabilityStore;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/device_store.rs` | `DeviceCapabilityStore` struct with `new()` constructor and `lookup()` method |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod device_store;` and `pub use device_store::DeviceCapabilityStore;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-registry/tests/device_store_tests.rs` | ≥4 integration tests for `DeviceCapabilityStore::lookup` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/device_store_tests.rs` | `test_lookup_known_pciid_returns_caps` | A known PCI-ID pair (vendor=0x10de, device=0x2684) with all-true caps returns `Some(InferenceCaps)` with correct bool values | Row manually inserted into `device_capabilities` | `vendor_id=0x10DE, device_id=0x2684` | `Ok(Some(InferenceCaps { fp32: true, fp16: true, bf16: true, fp8: true, fp4: false, flash_attention: true }))` | `cargo test -p anvilml-registry --test device_store_tests -- test_lookup_known_pciid_returns_caps` exits 0 |
| `tests/device_store_tests.rs` | `test_lookup_unknown_pciid_returns_none` | An unknown PCI-ID pair returns `Ok(None)`, never `Err` | No row inserted for the queried ID | `vendor_id=0xFFFF, device_id=0xFFFF` | `Ok(None)` | `cargo test -p anvilml-registry --test device_store_tests -- test_lookup_unknown_pciid_returns_none` exits 0 |
| `tests/device_store_tests.rs` | `test_lookup_boundary_0xffff` | Boundary value vendor_id=0xFFFF, device_id=0xFFFF is handled correctly (returns `None` since no row exists at that ID) | No row inserted at 0xFFFF/0xFFFF | `vendor_id=0xFFFF, device_id=0xFFFF` | `Ok(None)` | `cargo test -p anvilml-registry --test device_store_tests -- test_lookup_boundary_0xffff` exits 0 |
| `tests/device_store_tests.rs` | `test_lookup_integer_to_bool_mapping` | INTEGER 0/1 columns correctly map to `false`/`true` — verifies the `value != 0` conversion is correct | Row inserted with mixed 0/1 values (fp32=1, fp16=0, bf16=1, fp8=0, fp4=0, flash=1) | `vendor_id=0x1234, device_id=0x5678` | `Ok(Some(InferenceCaps { fp32: true, fp16: false, bf16: true, fp8: false, fp4: false, flash_attention: true }))` | `cargo test -p anvilml-registry --test device_store_tests -- test_lookup_integer_to_bool_mapping` exits 0 |
| `tests/device_store_tests.rs` | `test_lookup_multiple_ids_no_interference` | Inserting multiple rows does not cause cross-contamination — each lookup returns only its own row's caps | Three rows inserted with different PCI-IDs and different cap values | Three lookups: (0x1001, 0x1111), (0x1002, 0x2222), (0x10DE, 0x3333) | Each returns `Some` with its own correct caps | `cargo test -p anvilml-registry --test device_store_tests -- test_lookup_multiple_ids_no_interference` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/device_store_tests.rs` is a standard integration test in `crates/anvilml-registry/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` (the CI's Rust test command). No new file types, gates, or test modules are introduced beyond what the CI already handles.

## Platform Considerations

None identified. The `device_capabilities` table schema and the `lookup` query are platform-neutral — SQLite is used in-memory with no platform-specific paths, file I/O, or `#[cfg]` guards. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::query_as!` column name mismatch — if the helper struct field names don't exactly match the SQL column aliases, compilation fails at build time. | Low | High (build failure) | The column names are taken verbatim from `001_initial.sql` (lines 31–40). The struct fields use identical names. sqlx maps by column name, so this is a compile-time guarantee. |
| In-memory database isolation — multiple tests using `:memory:` without unique cache names share the same database, causing cross-test interference. | Medium | High (flaky tests) | Follow the established `make_pool()` pattern from `store_tests.rs`: use `file:{uuid}?mode=memory&cache=shared` with a unique UUID per test, exactly as `store_tests.rs` does on line 25–31. |
| `bind(vendor_id as i64)` — u16 to i64 cast is safe (u16 max is 65535, well within i64 range), but the table stores `INTEGER` which sqlx maps to i64. | Low | Medium (silent data loss if cast were narrowing) | The cast `u16 as i64` is widening and lossless. Verified: u16 max (65535) < i64::MAX. No issue. |
| Missing `chrono` feature on sqlx — if the `chrono` feature is not enabled, `DateTime<Utc>` operations fail. | Low | High (build failure) | The `chrono` feature is already declared in `Cargo.toml` line 13. Confirmed present. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --test device_store_tests` exits 0 (≥4 tests)
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` outputs ≤80 (80-line cap)
- [ ] `grep "^## " .forge/reports/P6-A5_plan.md` shows all 12 required section headings
