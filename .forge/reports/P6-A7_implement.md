# Implementation Report: P6-A7

| Field       | Value                                              |
|-------------|----------------------------------------------------|
| Task ID     | P6-A7                                                |
| Phase       | 006 — Model Registry                                |
| Description | anvilml-server: GET /v1/models/:id and POST /v1/models/rescan |
| Implemented | 2026-06-04T11:30:00Z                                |
| Status      | COMPLETE                                             |

## Summary

Implemented two new HTTP API endpoints for the anvilml-server: `GET /v1/models/:id` returns a single model's metadata (200) or a 404 error JSON body, and `POST /v1/models/rescan` triggers a non-blocking background rescan of configured model directories (202 Accepted). Added `model_dirs: Vec<ModelDirConfig>` field to `AppState` so the rescan handler can access configured directories, wired both routes in the router, and updated `backend/src/main.rs` to pass model directories when constructing `AppState`. Added two inline unit tests verifying 404 and 202 responses.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source        |
|--------|----------|-----------------|---------------|
| crate  | tracing  | 0.1             | Lockfile (project already uses tracing) |
| crate  | serde_json | 1             | Lockfile (already in dev-deps, promoted to deps) |

Note: No new crates were added. `tracing` and `serde_json` were promoted from dev-dependencies to regular dependencies in `anvilml-server/Cargo.toml` because the new handlers use them at runtime.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `model_dirs: Vec<ModelDirConfig>` field to `AppState`; update `new()` and `new_with_hardware()` constructors to accept optional `Vec<ModelDirConfig>` parameter; update `Clone` impl |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `get_model` handler (GET /v1/models/:id) and `rescan_models` handler (POST /v1/models/rescan); add `RescanResponse` struct; add `tracing` import |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire routes for `/v1/models/rescan` and `/v1/models/:id`; add `post` routing import; add two inline unit tests (`get_model_returns_404_when_missing`, `rescan_returns_202`); update existing test calls to new constructor signatures |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `serde_json = "1"` and `tracing = "0.1"` to dependencies; add `tempfile = "3"` to dev-dependencies; remove `serde_json` from dev-dependencies (promoted) |
| Modify | `backend/src/main.rs` | Pass `Some(cfg.model_dirs.clone())` to `AppState::new_with_hardware()` |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Update `build_test_app_state()` to pass `dirs` to `AppState::new()` (4-arg constructor) |

## Commit Log

```
 .forge/reports/P6-A7_plan.md                 | 91 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |  6 +-
 .forge/state/state.json                      | 13 ++--
 Cargo.lock                                   |  2 +
 backend/src/main.rs                          |  9 ++-
 crates/anvilml-server/Cargo.toml             |  6 +-
 crates/anvilml-server/src/handlers/models.rs | 61 ++++++++++++++++++-
 crates/anvilml-server/src/lib.rs             | 75 +++++++++++++++++++++--
 crates/anvilml-server/src/state.rs           |  9 ++-
 crates/anvilml-server/tests/api_models.rs    |  6 +-
 10 files changed, 256 insertions(+), 22 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-8e2d3078a8d1b65f)

running 5 tests
test tests::env_returns_200_with_stub_report ... ok
test tests::health_returns_200 ... ok
test tests::rescan_returns_202 ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.14s

     Running tests/api_models.rs (target/debug/deps/api_models-8a5bffee8c8e00fb)

running 3 tests
test list_models_kind_filter_no_match ... ok
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_diffusion ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.03s
```

Full workspace test suite: **167 tests passed, 0 failed**.

## Platform Cross-Check

**Check 1 — Mock-hardware Windows-gnu:** `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.18s
```
**Result: PASS**

**Check 2 — Real-hardware Linux native:** `cargo check --bin anvilml`
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.65s
```
**Result: PASS**

**Check 3 — Real-hardware Windows-gnu:** `cargo check --bin anvilml --target x86_64-pc-windows-gnu`
```
error[E0423]: expected value, found struct `dxgi::DxgiDetector`
   --> crates/anvilml-hardware/src/lib.rs:112:32
    |
112 |             let dxgi_devices = dxgi::DxgiDetector.detect().unwrap_or_default();
    |                                ^^^^^^^^^^^^^^^^^^
help: use the path separator to refer to an item
    |
112 -             let dxgi_devices = dxgi::DxgiDetector.detect().unwrap_or_default();
112 +             let dxgi_devices = dxgi::DxgiDetector::detect().unwrap_or_default();
```
**Result: FAIL** (pre-existing bug in `anvilml-hardware` — verified on main branch without any changes)

## Project Gates

**Config Surface Sync:** `cargo test -p backend --features mock-hardware --test config_reference`
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-50ad1c4cbef3f7e5)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.00s
```
**Result: PASS**

## Deviations from Plan

- **Dependencies added:** The plan did not anticipate needing `tracing` and `serde_json` as regular (non-dev) dependencies of `anvilml-server`. Added because the new handlers use them at runtime. This is a minimal, necessary addition.
- **Test file update:** Updated `crates/anvilml-server/tests/api_models.rs` to pass the 4th argument (`model_dirs`) to `AppState::new()`. This file was not listed in the plan's "Files Affected" table but required updating because the constructor signature changed.
- **tempfile dev-dependency:** Added `tempfile = "3"` to dev-dependencies for the `get_model_returns_404_when_missing` unit test, which needs a file-based database with migrations (in-memory pool has no tables).

## Blockers

**Pre-existing dxgi build error (Check 3):** The real-hardware Windows-gnu cross-check (`cargo check --bin anvilml --target x86_64-pc-windows-gnu`) fails due to a pre-existing typo in `crates/anvilml-hardware/src/lib.rs:112`: `dxgi::DxgiDetector.detect()` should be `dxgi::DxgiDetector::detect()`. Verified this error exists on the main branch without any of my changes. This is outside the scope of this task (which only modifies server handler files) and does not affect any mock-hardware build or the primary platform cross-check.
