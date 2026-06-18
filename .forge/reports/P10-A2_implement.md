# Implementation Report: P10-A2

| Field         | Value                                                                        |
|---------------|------------------------------------------------------------------------------|
| Task ID       | P10-A2                                                                       |
| Phase         | 010 — Worker Crash Recovery                                                  |
| Description   | anvilml-worker/server: complete crash detection, respawn cycle, and operator restart |
| Implemented   | 2026-06-18T18:00:00Z                                                         |
| Status        | COMPLETE                                                                     |

## Summary

Implemented the complete Phase 010 crash recovery surface. `ManagedWorker::run()` now has
six `select!` arms: the original four (ready-timeout, event, child-wait, shutdown) plus two
new arms (heartbeat-timeout via `timeout_rx`, manual-restart via `restart_rx`). A shared
private `do_respawn(consult_policy: bool)` method handles the respawn cycle for all three
automatic/manual trigger paths. `WorkerPool` gains `restart_worker()` and a per-worker
`watch::Sender<u64>` for operator-initiated restarts. `POST /v1/workers/{id}/restart`
returns 202.

Key structural decisions resolved during implementation:

- `event_tx` changed to `Option<broadcast::Sender>` — required to avoid E0382 partial-move
  error when `do_respawn` takes `&mut self` after `run()` calls `.take()` to drop it.
- `on_timeout` closure uses `Arc<Mutex<Option<oneshot::Sender>>>` instead of a plain
  `Option` — required because `keepalive::start` takes `impl Fn()` (not `FnMut()`), so
  interior mutability is needed to call `.take()` from a shared-reference closure.
- `loop_child` pattern per `select!` iteration — `self.child.as_mut()` inside the child-wait
  arm conflicts with `&mut self.timeout_rx` in the same `select!` expansion when both are
  referenced directly; taking `self.child` into a local before each iteration eliminates the
  borrow overlap.
- Test assertions use `Dead || Respawning` — on the single-threaded tokio test runtime,
  `Dead` is written and immediately overwritten by `Respawning` inside `do_respawn` before
  the polling task is scheduled; either status proves crash detection fired.

Functionally verified: killing a Python worker in the OS task manager triggers immediate
respawn; `POST /v1/workers/worker-0/restart` returns 202 and the worker returns to Idle.

## Resolved Dependencies

None. No new external crates introduced. `tokio::sync::watch` is part of the existing
workspace `tokio` dependency (`full` feature set).

| Type   | Name  | Version resolved | Source |
|--------|-------|-----------------|--------|
| (none) | —     | —               | —      |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/managed.rs` | Add 7 fields; event_tx → Option; rewrite on_timeout closure; implement do_respawn(); add timeout/restart select! arms; loop_child pattern; extend child-exit arm with do_respawn(true) |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Add restart_tx: watch::Sender<u64> to WorkerHandle; build watch pair in spawn_all(), pass restart_rx to ManagedWorker::spawn(); add pub restart_worker() |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | New 18-arg new() signature; stub helpers (stub_cfg, stub_device, stub_transport, stub_timeout_pair, stub_restart_pair); updated all existing new() calls; test_child_exit_transitions_dead assertion updated to Dead\|\|Respawning; test_respawn_cycle_entered_after_child_exit added |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | New 18-arg new() signature; async make_test_worker; stub helpers |
| MODIFY | `crates/anvilml-server/src/handlers/workers.rs` | Add restart_worker handler; stub helpers for test |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register POST /v1/workers/{id}/restart in build_router |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | New 18-arg new() signature; stub helpers |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bumped patch version 0.1.20 → 0.1.21 |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.17 → 0.1.18 |
| MODIFY | `docs/TESTS.md` | Added entries for test_respawn_cycle_entered_after_child_exit and restart_worker tests |

## Commit Log

```
 crates/anvilml-worker/src/managed.rs               | 487 +++++++++++++------
 crates/anvilml-worker/src/pool.rs                  |  89 +++-
 crates/anvilml-worker/tests/managed_tests.rs        | 411 +++++++++++-----
 crates/anvilml-worker/tests/pool_tests.rs           | 180 ++++---
 crates/anvilml-server/src/handlers/workers.rs       |  56 ++-
 crates/anvilml-server/src/lib.rs                    |   6 +-
 crates/anvilml-server/tests/workers_tests.rs        | 141 ++++--
 crates/anvilml-worker/Cargo.toml                    |   2 +-
 crates/anvilml-server/Cargo.toml                    |   2 +-
 docs/TESTS.md                                       |  24 +
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

## Format Gate

```
Checking anvilml-worker v0.1.21
Checking anvilml-server v0.1.18
```

## Platform Cross-Check

```
cargo fmt --all -- --check                                          exit 0
cargo clippy --workspace --features mock-hardware -- -D warnings   exit 0
cargo test --workspace --features mock-hardware                     exit 0 (all tests pass)
cargo run (manual verification): worker kill triggers respawn;
  POST /v1/workers/worker-0/restart returns 202
```

## Project Gates

```
config_reference: not applicable — no ServerConfig fields added or removed
openapi_drift:    not applicable — new route follows existing handler pattern
```

## Public API Delta

```diff
+pub struct ManagedWorker {
+    // event_tx: Option<broadcast::Sender<(String, WorkerEvent)>>  (was non-Option)
+    pub(crate) crash_count: u32,
+    pub(crate) last_crash: Instant,
+    pub(crate) cfg: ServerConfig,
+    pub(crate) device: GpuDevice,
+    pub(crate) transport: Arc<RouterTransport>,
+    pub(crate) timeout_rx: oneshot::Receiver<()>,
+    pub(crate) restart_rx: tokio::sync::watch::Receiver<u64>,
+}
+pub async fn restart_worker(&self, worker_id: &str) -> Result<(), AnvilError>  // WorkerPool
+pub async fn restart_worker(State, Path<String>) -> Result<StatusCode, AnvilError>  // handler
```

## Deviations from Plan

- P10-A3 and P10-B1 tasks were absorbed into this implementation rather than implemented as
  separate sequential tasks. The combined scope is identical to the original three tasks'
  union; no features were added beyond what those tasks specified.
- The respawn cycle does not achieve `Idle` in tests because no Python venv exists in the
  test environment — `do_respawn` returns `Err` after setting `Respawning` when `routes` is
  `None`. Test assertions are scoped to `Dead || Respawning` accordingly. Full `Idle`
  recovery is verified only via `cargo run` with a real Python worker.
- `restart_rx` is passed into `spawn()` as a parameter (not returned as part of a tuple)
  and uses `watch::Receiver` (not `oneshot`) to survive across multiple respawns without
  requiring `WorkerPool` to replace its stored sender.

## Blockers

None.