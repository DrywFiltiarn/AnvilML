# Implementation Report: P18-D2

| Field         | Value                                                    |
|---------------|-----------------------------------------------------------|
| Task ID       | P18-D2                                                     |
| Phase         | 18 — HTTP/WebSocket Server Completion                      |
| Description   | anvilml-worker: WorkerPool::spawn_worker() reusable single-worker spawn |
| Implemented   | 2026-07-12T14:00:00Z                                       |
| Status        | COMPLETE                                                   |

## Summary

Extracted `spawn_worker(&self, device: GpuDevice) -> Result<WorkerHandle, AnvilError>`
from `spawn_all_impl()`'s per-device loop body. `spawn_all_impl()` now captures its
`ServerConfig`/`WorkerSpawner`/`NodeTypeRegistry`-derived construction context once into
a new private `PoolSpawnConfig`, then runs a thin per-device loop calling
`spawn_worker()`. `WorkerPool.handles` changed from a plain `Vec<WorkerHandle>` to
`std::sync::RwLock<Vec<WorkerHandle>>` so `spawn_worker()` (and `P18-D3`'s
`restart_worker()`) can mutate it through a shared `&self` — necessary because
`AppState.workers` is `Arc<WorkerPool>` with no outer lock, and there is no path to
exclusive `&mut` access from inside a live HTTP handler. `handles()` now returns an
owned `Vec<WorkerHandle>` snapshot instead of `&[WorkerHandle]`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/pool.rs` | `handles` field → `RwLock`; new `PoolSpawnConfig`; `spawn_worker()` extracted (`&self`); `spawn_all_impl()` reduced to a thin loop; `handles()`/`list()`/`shutdown_all()`/`set_up_test_workers()` updated for the lock |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | No functional change from this task alone (see `P18-D3_implement.md` for the `RestartOutcome` export) |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | One line in `test_spawn_all_creates_one_handle_per_device` changed from collecting `&str` to owned `String` — see Deviations |

## Test Results

Not run in this session (no Rust toolchain in this environment). Dryw applied and
committed this task's patch locally and reported one compile failure in
`pool_tests.rs` (`E0716`, the exact line addressed below); the fix was applied and
verified with `git apply --check` against a fresh clone, but the actual
`cargo test -p anvilml-worker --features mock-hardware --test pool_tests` run — and
the full workspace suite — is Dryw's to confirm locally, per this project's own
"gate results marked not-run-in-session until confirmed locally" convention.

## Deviations from Plan

- **`handles()`'s return type changed** from `&[WorkerHandle]` to an owned
  `Vec<WorkerHandle>` — required by the interior-mutability design (see
  `P18-D2_plan.md`). Every call site across the workspace turned out to be
  source-compatible with this change **except one**: `pool_tests.rs`'s
  `test_spawn_all_creates_one_handle_per_device` collected `&str` (borrowed from
  `handles()`'s return value) into a `let`-bound `Vec<&str>` used in a later statement
  (`.sort()`). A `&str` can't outlive the temporary `Vec<WorkerHandle>` it's borrowed
  from, so this failed to compile (`E0716`). Fixed by collecting owned `String`s
  instead (`h.worker_id.clone()` in place of `.as_str()`) — the assertion's meaning is
  unchanged (`Vec<String>` compares equal to `vec!["0","1","2"]` via
  `String: PartialEq<&str>`).
- This is a genuine, if narrow, exception to this task's own acceptance text ("existing
  `pool_tests.rs` suite continues to pass unmodified"). Logged as a `docs/PHASES_GRAPH.md`
  gap-table entry rather than silently left undocumented, per this project's convention
  that fixes to already-completed phases (Phase 8, here) get tracked even when the
  triggering task is in a later phase.

## Blockers

None.
