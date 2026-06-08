# Plan Report: P902-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P902-B2                                     |
| Phase       | 902 — Stabilisation Retrofit                |
| Description | anvilml-scheduler: retrofit mandatory job-store and queue DEBUG log points (job_store.rs, queue.rs) |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-08T17:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Add four mandatory §11.5 DEBUG-level `tracing::debug!` log points to the job-store (`job_store.rs`) and in-memory queue (`queue.rs`) modules of `anvilml-scheduler`. These are pure instrumentation additions — no logic changes, no behavioral changes, no test changes.

## Scope

### In Scope
- `crates/anvilml-scheduler/src/job_store.rs`: add one `tracing::debug!` at end of `insert_job()`, one at end of `update_status()`
- `crates/anvilml-scheduler/src/queue.rs`: add one `tracing::debug!` at end of `enqueue()`, one when `pop_next()` returns `Some`
- Bump `anvilml-scheduler` crate patch version from `0.1.8` to `0.1.9` (FORGE_AGENT_RULES §12)

### Out of Scope
- No changes to any other crate or file
- No test modifications
- No CI, build, or config file changes
- No behavior/logic changes beyond adding log calls

## Approach

1. **job_store.rs — `insert_job()`**: After the `.execute(pool).await?;` line and before `Ok(job.id)`, insert:
   ```rust
   tracing::debug!(job_id = %job.id, "job inserted into DB");
   ```

2. **job_store.rs — `update_status()`**: After `Ok(rows_affected.rows_affected() > 0)` is computed (before the `Ok(...)` return), insert:
   ```rust
   tracing::debug!(job_id = %id, status = ?new_status, "job status updated in DB");
   ```

3. **queue.rs — `enqueue()`**: After `inner.push_back(job);`, insert:
   ```rust
   tracing::debug!(job_id = %job.id, queue_len = self.len(), "job enqueued");
   ```

4. **queue.rs — `pop_next()`**: In the `.map(...)` closure on the last line, after removing the job from the deque, insert:
   ```rust
   tracing::debug!(job_id = %removed_job.id, "job dequeued");
   ```
   Since the current code does `inner.iter().position(...).map(|pos| inner.remove(pos).expect("position found"))`, we need to restructure slightly: bind the result of `remove` to a variable first, log, then return it. The restructuring is minimal and preserves the exact same behavior — just extracting the returned `Job` into a local binding so its `.id` can be logged before returning.

5. **Version bump**: Read current version `0.1.8` from `crates/anvilml-scheduler/Cargo.toml`, write `0.1.9`.

6. **Verification**: Run `cargo test -p anvilml-scheduler --features mock-hardware` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Add 2× `tracing::debug!` calls (§11.5) |
| Modify | `crates/anvilml-scheduler/src/queue.rs` | Add 2× `tracing::debug!` calls (§11.5); minor refactor in `pop_next()` to bind removed job for logging |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.8 → 0.1.9` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| (existing) | `test_insert_and_get` (job_store.rs) | Confirms insert_job + get_job still work after DEBUG addition |
| (existing) | `test_list_jobs_*` (job_store.rs) | Confirms list operations unaffected |
| (existing) | `test_update_status` (job_store.rs) | Confirms update_status still works after DEBUG addition |
| (existing) | `test_enqueue_pop_order` (queue.rs) | Confirms FIFO enqueue/pop still works after DEBUG additions |
| (existing) | `test_cancel_skipped_on_pop` (queue.rs) | Confirms cancellation skip logic unaffected |

No new test files are needed — the existing tests exercise all four modified functions and must exit 0.

## CI Impact

No CI changes required. The task only adds DEBUG-level log calls to existing code paths; no new files, no build scripts, no configuration changes. All existing CI gates (format, clippy, tests, cross-checks) continue to apply unchanged.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobStatus` does not implement `Debug`, causing compile error on `?new_status` in `update_status()` log | Low | Build failure | Verify `JobStatus` derives/implements `Debug`; if not, use `%new_status` (Display) instead |
| Minor refactor of `pop_next()` to bind the removed job changes observable behavior | Very low | Logic change | The binding is purely local; the same `Job` value is returned. No control-flow or side-effect changes |
| `tracing` crate not available in `anvilml-scheduler` | None | Build failure | Verified: `tracing = { workspace = true }` is declared in `Cargo.toml` line 14 |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] Four DEBUG log points are present at the exact locations specified in Approach steps 1–4
- [ ] `anvilml-scheduler` crate version bumped to `0.1.9`
- [ ] No public API signatures changed (no new/removed `pub` items, no changed function signatures)
- [ ] `cargo fmt --all -- --check` exits 0
