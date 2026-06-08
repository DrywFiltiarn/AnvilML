# Tasks: Phase 902 — Stabilisation Retrofit

| Field | Value |
|-------|-------|
| Phase | 902 |
| Name | Stabilisation Retrofit |
| Milestone group | Retrofit |
| Project(s) | anvilml |
| Status | Draft |
| Depends on phases | 0–12 (via P901-A1) |
| Task file | `.forge/tasks/tasks_phase902.json` |
| Tasks | 10 |

---

## Overview

Phase 902 is a retrofit phase inserted between Phase 12 and Phase 13. It resolves three categories of accumulated debt before the dispatch loop (Phase 13) builds further on the scheduler and worker subsystems.

**What the audit found:**

Running `cargo clippy --workspace --features mock-hardware -- -D warnings -W dead_code -W unused_imports -W unused_variables` produced zero warnings and zero errors. All `#[allow(…)]` suppressions in the codebase are load-bearing. The Python worker has 11 passing tests and no defects. There is no dead code cleanup work in this phase.

The actual debt is:

1. **Runtime bugs** — the respawn loop has two defects: `WorkerPool.workers` (a plain `Vec`) is never updated after respawn so public methods permanently operate on the dead worker; and no event listener is spawned for the replacement worker so its events are unobserved. Additionally, `ipc-probe` bypasses `write_frame` and produces the wrong wire format, failing with `_type field missing`.
2. **Test isolation** — four spawning tests in `anvilml-worker` use `std::env::set_var` with `#[serial_test::serial]` as a workaround for process-global env-var contamination.
3. **Logging gaps** — phases 9–12 added the worker pool, IPC bridge, job store, queue, and scheduler. The §11.5 mandatory DEBUG log points were not added to these subsystems.

**Phase 13 dependency:** `P13-A1` currently prereqs `["P12-A5"]`. Before Phase 902 runs, update `tasks_phase013.json` so that `P13-A1` prereqs `["P902-D1"]`. Commit this manually before The Forge picks up Phase 902.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-ipc / anvilml-worker | P902-A1, A2a, A2b, A4–A6 | ipc-probe fix; pool workers unification; respawn listener; env isolation fix; IPC DEBUG log points; pool spawn/status DEBUG log points |
| B | anvilml-scheduler / anvilml-server | P902-B1–B3 | scheduler submit DEBUG point; job-store and queue DEBUG points; TraceLayer request/response middleware |
| D | Gate | P902-D1 | Full workspace clean gate — no source changes, verbatim output only |

---

## Prerequisites

All tasks in phases 000 through 012 must be complete. `P901-A1` must be complete. `tasks_phase013.json` must have `P13-A1.prereqs` updated to `["P902-D1"]` before The Forge starts this phase.

---

## Task Descriptions

### Group A — anvilml-ipc / anvilml-worker

#### P902-A1: Fix ipc-probe binary to use write_frame/read_frame correctly

**File:** `crates/anvilml-ipc/src/bin/ipc-probe.rs`

The probe binary was written during P8-A4 with the original framing design where `read_frame` used serde's native enum encoding. Later changes to `framing.rs` switched `read_frame` to expect Python's flat dict format (`{"_type": "Pong", ...}`) via `worker_event_from_map()`. The probe was not updated, so it hand-rolls a raw `WorkerEvent::Pong` frame using `rmp_serde::to_vec_named` directly — producing serde native format — which `read_frame` cannot parse, causing `_type field missing or not a string`.

Replace the hand-rolled write side with `write_frame(&mut tx, &WorkerMessage::Ping { seq: 7 }).await?`. `write_frame` calls `serialize_message` internally, which produces the correct flat dict. The read side is already correct. No changes to `framing.rs`, `messages.rs`, or any other file.

**Acceptance criterion:** `cargo run -p anvilml-ipc --bin ipc-probe` prints `OK seq=7` and exits 0 on both Linux and Windows.

---

#### P902-A2a: Unify WorkerPool.workers with shared_workers Arc (pool.rs)

**File:** `crates/anvilml-worker/src/pool.rs` only.

`WorkerPool.workers` is a plain `Vec<Arc<ManagedWorker>>` that is never updated after respawn. The respawn task writes the replacement worker into `shared_workers` (a separate `Arc<RwLock<Vec>>`), but all public methods — `list()`, `acquire_idle()`, `set_busy()`, `set_idle()`, `send()`, `pid_for()` — iterate the original stale `Vec`. After the first respawn the pool permanently operates on a dead worker.

**Changes:**

1. `WorkerPool` struct field: `workers: Vec<Arc<ManagedWorker>>` → `workers: Arc<RwLock<Vec<Arc<ManagedWorker>>>>`
2. `spawn_all` construction: `let mut workers: Vec<...> = Vec::with_capacity(...)` → `Arc::new(RwLock::new(Vec::with_capacity(...)))`. Both `push` sites become `workers.write().await.push(worker)`.
3. Remove `let shared_workers = Arc::new(tokio::sync::RwLock::new(workers.clone()))`. Replace with `let shared_workers = workers.clone()`. Both names now refer to the same `Arc`. The respawn task already uses `shared_workers` correctly — no changes needed inside the respawn closure.
4. `pool_workers` snapshot: `let pool_workers = workers.clone()` → `let pool_workers = { let l = workers.read().await; l.clone() }`.
5. All public methods: add `let locked = self.workers.read().await;` and iterate `&*locked`.
6. Tests constructing `WorkerPool` manually: `workers: vec![...]` → `workers: Arc::new(RwLock::new(vec![...]))`. Direct `pool.workers[0]` accesses: wrap in `{ let l = pool.workers.read().await; l[0].<method>().await }`.

**Acceptance criterion:** `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0. `cargo test -p anvilml-worker --features mock-hardware` exits 0.

---

#### P902-A2b: Spawn event listener for replacement worker after respawn (pool.rs)

**File:** `crates/anvilml-worker/src/pool.rs` only. Starting from the P902-A2a output.

Inside the respawn `spawn(async move { ... })` block the variables in scope are `wid`, `device_index`, `hw` (which is `pool_hardware.clone()` captured earlier), `tx` (which is `pool_event_tx.clone()` captured earlier), `workers_clone`, `cfg`. The names `hardware` and `event_tx` are **not** in scope — they were moved into the outer per-worker listener closure. Using them will produce E0382.

**Two changes:**

**(1)** Immediately before the write-lock block `{ let mut locked = workers_clone.write().await; locked[idx] = new_worker; }`, add:

```rust
let new_worker_for_listener = new_worker.clone();
```

This clone must precede the move of `new_worker` into `locked[idx]`. After `locked[idx] = new_worker`, `new_worker` is moved and cannot be used.

**(2)** Immediately after the write-lock block, before `info!("worker respawned successfully")`, add:

```rust
let new_wid = wid.clone();
let new_device_index = device_index;
let new_hw = hw.clone();
let new_tx = tx.clone();
spawn(async move {
    let mut rx = new_worker_for_listener.subscribe();
    loop {
        match rx.recv().await {
            Ok((_, event)) => {
                if let WorkerEvent::Ready {
                    arch, fp16, bf16, flash_attention,
                    vram_total_mib, vram_free_mib, ..
                } = &event {
                    let mut h = new_hw.lock().await;
                    if let Some(gpu) = h.gpus.iter_mut().find(|g| g.index == new_device_index) {
                        gpu.arch = Some(arch.clone());
                        gpu.caps.fp16 = *fp16;
                        gpu.caps.bf16 = *bf16;
                        gpu.caps.flash_attention = *flash_attention;
                        gpu.vram_total_mib = *vram_total_mib;
                        gpu.vram_free_mib = *vram_free_mib;
                        gpu.capabilities_source = CapabilitySource::Worker;
                        info!(
                            worker_id = %new_wid,
                            device_index = new_device_index,
                            "respawn: worker ready — capabilities merged"
                        );
                    }
                }
                let _ = new_tx.send((new_wid.clone(), event.clone()));
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                debug!(lagged = n, worker_id = %new_wid, "respawn listener dropped events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                debug!(worker_id = %new_wid, "respawn listener channel closed");
                break;
            }
        }
    }
});
```

**Acceptance criterion:** `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0. `cargo test -p anvilml-worker --features mock-hardware` exits 0. Manual: `cargo run --features mock-hardware`; kill worker PID; `GET /v1/workers` shows `status: "Idle"` within 5 seconds; no second respawn cycle.

#### P902-A4: Replace serial_test env-var workaround with scoped env isolation (managed.rs)

**File:** `crates/anvilml-worker/src/managed.rs`, `crates/anvilml-worker/Cargo.toml`

Four spawning tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) call `std::env::set_var("ANVILML_WORKER_MOCK", "1")` and rely on `#[serial_test::serial]` to prevent parallel env-var contamination. Replace each test body with `temp_env::async_with_var("ANVILML_WORKER_MOCK", Some("1"), async { <test body> }).await`. Add `temp_env` to `[dev-dependencies]`. Remove all four `#[serial_test::serial]` attributes. Remove `serial_test` from `[dev-dependencies]`.

No changes to test assertions.

**Acceptance criterion:** `env -i HOME=$HOME PATH=$PATH cargo test -p anvilml-worker --features mock-hardware` exits 0 with all 16 tests passing.

---

#### P902-A5: Retrofit mandatory IPC DEBUG log points (managed.rs)

**File:** `crates/anvilml-worker/src/managed.rs`

Add `tracing::debug!(worker_id = %worker_id, message_type = msg_discriminant(&msg))` in the writer task immediately before each IPC frame is written to stdin. Add `tracing::debug!(worker_id = %worker_id, event_type = event_discriminant(&event))` in the reader task immediately after each `WorkerEvent` is successfully deserialized. `msg_discriminant()` and `event_discriminant()` already exist in the file. No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0.

---

#### P902-A6: Retrofit mandatory spawn and status-transition DEBUG log points (pool.rs)

**File:** `crates/anvilml-worker/src/pool.rs`

Add `tracing::debug!(worker_id = %worker_id, device_index = device_index)` immediately after each `ManagedWorker::new()` call in `spawn_all()` (both GPU loop and CPU fallback paths). Add `tracing::debug!(worker_id = %worker_id, from = %old_status, to = %new_status)` in `set_busy()` and `set_idle()` before each status transition. The existing `info!` calls are unchanged. No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0.

---

### Group B — anvilml-scheduler

#### P902-B1: Retrofit mandatory job state-transition DEBUG log point (scheduler.rs)

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

`submit()` has `tracing::info!(job_id = %job_id)` but is missing the §11.5 mandatory DEBUG job state-transition point for the Queued transition. Add `tracing::debug!(job_id = %job_id, status = "Queued", "job status transition")` after `insert_job` succeeds. The dispatch loop's Running transition belongs in Phase 13, not here. No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0.

---

#### P902-B2: Retrofit mandatory job-store and queue DEBUG log points (job_store.rs, queue.rs)

**Files:** `crates/anvilml-scheduler/src/job_store.rs`, `crates/anvilml-scheduler/src/queue.rs`

In `job_store.rs`: add `tracing::debug!(job_id = %job.id, "job inserted into DB")` at end of `insert_job()`; add `tracing::debug!(job_id = %id, status = ?status, "job status updated in DB")` at end of `update_status()`. In `queue.rs`: add `tracing::debug!(job_id = %job.id, queue_len = self.len(), "job enqueued")` at end of `enqueue()`; add `tracing::debug!(job_id = %job.id, "job dequeued")` when `pop_next()` returns `Some`. No logic changes.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0.

---

#### P902-B3: Add TraceLayer request/response DEBUG logging middleware (anvilml-server)

**Files:** `Cargo.toml` (workspace), `crates/anvilml-server/Cargo.toml`, `crates/anvilml-server/src/lib.rs`

`ANVILML_DESIGN.md §10.2` specifies `TraceLayer` as the first middleware in the stack, but it was never implemented. All endpoints currently emit no structured request/response log points. This task adds `tower-http`'s `TraceLayer` to the router, which automatically emits a `DEBUG` span on every incoming request (method, URI) and a `DEBUG` event on every outgoing response (status code, latency) via `tracing` — covering all 10 routes with zero handler changes.

**Three changes only:**

1. **Workspace `Cargo.toml`**: add `tower-http = { version = "0.6", features = ["trace"] }` to `[workspace.dependencies]`.

2. **`crates/anvilml-server/Cargo.toml`**: add `tower-http = { workspace = true }` to `[dependencies]`.

3. **`crates/anvilml-server/src/lib.rs`**: add `use tower_http::trace::TraceLayer;` to imports. In `build_router()`, append `.layer(TraceLayer::new_for_http())` to the router chain, immediately after `.with_state(state_arc)`.

No changes to any file in `handlers/`. No new handler files.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0. `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0. Running `RUST_LOG=debug cargo run --features mock-hardware` and sending `GET /health` produces a DEBUG log line containing the request method and URI and a second DEBUG log line containing the response status.

---

### Group D — Gate

#### P902-D1: Full workspace stabilisation gate

**No files modified.**

Prereqs: P902-B3. Run and record verbatim output:

```bash
# 1. Lint
cargo clippy --workspace --features mock-hardware -- -D warnings

# 2. Tests — ambient env cleared
env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv \
  cargo test --workspace --features mock-hardware

# 3. Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# 4. Python worker
python -m pytest worker/tests/ -v
```

All four must exit 0. Write verbatim outputs as the implementation report body. Task is COMPLETE only when all four exit 0.

---

## Phase Acceptance Criteria

```bash
cargo clippy --workspace --features mock-hardware -- -D warnings
env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv \
  cargo test --workspace --features mock-hardware
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
python -m pytest worker/tests/ -v
```

All four must exit 0.

---

## Known Constraints and Gotchas

- **P902-A2a: no restructuring.** Make only the six listed changes. Do not rename variables, reorder the closure setup, or refactor `spawn_all` beyond what is specified.
- **P902-A2b: tx is already in scope.** Clone it as `new_tx` inside the respawn closure — do not introduce a separate `let respawn_event_tx` before the for-loop, which caused E0382 in a previous attempt by being moved in the first iteration and unavailable in subsequent ones.
- **P902-A4: async variant.** Use `temp_env::async_with_var` (requires the `async` feature flag on `temp_env`). If unavailable, use an inline RAII guard with `set_var`/`remove_var` in a `Drop` impl — document the choice in the plan report.
- **P902-A4: `-i` env clear test.** Unix-only; Windows CI relies on the test suite not having `ANVILML_WORKER_MOCK` set in the ambient environment.
- **P902-A6: old_status capture.** `set_busy` and `set_idle` do not currently read the old status before transitioning. Capture it with `let old_status = worker.get_status().await` immediately before `worker.set_status(...)` to provide the `from` field for the DEBUG log.
- **P902-D1: ANVILML_VENV_PATH.** Substitute the actual venv path from `ENVIRONMENT.md §2` if it differs from `./worker/.venv`.
- **P902-B3: tower-http version.** Use `"0.6"` — this is the version compatible with axum `"0.8"` (both depend on tower `"0.5"`). Do not use `"0.5"` or earlier.
- **P13-A1 prereq must be updated manually** from `["P12-A5"]` to `["P902-D1"]` in `tasks_phase013.json` before The Forge runs Phase 902.