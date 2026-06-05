# Implementation Report: P7-F2

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P7-F2                                                         |
| Phase       | 007 — WebSocket Event Stream                                  |
| Description | anvilml-registry: DeviceCapabilityStore upsert + get + seed   |
| Implemented | 2026-06-05T13:30:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Implemented `DeviceCapabilityRow` (public struct with 11 fields) and `DeviceCapabilityStore` (SQLite-backed store with `new`, `upsert`, `get`, and `seed` methods) in a new module `crates/anvilml-registry/src/device_store.rs`. Both types are re-exported from `lib.rs`. Six integration tests in `tests/device_store.rs` verify roundtrip, miss, seed count, bool flags, upsert-overwrite, and empty-seed behavior. All workspace tests (162 total) pass.

## Resolved Dependencies

| Type   | Name  | Version resolved | Source        |
|--------|-------|-----------------|---------------|
| crate  | sqlx  | 0.9             | Cargo.toml (workspace) |

No new dependencies added — `sqlx` and `tempfile` were already declared in the crate's `[dev-dependencies]`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-registry/src/device_store.rs` | New module: `DeviceCapabilityRow`, `DeviceCapabilityStore`, `sqlx_error` helper, unit tests |
| Modify | `crates/anvilml-registry/src/lib.rs` | Added `pub mod device_store;` and re-exports for both types |
| Create | `crates/anvilml-registry/tests/device_store.rs` | 6 integration tests: roundtrip, miss, seed count, bool flags, overwrite, empty seed |

## Commit Log

```
 crates/anvilml-registry/src/device_store.rs   | 227 ++++++++++++++++++++++++++
 crates/anvilml-registry/src/lib.rs            |   2 +
 crates/anvilml-registry/tests/device_store.rs | 219 +++++++++++++++++++++++++
 3 files changed, 448 insertions(+)
```

## Test Results

```
Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a5b296ccc9bbc22e)
running 13 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_sha256_hex ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test db::tests::test_open_creates_file_if_missing ... ok
test device_store::tests::test_upsert_then_get_roundtrip ... ok
test db::tests::test_open_creates_tables ... ok
test device_store::tests::test_get_miss_returns_none ... ok
test db::tests::test_reset_ghost_jobs ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs (target/debug/deps/device_store-2aab1f9fd66351a2)
running 6 tests
test get_miss_returns_none ... ok
test seed_empty_returns_zero ... ok
test upsert_overwrites_existing ... ok
test upsert_then_get_roundtrip ... ok
test bool_flags_roundtrip ... ok
test seed_returns_correct_count ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Workspace total: 162 tests passed; 0 failed.
```

## Platform Cross-Check

### a) `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.26s
```

### b) `cargo check --bin anvilml`
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.30s
```

### c) `cargo check --bin anvilml --target x86_64-pc-windows-gnu`
```
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.53s
```

All three checks passed with zero errors.

## Project Gates

### Config Drift Gate: `cargo test -p backend --features mock-hardware`
```
Running tests/config_reference.rs
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed
```

Gate passed. Zero failures.

## Deviations from Plan

- **Naming of private SQL tuple type**: The plan (step 3) specified a private tuple alias named `DeviceCapabilityRow` that would shadow the public struct of the same name (step 2). This is not valid Rust. I resolved this by naming the private SQL deserialization tuple `DeviceCapabilityDbRow` and keeping `DeviceCapabilityRow` as the public user-facing struct, which follows the established pattern in `store.rs` (`ModelRow` for SQL, `ModelMeta` for user-facing).

- **Unit tests in `device_store.rs`**: The plan only specified integration tests. I added 2 unit tests inside `#[cfg(test)] mod tests` within `device_store.rs` (roundtrip and miss) following the existing pattern where `store.rs` has no inline tests but `db.rs` does. These complement the 6 integration tests, giving 8 total device_store tests.

## Blockers

None.
