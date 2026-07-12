# Plan Report: P18-D2

| Field       | Value                                                    |
|-------------|-----------------------------------------------------------|
| Task ID     | P18-D2                                                     |
| Phase       | 18 — HTTP/WebSocket Server Completion                      |
| Description | anvilml-worker: WorkerPool::spawn_worker() reusable single-worker spawn |
| Depends on  | P18-D1                                                     |
| Project     | anvilml                                                    |
| Planned at  | 2026-07-12T14:00:00Z                                       |
| Attempt     | 2 (attempt 1 failed in ACT — see Deviations below)         |

## Objective

Extract `spawn_all_impl()`'s per-device worker-construction body (`WorkerEnv` build,
`ManagedWorker::new()`, `tokio::spawn(worker.run(...))`, `WorkerHandle` construction,
push into `self.handles`) into a standalone `spawn_worker()` method, so `P18-D3`'s
restart handler can spawn one replacement worker into an existing slot without
duplicating `spawn_all()`'s bulk-construction logic.

## Scope

### In Scope
- `crates/anvilml-worker/src/pool.rs` — extract `spawn_worker()`; `spawn_all_impl()`
  becomes a thin per-device loop calling it.
- Capture `ServerConfig`/`WorkerSpawner`/`NodeTypeRegistry`-derived construction context
  (`PoolSpawnConfig`) once, at `spawn_all_impl()` time, so `spawn_worker()` doesn't need
  those as parameters — required so `P18-D3` can call it with only a `GpuDevice`.
- Give `WorkerPool.handles` interior mutability so `spawn_worker()` is callable through
  a shared `Arc<WorkerPool>` (see Existing Codebase Assessment).

### Out of Scope
- The restart handler itself — `P18-D3`.
- Any change to `spawn_all()`'s/`spawn_all_with_spawner()`'s own public signatures.

## Existing Codebase Assessment

**Attempt 1 (failed in ACT):** The task's literal signature,
`spawn_worker(&mut self, device: GpuDevice) -> Result<WorkerHandle, AnvilError>`, does
not compile as a pure cut-and-paste: the original per-device body also reads `cfg`,
`spawner`, and `node_registry`, none of which fit that signature. Fixed this attempt via
`PoolSpawnConfig`, captured once by `spawn_all_impl()`.

**A second, more consequential gap found while re-planning `P18-D3` in parallel:**
`AppState.workers` is a bare `Arc<WorkerPool>` — no lock. `P18-D3`'s restart handler
needs `&mut` access to `self.handles` (to call `request_shutdown()` on the live handle,
then splice in a replacement) from inside a live HTTP handler, where `Arc<WorkerPool>`
never yields exclusive access. This is true independent of `P18-D2`'s own text, but
`P18-D2` is where it has to be resolved, since `spawn_worker()` is the method `P18-D3`
calls.

**Resolved as:** `handles` becomes `std::sync::RwLock<Vec<WorkerHandle>>` (not
`tokio::sync::RwLock` — every access here is a brief, synchronous read-or-mutate, never
held across an `.await`). `spawn_worker()` becomes `&self` (not `&mut self`), pushing
through the lock. `spawn_all_impl()` stays `&mut self` (still only ever called once,
pre-`Arc`-wrap) and calls `self.spawn_worker(...)` via an automatic `&mut self -> &self`
reborrow — no signature change to its own public callers.

**Consequence surfaced during a from-scratch review of the alternatives:** `handles()`
(the public accessor) can no longer return `&[WorkerHandle]` — a slice reference can't
outlive the lock guard that produces it. Two candidates were considered:
1. Return a lock guard directly. Rejected: `dispatch_one()`
   (`anvilml-scheduler`, the hot per-job dispatch path) iterates this return value while
   calling `.await` on each handle — holding a `std::sync::RwLockReadGuard` across those
   awaits is a real anti-pattern, and this method's own doc comment used to promise a
   plain, lock-free slice.
2. Return an owned `Vec<WorkerHandle>` snapshot (chosen). Cheap — `WorkerHandle::clone()`
   is a few `Arc` bumps plus a small `String`. Zero lock-across-`.await` hazard.

**Not fully compatible in practice:** one call site in `pool_tests.rs`
(`test_spawn_all_creates_one_handle_per_device`) collects `&str` borrowed from
`handles()`'s return value into a `let`-bound `Vec<&str>` used in a later statement —
this doesn't compile against an owned-temporary return type (`&str` can't outlive the
temporary `Vec<WorkerHandle>` it's borrowed from). This is the one exception to
"`pool_tests.rs` passes unmodified" this task's own acceptance text originally assumed;
see `P18-D2_implement.md`'s Deviations section and the corresponding
`docs/PHASES_GRAPH.md` gap-table entry.

## Approach

1. Add `handles: std::sync::RwLock<Vec<WorkerHandle>>` (replacing the plain
   `Vec<WorkerHandle>`), a private `PoolSpawnConfig` struct, and a
   `restart_lock: tokio::sync::Mutex<()>` field (reserved for `P18-D3`).
2. `spawn_all_impl()` computes `log_level`/`mock` as before, stores them plus
   `venv_path`/`max_ipc_payload_mib`/`spawner`/`node_registry` into
   `self.spawn_config`, then loops calling `self.spawn_worker(device.clone())`.
3. `spawn_worker(&self, device)` reads `self.spawn_config` (errors with
   `AnvilError::Internal` if `None` — unreachable via `spawn_all_impl()`'s own call
   order, but a real possibility for any hypothetical future caller), builds the
   `ManagedWorker` exactly as the original inline body did, and pushes the resulting
   handle onto `self.handles`'s write lock.
4. `handles()` becomes `self.handles.read().unwrap().clone()`.
5. `list()` and `shutdown_all()` updated to go through the lock (`list()` clones a
   snapshot before any `.await`; `shutdown_all()` uses `get_mut()`, since it already
   holds `&mut self` exclusively — no locking needed there at all).
6. `set_up_test_workers()` (`test-utils`-gated) updated to `get_mut()` for the same
   reason.

## Resolved Dependencies

No new external dependencies. Uses only `std::sync::RwLock`/`tokio::sync::Mutex`, both
already transitively available (`std`, `tokio` already a dependency).
