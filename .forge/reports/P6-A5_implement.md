# Implementation Report: P6-A5

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P6-A5                                       |
| Phase         | 006 — Model Registry & Artifacts            |
| Description   | anvilml-registry: DeviceCapabilityStore PCI-ID lookup |
| Implemented   | 2026-06-29T20:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented `DeviceCapabilityStore`, a read-only SQLite-backed query layer over the `device_capabilities` table in `anvilml-registry`. The single `lookup(vendor_id, device_id)` method queries the composite PK, maps INTEGER 0/1 columns to `InferenceCaps::bool` fields, and returns `Ok(None)` for unknown PCI-ID pairs. Created 5 integration tests covering known lookups, unknown lookups, boundary values, integer-to-bool mapping, and multi-row isolation. Updated `lib.rs` to export the new module and re-export the struct. Bumped `anvilml-registry` patch version from 0.1.3 to 0.1.4.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | sqlx    | 0.9.0           | rust-docs MCP |

The `sqlx` version 0.9.0 was confirmed via `rust-docs_get_crate_version` as the latest stable release (2026-05-21, MSRV 1.94.0). No new dependencies are introduced — this task only reads from the existing `SqlitePool`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/device_store.rs` | `DeviceCapabilityStore` struct with `new()` constructor and `lookup()` method; private `DeviceCapsRow` helper struct |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod device_store;` and `pub use device_store::DeviceCapabilityStore;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bumped patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-registry/tests/device_store_tests.rs` | 5 integration tests for `DeviceCapabilityStore::lookup` |
| MODIFY | `docs/TESTS.md` | Added 5 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P6-A5_plan.md                       | 216 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   2 +-
 crates/anvilml-registry/Cargo.toml                 |   2 +-
 crates/anvilml-registry/src/device_store.rs        | 138 +++++++++++++
 crates/anvilml-registry/src/lib.rs                 |   2 +
 .../anvilml-registry/tests/device_store_tests.rs   | 222 +++++++++++++++++++++
 docs/TESTS.md                                      |  60 ++++++
 9 files changed, 650 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/device_store_tests.rs (target/debug/deps/device_store_tests-d9c9e2c03f9198cc)

running 5 tests
test test_lookup_boundary_0xffff ... ok
test test_lookup_unknown_pciid_returns_none ... ok
test test_lookup_integer_to_bool_mapping ... ok
test test_lookup_known_pciid_returns_caps ... ok
test test_lookup_multiple_ids_no_interference ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Full workspace test suite: 125 tests passed, 0 failed (all crates including anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-server, anvilml-scheduler, anvilml-worker, and backend).

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift after pass 3 (in-place reformat).

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.88s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.79s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.15s
```

All four checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  → test tests::config_reference_matches_defaults ... ok
  → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; filtered out
```

Gate 1 passes. No config fields were added/modified by this task.

## Public API Delta

```
+pub mod device_store;
+pub use device_store::DeviceCapabilityStore;
```

New `pub` items in `crates/anvilml-registry/src/device_store.rs`:
- `pub struct DeviceCapabilityStore` — the store struct holding a `SqlitePool`
- `pub fn new(pool: SqlitePool) -> Self` — constructor
- `pub async fn lookup(&self, vendor_id: u16, device_id: u16) -> Result<Option<InferenceCaps>, AnvilError>` — PCI-ID lookup

All match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

- **Test file structure adjustment:** The plan's test approach called `make_pool()` then `DeviceCapabilityStore::new(pool)` before inserting test data. Since `new()` takes ownership of the pool, I reordered the test functions to insert data first, then create the store. This is a necessary structural change — the pool is a non-Copy `Pool<Sqlite>` that cannot be borrowed after move.
- **Clippy dead_code fix:** Added `#[allow(dead_code)]` on `vendor_id` and `device_id` fields of the private `DeviceCapsRow` struct. These fields are required by `sqlx::FromRow` to match the SQL column names, but are not read by `row_to_caps` (which only needs the capability columns). This follows the same pattern as `name` and `arch` fields already annotated in the struct.
- **Removed unused import:** The test file initially imported `InferenceCaps` from `anvilml_core` but only used it indirectly via the `lookup()` return type. Removed the unused import to satisfy clippy's `-D warnings` setting.

## Blockers

None.
