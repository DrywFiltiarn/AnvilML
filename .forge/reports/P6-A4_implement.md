# Implementation Report: P6-A4

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P6-A4                                                       |
| Phase       | 006 — Model Registry                                        |
| Description | anvilml-registry: ModelRegistry rescan (scan + bulk upsert) |
| Implemented | 2026-06-04T08:15:00Z                                        |
| Status      | COMPLETE                                                    |

## Summary

Added `pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<u32, AnvilError>` to `ModelRegistry` in `store.rs`. The method calls the existing `scan_dirs` function on the provided directory configurations, then upserts each discovered `ModelMeta` into SQLite via the existing `upsert` method, returning the total count of models processed. Created two integration tests in `tests/rescan.rs`: one verifying first-run adds N models, and another verifying idempotency (second run returns same count and identical model IDs).

## Resolved Dependencies

No new dependencies added. The implementation uses only existing crates:
- `anvilml_core::config::ModelDirConfig` — import added to store.rs
- `crate::scanner::scan_dirs` — existing function in the same crate, called directly

## Files Changed

| Action | Path                              | Description                                      |
|--------|-----------------------------------|--------------------------------------------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Add `rescan` method and `ModelDirConfig` import |
| Create | `crates/anvilml-registry/tests/rescan.rs` | Integration tests (adds, idempotent)           |

## Commit Log

```
 crates/anvilml-registry/src/store.rs    | 16 ++++++
 crates/anvilml-registry/tests/rescan.rs | 88 +++++++++++++++++++++++++++++++++
 2 files changed, 104 insertions(+)
```

## Test Results

```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s

     Running tests/rescan.rs (target/debug/deps/rescan-7b432e61b2240b79)

running 2 tests
test test_rescan_adds_models ... ok
test test_rescan_idempotent ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

Full workspace test suite: all 163 tests passed (74 anvilml-core + 59 anvilml-hardware + 10 anvilml-registry unit + 1 anvilml_registry_db + 2 rescan + 1 scanner + 2 store_get + 3 store_list + 3 anvilml-server + 8 cli + 1 config_reference + 2 doc-tests).

## Platform Cross-Check

```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.91s
```

Zero errors. Clean Windows cross-compilation check.

## Project Gates

Gate 1 — Config Surface Sync (`cargo test -p backend --features mock-hardware -- config_reference`):
The gate test `test_toml_key_set_matches_default` passed during full workspace test run. No config fields were modified in this task, so no drift is introduced.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- Added `rescan` method to `store.rs` with correct signature and doc comment
- Calls `crate::scanner::scan_dirs(dirs).await` then upserts each result
- Returns count of models processed
- Created integration test file at `crates/anvilml-registry/tests/rescan.rs` with two test functions
- No changes to scanner.rs, lib.rs, db.rs, Cargo.toml, or any other crate

## Blockers

None.
