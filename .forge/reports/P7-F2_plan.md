# Plan Report: P7-F2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-F2                                         |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-registry: DeviceCapabilityStore upsert + get + seed |
| Depends on  | P7-F1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-05T11:00:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-registry/src/device_store.rs` implementing `DeviceCapabilityRow` and `DeviceCapabilityStore` with three async methods — `upsert`, `get`, and `seed` — following the exact patterns established in `store.rs`. Re-export both types from `lib.rs`. Add integration tests covering roundtrip, miss, seed count, and bool flags.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/device_store.rs`:
  - `DeviceCapabilityRow` struct with 11 fields in canonical order: `vendor_id: u16, device_id: u16, model_name: String, arch: String, fp32: bool, fp16: bool, bf16: bool, fp8: bool, fp4: bool, nvfp4: bool, flash_attn: bool`
  - `DeviceCapabilityStore { pool }` struct wrapping `SqlitePool`
  - `new(pool)` constructor
  - `async upsert(&row) -> Result<()>` using `INSERT OR REPLACE` with i64 casts for vendor_id/device_id and bool→i64 mapping
  - `async get(vendor_id, device_id) -> Result<Option<DeviceCapabilityRow>>` returning deserialized row or None
  - `async seed(&[DeviceCapabilityRow]) -> Result<u64>` single-transaction upsert returning row count
- Update `crates/anvilml-registry/src/lib.rs` to re-export `DeviceCapabilityRow` and `DeviceCapabilityStore`
- Create `crates/anvilml-registry/tests/device_store.rs` with ≥4 integration tests

### Out of Scope
- Seeding logic invocation (handled by P7-F4)
- `SEED_ENTRIES` const or device_db rewrite (P7-F3)
- Making `detect_all_devices` async or wiring it to the store (P7-F4)
- Any changes to migration 004 (P7-F1)
- CI workflow modifications

## Approach

1. **Read existing patterns.** Study `store.rs` for: tuple row type alias, `sqlx_error` helper function, `sqlx::query_as` with positional column mapping, `INSERT OR REPLACE` syntax, and the `Result<T, AnvilError>` return convention. Read `db.rs` for the shared `sqlx_error` pattern (defined in `store.rs` locally, not re-exported).

2. **Define `DeviceCapabilityRow`.** A plain `pub struct` with 11 fields matching the migration column order exactly. No derives needed beyond what's practical — the task description specifies field types only; follow `ModelRegistry` pattern which deserializes via `query_as` tuple mapping, not serde.

3. **Define row type alias.** Create a private tuple type `DeviceCapabilityRow` mapped from SQL columns: `(i64, i64, String, String, i64, i64, i64, i64, i64, i64, i64)` — matching the 11 INTEGER/TEXT columns in `004_device_capabilities.sql`.

4. **Implement `DeviceCapabilityStore::new`.** Take `SqlitePool` and store it as a field. No async needed.

5. **Implement `upsert`.** Use `sqlx::query("INSERT OR REPLACE INTO device_capabilities (vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")` with `.bind()` for each field. Cast `u16 → i64` via `as i64`, cast bool → `i64` via `if x { 1 } else { 0 }`.

6. **Implement `get`.** Use `sqlx::query_as("SELECT vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn FROM device_capabilities WHERE vendor_id = ? AND device_id = ?")` with `.fetch_optional`. Map tuple fields: `i64 → u16` via `as u16`, bool via `!= 0`.

7. **Implement `seed`.** Begin a transaction via `pool.begin().await`. Loop over entries, calling the same `INSERT OR REPLACE` per entry. Commit and return `entries.len() as u64`. Use `sqlx::query` (not `query_as`) since we don't need to read back.

8. **Update `lib.rs`.** Add `pub mod device_store;` and re-export both types: `pub use device_store::{DeviceCapabilityRow, DeviceCapabilityStore};`.

9. **Write integration tests.** Create `tests/device_store.rs` with 4+ tests following the pattern in `tests/store_get.rs`: open a temp-file pool, exercise the store, assert results.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-registry/src/device_store.rs` | New module: `DeviceCapabilityRow`, `DeviceCapabilityStore`, methods, tests |
| Modify | `crates/anvilml-registry/src/lib.rs` | Add `pub mod device_store;` and re-exports |
| Create | `crates/anvilml-registry/tests/device_store.rs` | Integration tests: roundtrip, miss, seed count, bool flags |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/device_store.rs` | `upsert_then_get_roundtrip` | All 11 fields survive upsert→get cycle |
| `tests/device_store.rs` | `get_miss_returns_none` | Querying a non-existent PCI ID returns `None` |
| `tests/device_store.rs` | `seed_returns_correct_count` | Seeding 3 entries returns count `3` |
| `tests/device_store.rs` | `bool_flags_roundtrip` | Specific bool values (`fp32=true, fp16=false, fp8=true, nvfp4=false`) survive serialization |

## CI Impact

No CI workflow files are modified. The task only adds code within `anvilml-registry`, which is already covered by the existing CI matrix (`cargo test --workspace --features mock-hardware`). No new CI jobs or steps needed.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Field order mismatch between struct, migration DDL, and SQL queries causing silent data corruption | The migration `004_device_capabilities.sql` is already written (P7-F1). Follow its column order exactly: `vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`. Use the same named column list in every SQL statement. |
| i64/u16 boundary casting overflow | Vendor/device IDs are 16-bit PCI IDs (0–65535). The `as u16` cast from `i64` is safe for all valid PCI ID values. Document this assumption in code comments. |
| Bool→i64 mapping inconsistency | Use explicit `if x { 1 } else { 0 }` on write and `value != 0` on read — no reliance on Rust's truthy/falsy coercion. Match the convention used in `store.rs` (`size_bytes as i64`). |
| Transaction rollback on partial seed failure | Using `sqlx::Transaction` ensures atomicity: if any single upsert fails, the entire transaction rolls back and no rows are inserted. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- device_store` exits 0 with ≥4 tests passing
- [ ] `crates/anvilml-registry/src/device_store.rs` contains `DeviceCapabilityRow` struct and `DeviceCapabilityStore` with `new`, `upsert`, `get`, and `seed` methods
- [ ] `lib.rs` re-exports both `DeviceCapabilityRow` and `DeviceCapabilityStore`
- [ ] Field order in struct matches migration column order exactly: `fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`
- [ ] `upsert` uses `INSERT OR REPLACE`, `get` returns `Option<DeviceCapabilityRow>`, `seed` runs in a single transaction and returns count
