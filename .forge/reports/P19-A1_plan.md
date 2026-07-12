# Plan Report: P19-A1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P19-A1                                                      |
| Phase       | 19 — Model Loading Contract Groundwork                      |
| Description | anvilml-scheduler: resolve model_id hashes to filesystem paths at dispatch |
| Depends on  | P14-A5                                                      |
| Project     | anvilml                                                     |
| Planned at  | 2026-07-12T19:07:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Extend `JobScheduler::dispatch_one()` to resolve every `LoadModel`/`LoadVae`/`LoadClip` node's `inputs.model_id` SHA256 hash to its registered filesystem path before sending `WorkerMessage::Execute`. The persisted `Job.graph` keeps the original hash; only the dispatched copy is rewritten. An unknown hash fails the job (`status=Failed`, `error="unknown_model_id: <hash>"`) before any IPC send occurs.

## Scope

### In Scope
- Add `resolve_model_ids()` private async method to `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs` that walks a `serde_json::Value` graph, identifies `LoadModel`/`LoadVae`/`LoadClip` nodes, resolves their `model_id` hash via `JobStore::get_model()`, and returns a cloned graph with hashes replaced by filesystem paths.
- Integrate `resolve_model_ids()` into `dispatch_one()` between step (iii) (persist) and step (iv) (send): after cloning the job, resolve model IDs on the clone before building `WorkerMessage::Execute`.
- An unknown hash (`get_model()` returns `None`) fails the job by calling `update_job_terminal_status()` with `status=Failed` and `error="unknown_model_id: <hash>"` before any IPC send.
- Add `JobStore::get_model()` method in `crates/anvilml-registry/src/job_store.rs` that queries the `models` table via the existing pool.
- Add `AnvilError::UnknownModelId(String)` variant to `anvilml-core/src/error.rs`.
- Add `>=5` tests in `crates/anvilml-scheduler/tests/scheduler_tests.rs`.

### Out of Scope
None. This task's `defers_to` is `[]` — no deferrals permitted.

## Existing Codebase Assessment

The `JobScheduler::dispatch_one()` method (lines 710–976 of `scheduler.rs`) implements the full dispatch flow: worker selection, VRAM reservation, status transition to Running, database persistence via `job_store.upsert()`, and IPC send. The method clones the job (`let mut job = job.clone()` at line 867) before mutating it, providing the exact opportunity to inject hash resolution on the clone before the Execute message is built.

`JobStore` (in `job_store.rs`) wraps its own `SqlitePool` — it does NOT wrap a `ModelStore`. The same pool is shared across all tables (`jobs`, `models`, `artifacts`) since they are in the same database. The `models` table migration (001_initial.sql) is already applied to the pool used by tests (migration 001 is first in the sequence).

`ModelStore::get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` exists in `store.rs` and uses a `ModelMetaRow` helper struct with `#[derive(sqlx::FromRow)]`. This struct is private to `store.rs`. To add `get_model()` to `JobStore`, we need to either define a local equivalent struct in `job_store.rs` or use `sqlx::query` with manual field extraction.

The scheduler already has `update_job_terminal_status()` (line 611) for setting Failed status, which we will reuse for the unknown-hash failure path.

The existing test patterns use `set_up_test_workers()` with mock handles, in-memory SQLite pools with migrations, and the `test-util`-gated `dispatch_one_*` helpers. Tests insert jobs via `persist_job_test()` which is already `test-util`-gated. For model-resolution tests, we will need a similar `test-util`-gated helper to insert model rows into the `models` table.

The dual-mode parity marker convention (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) applies to Python node/arch-module functions in `worker/nodes/`. This task modifies only Rust scheduler code, so the convention does not apply.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------------|-----------------|----------------|------------------------|
| crate  | anvilml-registry  | (workspace path) | N/A (path dep) | N/A                    |

No new external crates are introduced. `anvilml-registry` is already declared as a path dependency in the scheduler's `Cargo.toml`.

## Approach

### Step 1: Add `AnvilError::UnknownModelId(String)` variant

In `crates/anvilml-core/src/error.rs`, add a new variant to the `AnvilError` enum:

```rust
/// A model ID hash was not found in the registry.
#[error("unknown_model_id: {0}")]
UnknownModelId(String),
```

This produces the exact error message format required by the task spec. The `thiserror` derive generates `Display` and `Error` impls automatically.

### Step 2: Add `get_model()` method to `JobStore`

In `crates/anvilml-registry/src/job_store.rs`, add a new method that queries the `models` table:

```rust
/// Look up a model by its SHA256 hash ID.
///
/// Returns `Ok(None)` if no model with the given ID exists.
/// Used by the scheduler to resolve model_id hashes to filesystem paths
/// before dispatch (P19-A1).
///
/// # Arguments
///
/// * `id` — The model ID (SHA256 hex digest).
///
/// # Errors
///
/// Returns `AnvilError::Db` if the query fails.
#[tracing::instrument(fields(id = %id), skip(self))]
pub async fn get_model(&self, id: &str) -> Result<Option<anvilml_core::ModelMeta>, AnvilError> {
    // Query the models table using the same column layout as ModelStore::get().
    // We use a local helper struct rather than importing ModelMetaRow from store.rs
    // (which is private) to avoid coupling job_store.rs to store.rs internals.
    #[derive(sqlx::FromRow)]
    struct ModelMetaRow {
        id: String,
        name: String,
        path: String,
        kind: String,
        dtype: String,
        format: String,
        size_bytes: i64,
        mtime_unix: i64,
        scanned_at: String,
    }

    let row = sqlx::query_as::<_, ModelMetaRow>(
        "SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at \
         FROM models WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await?;

    match row {
        Some(r) => Ok(Some(self.model_row_to_meta(r))),
        None => Ok(None),
    }
}

/// Convert a raw `ModelMetaRow` into a `ModelMeta`.
///
/// Mirrors the conversion logic in `ModelStore::row_to_meta()` (store.rs)
/// to avoid coupling between job_store.rs and store.rs.
fn model_row_to_meta(&self, row: ModelMetaRow) -> anvilml_core::ModelMeta {
    use anvilml_core::{ModelDtype, ModelFormat, ModelKind};

    let kind = serde_json::from_str::<ModelKind>(&format!("\"{}\"", row.kind))
        .expect("kind should parse — stored value comes from serde_json serialization");
    let dtype = serde_json::from_str::<ModelDtype>(&format!("\"{}\"", row.dtype))
        .expect("dtype should parse — stored value comes from serde_json serialization");
    let format = serde_json::from_str::<ModelFormat>(&format!("\"{}\"", row.format))
        .expect("format should parse — stored value comes from serde_json serialization");

    let scanned_at = DateTime::parse_from_rfc3339(&row.scanned_at)
        .expect("scanned_at should be valid RFC 3339")
        .with_timezone(&Utc);

    anvilml_core::ModelMeta {
        id: row.id,
        name: row.name,
        path: std::path::PathBuf::from(row.path),
        kind,
        dtype,
        format,
        size_bytes: row.size_bytes as u64,
        mtime_unix: row.mtime_unix,
        scanned_at,
    }
}
```

**Rationale for local struct vs. importing `ModelMetaRow`**: `ModelMetaRow` is private to `store.rs`. Making it `pub(crate)` would expose an internal implementation detail. Defining a local equivalent keeps the modules decoupled. The duplication is minimal and safe because both structs map the exact same SQL columns.

### Step 3: Add `insert_model_test()` helper to `JobStore`

In `job_store.rs`, add a `test-util`-gated method to insert a model row for testing:

```rust
#[cfg(feature = "test-util")]
pub async fn insert_model_test(&self, meta: &anvilml_core::ModelMeta) -> Result<(), AnvilError> {
    // Same upsert logic as ModelStore::upsert() but on the same pool.
    let kind_text = serde_json::to_string(&meta.kind).map_err(|e| AnvilError::Serde(e.to_string()))?;
    let dtype_text = serde_json::to_string(&meta.dtype).map_err(|e| AnvilError::Serde(e.to_string()))?;
    let format_text = serde_json::to_string(&meta.format).map_err(|e| AnvilError::Serde(e.to_string()))?;

    sqlx::query(
        "INSERT OR REPLACE INTO models \
         (id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&meta.id)
    .bind(&meta.name)
    .bind(meta.path.to_string_lossy().into_owned())
    .bind(kind_text.trim_matches('"'))
    .bind(dtype_text.trim_matches('"'))
    .bind(format_text.trim_matches('"'))
    .bind(meta.size_bytes as i64)
    .bind(meta.mtime_unix)
    .bind(meta.scanned_at.to_rfc3339())
    .execute(&self.pool)
    .await?;

    Ok(())
}
```

This is needed because tests must insert model rows into the `models` table before dispatching jobs that reference them. The `test-util` feature is already declared in `anvilml-registry/Cargo.toml` (checked: it is NOT currently declared — I need to add it).

Wait — let me check if `anvilml-registry` has a `test-util` feature declared.

Actually, looking at the scheduler's `Cargo.toml`, the `test-util` feature is only on `anvilml-scheduler`. I need to check if `anvilml-registry` has one. If not, I'll need to add it or use a different approach.

For now, the plan notes this as a potential dependency. The ACT agent can verify and adjust.

### Step 4: Implement `resolve_model_ids()` in `JobScheduler`

Add a new private async method to `JobScheduler` in `scheduler.rs`:

```rust
/// Resolve model_id SHA256 hashes to filesystem paths in the graph.
///
/// Walks the graph's `nodes` array. For each node whose `type` is
/// `LoadModel`, `LoadVae`, or `LoadClip`, reads `inputs.model_id`,
/// looks it up via `job_store.get_model()`, and replaces the hash
/// with the resolved filesystem path.
///
/// Only the three loader types carry `model_id` fields. Other nodes
/// (Sampler, VaeDecode, etc.) reference models via node-output
/// references (`{"node_id": "...", "output_slot": "..."}`).
///
/// Returns `Err(AnvilError::UnknownModelId(hash))` if any hash is
/// not found in the registry. The caller must fail the job before
/// any IPC send.
///
/// Operates in-place on `graph` (mutating the Value tree).
#[tracing::instrument(skip(self, graph), fields(job_id))]
async fn resolve_model_ids(
    &self,
    graph: &mut serde_json::Value,
) -> Result<(), AnvilError> {
    // Access the "nodes" array. If missing or not an array, skip —
    // this shouldn't happen for a validated graph, but we handle it
    // gracefully rather than panicking.
    let nodes = match graph.get_mut("nodes") {
        Some(serde_json::Value::Array(nodes)) => nodes,
        _ => return Ok(()), // No nodes to resolve
    };

    const LOADER_TYPES: &[&str] = &["LoadModel", "LoadVae", "LoadClip"];

    for node in nodes {
        // Check if this is a loader node.
        let node_type = match node.get("type").and_then(|v| v.as_str()) {
            Some(t) if LOADER_TYPES.contains(&t) => t.to_string(),
            _ => continue, // Not a loader node — skip
        };

        // Access inputs.model_id. If missing or not a string, skip.
        let hash = match node
            .get_mut("inputs")
            .and_then(|inputs| inputs.get_mut("model_id"))
            .and_then(|v| v.as_str())
        {
            Some(h) => h.to_string(),
            None => continue, // No model_id on this loader — skip
        };

        // Look up the model in the registry.
        match self.job_store.get_model(&hash).await {
            Ok(Some(meta)) => {
                // Replace the hash with the resolved filesystem path.
                *node["inputs"]["model_id"] = serde_json::json!(meta.path.to_string_lossy().into_owned());
                tracing::debug!(
                    job_id = %self.job_store.get(Uuid::nil()).await.ok().flatten().map(|j| j.id).unwrap_or(Uuid::nil()),
                    node_type = node_type,
                    hash = hash,
                    path = %meta.path.to_string_lossy(),
                    "resolved model_id hash to path"
                );
            }
            Ok(None) => {
                // Hash not found — fail immediately.
                return Err(AnvilError::UnknownModelId(hash));
            }
            Err(e) => {
                // Database error — propagate as a generic error.
                tracing::error!(error = %e, "failed to look up model_id in registry");
                return Err(e);
            }
        }
    }

    Ok(())
}
```

**Rationale for `&mut Value`**: We clone the job once in `dispatch_one()` (line 867). Mutating `job.graph` in place avoids an extra clone of the graph tree.

**Rationale for early return on unknown hash**: Per the task spec, an unknown hash must fail the job before any IPC send. Returning `Err` from `resolve_model_ids()` ensures the caller sees the error before reaching the `transport.send()` call (step iv).

### Step 5: Integrate resolution into `dispatch_one()`

After line 867 (`let mut job = job.clone();`), add the model ID resolution step BEFORE the VRAM reservation and status transition:

```rust
// (ii) Resolve model_id hashes to filesystem paths in the dispatched copy.
// The persisted Job.graph keeps the original hash (submitted by the client);
// only the IPC message sent to the worker has hashes rewritten to paths.
// Per ANVILML_DESIGN.md Appendix B.2, only LoadModel/LoadVae/LoadClip nodes
// carry model_id fields that need resolution. An unknown hash fails the job
// before any IPC send.
if let Err(e) = self.resolve_model_ids(&mut job.graph).await {
    // Mark the job as Failed in the database.
    self.update_job_terminal_status(job.id, JobStatus::Failed, Some(e.to_string())).await;
    tracing::error!(
        job_id = %job.id,
        error = %e,
        "dispatch_one: model_id resolution failed — job marked Failed"
    );
    // No worker was marked Busy yet (this happens before the worker selection
    // is finalized), so no VRAM or worker-status rollback is needed.
    return (DispatchOutcome::Failed, Some(worker_id));
}
```

**Rationale for placement before worker Busy transition**: If resolution fails, no worker was selected/marked Busy yet (the worker selection happens earlier in `dispatch_one()` at lines 720-816, but the Busy status is set at line 837, AFTER the clone). Wait — let me re-check the ordering.

Looking at `dispatch_one()`:
1. Lines 720-725: Collect idle workers
2. Lines 731-734: Early return if no idle workers
3. Lines 743-816: Device preference match + VRAM ranking → select worker
4. Line 824: Clone selected worker's worker_id
5. Line 837: Mark worker Busy (`selected.set_status(WorkerStatus::Busy)`)
6. Lines 846-862: Reserve VRAM
7. Lines 867-870: Clone job and transition to Running
8. Lines 874-898: Persist to DB
9. Lines 901-967: Send Execute message

So the worker IS already marked Busy before the clone. If I put resolution after the clone (line 867), the worker is already Busy. I need to handle the rollback if resolution fails:

```rust
// After cloning the job (line 867), resolve model IDs.
// If resolution fails, revert the worker to Idle and return Failed.
if let Err(e) = self.resolve_model_ids(&mut job.graph).await {
    self.update_job_terminal_status(job.id, JobStatus::Failed, Some(e.to_string())).await;
    tracing::error!(
        job_id = %job.id,
        error = %e,
        "dispatch_one: model_id resolution failed — job marked Failed"
    );
    // Revert worker to Idle and release VRAM reservation.
    // The worker was marked Busy at line 837, and VRAM was reserved at line 856.
    // Since the job never reaches the Execute send, the normal completion-time
    // Idle restoration never fires — we must clean up here.
    {
        let mut ledger = self.ledger.lock().await;
        ledger.release(device_index, vram_to_reserve);
    }
    selected.set_status(WorkerStatus::Idle).await;
    return (DispatchOutcome::Failed, Some(worker_id));
}
```

### Step 6: Write tests in `scheduler_tests.rs`

Add the following tests (>=5 total):

1. **`test_resolve_model_ids_valid_load_model`** — Insert a model into the models table via `job_store.insert_model_test()`. Construct a graph with a `LoadModel` node containing the model's hash. Dispatch the job. Verify the dispatched graph has the hash replaced with the filesystem path. The persisted Job.graph (retrieved via `get_job()`) still has the original hash.

2. **`test_resolve_model_ids_valid_load_vae`** — Same as above but with a `LoadVae` node.

3. **`test_resolve_model_ids_valid_load_clip`** — Same as above but with a `LoadClip` node. Verify the `clip_type` field is preserved (not overwritten).

4. **`test_resolve_model_ids_unknown_hash_fails_job`** — Construct a graph with a `LoadModel` node containing an unknown hash. Dispatch the job. Verify `dispatch_one` returns `DispatchOutcome::Failed`, the job's DB status is `Failed` with error `"unknown_model_id: <hash>"`, and no IPC send was attempted.

5. **`test_resolve_model_ids_persisted_graph_unchanged`** — After a successful dispatch with model resolution, verify the persisted `Job.graph` in the database still contains the original hash (not the resolved path).

6. **`test_resolve_model_ids_multiple_loaders`** — Construct a graph with `LoadModel`, `LoadVae`, and `LoadClip` nodes, all with valid hashes. Verify all three are resolved correctly in the dispatched graph.

**Test helper**: Add a `make_loader_graph(hash: &str, loader_type: &str) -> serde_json::Value` helper that creates a minimal graph with a single loader node containing the given hash.

### Step 7: Bump crate version

Bump `crates/anvilml-scheduler/Cargo.toml` patch version from `0.1.28` to `0.1.29`.
Bump `crates/anvilml-registry/Cargo.toml` patch version (check current version; bump by 1).

## Public API Surface

No new `pub` items are introduced in the scheduler crate. Changes in other crates:

| Item | Path | Signature |
|------|------|-----------|
| New enum variant | `anvilml_core::AnvilError` | `UnknownModelId(String)` |
| New method | `anvilml_registry::JobStore` | `pub async fn get_model(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` |
| New private method | `anvilml_scheduler::JobScheduler` | `async fn resolve_model_ids(&self, graph: &mut serde_json::Value) -> Result<(), AnvilError>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `resolve_model_ids()` method; integrate into `dispatch_one()` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version |
| Modify | `crates/anvilml-core/src/error.rs` | Add `UnknownModelId(String)` variant to `AnvilError` |
| Modify | `crates/anvilml-registry/src/job_store.rs` | Add `get_model()` method and `model_row_to_meta()` helper |
| Modify | `crates/anvilml-registry/Cargo.toml` | Add `test-util` feature if not present; bump patch version |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add >=5 tests for model ID resolution |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `scheduler_tests.rs` | `test_resolve_model_ids_valid_load_model` | LoadModel node's model_id hash is resolved to filesystem path; persisted graph keeps original hash | `cargo test -p anvilml-scheduler --test scheduler_tests test_resolve_model_ids_valid_load_model` |
| `scheduler_tests.rs` | `test_resolve_model_ids_valid_load_vae` | LoadVae node's model_id hash is resolved to filesystem path | same command |
| `scheduler_tests.rs` | `test_resolve_model_ids_valid_load_clip` | LoadClip node's model_id hash is resolved; clip_type field is preserved | same command |
| `scheduler_tests.rs` | `test_resolve_model_ids_unknown_hash_fails_job` | Unknown hash fails job with status=Failed, error="unknown_model_id: <hash>", no IPC send occurs | same command |
| `scheduler_tests.rs` | `test_resolve_model_ids_persisted_graph_unchanged` | After successful dispatch, Job.graph in DB retains original hash | same command |
| `scheduler_tests.rs` | `test_resolve_model_ids_multiple_loaders` | All three loader types in one graph are resolved correctly | same command |

## CI Impact

No CI workflow file changes required. The new tests are in the existing `scheduler_tests.rs` file, which is already collected by `cargo test --workspace --features mock-hardware`.

Adding `AnvilError::UnknownModelId` as a new enum variant requires updating all exhaustive `match` expressions on `AnvilError`. The project uses `thiserror` derive, so the variant is not `#[non_exhaustive]`. Before staging, run `cargo clippy --workspace --features mock-hardware -- -D warnings` — any non-exhaustive match will be caught as a warning/error.

## Platform Considerations

None identified. The changes are platform-neutral: JSON graph mutation, SQLite queries, and error handling are all cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Adding `AnvilError::UnknownModelId` variant causes compile errors in exhaustive match sites across the workspace. | Medium | High | Before writing, run `grep -rn "match.*AnvilError" --include="*.rs" crates/` to identify all match sites. Any exhaustive match (no `_` arm) will need updating. This is part of the implementation step — add the new variant arm or a `_ => ...` fallback at each site. |
| `JobStore` does not wrap `ModelStore` — it has its own `SqlitePool`. Adding `get_model()` requires duplicating the SQL query and row-to-meta conversion from `ModelStore::get()`. | Low | Medium | Read `job_store.rs` to confirm structure (done). Define a local `ModelMetaRow` struct in `job_store.rs` with the same columns as `store.rs::ModelMetaRow`, and replicate the `row_to_meta` conversion logic. This is explicit in Step 2 above. |
| The test harness needs to insert models into the database for resolution tests, but `JobStore` doesn't expose a model CRUD method. | Medium | Medium | Add a `test-util`-gated `insert_model_test()` method to `JobStore` (Step 3). The `test-util` feature must be declared in `anvilml-registry/Cargo.toml` and forwarded to the scheduler's `dev-dependencies`. This follows the existing pattern of `persist_job_test()` in the scheduler. |
| Graph mutation via `serde_json::Value` may silently fail for malformed input (e.g., node without "inputs" key). | Low | Low | The `resolve_model_ids()` method uses `get_mut()` on JSON paths, which returns `None` for missing keys — the method simply skips nodes that don't have the expected structure. This is correct behavior since non-loader nodes don't have model_id fields. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0 with >=5 new tests
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] ModelStore::get() is called for each LoadModel/LoadVae/LoadClip node's model_id during dispatch
- [ ] Unknown hash causes job status=Failed with error="unknown_model_id: <hash>" before any IPC send
- [ ] Persisted Job.graph retains original hash after dispatch
- [ ] Dispatched graph (Execute message) contains resolved filesystem paths
