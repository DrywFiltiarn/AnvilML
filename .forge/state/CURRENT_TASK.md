# Task: P19-A1 — Implement model_id hash-to-path resolution at dispatch

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P19-A1                          |
| Phase         | 19 — Model ID Resolution        |
| Description   | Implement model_id hash-to-path resolution at dispatch in anvilml-scheduler |
| Status        | COMPLETE                          |
| Step          | 17 — STOP                       |
| Completed     | 2026-07-12T00:00:00Z            |
| Defers To     | NONE                            |

## Summary

Implemented model_id hash-to-path resolution at dispatch in `anvilml-scheduler`. All tests pass (59/59), all gates pass, format is clean.

## Files Changed

- `crates/anvilml-core/src/error.rs` — Added `UnknownModelId(String)` variant
- `crates/anvilml-core/tests/error_tests.rs` — Added 404 test
- `crates/anvilml-registry/Cargo.toml` — Added `test-util` feature
- `crates/anvilml-registry/src/job_store.rs` — Added `get_model()`, `insert_model_test()`, `ModelMetaRow`, `model_row_to_meta()`
- `crates/anvilml-scheduler/Cargo.toml` — Enabled `test-util`, bumped version
- `crates/anvilml-scheduler/src/scheduler.rs` — Added `resolve_model_ids()`, integrated into `dispatch_one()`
- `crates/anvilml-scheduler/tests/scheduler_tests.rs` — Added 6 tests, `make_registry_with_types()` helper

## Report

Implementation report: `.forge/reports/P19-A1_implement.md`
