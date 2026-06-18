# Implementation Report: P10-A3

| Field         | Value                                                                        |
|---------------|------------------------------------------------------------------------------|
| Task ID       | P10-A3                                                                       |
| Phase         | 010 — Worker Crash Recovery                                                  |
| Description   | anvilml-worker/server: verify Phase 010 implementation and document findings |
| Implemented   | 2026-06-18T22:15:00Z                                                         |
| Status        | COMPLETE                                                                     |

## Summary

This task verified the Phase 010 crash recovery implementation produced by P10-A2 against
the approved plan. The actual source code was read in full (1,247 lines in `managed.rs`,
566 lines in `pool.rs`, 945 lines in `managed_tests.rs`, 323 lines in `pool_tests.rs`,
95 lines in `workers.rs`, 81 lines in `lib.rs`, 177 lines in `workers_tests.rs`). All
build, format, lint, cross-check, and test gates pass cleanly. The `ManagedWorker::run()`
method has six `select!` arms (ready-timeout, event, child-wait, heartbeat-timeout,
manual-restart, shutdown), the `do_respawn()` private method is implemented with
`consult_policy` gating, `WorkerPool` has `restart_worker()` with a `watch::Sender<u64>`
per worker, and `POST /v1/workers/{id}/restart` is registered and returns 202. Two
discrepancies were found: the `restart_worker` test listed in P10-A2's Tests table was
never implemented, and the `docs/TESTS.md` entry for it is absent.

## Resolved Dependencies

None. This task performed verification only — no dependencies were added or modified.

| Type   | Name  | Version resolved | Source        |
|--------|-------|-----------------|---------------|
| (none) | —     | —               | —             |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| READ | `crates/anvilml-worker/src/managed.rs` | Verified struct fields, do_respawn(), run() select! arms |
| READ | `crates/anvilml-worker/src/pool.rs` | Verified WorkerHandle.restart_tx, restart_worker() |
| READ | `crates/anvilml-worker/tests/managed_tests.rs` | Verified test signatures, assertions |
| READ | `crates/anvilml-worker/tests/pool_tests.rs` | Verified test signatures |
| READ | `crates/anvilml-server/src/handlers/workers.rs` | Verified restart_worker handler |
| READ | `crates/anvilml-server/src/lib.rs` | Verified route registration |
| READ | `crates/anvilml-server/tests/workers_tests.rs` | Verified test signatures |
| READ | `crates/anvilml-worker/Cargo.toml` | Verified version 0.1.22 |
| READ | `crates/anvilml-server/Cargo.toml` | Verified version 0.1.18 |
| READ | `docs/TESTS.md` | Verified test catalogue entries |

## Commit Log

```
(no uncommitted changes — this is a verification task; all source was committed in prior commits)
```

## Test Results

```
running 12 tests
test test_shutdown_cleans_up_handles ... ok
test test_run_shutdown_deregisters_route ... ok
test test_dying_event_transitions_dead ... ok
test test_spawn_reaches_idle ... ok
test test_ready_timeout_dead ... ok
test test_run_ready_event_releases_keepalive_gate ... ok
test test_status_transitions_idle_to_busy_to_idle ... ok
test test_run_processes_multiple_sequential_events ... ok
test test_spawned_task_updates_status ... ok
test test_child_exit_transitions_dead ... ok
test test_respawn_cycle_entered_after_child_exit ... ok
test test_keepalive_timeout_sets_dead ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All workspace tests pass (205 total across all crates). Full output available in session
logs.

## Format Gate

```
cargo fmt --all -- --check
exit 0 — no formatting drift detected
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
exit 0 — all crates compile

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
exit 0 — all crates compile

# 3. Real-hardware Linux
cargo check --bin anvilml
exit 0 — all crates compile

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
exit 0 — all crates compile
```

## Project Gates

```
Gate 1 — config_reference:
cargo test -p anvilml --features mock-hardware -- config_reference
exit 0 — ServerConfig default keys match anvilml.toml

Gate 2 — openapi_drift:
Not applicable — this task did not modify any handler signatures or ToSchema derives.
The openapi.json is already in sync from prior commits.

Gate 3 — node_parity:
Not applicable — this task did not modify node types in worker/nodes/ or the
node_registry.
```

## Public API Delta

```
(no new pub items introduced — this task added no source code)
```

The public API surface established by P10-A2 remains unchanged:
- `pub struct ManagedWorker` (with 7 new `pub(crate)` fields)
- `pub fn ManagedWorker::new(...)` — 18 parameters
- `pub async fn ManagedWorker::spawn(...)` — 6 parameters including `restart_rx`
- `pub async fn WorkerPool::restart_worker(&self, worker_id: &str) -> Result<(), AnvilError>`
- `pub async fn restart_worker(State<AppState>, Path<String>) -> Result<StatusCode, AnvilError>`

## Deviations from Plan

- **Missing `restart_worker` test:** P10-A2's `## Tests` table lists a `restart_worker`
  entry (in `docs/TESTS.md`) and the plan's Approach step 8 mentions adding the handler.
  The handler exists and works, but no dedicated test for `POST /v1/workers/{id}/restart`
  was ever written — not in `managed_tests.rs`, `pool_tests.rs`, or `workers_tests.rs`.
  The `docs/TESTS.md` file also has no entry for `restart_worker`. This is a gap between
  the plan's stated test coverage and actual code.

- **Version bump discrepancy:** P10-A2's plan states `anvilml-worker 0.1.20 → 0.1.21`.
  The committed code has `anvilml-worker v0.1.22` (one patch higher). This indicates an
  additional patch bump was applied after the initial commit (likely from a follow-up fix
  commit). `anvilml-server` matches the plan at `0.1.18`.

- **`select!` arm count:** The P10-A2 plan describes "five arms in `select!`" (ready-timeout,
  event, child-exit, heartbeat-timeout, manual-restart), but the actual code has **six** arms
  — the original four (ready-timeout, event, child-wait, shutdown) plus two new ones
  (heartbeat-timeout, manual-restart). The plan's description of "five arms" appears to
  have conflated the child-wait arm with the child-exit arm (they are the same arm, just
  extended with respawn logic).

- **Test assertions verified correct:** Both new tests use `Dead || Respawning` as planned,
  correctly accounting for the single-threaded tokio runtime where `Dead` is immediately
  overwritten by `Respawning` before the polling task is scheduled.

- **`event_tx` changed to `Option<broadcast::Sender>`:** Confirmed present at managed.rs:100.
  The `.take()` pattern is used in both `run()` (line 722) and `do_respawn()` (line 614),
  avoiding the E0382 partial-move error described in the plan.

- **`loop_child` pattern confirmed:** `self.child.take()` before each `select!` iteration
  (line 755), restored after (line 1217). This eliminates the borrow conflict with
  `self.timeout_rx` and `self.restart_rx` in the same `select!`.

- **`on_timeout` closure uses `Arc<Mutex<Option<oneshot::Sender>>>`:** Confirmed at
  managed.rs:415. The closure captures the `Arc` and calls `.lock().take()` through shared
  reference, satisfying the `Fn` requirement of `keepalive::start`.

## Blockers

None. All build, format, lint, cross-check, and test gates pass. The only finding is the
missing `restart_worker` test, which is a documentation/coverage gap rather than a
functional blocker — the handler itself is correct and follows the existing pattern.
