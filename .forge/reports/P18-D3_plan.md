# Plan Report: P18-D3

| Field       | Value                                                    |
|-------------|-----------------------------------------------------------|
| Task ID     | P18-D3                                                     |
| Phase       | 18 — HTTP/WebSocket Server Completion                      |
| Description | anvilml-server: POST /v1/workers/:id/restart via explicit respawn |
| Depends on  | P18-D2                                                     |
| Project     | anvilml                                                    |
| Planned at  | 2026-07-12T14:00:00Z                                       |
| Attempt     | 1                                                          |

## Objective

Expose worker restart as `POST /v1/workers/{id}/restart`: explicitly request the
current worker's graceful shutdown, wait for it to exit, then spawn a replacement into
the same device slot via `P18-D2`'s `spawn_worker()`. Per the audit finding this task
closes, `request_shutdown()` alone does not respawn a worker — see
`docs/PHASES_GRAPH.md` gap #26.

## Scope

### In Scope
- `crates/anvilml-worker/src/pool.rs` — `WorkerPool::restart_worker()`, the actual
  shutdown-then-respawn sequence, plus a `RestartOutcome` enum (mirroring
  `anvilml-scheduler`'s `CancelOutcome` pattern).
- `crates/anvilml-server/src/handlers/workers.rs` — `restart_worker()` HTTP handler,
  mapping `RestartOutcome` to `202`/`404`/`409`.
- `crates/anvilml-server/src/lib.rs` — route registration.
- `crates/anvilml-server/tests/workers_tests.rs` — ≥4 new tests.

### Out of Scope
- Any change to `AppState.workers`'s own type (stays `Arc<WorkerPool>` — see Existing
  Codebase Assessment for why this was reachable without changing it).

## Existing Codebase Assessment

**Where the restart logic belongs:** Two placements were considered — doing the
lookup/shutdown/respawn/splice sequence directly in the HTTP handler
(`anvilml-server`), or as a `WorkerPool` method (`anvilml-worker`). Chosen: a
`WorkerPool::restart_worker()` method. Reasons: (1) it needs direct access to
`self.handles`'s interior lock, which is private to `pool.rs`; (2) it's a pool-level
operation in the same family as `spawn_all()`/`shutdown_all()`, not HTTP-specific logic;
(3) it keeps the handler itself thin — a single `match` on `RestartOutcome`, matching
`cancel_job()`'s own established shape in `anvilml-server/src/handlers/jobs.rs`.

**Concurrency:** `restart_worker()` takes `&self` (not `&mut self`) — `AppState.workers`
is `Arc<WorkerPool>`, shared with the dispatch loop and event loop, so an HTTP handler
never has exclusive access. A `restart_lock: tokio::sync::Mutex<()>` field, held for the
whole restart sequence, serializes concurrent restarts globally (not per-worker) —
restarts are a rare, operator-triggered action, not a hot path, so this simple
tradeoff was preferred over finer-grained per-worker locking. It also removes a real
correctness hazard: `spawn_worker()` appends to the tail of `self.handles` (`P18-D2`'s
own documented contract); without serialization, two concurrent restarts of *different*
workers could each push to the tail, making "which tail entry is mine" ambiguous when
splicing the new handle into its slot.

**Why the old handle, not a clone, for `request_shutdown()`:** `WorkerHandle::clone()`
never carries `shutdown_tx`/`force_shutdown_tx` (both `None` on a clone — see that
impl's own doc comment). `restart_worker()` must call `request_shutdown()` on the
handle still living in `self.handles[pos]`, not a snapshot clone taken for lookup
purposes.

**409 semantics:** The task's own text says "409 if the handle is already mid-shutdown
or gone." "Gone" is covered by `NotFound` → 404 (a `worker_id` either exists or it
doesn't; there's no separate half-removed state). "Mid-shutdown" is checked via
`WorkerStatus::Dying` — set by `request_shutdown()`'s own eventual effect on the
`ManagedWorker::run()` loop, and also by `shutdown_all()`. A second restart call while
one is already in flight and observably `Dying` returns `Conflict` rather than queuing
behind `restart_lock` and silently restarting again.

## Approach

1. `RestartOutcome` enum (`Accepted(WorkerHandle)` / `NotFound` / `Conflict`) in
   `pool.rs`, exported via `lib.rs`.
2. `restart_worker(&self, worker_id: &str) -> Result<RestartOutcome, AnvilError>`:
   acquire `restart_lock`; look up `(pos, old_handle)` under a brief read lock; return
   `NotFound` if absent; return `Conflict` if `old_handle.status().await ==
   WorkerStatus::Dying`; look up the corresponding `GpuDevice` by `pos`; call
   `request_shutdown()` on `self.handles[pos]` (not the clone) under a brief write lock;
   `old_handle.await_exit(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT).await` (valid on the clone —
   it shares the same `join_handle` `Arc`); `spawn_worker(device).await?`; splice the new
   handle into `self.handles[pos]`, discarding the tail entry `spawn_worker()` just
   pushed.
3. HTTP handler: `match state.workers.restart_worker(&id).await? { Accepted(_) => 202,
   NotFound => Err(AnvilError::WorkerNotFound(id)), Conflict => 409 }`.
4. Route: `POST /v1/workers/{id}/restart` (axum 0.8 `{capture}` syntax, matching every
   other parameterised route already in `build_router()`).
5. Tests use `spawn_all_with_spawner()` with a local `MockWorkerSpawner` (not
   `set_up_test_workers()` — the latter never populates `spawn_config`, which
   `spawn_worker()` requires). New dev-dependencies `zeromq`/`bytes`/`rmp-serde` added
   to `anvilml-server/Cargo.toml`, mirroring `anvilml-worker`'s own, for the
   Ready-event test proving a respawned worker reaches `Idle`.

## Resolved Dependencies

| Type   | Name      | Version     | Source |
|--------|-----------|-------------|--------|
| crate  | zeromq    | 0.6         | Matches `anvilml-worker`'s existing dev-dependency |
| crate  | bytes     | 1           | Matches `anvilml-worker`'s existing dev-dependency |
| crate  | rmp-serde | 1           | Matches `anvilml-worker`'s existing dev-dependency |

All three are dev-dependency-only additions to `anvilml-server/Cargo.toml`; no
production dependency changes.
