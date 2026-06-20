# Plan Report: P15-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-A2                                        |
| Phase       | 015 — Artifact Storage                        |
| Description | anvilml-scheduler: persist ImageReady artifact and update job |
| Depends on  | P15-A1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-20T14:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Wire `ArtifactStore` into the scheduler's event loop so that when a worker emits `WorkerEvent::ImageReady`, the scheduler base64-decodes the image payload, persists it via `ArtifactStore::save()`, and broadcasts `WsEvent::JobImageReady` to WebSocket clients. This completes the artifact persistence half of Phase 015 — P15-A1 created the `ArtifactStore` persistence layer, and P15-A3 will expose the retrieval endpoints.

## Scope

### In Scope
- Move `ArtifactStore` from `anvilml-server` to `anvilml-ipc` (to break the dependency cycle: scheduler cannot depend on server).
- Add `base64 = "0.22"` dependency to `anvilml-ipc` Cargo.toml.
- Add `artifact_store: Arc<ArtifactStore>` field to `JobScheduler` struct.
- Update `JobScheduler::new()` to accept `Arc<ArtifactStore>` and store it.
- Extend `event_loop.rs` to handle `WorkerEvent::ImageReady`: base64-decode, call `save()`, broadcast `WsEvent::JobImageReady`.
- Add `artifact_store: Arc<ArtifactStore>` field to `AppState` in `anvilml-server`.
- Update `AppState::new()` and `AppState::new_with_hardware()` to accept and store `artifact_store`.
- Update `anvilml-ipc/src/lib.rs` to re-export `ArtifactStore`.
- Update `anvilml-ipc/Cargo.toml` with `sha2` and `base64` dependencies.
- Add `artifact_store` re-export to `anvilml-core` types module (optional, for convenience).
- Add tests in `crates/anvilml-scheduler/tests/image_ready_tests.rs`.
- Bump patch version of `anvilml-ipc` and `anvilml-scheduler` crates.

### Out of Scope
- P15-A1 artifact store implementation (already done).
- P15-A3 HTTP artifact retrieval endpoints (`GET /v1/artifacts`, `GET /v1/artifacts/:hash`).
- Any changes to the Python worker side.
- WebSocket handler changes (the broadcaster already supports `WsEvent::JobImageReady`).

## Existing Codebase Assessment

**What exists:** P15-A1 has already implemented `ArtifactStore` in `crates/anvilml-server/src/artifact/store.rs` with `save()`, `get()`, and `list()` methods. It uses SHA-256 hashing, content-addressed file storage, and SQLite metadata. The `EventBroadcaster` in `anvilml-ipc` already broadcasts both `WsEvent` and `WorkerEvent` through two independent broadcast channels. The scheduler's `event_loop.rs` handles `Completed` and `Failed` events but explicitly ignores `ImageReady` (the match arm has a comment: "Future phases (P15+) will handle ImageReady"). The `WsEvent::JobImageReady` variant already exists in `anvilml-core` with fields `job_id`, `artifact_hash`, `width`, `height`, `seed`, and `steps`.

**Established patterns:** Tests use `#[serial]` annotation with `serial_test` crate, create in-memory databases via `open_in_memory()`, build schedulers with `JobScheduler::new()`, and use `broadcaster.broadcast_worker_event()` to inject test events. The event loop pattern is: subscribe to worker events → match on variant → DB update + VRAM release + WsEvent broadcast. Logging uses structured `tracing::info!` and `tracing::debug!` with field notation.

**Gap:** `ArtifactStore` lives in `anvilml-server` but the scheduler needs access to it. The dependency graph (`anvilml-server` → `anvilml-scheduler`) means the scheduler cannot import from the server. This requires relocating `ArtifactStore` to a shared crate. The scheduler event loop currently has a catch-all `_` arm that logs "ignoring non-terminal worker event" — `ImageReady` will need its own handler.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source  | Feature flags confirmed |
|--------|----------|-----------------|-------------|------------------------|
| crate  | base64   | 0.22.1          | Cargo.lock  | n/a                    |
| crate  | sha2     | (already present) | Cargo.lock | n/a                    |

**Notes:** `base64` 0.22.1 is already in `Cargo.lock` as a transitive dependency (pulled in by another crate). The API surface is `base64::engine::general_purpose::STANDARD` for decoding. `sha2` is already used by `anvilml-server/src/artifact/store.rs` and is in the lockfile. No new external dependencies beyond `base64` are needed — the scheduler already has `anvilml-ipc` as a dependency, and `anvilml-ipc` already has `sqlx` and `sha2` available through its own deps.

## Approach

1. **Move `ArtifactStore` from `anvilml-server` to `anvilml-ipc`.**
   - Copy `crates/anvilml-server/src/artifact/store.rs` to `crates/anvilml-ipc/src/artifact_store.rs`.
   - Update imports: replace `anvilml_core::types::ArtifactMeta` with `anvilml_core::ArtifactMeta` (the re-export path).
   - Add `sha2 = "0.10"` and `base64 = "0.22"` to `anvilml-ipc/Cargo.toml` dependencies.
   - Add `pub mod artifact_store; pub use artifact_store::ArtifactStore;` to `anvilml-ipc/src/lib.rs`.
   - Remove the `artifact/` module from `anvilml-server/src/lib.rs` and `crates/anvilml-server/src/artifact/` directory.
   - Update `anvilml-server/src/state.rs` to import `ArtifactStore` from `anvilml_ipc` instead of `crate::artifact`.

2. **Add `artifact_store: Arc<ArtifactStore>` to `JobScheduler`.**
   - Add field: `artifact_store: Arc<ArtifactStore>` to `JobScheduler` struct in `scheduler.rs`.
   - Update `JobScheduler::new()` signature to accept `Arc<ArtifactStore>` as a new parameter.
   - Store the field in the `Self { ... }` initializer.
   - Add a `#[doc(hidden)]` accessor method `pub fn artifact_store(&self) -> &Arc<ArtifactStore>` for test access (same pattern as `ledger()` and `broadcaster()`).
   - Update all call sites of `JobScheduler::new()` — this includes:
     - `anvilml-server` startup code (in `backend/src/main.rs` or wherever `AppState::new_with_hardware` is called).
     - All test files that construct a `JobScheduler` directly: `scheduler_tests.rs`, `dispatch_tests.rs`, `event_loop_tests.rs`, `dag_tests.rs`, `queue_tests.rs`, `ledger_tests.rs`, `node_registry_tests.rs`.

3. **Handle `WorkerEvent::ImageReady` in the event loop.**
   - In `event_loop.rs`, replace the catch-all `_` arm with an explicit `WorkerEvent::ImageReady { job_id, image_b64, width, height, .. }` arm.
   - Inside the handler:
     a. Decode the base64 payload: `let bytes = base64::engine::general_purpose::STANDARD.decode(&image_b64).map_err(|e| ...)` — log error at WARN and return early if decode fails.
     b. Call `artifact_store.save(job_id, &bytes).await` — this returns `ArtifactMeta` with the computed hash.
     c. On success: broadcast `WsEvent::JobImageReady { job_id, artifact_hash: meta.hash, width, height, seed: 0, steps: 0 }`. Note: `seed` and `steps` are set to 0 because the event loop doesn't have access to them — they would need to be extracted from the event. Looking at `WorkerEvent::ImageReady`, it carries `seed` and `steps` fields! So use those directly: `seed: image_ready.seed, steps: image_ready.steps`.
     d. Log at INFO: `tracing::info!(job_id = %job_id, artifact_hash = %meta.hash, size_bytes = meta.size_bytes, "artifact saved")` — this satisfies the mandatory INFO log point obligation (per FORGE_AGENT_RULES §11.7, tasks touching the scheduler subsystem must include mandatory log calls).
   - Remove the comment in the `_` arm that says "Future phases (P15+) will handle ImageReady" since this task fulfills that.

4. **Add `artifact_store` to `AppState`.**
   - Add field `artifact_store: Arc<ArtifactStore>` to `AppState` struct.
   - Update `AppState::new()` to accept `Arc<ArtifactStore>` as a new parameter and store it.
   - Update `AppState::new_with_hardware()` to accept `Arc<ArtifactStore>` as a new parameter and store it.
   - Update `AppState::new_with_hardware_no_workers()` similarly.

5. **Update `backend/src/main.rs` (or wherever AppState is constructed).**
   - Create `ArtifactStore` before constructing `AppState`: `let artifact_store = Arc::new(ArtifactStore::new(cfg.artifact_dir.clone(), db.clone()).await);`
   - Pass `artifact_store` to both `JobScheduler::new()` and `AppState::new_with_hardware()`.

6. **Add tests.**
   - Create `crates/anvilml-scheduler/tests/image_ready_tests.rs`.
   - Test: `test_image_ready_persists_artifact` — submit a job, manually set it Running, send `WorkerEvent::ImageReady` with a known base64 payload, verify `ArtifactStore::list(Some(job_id))` returns exactly one entry after processing.
   - Test: `test_image_ready_broadcasts_job_image_ready` — same setup, subscribe to WsEvent channel, verify `WsEvent::JobImageReady` is broadcast with correct fields.
   - Test: `test_image_ready_invalid_base64_is_ignored` — send `WorkerEvent::ImageReady` with invalid base64, verify the job status is unchanged and no WsEvent is broadcast.

7. **Bump crate versions.**
   - `anvilml-ipc`: bump patch version (e.g., 0.1.X → 0.1.(X+1)).
   - `anvilml-scheduler`: bump patch version.

## Public API Surface

| Item | Crate | Signature / Description |
|------|-------|------------------------|
| `ArtifactStore` (moved) | `anvilml_ipc` | `pub struct ArtifactStore { dir: PathBuf, db: SqlitePool }` — same as before, moved from `anvilml_server::artifact` |
| `ArtifactStore::new` (moved) | `anvilml_ipc` | `pub async fn new(dir: PathBuf, db: SqlitePool) -> Self` |
| `ArtifactStore::save` (moved) | `anvilml_ipc` | `pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta>` |
| `ArtifactStore::get` (moved) | `anvilml_ipc` | `pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>>` |
| `ArtifactStore::list` (moved) | `anvilml_ipc` | `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>>` |
| `JobScheduler::new` (modified) | `anvilml_scheduler` | New parameter: `artifact_store: Arc<ArtifactStore>` added to constructor |
| `JobScheduler::artifact_store` (new) | `anvilml_scheduler` | `pub fn artifact_store(&self) -> &Arc<ArtifactStore>` — test accessor |
| `AppState::artifact_store` (new field) | `anvilml_server` | `pub artifact_store: Arc<ArtifactStore>` |
| `AppState::new` (modified) | `anvilml_server` | New parameter: `artifact_store: Arc<ArtifactStore>` |
| `AppState::new_with_hardware` (modified) | `anvilml_server` | New parameter: `artifact_store: Arc<ArtifactStore>` |
| `AppState::new_with_hardware_no_workers` (modified) | `anvilml_server` | New parameter: `artifact_store: Arc<ArtifactStore>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/artifact_store.rs` | Move `ArtifactStore` from `anvilml-server` |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `pub mod artifact_store; pub use artifact_store::ArtifactStore;` |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add `sha2` and `base64` dependencies; bump patch version |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `artifact_store` field to `JobScheduler`, update `new()` |
| MODIFY | `crates/anvilml-scheduler/src/event_loop.rs` | Handle `WorkerEvent::ImageReady` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version |
| CREATE | `crates/anvilml-scheduler/tests/image_ready_tests.rs` | Tests for ImageReady handling |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `artifact_store` field to `AppState`, update constructors |
| REMOVE | `crates/anvilml-server/src/artifact/mod.rs` | Removed — module moved to `anvilml-ipc` |
| REMOVE | `crates/anvilml-server/src/artifact/store.rs` | Removed — moved to `anvilml-ipc` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Remove `pub mod artifact` declaration |
| MODIFY | `backend/src/main.rs` | Wire `ArtifactStore` into scheduler and AppState construction |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/image_ready_tests.rs` | `test_image_ready_persists_artifact` | ImageReady event triggers artifact save; `ArtifactStore::list` returns entry | In-memory DB, scheduler with ArtifactStore, job submitted and set Running | `WorkerEvent::ImageReady` with valid base64 PNG payload | `ArtifactStore::list(Some(job_id))` returns 1 entry with correct hash; job status unchanged | `cargo test -p anvilml-scheduler --features mock-hardware -- image_ready_tests::test_image_ready_persists_artifact` exits 0 |
| `crates/anvilml-scheduler/tests/image_ready_tests.rs` | `test_image_ready_broadcasts_job_image_ready` | ImageReady event triggers `WsEvent::JobImageReady` broadcast | Same setup as above | `WorkerEvent::ImageReady` with valid base64 payload | `WsEvent::JobImageReady` received on WsEvent channel with correct job_id and artifact_hash | `cargo test -p anvilml-scheduler --features mock-hardware -- image_ready_tests::test_image_ready_broadcasts_job_image_ready` exits 0 |
| `crates/anvilml-scheduler/tests/image_ready_tests.rs` | `test_image_ready_invalid_base64_is_ignored` | Invalid base64 in ImageReady is logged and does not crash event loop | Same setup, job Running | `WorkerEvent::ImageReady` with `"!!!invalid!!!"` as image_b64 | Job status remains Running; no WsEvent broadcast; no panic | `cargo test -p anvilml-scheduler --features mock-hardware -- image_ready_tests::test_image_ready_invalid_base64_is_ignored` exits 0 |

## CI Impact

No CI changes required. The tests added are picked up by `cargo test --workspace --features mock-hardware` which already runs in CI (rust-linux and rust-windows jobs). No new file types, gates, or test modules are introduced that would require CI configuration changes. The `anvilml-ipc` crate is part of the workspace build, so `cargo check --workspace` will exercise it.

## Platform Considerations

None identified. The `base64` crate's `general_purpose::STANDARD.decode()` is platform-neutral. The `sha2` crate's SHA-256 implementation is deterministic across platforms. The artifact file path uses `PathBuf::join()` which handles platform-specific separators. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Moving `ArtifactStore` to `anvilml-ipc` changes the crate's responsibility boundary — `anvilml-ipc` was designed as "ZeroMQ ROUTER transport + message types", not as a general persistence layer. This may surprise future maintainers. | Medium | Medium | Document the rationale in `artifact_store.rs` module doc: "Moved from anvilml-server to anvilml-ipc because the scheduler needs access to ArtifactStore, and anvilml-scheduler cannot depend on anvilml-server (dependency graph constraint)." The alternative (duplicating storage logic in the scheduler) is worse. |
| `JobScheduler::new()` signature change breaks all call sites — tests in 6+ test files plus `backend/src/main.rs`. Missing one call site will cause a compile error. | High | High | The compiler will catch any missing call site. During implementation, run `cargo check --workspace --features mock-hardware` after each file change to catch errors early. All call sites are listed in the "Files Affected" table. |
| The event loop handles `ImageReady` synchronously within the existing `handle_event()` match arm. If `artifact_store.save()` is slow (disk I/O), it blocks the event loop from processing other events. | Low | Medium | The save operation is expected to be fast for small PNG files (< 1 MiB). If this becomes a problem, the `save()` call should be moved to a `tokio::spawn` block. For this task, synchronous handling is acceptable. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- image_ready_tests` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (full suite, no regressions)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace)
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `grep "^pub use artifact_store::ArtifactStore" crates/anvilml-ipc/src/lib.rs` returns non-empty (ArtifactStore is exported from anvilml-ipc)
- [ ] `grep "artifact_store: Arc<ArtifactStore>" crates/anvilml-scheduler/src/scheduler.rs` returns non-empty (JobScheduler has artifact_store field)
- [ ] `grep "WorkerEvent::ImageReady" crates/anvilml-scheduler/src/event_loop.rs` returns non-empty (event loop handles ImageReady)
