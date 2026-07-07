# Plan Report: P13-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-C1                                      |
| Phase       | 13 — Job Queue                              |
| Description | backend: wire reset_ghost_jobs() into server startup sequence |
| Depends on  | P13-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-07T08:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Connect the binary's normal (non-`hw-probe`) startup path to the ghost-job reset by constructing a `SqlitePool` and `JobStore`, calling `reset_ghost_jobs()`, and logging the affected count at INFO level if nonzero. This is the first time `main.rs` actually uses a real `SqlitePool` in its normal run path — Phase 6 built the capability, but `main.rs` discarded the pool with `_pool` and never used it.

## Scope

### In Scope
- Modify `backend/src/main.rs`: import `JobStore` from `anvilml_registry`, construct it from the existing pool, call `reset_ghost_jobs()` after the seed loader and before the server starts accepting connections, log the count at INFO level if nonzero.
- No new files, no new public API items, no new dependencies.

### Out of Scope
None. `defers_to (from JSON): []` — this task has an empty defers_to field and implements its full scope. `AppState` does not gain a `db` field in this task — that is deliberately deferred to a later task once more of `AppState` needs the pool, per the Known Constraints in `TASKS_PHASE013.md`.

## Existing Codebase Assessment

**What already exists:**
- `anvilml_registry::create_pool()` (in `db.rs`) creates and migrates a `SqlitePool` — already imported and called in `main.rs` at line 110, but the result is bound to `_pool` (unused) and discarded.
- `anvilml_registry::JobStore` (in `job_store.rs`) has a `new(pool)` constructor and an async `reset_ghost_jobs(&self) -> Result<u32, AnvilError>` method that updates `Queued`/`Running` jobs to `Failed` with `error = "server_restart"`, per `ANVILML_DESIGN.md §19.2`. The method already logs `tracing::info!(count, "ghost jobs reset to failed")` internally when count > 0.
- `anvilml-registry/src/lib.rs` already re-exports `JobStore` as `pub use job_store::JobStore`.
- The seed loader (`SeedLoader::new(pool.clone())`) already clones the pool for its own use, proving the pool is alive and migratable at this point in the startup sequence.

**Established patterns:**
- Error handling: `create_pool()` uses `.map_err(|e| eprintln!(...)).unwrap()` — the same pattern is used for the seed loader. This task follows the same pattern for `reset_ghost_jobs()`.
- Logging: Uses `tracing::info!` with structured fields (e.g. `count = %count`).
- Pool ownership: The pool is created once in the `None` branch, cloned for the seed loader, and will be cloned again for the JobStore.

**Gap between design doc and current source:**
The current `main.rs` creates a pool but discards it (`_pool`). The task context says this should happen "after config_load::load() and before the server starts accepting connections." The pool creation is already at line 110 (in the `None` branch, after the seed loader). The task is purely about wiring: import `JobStore`, construct it, call the method.

## Resolved Dependencies

None. This task only uses existing crates that are already declared in `backend/Cargo.toml`: `anvilml-registry` (already a path dependency) and its transitive dependencies (`sqlx`, `uuid`, `chrono`). No new external crates are introduced.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) |         |                 |                |                        |

## Approach

1. **Add `JobStore` import to `main.rs`.** Add `use anvilml_registry::JobStore;` to the existing import block alongside `use anvilml_registry::create_pool;`. This is a single-line addition at line 10, after the `create_pool` import.

2. **Rename `_pool` to `pool`.** The pool is currently bound to `_pool` (unused). Since the task now uses it via `JobStore::new(pool)` and `pool.clone()` for the seed loader, rename to `pool` to suppress the unused-variable warning. This is a mechanical change at line 110.

3. **Construct `JobStore` and call `reset_ghost_jobs()`.** After the seed loader completes (after line 143, after the `.unwrap()` on the seed loader result, and before the `start_time = Instant::now()` capture at line 147), add:
   ```rust
   // Reset any stale "ghost" jobs left over from a previous run.
   // Ghost jobs are those in Queued or Running state — they may have been
   // in-flight when the server crashed or was restarted. The reset transitions
   // them to Failed with error = "server_restart" so they are visible to the
   // operator and can be retried or discarded.
   // The pool is cloned for the JobStore; the clone is cheap (shared connection
   // pool, not a new database connection).
   let job_store = JobStore::new(pool.clone());
   let ghost_count = job_store.reset_ghost_jobs().await.map_err(|e| {
       eprintln!("Failed to reset ghost jobs: {e}");
       std::process::exit(1);
   }).unwrap();
   ```
   The `reset_ghost_jobs()` method already logs `tracing::info!(count, "ghost jobs reset to failed")` internally when `count > 0`, so no additional logging is needed at the call site. The `map_err`/`.unwrap()` pattern matches the existing error handling style for `create_pool()` and the seed loader.

4. **Verify build.** Run `cargo build -p anvilml` to confirm the change compiles. Run `cargo test --workspace --features mock-hardware` to confirm no regression.

## Public API Surface

None. This task does not introduce any new `pub` items, functions, structs, or traits. It only calls existing public APIs (`JobStore::new`, `JobStore::reset_ghost_jobs`) from `main.rs`, which is a binary (not a library) — its `main()` function is already public by default.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add `JobStore` import, rename `_pool` to `pool`, add `reset_ghost_jobs()` call after seed loader |

## Tests

This task modifies `main.rs` to add a startup-time side effect. The `reset_ghost_jobs()` method itself is tested in `crates/anvilml-registry/tests/job_store_tests.rs` (written by P13-B1). No new tests are needed for this wiring task — the acceptance criteria are the build and full workspace test suite passing, which exercise the code path through the binary's normal startup.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (build) | `cargo build -p anvilml` | The modified `main.rs` compiles without errors or warnings | `cargo build -p anvilml` exits 0 |
| (regression) | `cargo test --workspace --features mock-hardware` | No regression to existing tests (health, nodes, hw-probe) | `cargo test --workspace --features mock-hardware` exits 0 |

## CI Impact

No CI changes required. The task only modifies `backend/src/main.rs` (a single source file) and does not add new test files, new CI gates, or new file types. The existing CI jobs (`rust-linux`, `rust-windows`) already build and test the `backend` crate with `--features mock-hardware`, so they will automatically pick up this change.

## Platform Considerations

None identified. The change is a simple async function call on a SQLite-backed `JobStore`. There are no `#[cfg(unix)]` or `#[cfg(windows)]` guards needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `reset_ghost_jobs()` returns an error on a fresh database (no `jobs` table yet) because the migration may not have run before the seed loader runs the job store query. | Low | High | The pool is created by `create_pool()`, which runs all migrations from `database/migrations/` (including `003_jobs.sql` from P13-A1) before returning. The seed loader clones this already-migrated pool. Therefore `reset_ghost_jobs()` always operates on a database with the `jobs` table. Verified by reading `db.rs` line 70: `sqlx::migrate!("../../database/migrations").run(&pool).await`. |
| The `_pool` variable shadowing/unused pattern causes a clippy warning if changed to `pool` without actual usage. | Low | Low | This is the entire point of the task — `pool` will be used via `JobStore::new(pool.clone())` and `pool.clone()` for the seed loader, so no unused warning. Verified by the fact that the seed loader already clones the pool. |
| The `reset_ghost_jobs()` call adds latency to startup if there are many ghost jobs to reset. | Low | Low | The method executes a single `UPDATE ... WHERE status IN ('queued', 'running')` — this is an O(1) indexed update on the `status` column (indexed per `003_jobs.sql`). Even with thousands of ghost jobs, the update is sub-millisecond in SQLite. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
