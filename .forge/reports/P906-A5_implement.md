# Implementation Report: P906-A5

| Field       | Value                                                           |
|-------------|-----------------------------------------------------------------|
| Task ID     | P906-A5                                                         |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit                         |
| Description | anvilml-registry: fix stale-model LIKE query fails on Windows paths |
| Implemented | 2026-06-12T18:50:00Z                                            |
| Status      | COMPLETE                                                        |

## Summary

Fixed the stale-model removal logic in `anvilml-registry::ModelRegistry::rescan` so that
path comparison works correctly on Windows, where file paths use backslash separators.
Added a `norm_path` helper that normalises backslashes to forward slashes, applied it at
all four path-handling points in `upsert` and `rescan`, and replaced the two-pass SQL
`LIKE`+`exact` query with a single `SELECT id, path FROM models` followed by Rust-side
filtering using normalised prefix comparison. Bumped `anvilml-registry` patch version
from `0.1.4` to `0.1.5`.

## Resolved Dependencies

No new dependencies added or modified.

## Files Changed

| Action | Path                                  | Description                                      |
|--------|---------------------------------------|--------------------------------------------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Add `norm_path` helper; normalise paths in `upsert` and `rescan`; replace two-pass SQL with single query + Rust filter |
| Modify | `crates/anvilml-registry/Cargo.toml`  | Bump patch version `0.1.4` → `0.1.5`             |

## Commit Log

```
 .forge/reports/P906-A5_plan.md       | 134 +++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 ++--
 Cargo.lock                           |   2 +-
 crates/anvilml-registry/Cargo.toml   |   2 +-
 crates/anvilml-registry/src/store.rs |  52 ++++++-------
 6 files changed, 171 insertions(+), 38 deletions(-)
```

## Test Results

```
     Running tests/rescan_stale.rs (target/debug/deps/rescan_stale-f29d17cdf4bd990e)

running 1 test
test test_rescan_removes_stale_models ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

Full workspace test suite:
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-afc26e32303a2976)
running 76 tests
test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-f7885899b0d95900)
running 56 tests
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-797e121f25ccc9f1)
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-9a19c1c219fdde5c)
running 28 tests
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/patch_meta.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan_stale.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/safetensors_header.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader.rs
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-a52ea907379e090c)
running 43 tests
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-ca830262c7d8be8f)
running 45 tests
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_save.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_serve.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-7f1d61376ac13df1)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-b24276a02da5ea31)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_cancel.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_delete.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_lifecycle.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/preflight_check.rs
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

TOTAL: 276 passed; 0 failed; 0 ignored
```

## Format Gate

```
Not applicable — formatter exited 0 with no drift on pass 2 (`cargo fmt --all -- --check` produced no output).
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Checking anvilml-registry v0.1.5 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.19 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.2 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.14 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.96s

# 2. Mock-hardware Windows cross-check
Checking anvilml-registry v0.1.5 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.19 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.14 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.2 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.15s

# 3. Real-hardware Linux check
Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.19 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.14 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.36s

# 4. Real-hardware Windows cross-check
Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.19 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.14 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.99s
```

All four checks exited 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-7de3ad229d57a5cc)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 — OpenAPI Drift: Not required — no handler signatures or `ToSchema`-derived types were modified by this task.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
