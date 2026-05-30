# Tasks: Phase 005 — Worker Management

| Field            | Value                                                                       |
|------------------|-----------------------------------------------------------------------------|
| Phase            | 005                                                                         |
| Name             | Worker Management                                                           |
| ANVIL Milestone  | M2 (part 2)                                                                 |
| Status           | Draft                                                                       |
| Depends on phases| 1, 2, 3, 4                                                                  |
| Task file        | `forge/tasks/tasks_phase005.json`                                           |
| Design reference | `ANVILML_DESIGN.md` §8 (Worker Management), §6 (Python Environment), §22.4 |

---

## Overview

Phase 005 implements `anvilml-worker`: the Rust subsystem that spawns one Python child process per GPU device, maintains bidirectional async IPC through that process's stdin/stdout pipes, supervises its health, and respawns it when it dies. This is the most platform-sensitive phase in the entire build; the OS-specific orphan-cleanup mechanisms (`PR_SET_PDEATHSIG` on Linux, Job Object on Windows) are implemented here.

This phase closes out M2 together with phase 004. The M2 exit criterion is "real mock Python worker does `Ping→Pong`; models scan into DB." Phase 004 delivers the DB scan half; this phase delivers the Ping→Pong half, with the real Python worker started by the Rust `WorkerPool` under `ANVILML_WORKER_MOCK=1`. The Python worker stub from phases 001 and the real `worker_main.py` from phase 009 share the same spawn path; implementing it correctly now means phase 009 only needs to fill in the Python logic, not change the Rust side.

At the end of this phase: `cargo test -p anvilml-worker --features mock-hardware` passes, including an integration test that spawns the real Python worker in mock mode and exercises the Ping→Pong sequence.

---

## Group Reference

| Group | Subsystem       | Tasks          | Summary                                                          |
|-------|-----------------|----------------|------------------------------------------------------------------|
| A     | anvilml-worker  | P5-A1 … P5-A3  | env.rs, ManagedWorker spawn/bridge/watchdog, WorkerPool          |

---

## Prerequisites

- P4-A3 complete: `ModelRegistry` exists (worker env building references `ServerConfig` which references model dirs).
- The Python worker stub at `worker/worker_main.py` exists from P1-A4.
- `anvilml-ipc` framing layer is complete (P2-B2).
- `anvilml-hardware` types and detection are complete (P3-B1).

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks | What must match                                                     |
|---------------------------|----------------|---------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §8.3  | P5-A1          | Exact env var set per `DeviceType`; thread var names               |
| `ANVILML_DESIGN.md` §8.1  | P5-A2          | Worker lifecycle state machine: Initializing→Idle→Busy→Dead→Respawning |
| `ANVILML_DESIGN.md` §8.4  | P5-A2          | Two async tasks per worker (stdin writer + stdout reader)           |
| `ANVILML_DESIGN.md` §8.5  | P5-A3          | Ping interval 30 s, Pong timeout 10 s; restart sequence            |
| `ANVILML_DESIGN.md` §6    | P5-A2          | Interpreter path resolution: Linux `{venv}/bin/python3`, Windows `{venv}\Scripts\python.exe` |
| `ANVILML_DESIGN.md` §22.4 | P5-A2          | `PR_SET_PDEATHSIG` on Linux; Job Object on Windows                 |

---

## Task Descriptions

### Group A — anvilml-worker

#### P5-A1: anvilml-worker — env.rs: build_worker_env per device type

**Goal:** Implement the function that constructs the complete environment variable map injected into each spawned Python worker process.

**Files to create or modify:**
- `crates/anvilml-worker/src/env.rs` — `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig) -> HashMap<String, String>`
- `crates/anvilml-worker/src/lib.rs` — expose `env::build_worker_env`
- `crates/anvilml-worker/Cargo.toml` — add `anvilml-core`, `anvilml-hardware` path deps

**Key implementation notes:**
- For `DeviceType::Cuda`: inject `CUDA_VISIBLE_DEVICES={device.index}`.
- For `DeviceType::Rocm`: inject `HIP_VISIBLE_DEVICES={device.index}`, `ROCBLAS_USE_HIPBLASLT=1` (or `0` if `cfg.rocm.use_hipblaslt` is false), and `HSA_OVERRIDE_GFX_VERSION={v}` only if `cfg.rocm.hsa_override_gfx_version` is `Some(v)`.
- For all device types: inject `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `VECLIB_MAXIMUM_THREADS` all set to `cfg.num_threads.to_string()`; inject `ANVILML_NUM_THREADS`, `ANVILML_NUM_INTEROP_THREADS`, `ANVILML_WORKER_ID` (`"worker-{device.index}"`), `ANVILML_DEVICE_INDEX` (`device.index.to_string()`).
- If `ANVILML_WORKER_MOCK` is set in the server's own environment, propagate it to the child with the same value.
- The returned map should contain **only** the variables to inject; it is merged with the child's inherited environment by the caller (`Command::envs`), not used as a replacement.
- Write three fixture tests: one per device type, asserting the correct device-visibility variable is present and the thread vars are all set.

**Acceptance criterion:** `cargo test -p anvilml-worker -- env` exits 0 with 3 tests passing.

---

#### P5-A2: anvilml-worker — ManagedWorker: spawn, IPC bridge, watchdog

**Goal:** Implement the `ManagedWorker` struct that owns one Python child process and drives all communication with it through the `anvilml-ipc` framing layer.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — `ManagedWorker` struct and `spawn()` constructor
- `crates/anvilml-worker/Cargo.toml` — add `tokio` (features: full), `anvilml-ipc` path dep, `tracing`

**Key implementation notes:**
- **Interpreter resolution**: on Linux/macOS, `{cfg.venv_path}/bin/python3`; on Windows, `{cfg.venv_path}\Scripts\python.exe`. Use `#[cfg(windows)]` / `#[cfg(unix)]` for the path construction. If the resolved path does not exist or is not executable, return `Err(AnvilError::WorkerDead("python_missing".to_string()))`.
- **Spawn**: `tokio::process::Command` with `stdin(Stdio::piped())`, `stdout(Stdio::piped())`, `stderr(Stdio::piped())`. Pass `worker/worker_main.py --worker-id worker-{n} --device-index {n}` as arguments. Apply `cmd.envs(build_worker_env(...))`.
- **Orphan cleanup**:
  - On Linux: use `unsafe { pre_exec(|| { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0); Ok(()) }) }`. Add `libc` as a `[target.'cfg(unix)'.dependencies]` entry.
  - On Windows: after spawn, create a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, assign the child PID to it, and hold the Job Object handle alive in `ManagedWorker` for its lifetime. Add `windows` crate or `winapi` as `[target.'cfg(windows)'.dependencies]`.
- **stderr capture**: redirect stderr to a log file at `{cfg.worker_log_dir}/worker-{n}.log`. Use a simple background task that reads from the stderr pipe line by line and appends to the file. Rotation (10 MiB, 3 retained) is a future improvement; for this phase, truncate-on-restart is acceptable.
- **IPC bridge**: spawn two `tokio::spawn` tasks inside `spawn()`:
  - *stdin writer*: `tokio::sync::mpsc::Receiver<WorkerMessage>` → call `write_frame(&mut stdin, &msg)` → forward to child stdin.
  - *stdout reader*: loop `read_frame(&mut stdout, max_mib)` → on `Ok(event)`: `event_tx.send((worker_id.clone(), event))` (ignore send errors — no subscribers is fine); on `Err(_)` or EOF: set `status = Dead`, send `WorkerStatusChanged` event, schedule respawn after 2 s via `tokio::time::sleep`.
- `ManagedWorker::send(&self, msg: WorkerMessage) -> Result<(), AnvilError>`: send to the mpsc channel; if the channel is closed (worker dead), return `Err(AnvilError::WorkerDead(...))`.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0.

---

#### P5-A3: anvilml-worker — WorkerPool: spawn_all, acquire/release, keepalive, restart, shutdown

**Goal:** Implement the `WorkerPool` that aggregates all `ManagedWorker` instances and provides the public interface used by the scheduler and server.

**Files to create or modify:**
- `crates/anvilml-worker/src/pool.rs` — `WorkerPool` struct
- `crates/anvilml-worker/src/lib.rs` — `pub use pool::WorkerPool; pub use managed::ManagedWorker`

**Key implementation notes:**
- `spawn_all(hw: &HardwareInfo, cfg: &ServerConfig) -> Result<Self>`: iterate `hw.gpus`; if empty, spawn one CPU worker using a synthetic `GpuDevice { device_type: Cpu, index: 0, ... }`. Each `ManagedWorker::spawn()` failure should be logged at `error` but should not abort the whole pool — a partial pool is valid. Return `Err` only if no workers spawned at all.
- `list() -> Vec<WorkerInfo>`: snapshot current status of all workers.
- `acquire_idle(device_index: Option<u32>) -> Option<WorkerRef>`: if `device_index = Some(n)`, return the worker at index `n` only if `Idle`; else return `None` (do not re-route). If `device_index = None`, return any `Idle` worker.
- `set_busy(worker_id, job_id)` / `set_idle(worker_id)`: update the worker's status field and `current_job_id`.
- `subscribe_events() -> broadcast::Receiver<(String, WorkerEvent)>`: subscribe to the shared event broadcast.
- **Ping keepalive**: a background `tokio` task per worker sends `Ping { seq }` every 30 s and expects `Pong { seq }` within 10 s. On timeout, call `Child::kill()` (SIGKILL on Unix, TerminateProcess on Windows via `tokio::process::Child::kill()`); the Dead→Respawning path in `ManagedWorker` handles the rest.
- `restart(worker_id)`: send `Shutdown`, wait up to 5 s for `WorkerEvent::Dying`, then force-kill and call `ManagedWorker::spawn()` to replace the entry.
- `shutdown_all()`: send `Shutdown` to all workers, wait up to 10 s per worker for `Dying`, then force-kill any that have not exited.
- **Integration test**: use `ANVILML_WORKER_MOCK=1`; spawn a `WorkerPool` with one CPU worker; send `Ping { seq: 42 }`; assert a `Pong { seq: 42 }` is received on the event broadcast within 2 s.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0 with the Ping→Pong integration test passing.

---

## Phase Acceptance Criteria

```
cargo test -p anvilml-worker --features mock-hardware
cargo clippy --workspace --features mock-hardware -- -D warnings
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
```

---

## Known Constraints and Gotchas

- `pre_exec` on Linux requires `unsafe`. The `libc` crate must be a `[target.'cfg(unix)'.dependencies]` entry in `anvilml-worker/Cargo.toml`, not a regular dependency, to avoid introducing it on Windows.
- The Windows Job Object approach requires the `windows` or `winapi` crate. Use `[target.'cfg(windows)'.dependencies]`. The Job Object handle (`HANDLE`) must be kept alive in `ManagedWorker` as long as the child lives; dropping it would release the limit.
- `tokio::process::Child::kill()` is the cross-platform force-kill mechanism. On Unix it sends `SIGKILL`; on Windows it calls `TerminateProcess`. There is no need for conditional compilation on the kill call itself.
- The `broadcast::Sender` capacity must be set to `cfg.limits.ws_broadcast_capacity` (default 256). A lagging subscriber causes `SendError::Lagged`; the sender should not panic on this. The server's WS handler (phase 007) will disconnect lagged clients with close code 1008.
- `Stdio::piped()` on Windows allocates an anonymous pipe. The pipe handle is inherited by the child. Ensure the child's handle is correctly piped — `tokio::process::Command` handles this correctly when `stdin/stdout` are set to `piped()`, but verify the child is not inheriting unnecessary handles by checking that `Command` does not call `inherit()` for them.
- The `ANVILML_WORKER_MOCK` propagation in `build_worker_env` must check `std::env::var("ANVILML_WORKER_MOCK")` — not a hardcoded value — so that the server process's environment setting flows automatically to all children.
