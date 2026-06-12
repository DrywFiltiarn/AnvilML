# Implementation Report: P906-A1

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P906-A1                                       |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit       |
| Description | anvilml-openapi: add missing ModelKind schema to component registration |
| Implemented | 2026-06-12T16:05:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Added the missing `ModelKind` schema registration to the `anvilml-openapi` generator so that
`backend/openapi.json` no longer contains dangling `$ref` pointers to `#/components/schemas/ModelKind`.
The import was added to the existing `anvilml_core` import block, the schema was registered in the
components builder chain alongside the other config-type enums, and the crate patch version was
bumped from `0.1.1` to `0.1.2`. All build, clippy, format, cross-check, test, and gate checks pass.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source         |
|--------|--------|------------------|----------------|
| crate  | N/A    | N/A              | No new deps    |

No new dependencies were added. `ModelKind` is already re-exported at the `anvilml-core` crate root
(`pub use types::model::ModelKind` in `lib.rs`), so no external lookup was needed.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-openapi/src/main.rs` | Added `ModelKind` to the `anvilml_core` import block; added `.schema("ModelKind", ModelKind::schema())` to the components builder chain after `CapabilitySource` |
| Modify | `crates/anvilml-openapi/Cargo.toml` | Bumped patch version `0.1.1 → 0.1.2` |

## Commit Log

```
 .forge/reports/P906-A1_plan.md              | 75 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |  6 +--
 .forge/state/state.json                     | 13 ++++---
 Cargo.lock                                  |  2 +-
 crates/anvilml-openapi/Cargo.toml           |  2 +-
 crates/anvilml-openapi/src/main.rs          |  4 +-
 6 files changed, 90 insertions(+), 12 deletions(-)
```

## Test Results

```
test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_core)
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_hardware)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_ipc)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_openapi)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_registry)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_registry db test)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (device_store test)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (patch_meta test)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (rescan test)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (rescan_stale test)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (safetensors_header test)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (scanner test)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (seed_loader test)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (store_get test)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (store_list test)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_scheduler)
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_server)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_artifact_save)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_artifact_serve)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_models)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_ws_events)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_worker)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml binary)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_cancel)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_delete)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_ws_lifecycle)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (config_reference)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (preflight_check)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_core)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_hardware)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_ipc)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_registry)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_scheduler)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_server)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml_worker)
```

Total: 262 passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Checking anvilml-openapi v0.1.2 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.48s

# 2. Mock-hardware Windows cross-check
    Checking anvilml-openapi v0.1.2 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.26s

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s

# 4. Real-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
```

All four checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
    Blocking waiting for file lock on artifact directory
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.67s
     Running unittests src/main.rs (target/debug/deps/anvilml-4f2d6d1a888d24b5)
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out
    Running tests/config_reference.rs (target/debug/deps/config_reference-e1f576fae958ffb8)
    running 0 tests
    test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```
(Note: `-- config_reference` filtered all tests in `backend` since the test is in `backend/tests/config_reference.rs` but the filter matched against `config_reference` in the test name; the actual test ran in the full suite above and passed: `test test_toml_key_set_matches_default ... ok`)

### Gate 2 — OpenAPI Drift
Not triggered — this task modifies `anvilml-openapi` schema registration but does not modify handler signatures, `#[utoipa::path]` annotations, or `ToSchema`-derived types in `anvilml-core`. The OpenAPI spec regeneration is owned by P906-A4.

## Deviations from Plan

None. Implementation followed the approved plan exactly.

## Blockers

None.
