# Plan Report: P903-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-A1                                     |
| Phase       | 903 — Pipeline Cache & Model Path Resolution Retrofit |
| Description | anvilml-scheduler: resolve model_id hash to filesystem path at dispatch time |
| Depends on  | P18-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-21T20:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add model ID resolution to the scheduler's dispatch loop so that every `LoadModel`, `LoadVae`, and `LoadClip` node's `inputs.model_id` (a SHA256 hex digest) is rewritten to the resolved absolute filesystem path from `ModelStore` before the `WorkerMessage::Execute` is sent. If any model ID cannot be resolved, the job is marked `Failed` in the database with a clear, actionable error and `Execute` is never dispatched. This fills a gap discovered in Phase 903: job graphs contain opaque SHA256 hashes but no mechanism existed to resolve them to paths inside the Python worker.

## Scope

### In Scope
- Add `model_store: Arc<ModelStore>` field to `JobScheduler` struct (same pattern as existing `node_registry: Arc<NodeTypeRegistry>`).
- Update `JobScheduler::new()` constructor to accept `Arc<ModelStore>` as a new parameter.
- Implement `resolve_model_ids(graph: &mut serde_json::Value, model_store: &ModelStore) -> Result<(), String>` helper that deep-walks the graph's `nodes` array, matches nodes by `type` against `LoadModel`, `LoadVae`, `LoadClip`, and replaces `inputs.model_id` from hash to path.
- Integrate the resolver into `dispatch_once()` before constructing `WorkerMessage::Execute`.
- On resolution failure: mark job `Failed` in DB with error message `model_id "<hash>" not found in registry -- run POST /v1/models/rescan`, broadcast `WsEvent::JobFailed`, and continue to next job (existing per-job continue pattern).
- Add `tests/model_resolve_tests.rs` with ≥ 3 tests.
- Bump `anvilml-scheduler` crate patch version.

### Out of Scope
- **P903-A2** (worker/worker_main.py: wire real PipelineCache into NodeContext) — deferred to the next task in this phase.
- Any changes to the IPC message format (`WorkerMessage::Execute`) — graph is rewritten in place before encoding, no new fields.
- Any changes to `anvilml-registry` or `ModelStore` — the store's `get()` API is used as-is.
- Submit-time resolution — this task resolves at dispatch time only, as specified.

## Existing Codebase Assessment

The `JobScheduler` struct (`crates/anvilml-scheduler/src/scheduler.rs`) already holds `node_registry: Arc<NodeTypeRegistry>` and passes it through the constructor. The dispatch loop runs in `dispatch_once()`, a static async method called from `start_dispatch_loop()`. Jobs are stored as `Job { graph: serde_json::Value, ... }` — the graph is opaque JSON that gets cloned into `WorkerMessage::Execute` at line 944-949 of scheduler.rs.

`ModelStore` (`crates/anvilml-registry/src/store.rs`) provides `async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` — exactly the API needed. `ModelMeta.path` is a `String` (not `PathBuf` as the design doc initially suggested). The scheduler crate already lists `anvilml-registry` as a path dependency in its `Cargo.toml`.

The existing test pattern (seen in `tests/dispatch_tests.rs`) uses `open_in_memory()` from `anvilml_registry`, builds a `WorkerPool` with pre-built status handles, creates a scheduler, registers devices in the ledger, submits jobs, starts the dispatch loop, and verifies outcomes via queue state, ledger state, and direct DB queries. The `#[serial]` attribute is used throughout because the in-memory SQLite pool is single-connection.

## Resolved Dependencies

| Type   | Name             | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------------|-----------------|----------------|------------------------|
| crate  | anvilml-registry | 0.1.4 (path dep) | Cargo.toml     | n/a                    |

No new external dependencies introduced. `anvilml-registry` is already a path dependency of `anvilml-scheduler`. The `ModelStore::get()` method signature was confirmed by reading `crates/anvilml-registry/src/store.rs`: `pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>`. `ModelMeta.path` is `String` (confirmed in `crates/anvilml-core/src/types/model.rs`).

## Approach

1. **Add `model_store` field to `JobScheduler`.** In `scheduler.rs`, add `model_store: Arc<ModelStore>` to the struct (after `artifact_store`, before `notify`). This mirrors the existing `node_registry` field pattern.

2. **Update `JobScheduler::new()` constructor.** Add `model_store: Arc<ModelStore>` as a new parameter (after `workers`). Store it in `Self { model_store, ... }`. The `#[tracing::instrument]` skip list must include `model_store`.

3. **Implement the graph resolver function.** Add a new private async method `resolve_model_ids(&self, graph: &mut serde_json::Value) -> Result<(), String>` to `JobScheduler`. This function:
   - Checks `graph["nodes"]` exists and is an array; returns `Ok(())` early if absent or not an array (forward compatible — silently skips malformed graphs).
   - Iterates each element of the `nodes` array.
   - For each node, reads `node["type"]` as a string.
   - If the type is exactly `"LoadModel"`, `"LoadVae"`, or `"LoadClip"`:
     - Reads `node["inputs"]["model_id"]` as a string (the SHA256 hash).
     - If `model_id` is absent or not a string, logs at WARN and skips that node (robustness — future node types may not use `model_id`).
     - Calls `self.model_store.get(&model_id).await`.
     - If `get()` returns `Ok(None)`, returns `Err(format!("model_id \"{}\" not found in registry -- run POST /v1/models/rescan", model_id))` immediately — no partial resolution.
     - If `get()` returns `Ok(Some(meta))`, replaces `node["inputs"]["model_id"]` with `meta.path.clone()`.
   - Returns `Ok(())` after all loader nodes resolved successfully.

4. **Integrate resolver into `dispatch_once()`.** In the `dispatch_once` method, after popping the job from the queue (line 897) and before the DB status updates (line 903), call `self.resolve_model_ids(&mut graph).await`. If it returns `Err(msg)`:
   - Update the job's DB status to `failed` with the error message.
   - Broadcast `WsEvent::JobFailed { job_id, error: msg }`.
   - Log at WARN with `job_id` and `error`.
   - `continue` to the next job in the dispatch loop (the existing per-job continue pattern used on IPC send failure at line 953).
   - Do NOT reserve VRAM, do NOT send Execute.

5. **Update `start_dispatch_loop()` to pass `model_store`.** Since `dispatch_once` needs `&self` (to access `model_store`), convert `dispatch_once` from a static method to a non-static method on `&self`. Update `start_dispatch_loop` to call `self.dispatch_once().await` instead of `Self::dispatch_once(...)`. Clone `model_store` into the spawned task closure.

6. **Add tests in `tests/model_resolve_tests.rs`.** Three tests using the established pattern from `dispatch_tests.rs`:
   - `test_resolves_known_model_id`: Seed the in-memory DB with a `ModelMeta` whose `id` matches the hash in the graph, submit a job with a `LoadModel` node, verify the resolved path appears in the Execute message (verified by checking the DB `graph` column after dispatch, since the Execute is sent but we inspect the graph that was dispatched).
   - `test_unknown_model_id_fails_job_without_dispatch`: Submit a job with a `LoadModel` node whose model ID does not exist in the store, verify the job status becomes `Failed` and the job remains in the queue (not dispatched).
   - `test_non_loader_node_inputs_untouched`: Submit a job with a `Sampler` node (not a loader type) carrying a `seed` input, verify the node's inputs are unchanged after the resolution pass.

7. **Bump crate version.** Increment `anvilml-scheduler` version from `0.1.13` to `0.1.14` in `Cargo.toml`.

## Public API Surface

No new `pub` items are introduced. All changes are internal to `JobScheduler`:

- **Modified field:** `JobScheduler` gains `model_store: Arc<ModelStore>` (private field).
- **Modified signature:** `JobScheduler::new()` gains `model_store: Arc<ModelStore>` parameter.
- **New private method:** `JobScheduler::resolve_model_ids(&self, graph: &mut serde_json::Value) -> Result<(), String>` — not public, called only from `dispatch_once`.
- **Signature change:** `dispatch_once` changes from `async fn dispatch_once(queue, ledger, db, workers, broadcaster)` (static) to `async fn dispatch_once(&self)` (instance method). This is an internal-only change — `dispatch_once` is not `pub`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `model_store` field, update constructor, implement resolver, integrate into dispatch loop |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.13 → 0.1.14 |
| CREATE | `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | ≥ 3 integration tests for model ID resolution |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | `test_resolves_known_model_id` | A `LoadModel` node's `model_id` is rewritten from hash to resolved path before dispatch | `ModelStore` seeded with one `ModelMeta` whose `id` matches the hash in the submitted graph; ledger registered with device 0; idle worker pool available | Job graph with `LoadModel{inputs: {model_id: "<known-hash>"}}` | Dispatch completes; DB `graph` column for the dispatched job contains the resolved absolute path instead of the hash; job status is `running` | `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0 |
| `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | `test_unknown_model_id_fails_job_without_dispatch` | An unresolvable hash fails the job and never sends Execute, VRAM is not reserved | Empty `ModelStore` (in-memory DB with no models table data); ledger registered; idle worker pool available | Job graph with `LoadModel{inputs: {model_id: "nonexistent_hash"}}` | Job status becomes `Failed`; error message contains `model_id "nonexistent_hash" not found in registry -- run POST /v1/models/rescan`; job remains in queue; VRAM reservation is 0 | `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0 |
| `crates/anvilml-scheduler/tests/model_resolve_tests.rs` | `test_non_loader_node_inputs_untouched` | Nodes other than LoadModel/LoadVae/LoadClip pass through the resolution pass untouched | `ModelStore` seeded with a model; ledger registered; idle worker pool available | Job graph with a `Sampler` node carrying `inputs: {seed: 42}` (a value that happens to be hash-like in format) | `Sampler.inputs.seed` is unchanged (`42`) after resolution pass; job dispatches normally | `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0 |

## CI Impact

No new CI jobs. The new `model_resolve_tests.rs` file is automatically discovered by `cargo test --workspace --features mock-hardware` (standard Rust test crate discovery in `crates/{name}/tests/`). The `rust-linux` and `rust-windows` CI matrix jobs pick it up. No CI configuration changes needed.

## Platform Considerations

None identified. The resolver operates on `serde_json::Value` (platform-neutral) and queries SQLite via `SqlitePool` (already used cross-platform). The `ModelMeta.path` field is a `String` regardless of platform. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `dispatch_once` changing from static to instance method may require updates in `start_dispatch_loop` and any other call sites | Low | Medium | `dispatch_once` is only called from `start_dispatch_loop` within the same file. Update both in the same edit. No other call sites exist (confirmed by grep). |
| Graph structure mismatch: the actual graph JSON may not have `inputs` as a flat dict — it may use a different key or nested structure | Low | High | The resolver uses `node.get("inputs")?.get("model_id")` which safely returns `None` if keys are absent, preventing panics. The test `test_non_loader_node_inputs_untouched` verifies the resolver doesn't mutate non-loader nodes. If the ACT agent discovers a different structure at implementation time, the resolver is adjusted to match. |
| `ModelStore::get()` is async but called inside a loop that previously had no `.await` | Low | Low | The resolver is a new async method; `dispatch_once` is already async. The `.await` on `model_store.get()` is bounded by the DB query latency (microseconds for in-memory, milliseconds for real DB). |
| VRAM reservation happens before model resolution — if resolution fails, VRAM is already reserved | Low | Medium | The resolver is called AFTER VRAM reservation in the current code. Fix: move the resolver call to BEFORE the VRAM reservation block (before line 891 in current code), so a resolution failure prevents VRAM reservation entirely. This is the correct ordering. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- model_resolve` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (all existing tests still pass)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions in other crates)
- [ ] `grep -rn "model_store" crates/anvilml-scheduler/src/scheduler.rs` returns ≥ 5 hits (field, constructor param, method impl, dispatch loop call, start_dispatch_loop clone)
- [ ] `head -1 crates/anvilml-scheduler/Cargo.toml && grep '^version' crates/anvilml-scheduler/Cargo.toml` shows version `0.1.14`
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
