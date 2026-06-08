# Plan Report: P902-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A6                                           |
| Phase       | 902 — Stabilisation Retrofit                      |
| Description | Retrofit mandatory spawn and status-transition DEBUG log points (pool.rs) |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-08T16:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add three mandatory §11.5 DEBUG log points to `crates/anvilml-worker/src/pool.rs`:
one per worker in the spawn path (GPU + CPU) and one per status transition in `set_busy()` / `set_idle()`. These are pure instrumentation additions with zero logic changes.

## Scope

### In Scope
- Add `tracing::debug!(worker_id = %worker_id, device_index = device_index)` after each `ManagedWorker::new()` call in `spawn_all()` (GPU loop at line ~287, CPU fallback at line ~327).
- Add `tracing::debug!(worker_id = %worker_id, from = %old_status, to = %new_status)` in `set_busy()` before the status transition.
- Add `tracing::debug!(worker_id = %worker_id, from = %old_status, to = %new_status)` in `set_idle()` before the status transition.
- Bump `anvilml-worker` patch version from `0.1.13` to `0.1.14`.

### Out of Scope
- No changes to `managed.rs`, `scheduler.rs`, `job_store.rs`, `queue.rs`, or any other crate.
- No logic changes to worker lifecycle, IPC, or dispatch.
- No new tests — this task adds only log calls (§4.6 refactor rule).
- No CI workflow modifications.

## Approach

1. **GPU spawn DEBUG point** (pool.rs, line 287): After `let worker = Arc::new(ManagedWorker::new(format!("worker-{i}"), i as u32));`, add:
   ```rust
   tracing::debug!(worker_id = %format!("worker-{i}"), device_index = i, "spawned worker");
   ```

2. **CPU spawn DEBUG point** (pool.rs, line 327): After `let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));`, add:
   ```rust
   tracing::debug!(worker_id = "worker-0", device_index = 0u32, "spawned worker");
   ```

3. **set_busy DEBUG point** (pool.rs, line 400): Before `worker.set_status(WorkerStatus::Busy).await;`, capture old status and log:
   ```rust
   let old_status = worker.get_status().await;
   tracing::debug!(worker_id = %worker_id, from = ?old_status, to = "Busy", "status transition");
   ```

4. **set_idle DEBUG point** (pool.rs, line 413): Before `worker.set_status(WorkerStatus::Idle).await;`, capture old status and log:
   ```rust
   let old_status = worker.get_status().await;
   tracing::debug!(worker_id = %worker_id, from = ?old_status, to = "Idle", "status transition");
   ```

5. **Version bump**: Read `crates/anvilml-worker/Cargo.toml` current version (`0.1.13`), update to `0.1.14`.

6. **Verification**: Run `cargo test -p anvilml-worker --features mock-hardware` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/pool.rs` | Add 4 DEBUG log points (2 spawn, 2 status transition) |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.13 → 0.1.14` |

## Tests

None. This task adds only instrumentation (DEBUG log calls) — no test files are written or modified. Per FORGE_AGENT_RULES §4.6, refactor tasks that add mandatory §11.5 log points without changing behavior do not require new tests. The existing test suite validates that the code still compiles and behaves identically.

## CI Impact

No CI changes required. The task modifies only `anvilml-worker` source and manifest. The existing CI gates (`cargo clippy --workspace --features mock-hardware`, `cargo test --workspace --features mock-hardware`) will automatically cover these changes. No new jobs, steps, or workflow files are added or modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `get_status()` call in `set_busy`/`set_idle` introduces a read-lock contention window | Low | Low | The existing `locked` read lock is already held during iteration; the `get_status()` call happens inside that same locked scope, so no additional contention is introduced. |
| DEBUG log fields mismatch §11.5 spec (wrong field names) | Low | Medium | Plan uses exact field names from §11.5: `worker_id`, `from`, `to` for status transitions; `worker_id`, `device_index` for spawn. Verified against FORGE_AGENT_RULES §11.5 table. |
| Version bump applied to wrong manifest line | Low | Medium | Only the `[package] version = "..."` line is targeted, as documented in ENVIRONMENT.md §10. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0
- [ ] Four DEBUG log calls present in pool.rs (2 after `ManagedWorker::new()` in `spawn_all()`, 1 each in `set_busy()` and `set_idle()`)
- [ ] `anvilml-worker` Cargo.toml version is `0.1.14`
- [ ] No changes to any file outside the two listed in "Files Affected"
