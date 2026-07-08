# Plan Report: P15-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-C1                                      |
| Phase       | 15 — Artifact Storage Wiring                |
| Description | anvilml-scheduler: dispatch_one persists ArtifactMeta on ImageReady |
| Depends on  | P15-B2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-08T21:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-scheduler/src/event_loop.rs`, the module named in `ANVILML_DESIGN.md §12.1`'s layout since the design was written but never created by any prior phase. This module implements the first real consumer of `WorkerEvent::ImageReady` — base64-decoding the image payload, constructing an `ArtifactMeta`, and calling `artifact_store.save()` to persist the decoded PNG bytes under its content hash. It also adds an `Arc<ArtifactStore>` constructor field to `JobScheduler` so the scheduler can reach the artifact store, and declares `mod event_loop;` in `lib.rs`.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/event_loop.rs` with a `handle_image_ready()` function that takes an `Arc<ArtifactStore>`, a `WorkerEvent::ImageReady`, and a `Uuid` (job_id), decodes the base64 image, constructs `ArtifactMeta`, and calls `artifact_store.save()`.
- Add `artifact_store: Arc<ArtifactStore>` field to `JobScheduler` and update its `new()` constructor to accept it.
- Declare `pub mod event_loop;` in `crates/anvilml-scheduler/src/lib.rs` and re-export `handle_image_ready`.
- Add `base64 = "0.22.1"` dependency to `crates/anvilml-scheduler/Cargo.toml` (version verified via MCP; matches Cargo.lock transitive version).
- Bump `anvilml-scheduler` patch version from `0.1.19` to `0.1.20`.
- Create `crates/anvilml-scheduler/tests/event_loop_tests.rs` with ≥4 tests.

### Out of Scope
None. defers_to (from JSON): []. This task implements its full scope — no functionality is deferred.

## Existing Codebase Assessment

**What already exists:** `anvilml-artifacts`'s `ArtifactStore` is fully implemented (`save()`, `get()`, `list()`) with SQLite metadata persistence and content-addressed file storage. `ArtifactMeta` is defined in `anvilml-core/src/types/artifact.rs` with fields: `hash`, `job_id`, `width`, `height`, `seed`, `steps`, `created_at`, `file_path`. `WorkerEvent::ImageReady` is fully defined in `anvilml-ipc/src/messages.rs` with fields: `job_id`, `image_b64`, `width`, `height`, `format`, `seed`, `steps`. `JobScheduler` exists in `scheduler.rs` with fields for queue, ledger, job_store, node_registry, and dispatch_notify — but no artifact_store field yet.

**Established patterns:** Error handling uses `AnvilError` with `thiserror::Error` derives. Logging uses `tracing::instrument` on async functions and structured field notation (e.g. `tracing::info!(job_id = %id, ...)`). Tests go in `crates/{name}/tests/` as separate test crate files. The `base64` crate is already a transitive dependency (version 0.22.1 in Cargo.lock). The `interim_job_completion.rs` module in this crate shows the pattern for spawning background tasks and consuming events.

**Gap between design and source:** `event_loop.rs` was named in the design doc (§12.1) but never created. The `JobScheduler` struct does not yet have an `Arc<ArtifactStore>` field — this is the structural change this task introduces. The `interim_job_completion.rs` module exists as a Phase 14 retrofit and is separate from the real event loop this task creates; no code in this task touches `interim_job_completion.rs`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | base64  | 0.22.1          | rust-docs MCP  | std                    |

The `base64` crate is already present as a transitive dependency in `Cargo.lock` (version 0.22.1). Adding it as a direct dependency to the scheduler crate's `Cargo.toml` uses the same version to avoid conflicts. The API shape confirmed via MCP: `base64::engine::general_purpose::STANDARD` (const `GeneralPurpose` engine) and `base64::Engine::decode(&engine, input: &str) -> Result<Vec<u8>, DecodeError>`.

## Approach

1. **Add `base64` dependency and bump version.** In `crates/anvilml-scheduler/Cargo.toml`, add `base64 = "0.22.1"` to `[dependencies]`. Bump `[package] version` from `0.1.19` to `0.1.20`.

2. **Add `artifact_store` field to `JobScheduler`.** In `scheduler.rs`:
   - Add `artifact_store: Arc<ArtifactStore>` as a new field on the `JobScheduler` struct (after `dispatch_notify`).
   - Update `new()` to accept `artifact_store: Arc<ArtifactStore>` as a parameter and store it.
   - Add `use anvilml_artifacts::ArtifactStore;` at the top.
   - Update the struct doc comment to mention the artifact store field.
   - Add `#[allow(dead_code)]` on the new field for now (it will be used by event_loop.rs in this task, but the dispatch loop itself doesn't reference it yet — the field is needed for the handle_image_ready function).

3. **Create `crates/anvilml-scheduler/src/event_loop.rs`.** This module owns:
   - `pub async fn handle_image_ready(artifact_store: Arc<ArtifactStore>, event: WorkerEvent, job_id: Uuid) -> Result<String, AnvilError>` — the main handler.
   - The function takes `WorkerEvent` and `job_id` as separate parameters because the caller (interim_job_completion or future real event loop) will pattern-match on the event to extract the variant. This avoids forcing the caller to destructure.
   - Inside: match on `event`, only proceeding when `event == WorkerEvent::ImageReady { ref image_b64, width, height, format, seed, steps }`.
   - Base64 decode: `base64::engine::general_purpose::STANDARD.decode(image_b64)` — returns `Result<Vec<u8>, DecodeError>`. On error, return `AnvilError::Serde(format!("base64 decode failed: {err}"))`.
   - Construct `ArtifactMeta`: `hash` field will be filled in by `save()` (it computes SHA-256 from the bytes), so set it to empty string or omit — actually, `save()` ignores the hash and file_path fields and computes them from the bytes, so construct the meta with `hash: String::new()`, `job_id`, `width`, `height`, `seed`, `steps`, `created_at: chrono::Utc::now()`, `file_path: PathBuf::from("")`.
   - Call `artifact_store.save(&png_bytes, &meta).await` — returns `Result<String, AnvilError>` with the computed hash.
   - Log at INFO: `tracing::info!(job_id = %job_id, hash = %hash, width = width, height = height, "artifact saved from ImageReady")`.
   - Log at DEBUG: `tracing::debug!(job_id = %job_id, "processing ImageReady event")` at the start.
   - Add `#[tracing::instrument(fields(job_id), skip(artifact_store))]` to the function.

4. **Declare `mod event_loop;` in `lib.rs`.** Add `pub mod event_loop;` after the `interim_job_completion` module declaration. Add `pub use event_loop::handle_image_ready;` to re-export.

5. **Write tests in `crates/anvilml-scheduler/tests/event_loop_tests.rs`.** Four tests minimum:
   - `test_image_ready_saves_artifact`: Construct a mock `ArtifactStore` (in-memory SQLite + temp dir), create a valid `WorkerEvent::ImageReady` with a known base64-encoded PNG payload, call `handle_image_ready()`, verify the returned hash matches, and verify the artifact is retrievable by hash via `store.get(&hash)`.
   - `test_image_ready_artifact_meta_fields_match`: After saving, list artifacts and verify `width`, `height`, `seed`, `steps`, and `job_id` fields match the event's values.
   - `test_image_ready_malformed_base64_errors`: Pass a deliberately malformed base64 string (e.g. `"not-valid-base64!!!@@@")` in the event, verify `handle_image_ready()` returns `Err(AnvilError::Serde(...))` rather than panicking.
   - `test_image_ready_non_image_event_ignored`: Call `handle_image_ready()` with a `WorkerEvent::Completed` (not `ImageReady`), verify it returns an error or is handled gracefully — actually, the function should only accept `ImageReady`. The approach is: the caller matches on the event variant before calling `handle_image_ready()`, so `handle_image_ready()` always receives an `ImageReady`. Instead, test that the function correctly decodes a valid base64 payload that is NOT a valid PNG (e.g. random bytes base64-encoded) — `ArtifactStore::save()` does not validate PNG format, it just stores raw bytes, so this is fine. Better fourth test: `test_image_ready_empty_image_b64`: pass an empty base64 string, verify it decodes to empty bytes and `save()` succeeds (empty artifact stored).

   Test helper: `create_test_artifact_store()` — creates a temp directory + in-memory SQLite pool with artifacts table, returns `Arc<ArtifactStore>`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `handle_image_ready` | `anvilml-scheduler::event_loop` | `pub async fn handle_image_ready(artifact_store: Arc<ArtifactStore>, event: WorkerEvent, job_id: Uuid) -> Result<String, AnvilError>` |

**Structural change to existing pub item:**
| Item | Before | After |
|------|--------|-------|
| `JobScheduler::new()` | `fn new(job_store: JobStore, node_registry: Arc<NodeTypeRegistry>) -> Self` | `fn new(job_store: JobStore, node_registry: Arc<NodeTypeRegistry>, artifact_store: Arc<ArtifactStore>) -> Self` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/event_loop.rs` | New module; `handle_image_ready()` function |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod event_loop;` and `pub use event_loop::handle_image_ready;` |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `artifact_store` field and constructor parameter to `JobScheduler` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `base64 = "0.22.1"` dependency; bump version 0.1.19 → 0.1.20 |
| CREATE | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | ≥4 integration tests for event_loop functionality |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/event_loop_tests.rs` | `test_image_ready_saves_artifact` | Simulated ImageReady results in a saved artifact retrievable by hash | In-memory SQLite pool + temp artifact dir | Valid base64-encoded PNG bytes in `WorkerEvent::ImageReady` | `store.get(&hash)` returns `Ok(Some(bytes))` matching the decoded payload | `cargo test -p anvilml-scheduler --test event_loop_tests -- test_image_ready_saves_artifact` exits 0 |
| `tests/event_loop_tests.rs` | `test_image_ready_artifact_meta_fields_match` | ArtifactMeta fields (width, height, seed, steps, job_id) match the event values | In-memory SQLite pool + temp artifact dir | `WorkerEvent::ImageReady` with known width=512, height=512, seed=42, steps=20, job_id=known-uuid | `store.list(Some(job_id))` returns one row with matching fields | `cargo test -p anvilml-scheduler --test event_loop_tests -- test_image_ready_artifact_meta_fields_match` exits 0 |
| `tests/event_loop_tests.rs` | `test_image_ready_malformed_base64_errors` | Malformed base64 errors rather than panics | In-memory SQLite pool + temp artifact dir | `WorkerEvent::ImageReady` with `image_b64 = "not-valid-base64!!!@@@"` | Returns `Err(AnvilError::Serde(...))` — no panic | `cargo test -p anvilml-scheduler --test event_loop_tests -- test_image_ready_malformed_base64_errors` exits 0 |
| `tests/event_loop_tests.rs` | `test_image_ready_empty_image_b64` | Empty base64 string decodes to empty bytes and save succeeds | In-memory SQLite pool + temp artifact dir | `WorkerEvent::ImageReady` with `image_b64 = ""` | Returns `Ok(hash)`; artifact stored with 0 bytes | `cargo test -p anvilml-scheduler --test event_loop_tests -- test_image_ready_empty_image_b64` exits 0 |

## CI Impact

No CI changes required. The `base64` crate is already a transitive dependency in the workspace, so adding it as a direct dependency does not change the dependency tree or CI job behavior. The new test file is picked up by `cargo test --workspace --features mock-hardware` automatically (Cargo discovers `tests/*.rs` files).

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. Base64 decoding and SHA-256 hashing are platform-neutral. File path construction uses `PathBuf::join()` which handles platform separators. The temp directory for tests uses `std::env::temp_dir()` which is platform-correct.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobScheduler::new()` signature change breaks existing call sites (`backend/main.rs`, test fixtures) | High | High | The task context says `JobScheduler` needs an `Arc<ArtifactStore>` constructor field added. All existing call sites must be updated — `backend/main.rs` constructs the scheduler and will need to pass `artifact_store`. Test fixtures in `scheduler_tests.rs` also need updating. This is a known constraint noted in the TASKS_PHASE015.md "Known Constraints" section. |
| `base64::engine::general_purpose::STANDARD.decode()` API differs from training-data memory | Medium | Medium | MCP confirmed version 0.22.1 uses the Engine trait pattern: `STANDARD.decode(&str) -> Result<Vec<u8>, DecodeError>`. The plan uses this exact API. If MCP is unavailable at ACT time, the acting agent must confirm the API shape before writing code. |
| Test temp directory cleanup conflicts between parallel tests | Low | Low | Each test creates its own `tempfile::tempdir()` or `std::env::temp_dir()` subdirectory. `tempfile` crate is not a direct dependency — use `std::fs::create_dir_all()` with a UUID-based subdirectory name for isolation, and let the OS clean up temp dirs. |
| `interim_job_completion.rs` module doc says "delete this file and this line" when Phase 16 executes — but this task doesn't touch it | Low | None | This task only reads the interim module for context. No code changes to `interim_job_completion.rs` are made. The plan notes this explicitly to avoid scope creep. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests` exits 0
- [ ] `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo build -p anvilml` exits 0 (confirms `JobScheduler::new()` call sites updated)
