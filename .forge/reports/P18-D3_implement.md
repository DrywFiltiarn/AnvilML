# Implementation Report: P18-D3

| Field         | Value                                                    |
|---------------|-----------------------------------------------------------|
| Task ID       | P18-D3                                                     |
| Phase         | 18 — HTTP/WebSocket Server Completion                      |
| Description   | anvilml-server: POST /v1/workers/:id/restart via explicit respawn |
| Implemented   | 2026-07-12T14:00:00Z                                       |
| Status        | COMPLETE                                                   |

## Summary

Implemented `WorkerPool::restart_worker()` (explicit shutdown-then-respawn, serialized
by a new `restart_lock`) and the `POST /v1/workers/{id}/restart` HTTP handler that maps
its `RestartOutcome` to `202`/`404`/`409`. Five new integration tests in
`workers_tests.rs`, using a spawner-backed pool (`spawn_all_with_spawner()`, not
`set_up_test_workers()`) so `restart_worker()` has a populated `spawn_config` to work
from.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/pool.rs` | `RestartOutcome` enum; `restart_worker()`; `restart_lock` field |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Export `RestartOutcome` |
| MODIFY | `crates/anvilml-server/src/handlers/workers.rs` | `restart_worker()` handler |
| MODIFY | `crates/anvilml-server/src/lib.rs` | `POST /v1/workers/{id}/restart` route |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | 5 new tests (`MockWorkerSpawner`, dealer/Ready-event helpers, spawner-backed state builder) |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Added `zeromq`/`bytes`/`rmp-serde` dev-dependencies |
| MODIFY | `docs/TESTS.md` | 5 new test catalogue entries |

## Test Results

Not run in this session (no Rust toolchain in this environment) — see
`P18-D2_implement.md`'s identical note. Dryw applied and committed the combined
`P18-D2`/`P18-D3` patch locally; the one compile failure reported (`pool_tests.rs`,
Phase 8, tracked under `P18-D2`) has been fixed. `workers_tests.rs`'s five new tests
(`>=7 total in the file` per this task's own acceptance criterion; actual total is 8)
have not yet been run locally as of this report — Dryw's next local `cargo test -p
anvilml-server --features mock-hardware --test workers_tests` run is the confirming
gate.

## Deviations from Plan

- **Global, not per-worker, restart serialization.** The task's own text doesn't specify
  concurrency handling for overlapping restarts; a global `tokio::sync::Mutex<()>` was
  added (see `P18-D3_plan.md`'s Concurrency section) since it wasn't otherwise
  addressed and is required for `spawn_worker()`'s tail-append to be race-free under
  concurrent restarts of different workers.
- **`RestartOutcome` enum**, not a bare `Result`/`StatusCode` return from
  `WorkerPool::restart_worker()` — added to mirror `anvilml-scheduler`'s established
  `CancelOutcome` pattern, keeping the HTTP handler a simple `match` rather than
  string-inspecting an `AnvilError`.

## Blockers

None.
