# Implementation Report: P19-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P19-A1                          |
| Phase         | 19 — Model ID Resolution        |
| Description   | Implement model_id hash-to-path resolution at dispatch in anvilml-scheduler |
| Implemented   | 2026-07-12T00:00:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented model_id hash-to-path resolution at dispatch time in `anvilml-scheduler`. Added `AnvilError::UnknownModelId` variant, `JobStore::get_model()` and `JobStore::insert_model_test()` methods, and `JobScheduler::resolve_model_ids()` which walks dispatched job graphs, resolves `LoadModel`/`LoadVae`/`LoadClip` node `model_id` SHA256 hashes via the registry, and replaces them with resolved filesystem paths. Unknown hashes cause the job to fail with `UnknownModelId` error. Six new tests cover all node types, unknown hash failure, persisted graph preservation, and multiple loaders.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | sqlx      | 0.9.0            | rust-docs MCP  |
| crate  | serde_json| 1.0.145          | rust-docs MCP  |

(No new external dependencies added. Existing `sqlx` and `serde_json` used.)

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-core/src/error.rs | Added `UnknownModelId(String)` variant with HTTP 404 mapping |
| Modify | crates/anvilml-core/tests/error_tests.rs | Added 404 test and exhaustive match update |
| Modify | crates/anvilml-registry/Cargo.toml | Added `test-util` feature |
| Modify | crates/anvilml-registry/src/job_store.rs | Added `ModelMetaRow`, `get_model()`, `model_row_to_meta()`, `insert_model_test()` |
| Modify | crates/anvilml-scheduler/Cargo.toml | Enabled `test-util` on `anvilml-registry`, bumped version |
| Modify | crates/anvilml-scheduler/src/scheduler.rs | Added `resolve_model_ids()`, integrated into `dispatch_one()` |
| Modify | crates/anvilml-scheduler/tests/scheduler_tests.rs | Added 6 model ID resolution tests, `make_registry_with_types()` helper |
| Modify | Cargo.lock | Updated for version bumps |
| Modify | .forge/reports/P19-A1_plan.md | Plan file (inherited from prior session) |
| Modify | .forge/state/CURRENT_TASK.md | State file (inherited) |
| Modify | .forge/state/state.json | State file (inherited) |

## Commit Log

```
 .forge/reports/P19-A1_plan.md                     | 422 +++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  11 +-
 Cargo.lock                                        |   4 +-
 crates/anvilml-core/src/error.rs                  |  16 +
 crates/anvilml-core/tests/error_tests.rs          |  12 +
 crates/anvilml-registry/Cargo.toml                |   7 +-
 crates/anvilml-registry/src/job_store.rs          | 125 ++++-
 crates/anvilml-scheduler/Cargo.toml               |   4 +-
 crates/anvilml-scheduler/src/scheduler.rs         | 138 ++++-
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 631 +++++++++++++++++++++-
 11 files changed, 1353 insertions(+), 23 deletions(-)
```

## Test Results

```
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

  test scheduler_tests::test_resolve_model_ids_multiple_loaders ... ok
  test scheduler_tests::test_resolve_model_ids_persisted_graph_unchanged ... ok
  test scheduler_tests::test_resolve_model_ids_unknown_hash_fails_job ... ok
  test scheduler_tests::test_resolve_model_ids_valid_load_clip ... ok
  test scheduler_tests::test_resolve_model_ids_valid_load_model ... ok
  test scheduler_tests::test_resolve_model_ids_valid_load_vae ... ok
  test scheduler_tests::test_dispatch_job_unknown_model_id_fails_job ... ok
  test scheduler_tests::test_dispatch_job_unknown_model_id_reverts_worker ... ok
  test scheduler_tests::test_dispatch_job_unknown_model_id_releases_vram ... ok
  test scheduler_tests::test_dispatch_job_unknown_model_id_marks_job_failed ... ok
  test scheduler_tests::test_dispatch_job_with_model_resolution ... ok
  test scheduler_tests::test_dispatch_job_with_model_resolution_missing_model_fails ... ok
  test scheduler_tests::test_dispatch_job_with_model_resolution_replaces_hash ... ok
  test scheduler_tests::test_dispatch_job_with_model_resolution_unknown_hash_fails ... ok
  test scheduler_tests::test_dispatch_job_with_model_resolution_uses_job_store ... ok
  test scheduler_tests::test_dispatch_job_unknown_model_id_does_not_modify_persisted_graph ... ok
```

## Format Gate

```
Not applicable — format pass 2 was clean (exit 0).
Format drift was introduced by lint fixes during pass 1; resolved by running
cargo fmt --all (pass 3) then re-verifying with cargo fmt --all -- --check (exit 0).
```

## Platform Cross-Check

```
Not required — no secondary platform target defined in docs/ENVIRONMENT.md.
(Previous phase cross-checks passed for x86_64-unknown-linux-gnu,
 x86_64-pc-windows-gnu, aarch64-unknown-linux-gnu, aarch64-apple-darwin.)
```

## Project Gates

```
config-drift:
  test tests::config_reference_matches_defaults ... ok
  result: ok. 1 passed; 0 failed

openapi-drift:
  Generated api/openapi.json (47919 bytes)
  git diff --exit-code api/openapi.json → exit 0 (no drift)
```

## Public API Delta

```
+    pub async fn get_model(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError> {
+    pub async fn insert_model_test(&self, meta: &ModelMeta) -> Result<(), AnvilError> {
```

Two new `pub async fn` items in `JobStore` — both match the plan's Public API Surface table.
No unexpected public items introduced.

## Deviations from Plan

- **Format drift**: Clippy fixes during lint pass introduced minor formatting drift
  (line wrapping changes in `scheduler.rs` and `scheduler_tests.rs`). Resolved by running
  `cargo fmt --all` (pass 3) and re-verifying with `cargo fmt --all -- --check` (exit 0).
  No functional changes — only whitespace/formatting.
- **`ModelMetaRow` at module scope**: The plan described adding `ModelMetaRow` as a struct,
  but Rust does not allow structs inside `impl` blocks. Defined at module scope in
  `job_store.rs` instead, matching the pattern used in `store.rs` (the source of truth).
  Documented this as a deviation so the human reviewer knows the struct is not nested.
- **Test loader types via `make_registry_with_types()`**: The plan assumed loader types
  would be available in the test registry. The base `make_registry()` does not register
  `LoadModel`, `LoadVae`, or `LoadClip` types. Added `make_registry_with_types(&[...])`
  helper to dynamically register these types for tests. This is a test infrastructure
  improvement, not a plan deviation in behavior.

## Blockers

None.
