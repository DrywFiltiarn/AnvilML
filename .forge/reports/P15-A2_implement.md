# Implementation Report: P15-A2

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P15-A2                                          |
| Phase         | 015 — Artifact Storage                          |
| Description   | anvilml-scheduler: persist ImageReady artifact and update job |
| Implemented   | 2026-06-20T17:45:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Moved `ArtifactStore` from `anvilml-server` to `anvilml-ipc` to break the dependency cycle (the scheduler cannot depend on the server). Added `artifact_store: Arc<ArtifactStore>` field to `JobScheduler` and `AppState`, updated all constructors and call sites, and implemented `WorkerEvent::ImageReady` handling in the event loop: base64-decode the image payload, persist via `ArtifactStore::save()`, and broadcast `WsEvent::JobImageReady`. Added three tests verifying artifact persistence, broadcast correctness, and invalid base64 handling.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source          |
|--------|---------|------------------|-----------------|
| crate  | base64  | 0.22.1           | Cargo.lock (transitive) |
| crate  | sha2    | 0.10.x           | Cargo.lock (already present) |
| crate  | sqlx    | 0.9.0            | Workspace dep (added to anvilml-ipc) |
| crate  | chrono  | 0.4.45           | Workspace dep (added to anvilml-ipc) |

**Notes:** `base64` 0.22.1 was already in `Cargo.lock` as a transitive dependency. Added `base64`, `sha2`, `sqlx`, and `chrono` as direct dependencies of `anvilml-ipc` since the `ArtifactStore` needs them. Added `base64` to `anvilml-scheduler` for the event loop's base64 decoding.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/artifact_store.rs` | Moved ArtifactStore from anvilml-server |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Added `pub mod artifact_store; pub use artifact_store::ArtifactStore;` |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Added `base64`, `sha2`, `sqlx`, `chrono` deps; bumped 0.1.7 → 0.1.8 |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Added `artifact_store` field, updated `new()`, added accessor |
| MODIFY | `crates/anvilml-scheduler/src/event_loop.rs` | Added `WorkerEvent::ImageReady` handler |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Added `base64` dep; bumped 0.1.9 → 0.1.10 |
| CREATE | `crates/anvilml-scheduler/tests/image_ready_tests.rs` | 3 tests for ImageReady handling |
| MODIFY | `crates/anvilml-server/src/state.rs` | Added `artifact_store` field to AppState, updated all constructors |
| REMOVE | `crates/anvilml-server/src/artifact/mod.rs` | Removed — module moved to anvilml-ipc |
| REMOVE | `crates/anvilml-server/src/artifact/store.rs` | Removed — moved to anvilml-ipc |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Removed `pub mod artifact` declaration |
| MODIFY | `backend/src/main.rs` | Created ArtifactStore, passed to scheduler and AppState |
| MODIFY | `crates/anvilml-server/tests/artifact_store_tests.rs` | Updated import to `anvilml_ipc::ArtifactStore` |
| MODIFY | `crates/anvilml-server/tests/handler_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/health_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/jobs_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/models_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/nodes_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/state_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Updated to use test_state helper with artifact_store |
| MODIFY | `crates/anvilml-scheduler/tests/dispatch_tests.rs` | Updated make_scheduler to accept artifact_store |
| MODIFY | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Updated make_scheduler to accept artifact_store |
| MODIFY | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Updated make_scheduler to accept artifact_store |

## Commit Log

```
 .forge/reports/P15-A2_plan.md                      | 170 ++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   9 +-
 backend/src/main.rs                                |  10 +-
 crates/anvilml-ipc/Cargo.toml                      |   6 +-
 .../store.rs => anvilml-ipc/src/artifact_store.rs} |  10 +-
 crates/anvilml-ipc/src/lib.rs                      |   2 +
 crates/anvilml-scheduler/Cargo.toml                |   3 +-
 crates/anvilml-scheduler/src/event_loop.rs         | 134 +++++++-
 crates/anvilml-scheduler/src/scheduler.rs          |  25 +-
 crates/anvilml-scheduler/tests/dispatch_tests.rs   |  18 +-
 crates/anvilml-scheduler/tests/event_loop_tests.rs |  21 +-
 .../anvilml-scheduler/tests/image_ready_tests.rs   | 357 +++++++++++++++++++++
 crates/anvilml-scheduler/tests/scheduler_tests.rs  |  29 +-
 crates/anvilml-server/src/artifact/mod.rs          |   9 -
 crates/anvilml-server/src/lib.rs                   |   1 -
 crates/anvilml-server/src/state.rs                 |  22 ++
 .../anvilml-server/tests/artifact_store_tests.rs   |   2 +-
 crates/anvilml-server/tests/handler_tests.rs       |  38 ++-
 crates/anvilml-server/tests/health_tests.rs        |  32 +-
 crates/anvilml-server/tests/jobs_tests.rs          |  56 +++-
 crates/anvilml-server/tests/models_tests.rs        |  79 ++---
 crates/anvilml-server/tests/nodes_tests.rs         |  44 +--
 crates/anvilml-server/tests/state_tests.rs         |  62 ++--
 crates/anvilml-server/tests/system_tests.rs        |  57 ++--
 crates/anvilml-server/tests/workers_tests.rs       |  75 +++--
 27 files changed, 1036 insertions(+), 254 deletions(-)
```

## Test Results

```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

Not applicable — cross-checks are CI-only gates. The local `cargo check --workspace --features mock-hardware` passed.

## Project Gates

None defined — this task does not modify `ServerConfig` fields, handler signatures, or node types.

## Public API Delta

```
+pub struct ArtifactStore {
+    pub async fn new(dir: PathBuf, db: sqlx::SqlitePool) -> Self {
+    pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta> {
+    pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>> {
+    pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>> {
+pub mod artifact_store;
+pub use artifact_store::ArtifactStore;
+    pub fn artifact_store(&self) -> &Arc<ArtifactStore> {
+    pub artifact_store: Arc<ArtifactStore>,
```

Items:
- `ArtifactStore` struct (moved to `anvilml_ipc`)
- `ArtifactStore::new` (moved)
- `ArtifactStore::save` (moved)
- `ArtifactStore::get` (moved)
- `ArtifactStore::list` (moved)
- `pub mod artifact_store` (new in `anvilml_ipc`)
- `pub use artifact_store::ArtifactStore` (new in `anvilml_ipc`)
- `JobScheduler::artifact_store()` accessor (new, `#[doc(hidden)]`)
- `AppState::artifact_store` field (new)

## Deviations from Plan

- Added `sqlx` and `chrono` to `anvilml-ipc` dependencies (not mentioned in plan) — required because `ArtifactStore` uses these crates directly.
- Added `base64` to `anvilml-scheduler` dependencies (not mentioned in plan) — required for base64 decoding in the event loop.
- The plan's `JobScheduler::new()` signature change required updating 7+ test files in both `anvilml-scheduler` and `anvilml-server` — all were updated successfully.
- The plan mentioned adding `#[doc(hidden)]` accessor for `artifact_store()` — implemented as `pub fn artifact_store(&self) -> &Arc<ArtifactStore>` matching the `ledger()` and `broadcaster()` pattern.

## Blockers

None.
