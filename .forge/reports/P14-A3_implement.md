# Implementation Report: P14-A3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P14-A3                                        |
| Phase       | 014 — Artifact Storage                        |
| Description | anvilml-scheduler: handle ImageReady → ArtifactStore.save + JobImageReady |
| Implemented | 2026-06-09T19:20:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Wired `ArtifactStore` into the job scheduler so that when the dispatch loop receives a
`WorkerEvent::ImageReady` from a Python worker, it persists the artifact via
`ArtifactStore::save()` and broadcasts a `JobImageReady` WebSocket event containing only
metadata (hash, dimensions, seed) — no image bytes. The trait-based approach uses a generic
parameter `A: ArtifactSave` on both `JobScheduler` and `AppState` to avoid a circular
dependency between `anvilml-scheduler` and `anvilml-server`.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|-----------------|----------------|
| crate  | async-trait| 0.1.89         | crates.io      |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-core/src/types/artifact.rs` | Appended `ArtifactSave` trait + `ArtifactSaveInput` struct |
| Modify | `crates/anvilml-core/src/lib.rs` | Re-export `ArtifactSave`, `ArtifactSaveInput` |
| Modify | `crates/anvilml-core/Cargo.toml` | Added `async-trait = "0.1"` dependency; bumped version 0.1.0 → 0.1.1 |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Added `async-trait = "0.1"` dependency; bumped version 0.1.13 → 0.1.14 |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Made `JobScheduler<A: ArtifactSave + Clone + 'static>` generic; added `ImageReady` arm in dispatch loop; added `handle_image_ready()` async function; added `NoopArtifactStore` and `MockArtifactStore` test doubles; added `test_image_ready_broadcasts_event` test; updated all existing tests to pass `NoopArtifactStore` |
| Modify | `crates/anvilml-server/Cargo.toml` | Added `async-trait = "0.1"` dependency |
| Modify | `crates/anvilml-server/src/artifact/store.rs` | Added `#[derive(Clone)]` to `ArtifactStore`; implemented `ArtifactSave` trait |
| Modify | `crates/anvilml-server/src/state.rs` | Made `AppState<A: ArtifactSave + Clone + 'static>` generic; added `artifact_store: A` field; updated `new()` and `new_with_hardware()` signatures; updated `Clone` impl |
| Modify | `crates/anvilml-server/src/lib.rs` | Added `App` type alias; updated `build_router()` to take `App`; updated all tests to use `App` with `ArtifactStore` |
| Modify | `crates/anvilml-server/src/handlers/health.rs` | Updated to use `App` type alias |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Updated to use `App` type alias; updated `build_test_app()` to create `ArtifactStore`; updated test imports |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Updated to use `App` type alias |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Updated to use `crate::App` type alias |
| Modify | `crates/anvilml-server/src/handlers/workers.rs` | Updated to use `crate::App` type alias |
| Modify | `crates/anvilml-server/src/ws/handler.rs` | Updated to use `App` type alias |
| Modify | `crates/anvilml-server/src/ws/stats_tick.rs` | Updated to use `App` type alias; added `ArtifactStore` to test |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Updated to use `App` type alias with `ArtifactStore` |
| Modify | `crates/anvilml-server/tests/api_ws_events.rs` | Updated to use `App` type alias with `ArtifactStore` |
| Modify | `backend/src/main.rs` | Created `ArtifactStore`; passed it to `JobScheduler::new()` and `App::new_with_hardware()` |

## Commit Log

```
 .forge/reports/P14-A3_plan.md                 | 245 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +-
 Cargo.lock                                    |  18 +-
 backend/src/main.rs                           |  10 +-
 crates/anvilml-core/Cargo.toml                |   3 +-
 crates/anvilml-core/src/lib.rs                |   2 +-
 crates/anvilml-core/src/types/artifact.rs     |  33 +++
 crates/anvilml-scheduler/Cargo.toml           |   5 +-
 crates/anvilml-scheduler/src/scheduler.rs     | 282 +++++++++++++++++++++++++-
 crates/anvilml-server/Cargo.toml              |   1 +
 crates/anvilml-server/src/artifact/store.rs   |  25 +++
 crates/anvilml-server/src/handlers/health.rs  |   4 +-
 crates/anvilml-server/src/handlers/jobs.rs    |  21 +-
 crates/anvilml-server/src/handlers/models.rs  |  10 +-
 crates/anvilml-server/src/handlers/system.rs  |   8 +-
 crates/anvilml-server/src/handlers/workers.rs |   2 +-
 crates/anvilml-server/src/lib.rs              |  58 +++++-
 crates/anvilml-server/src/state.rs            |  21 +-
 crates/anvilml-server/src/ws/handler.rs       |   6 +-
 crates/anvilml-server/src/ws/stats_tick.rs    |  10 +-
 crates/anvilml-server/tests/api_models.rs     |  13 +-
 crates/anvilml-server/tests/api_ws_events.rs  |  17 +-
 23 files changed, 737 insertions(+), 76 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-7aeb786479c3659e)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-d9e452e06301c0f5)
running 56 tests
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-58c9512d8e872576)
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-217bdc1ab63526ad)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-5a84297e20c2d242)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs (target/debug/deps/device_store-3cc96624f1b5a374)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-3d7a01dc2461fb73)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-8c626801dc2461fb73)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-fe91fce64e)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-b2419867ed21)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-a2472ee70f72486)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-7114119030fce64e)
running 39 tests
test scheduler::tests::test_image_ready_broadcasts_event ... ok
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-eef3d75b3c556a52)
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_save.rs (target/debug/deps/api_artifact_save-e56585f9867c933b)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-314cf4c9464294f2)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-d21ec4bafd2c9bff)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-e8a8d4b7bbf16bf4)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-fe87e778bc433915)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-ec6875deaadb4861)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core ... test result: ok. 0 passed
   Doc-tests anvilml_hardware ... test result: ok. 2 passed
   Doc-tests anvilml_ipc ... test result: ok. 0 passed
   Doc-tests anvilml_registry ... test result: ok. 0 passed
   Doc-tests anvilml_scheduler ... test result: ok. 0 passed
   Doc-tests anvilml_server ... test result: ok. 0 passed
   Doc-tests anvilml_worker ... test result: ok. 0 passed

Total: 254 passed; 0 failed; 0 ignored
```

## Format Gate

```
(No drift after reformat pass 3)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.46s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.05s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.52s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.83s

All four checks exited 0.
```

## Project Gates

```
cargo test -p backend --features mock-hardware -- config_reference
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

## Deviations from Plan

- `anvilml-core` version was `0.1.0` (not `0.1.13` as stated in the plan). Bumped to `0.1.1` per the bump-by-1 rule.
- `AppState` generic parameter includes `Clone + 'static` bounds (beyond `ArtifactSave`) because `JobScheduler` requires `Clone` (for cloning in the dispatch loop) and `'static` (for `tokio::spawn`).
- Handler files updated to use `App` type alias (`AppState<ArtifactStore>`) rather than raw `AppState<A>` to avoid propagating the generic through every handler.
- `MockArtifactStore` in tests uses `Arc<tokio::sync::Mutex<...>>` instead of bare `tokio::sync::Mutex<...>` because `tokio::sync::Mutex` does not implement `Clone` (required for `JobScheduler<A: Clone>`).
- `ArtifactStore` derives `Clone` (fields `PathBuf` and `SqlitePool` both implement `Clone`).
- Added `#[expect(clippy::too_many_arguments)]` to `AppState::new()` and `handle_image_ready()` as required by clippy.
- Fixed deprecated `tempfile::TempDir::into_path()` to `keep()` across test files.

## Blockers

None.
