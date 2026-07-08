# Implementation Report: P15-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P15-C1                          |
| Phase         | 15 — Artifact Storage Wiring    |
| Description   | anvilml-scheduler: dispatch_one persists ArtifactMeta on ImageReady |
| Implemented   | 2026-07-08T22:15:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented the `event_loop` module in `anvilml-scheduler` that handles `WorkerEvent::ImageReady` events by base64-decoding the image payload, constructing an `ArtifactMeta`, and persisting the decoded PNG bytes to the artifact store via `ArtifactStore::save()`. Added an `Arc<ArtifactStore>` constructor field to `JobScheduler` and updated all call sites across `backend/main.rs`, `scheduler_tests.rs`, and five server test files to pass the new parameter. Added `base64 = "0.22.1"` dependency and bumped the crate version from `0.1.19` to `0.1.20`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | base64  | 0.22.1           | rust-docs MCP  |

The `base64` crate was already present as a transitive dependency in `Cargo.lock` at version 0.22.1. The API shape confirmed via MCP: `base64::engine::general_purpose::STANDARD` (const `GeneralPurpose` engine) and `base64::Engine::decode(&engine, input: &str) -> Result<Vec<u8>, DecodeError>`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/event_loop.rs` | New module; `handle_image_ready()` function |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod event_loop;` and `pub use event_loop::handle_image_ready;` |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `artifact_store` field and constructor parameter to `JobScheduler` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `base64 = "0.22.1"` dependency; bump version 0.1.19 → 0.1.20 |
| CREATE | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | 4 integration tests for event_loop functionality |
| MODIFY | `backend/src/main.rs` | Move artifact_store construction before scheduler; pass to `JobScheduler::new()` |
| MODIFY | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add `create_test_artifact_store()` helper; update all 24 `JobScheduler::new()` calls |
| MODIFY | `crates/anvilml-server/tests/artifacts_tests.rs` | Move artifact_store before scheduler; pass clone to `JobScheduler::new()` |
| MODIFY | `crates/anvilml-server/tests/cors_tests.rs` | Move artifact_store before scheduler; pass clone to `JobScheduler::new()` |
| MODIFY | `crates/anvilml-server/tests/health_tests.rs` | Move artifact_store before scheduler; pass clone to `JobScheduler::new()` |
| MODIFY | `crates/anvilml-server/tests/jobs_tests.rs` | Move artifact_store before scheduler; pass clone to `JobScheduler::new()` |
| MODIFY | `crates/anvilml-server/tests/nodes_tests.rs` | Move artifact_store before scheduler; pass clone to `JobScheduler::new()` |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Update 2 `JobScheduler::new()` calls to include artifact_store parameter |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new event_loop tests |

## Commit Log

```
 backend/src/main.rs                            | 19 ++++++----
 crates/anvilml-scheduler/Cargo.toml            |  2 +-
 crates/anvilml-scheduler/src/event_loop.rs     | 97 +++++++++++++++++++++++++
 crates/anvilml-scheduler/src/lib.rs            |  2 +
 crates/anvilml-scheduler/src/scheduler.rs      | 20 ++++++-
 crates/anvilml-scheduler/tests/event_loop_tests.rs | 126 +++++++++++++++++++++++
 crates/anvilml-scheduler/tests/scheduler_tests.rs  | 44 +++++++---
 crates/anvilml-server/tests/artifacts_tests.rs | 14 +++--
 crates/anvilml-server/tests/cors_tests.rs      | 14 +++--
 crates/anvilml-server/tests/health_tests.rs    | 14 +++--
 crates/anvilml-server/tests/jobs_tests.rs      | 14 +++--
 crates/anvilml-server/tests/nodes_tests.rs     | 14 +++--
 crates/anvilml-server/tests/state_tests.rs     | 11 +++-
 docs/TESTS.md                                  | 47 ++++++++++++
 14 files changed, 402 insertions(+), 32 deletions(-)
```

## Test Results

```
     Running tests/event_loop_tests.rs (target/debug/deps/event_loop_tests-b4faa4b8e5044c0a)

running 4 tests
test test_image_ready_malformed_base64_errors ... ok
test test_image_ready_saves_artifact ... ok
test test_image_ready_empty_image_b64 ... ok
test test_image_ready_artifact_meta_fields_match ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Full workspace test suite: 363 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
Check 1 (mock-hardware Linux):   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.69s
Check 2 (mock-hardware Windows):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.39s
Check 3 (real-hardware Linux):    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.53s
Check 4 (real-hardware Windows):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.75s
All 4 checks passed.
```

## Project Gates

```
Gate 1 (config_reference): test tests::config_reference_matches_defaults ... ok
Gate 1 passed.
Gate 2 (OpenAPI drift): Not triggered — task does not modify handler function signatures or ToSchema derives.
Gate 3 (Node Parity): Not triggered — task does not modify node types or node_registry.rs.
Gate 4 (Mock/Real Parity Markers): Not triggered — task does not add or modify a node's execute() or an arch module's load()/sample()/decode()/compute_latent_shape().
```

## Public API Delta

```
+pub mod event_loop;
+pub use event_loop::handle_image_ready;
+    pub fn new(
```

New public items:
- `pub mod event_loop` — module in `anvilml-scheduler`
- `pub use event_loop::handle_image_ready` — re-exported function in `anvilml-scheduler`
- `pub async fn handle_image_ready(artifact_store: Arc<ArtifactStore>, event: WorkerEvent, job_id: Uuid) -> Result<String, AnvilError>` — function in `anvilml-scheduler::event_loop`

Structural change to existing pub item:
- `JobScheduler::new()` signature changed from `(JobStore, Arc<NodeTypeRegistry>)` to `(JobStore, Arc<NodeTypeRegistry>, Arc<ArtifactStore>)`

## Deviations from Plan

- The plan's approach for the pattern match in `handle_image_ready()` was to only proceed when `event == WorkerEvent::ImageReady { ... }`. The actual implementation uses a `let` destructuring pattern that requires all fields to be matched. The `job_id` field from the event is explicitly ignored (`job_id: _`) since the `job_id` parameter is passed separately — this is the correct approach per the plan's stated design.
- A pre-existing syntax error in `crates/anvilml-server/tests/health_tests.rs` (duplicate `));` on line 68) was introduced by the edit that moved the artifact_store creation. This was fixed as part of the implementation.

## Blockers

None.
