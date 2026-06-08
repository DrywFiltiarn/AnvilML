# Plan Report: P902-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P902-B1                                     |
| Phase       | 902 — Stabilisation Retrofit                |
| Description | anvilml-scheduler: retrofit mandatory job dispatch and state-transition DEBUG log points (scheduler.rs) |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-08T17:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the §11.5 mandatory DEBUG-level job state-transition log point to `JobScheduler::submit()` in `crates/anvilml-scheduler/src/scheduler.rs`, so that every successful job insertion into the database is accompanied by a structured debug log recording the `job_id` and the `Queued` status transition.

## Scope

### In Scope
- Add one `tracing::debug!` call in `scheduler.rs` `submit()` after `insert_job()` succeeds, with fields `job_id = %job_id`, `status = "Queued"`, and message `"job status transition"`.
- Bump `anvilml-scheduler` crate patch version from `0.1.7` to `0.1.8` in `Cargo.toml` (FORGE_AGENT_RULES §12).

### Out of Scope
- No changes to `job_store.rs` or `queue.rs` — those are covered by task P902-B2.
- No dispatch loop (`Running` transition) log points — those belong to Phase 13.
- No logic changes, no new functions, no test files.
- No dependency version changes.

## Approach

1. **Read** `crates/anvilml-scheduler/src/scheduler.rs` and locate the `submit()` method (line 59).
2. **Insert** the following line immediately after line 83 (`insert_job(...)` call), before the existing `tracing::info!` on line 85:

   ```rust
   tracing::debug!(job_id = %job_id, status = "Queued", "job status transition");
   ```

   The resulting sequence in `submit()` will be:
   - Line ~83: `insert_job(&self.db, &job).await.map_err(...)?;`
   - Line ~84: `tracing::debug!(job_id = %job_id, status = "Queued", "job status transition");` ← new
   - Line ~85: `tracing::info!(job_id = %job_id, "job submitted and persisted as Queued");`

3. **Bump** the `anvilml-scheduler` crate version in `crates/anvilml-scheduler/Cargo.toml` from `0.1.7` to `0.1.8` (only the patch digit).
4. **Verify** that no public signatures changed: `grep -n "^pub " crates/anvilml-scheduler/src/scheduler.rs`.
5. **Run** acceptance criterion: `cargo test -p anvilml-scheduler --features mock-hardware` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add one `tracing::debug!` call in `submit()` after `insert_job` succeeds. |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.7 → 0.1.8`. |

## Tests

None. This task adds only a logging call with no logic changes; existing tests in the crate exercise the same `submit()` path and will pass without modification. The acceptance criterion is that the existing test suite exits 0.

## CI Impact

No CI workflow files are modified. The existing `cargo test --workspace --features mock-hardware` gate (ENVIRONMENT.md §9) will cover this crate's tests. Adding a DEBUG-level log call cannot cause a test failure — it is invisible at the default INFO level.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing::debug!` macro syntax produces a compile error | Low | Build failure | Use the exact same structured-field pattern already present on line 85 (`job_id = %job_id, "message"`). Verify with `cargo check -p anvilml-scheduler --features mock-hardware`. |
| Version bump causes unexpected dependency resolution | None | N/A | Patch-only bump in a workspace path-dependency crate; no version pins to other crates. |
| Existing tests break due to log output capture | None | Test failure | DEBUG logs are filtered at INFO level by default; test assertions do not inspect log output. |

## Acceptance Criteria

- [ ] `tracing::debug!(job_id = %job_id, status = "Queued", "job status transition");` exists in `scheduler.rs` after `insert_job()` succeeds
- [ ] No other code paths or files are modified
- [ ] `anvilml-scheduler` Cargo.toml version is `0.1.8`
- [ ] No public function signatures changed (`grep -n "^pub " crates/anvilml-scheduler/src/scheduler.rs` confirms only pre-existing pub items)
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0
