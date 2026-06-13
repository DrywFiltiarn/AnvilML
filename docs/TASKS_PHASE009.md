# Tasks: Phase 009 — Worker Spawn & Handshake

| Field | Value |
|-------|-------|
| Phase | 009 |
| Name | Worker Spawn & Handshake |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 8 |

## Overview

Phase 009 builds the full worker lifecycle stack on top of the verified IPC transport: `WorkerPool`, `ManagedWorker`, the IPC bridge tasks, the keepalive heartbeat, the subprocess spawn module, the environment builder, and the mock Python worker entry point.

Each concern lives in its own file as defined in `ARCHITECTURE.md §2`:
- `pool.rs` — `WorkerPool`: Vec of `ManagedWorker`, routes dispatches, broadcasts status changes
- `managed.rs` — `ManagedWorker`: status state machine, coordinates sub-tasks
- `spawn.rs` — subprocess `Command` construction + env injection
- `bridge.rs` — two independent reader/writer tokio tasks using `RouterTransport`
- `keepalive.rs` — Ping/Pong heartbeat + pong timeout watchdog
- `env.rs` — `WorkerEnv`: builds environment variable map per `ENVIRONMENT.md §3.4`
- `respawn.rs` — `RespawnPolicy`: backoff logic and max-attempt guard

After Phase 009 the server spawns a mock Python worker on startup, the worker sends `Ready`, and `GET /v1/workers` shows `status: "Idle"`.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-worker Rust | P9-A1 … P9-A6 | env.rs, spawn.rs, bridge.rs, keepalive.rs, managed.rs, pool.rs |
| B | worker Python | P9-B1 | worker_main.py mock mode startup + Ready event |
| C | anvilml-server | P9-C1 | GET /v1/workers handler + AppState wiring |

## Prerequisites

Phase 008 complete: `RouterTransport` works with 1000-trip stress test passing. `WorkerMessage`, `WorkerEvent`, `NodeTypeDescriptor` types finalised.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §9.3` | P9-A5 | `ManagedWorker` state machine transitions |
| `ANVILML_DESIGN.md §9.6` | P9-A1 | Env var names injected into worker subprocess |
| `ENVIRONMENT.md §3.4` | P9-A1 | All 7 worker env vars with correct types |

## Task Descriptions

### Group A — anvilml-worker Rust

#### P9-A1: anvilml-worker: env.rs WorkerEnv

**Goal:** Implement `crates/anvilml-worker/src/env.rs` with `pub fn build_worker_env(device: &GpuDevice, cfg: &ServerConfig, port: u16) -> HashMap<String, String>` injecting all 7 env vars from `ENVIRONMENT.md §3.4`. Tests verify each key is present and has the correct value.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- env` exits 0 with ≥ 5 tests.

#### P9-A2: anvilml-worker: spawn.rs subprocess Command construction

**Goal:** Implement `crates/anvilml-worker/src/spawn.rs` with `pub fn build_command(cfg: &ServerConfig, device: &GpuDevice, port: u16) -> Command`. Command: Python interpreter path + `worker/worker_main.py`. Env vars from `build_worker_env`. `#[cfg(unix)]` sets `PR_SET_PDEATHSIG`. `#[cfg(windows)]` Job Object orphan cleanup.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- spawn` exits 0.

#### P9-A3: anvilml-worker: bridge.rs reader/writer IPC tasks

**Goal:** Implement `crates/anvilml-worker/src/bridge.rs` with `pub fn start(transport: Arc<RouterTransport>, worker_id: String, tx: mpsc::Sender<WorkerMessage>, event_tx: broadcast::Sender<(String, WorkerEvent)>) -> (JoinHandle<()>, JoinHandle<()>)`. Two independent tasks: writer reads from `tx` channel and sends via `transport.send`; reader calls `transport.recv` and broadcasts result. Both tasks log sent/received at DEBUG.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- bridge` exits 0 with ≥ 3 tests.

#### P9-A4: anvilml-worker: keepalive.rs heartbeat + pong timeout

**Goal:** Implement `crates/anvilml-worker/src/keepalive.rs` with `pub fn start(worker_id: String, tx: mpsc::Sender<WorkerMessage>, event_rx: broadcast::Receiver<(String, WorkerEvent)>, on_timeout: impl Fn() + Send + 'static)`. Sends `Ping{seq}` every 30 s; expects `Pong{seq}` within 10 s; calls `on_timeout` if pong not received.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0; timeout callback fires within 10 s when pong is not sent.

#### P9-A5: anvilml-worker: managed.rs ManagedWorker state machine

**Goal:** Implement `crates/anvilml-worker/src/managed.rs` with `ManagedWorker` owning status `Arc<RwLock<WorkerStatus>>`, message sender channel, event broadcast sender, child process handle, and join handles. `pub async fn spawn(cfg, device, transport) -> Result<Self>`. Transitions per `ANVILML_DESIGN.md §9.3`. Respawning uses `RespawnPolicy`.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- managed` exits 0 with ≥ 6 tests (status transitions, keepalive timeout triggers Dead, spawn reaches Idle).

#### P9-A6: anvilml-worker: pool.rs WorkerPool

**Goal:** Implement `crates/anvilml-worker/src/pool.rs` with `WorkerPool` holding `Vec<Arc<ManagedWorker>>` and the shared `Arc<RouterTransport>`. `pub async fn spawn_all(cfg, devices, transport) -> Self`. `pub async fn get_worker_infos(&self) -> Vec<WorkerInfo>`. Broadcasts `WsEvent::WorkerStatusChanged` when any worker status changes.

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware -- pool` exits 0; spawning N mock workers results in N Idle workers.

### Group B — worker Python

#### P9-B1: worker/worker_main.py mock mode startup

**Goal:** Implement `worker/worker_main.py` per `ANVILML_DESIGN.md §13.1`. In mock mode (`ANVILML_WORKER_MOCK=1`): skip torch import; call `ipc.connect(port, worker_id)`; send `WorkerEvent::Ready` with stub capability values and empty `node_types`; enter message dispatch loop. Handle `Ping → Pong`, `Shutdown → exit 0`.

**Files to create:**
- `worker/worker_main.py`
- `worker/__init__.py` (empty)
- `worker/tests/test_worker_main.py` — mock-mode startup test

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0 with ≥ 4 tests.

### Group C — anvilml-server

#### P9-C1: anvilml-server: GET /v1/workers + WorkerPool in AppState

**Goal:** Add `workers: Arc<WorkerPool>` to `AppState`. In `main.rs`: create `RouterTransport::bind()`, call `WorkerPool::spawn_all()`. Implement `list_workers` handler in `handlers/workers.rs` returning `Vec<WorkerInfo>`. Update `stats_tick.rs` to pass real `WorkerPool`. Mount `GET /v1/workers` in `build_router`.

**Acceptance criterion:** `curl /v1/workers` returns JSON array with at least one entry having `status: "Idle"` within 30 s of server start.

## Phase Acceptance Criteria

```bash
cargo test -p anvilml-worker --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
cargo run --features mock-hardware &
sleep 30
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; workers=json.load(sys.stdin); assert any(w['status']=='Idle' for w in workers)"
kill %1
```

## Known Constraints and Gotchas

- `managed.rs` must stay within its review threshold. The sub-modules (`spawn.rs`, `bridge.rs`, `keepalive.rs`, `respawn.rs`) are mandatory splits — `ManagedWorker` delegates all subprocess, IPC, and timing logic to them.
- The 60-second Ready timeout must be implemented in `managed.rs`: if no `Ready` event is received within 60 s, transition to Dead and trigger respawn.
- `WorkerPool` holds the `RouterTransport` because ROUTER is one socket for all workers. `ManagedWorker` instances send and receive via the shared transport, identified by their `worker_id`.
- The Python worker `ANVILML_WORKER_MOCK=1` path must not import `torch` or `diffusers`. Even attempting an import that fails will cause the test suite to fail in CI where torch is not installed.
