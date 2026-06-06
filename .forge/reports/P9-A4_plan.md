# Plan Report: P9-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A4                                              |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | anvilml-worker: ManagedWorker spawn + IPC bridge (writer/reader tasks) |
| Depends on  | P9-A3                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-06T10:00:00Z                              |
| Attempt     | 1                                                  |

## Objective

Create `crates/anvilml-worker/src/managed.rs` implementing the `ManagedWorker` struct that owns a Python worker child process lifecycle (spawn, stdin/stdout piping, IPC bridge) and re-exports it from `anvilml-worker`'s `lib.rs`. The task adds `tokio(full)`, `anvilml-ipc`, and `tracing` as dependencies.

## Scope

### In Scope
- Add `tokio` (full features), `tracing`, and `anvilml-ipc` to `anvilml-worker/Cargo.toml`
- Create `crates/anvilml-worker/src/managed.rs` with:
  - `ManagedWorker` struct fields: `worker_id`, `device_index`, `status: Arc<RwLock<WorkerStatus>>`, `tx: mpsc::Sender<WorkerMessage>`, `event_tx: broadcast::Sender<(String, WorkerEvent)>`, `child: Mutex<Option<Child>>`, `handle: JoinHandle<()>`
  - `ManagedWorker::new()` constructor (initializes status to `Initializing`, creates channels)
  - `ManagedWorker::spawn()` method: resolves venv python path (Linux `{venv}/bin/python3`, Windows `{venv}\Scripts\python.exe`), builds command with `build_worker_env`, pipes stdin/stdout, redirects stderr to log via `tokio::process::Command`, launches writer and reader tasks
  - **Writer task**: receives `WorkerMessage` from mpsc receiver, calls `framing::write_frame` on child stdin, logs sent messages at DEBUG (`worker_id=`, `message_type=`)
  - **Reader task**: calls `framing::read_frame` on child stdout in a loop, broadcasts each event via `event_tx`, on EOF sets status to `Dead` and emits `WorkerStatusChanged`
  - `ManagedWorker::send(msg)` method: forwards message through mpsc channel
  - `ManagedWorker::subscribe()` method: returns broadcast receiver clone
  - `ManagedWorker::get_status()` method: clones current status from Arc<RwLock>
  - `ManagedWorker::worker_id()` accessor
- Update `crates/anvilml-worker/src/lib.rs` to declare `pub mod managed;` and re-export `ManagedWorker`
- Add unit tests in `managed.rs` using mock hardware mode: a test that spawns the worker, sends Ping, receives Pong via broadcast, and verifies exit 0

### Out of Scope
- WorkerPool implementation (P9-A5)
- Keepalive/Ping timer loop (handled by WorkerPool or server layer)
- Respawning logic (handled by WorkerPool on Dead detection)
- `POST /v1/workers/:id/restart` endpoint (P9-A6)
- Any changes to backend/ or anvilml-server/

## Approach

1. **Update Cargo.toml**: Add three dependencies to `crates/anvilml-worker/Cargo.toml`:
   - `tokio = { workspace = true, features = ["full"] }` — the workspace already defines `tokio = { version = "1.52.3", features = ["full"] }` at line 34 of the root Cargo.toml
   - `tracing = { workspace = true }` — already defined in workspace as `"0.1.44"`
   - `anvilml-ipc = { path = "../anvilml-ipc" }` — **already present** in existing Cargo.toml (no change needed for this one)

2. **Create `src/managed.rs`**:
   a. **Imports**: `tokio::{sync::{mpsc, broadcast, Mutex, RwLock}, io::{AsyncWriteExt, AsyncReadExt}, process::Command, spawn, task::JoinHandle}`, `anvilml_ipc::{WorkerMessage, WorkerEvent, framing::{write_frame, read_frame}}`, `anvilml_core::types::{worker::{WorkerStatus, WorkerInfo}}, GpuDevice, ServerConfig`, `std::process::Stdio`, `tracing::{info, debug, warn}`.

   b. **Struct definition**:
   ```rust
   pub struct ManagedWorker {
       worker_id: String,
       device_index: u32,
       status: Arc<RwLock<WorkerStatus>>,
       tx: mpsc::Sender<WorkerMessage>,
       event_tx: broadcast::Sender<(String, WorkerEvent)>,
       child: Mutex<Option<tokio::process::Child>>,
       handle: JoinHandle<()>,
   }
   ```

   c. **`new(worker_id, device_index, event_tx)`**: Creates `mpsc::channel(64)` for messages, `broadcast::channel(256)` for events (matching `ws_broadcast_capacity` from config defaults of 256), initializes status to `Initializing`.

   d. **`spawn(device, cfg)`**:
      - Resolve python path: if `cfg.venv_path` is set, construct `python_path` using `std::path::Path::new(&cfg.venv_path)`:
        - On Unix: `{venv}/bin/python3`
        - On Windows: `{venv}\Scripts\python.exe`
      - Build command: `Command::new(python_path).arg("worker_main.py").args(["--worker-id", &self.worker_id, "--device-index", &self.device_index.to_string()])`
      - Inject env vars from `build_worker_env(&device, cfg)` via `.envs()`
      - Stdio: stdin `Stdio::piped()`, stdout `Stdio::piped()`, stderr `Stdio::piped()`
      - Spawn the child process
      - Take stdin/stdout handles
      - **Writer task**: clone `tx` (as receiver), loop `rx.recv().await`, call `write_frame(&mut stdin, &msg).await`, log at DEBUG level with `worker_id` and message discriminant
      - **Reader task**: loop `read_frame(&mut stdout, max_mib).await`, broadcast `(self.worker_id.clone(), event)` via `event_tx.send()`, on error/EOF set status to `Dead`, log at WARN, emit `WorkerStatusChanged` event
      - The combined handle joins both tasks

   e. **`send(msg)`**: `self.tx.send(msg).await.map_err(...)` with tracing

   f. **`subscribe()`**: `self.event_tx.subscribe()`

   g. **`get_status()`**: `*self.status.read().await`

3. **Update `lib.rs`**: Add `pub mod managed;` and `pub use managed::ManagedWorker;`.

4. **Tests** (in `managed.rs` under `#[cfg(test)]`):
   - Test `spawn + ping_pong`: spawn a mock-mode worker (via `ANVILML_WORKER_MOCK=1`), send `Ping { seq: 1 }`, read from broadcast receiver, assert `Pong { seq: 1 }` received within timeout, then send `Shutdown`, verify worker exits.
   - Test `status_transitions`: verify status goes `Initializing → (on Ready) → Idle` after receiving Ready event.
   - Test `eof_sets_dead`: simulate pipe close and verify status transitions to `Dead`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `tokio` (full) and `tracing` workspace deps (anvilml-ipc already present) |
| Create | `crates/anvilml-worker/src/managed.rs` | ManagedWorker struct, spawn, IPC bridge writer/reader tasks |
| Modify | `crates/anvilml-worker/src/lib.rs` | Declare `pub mod managed;` and re-export `ManagedWorker` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `managed.rs` (unit) | `spawn_ping_pong` | Full spawn → Ping sent → Pong received on broadcast channel → Shutdown → exit 0 |
| `managed.rs` (unit) | `status_transitions` | Status moves from Initializing → Idle after Ready event broadcast |
| `managed.rs` (unit) | `eof_sets_dead` | Pipe EOF triggers Dead status and WorkerStatusChanged event |

## CI Impact

No CI workflow files are modified. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy --workspace --features mock-hardware`, cross-checks for both Linux and Windows targets) will automatically exercise the new code since it is part of the workspace. The `anvilml-worker` crate is already included in `--workspace` builds.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `anvilml-ipc` framing functions (`write_frame`, `read_frame`) use `AnvilError` which may need to be mapped to a simpler error type for the writer/reader tasks | Medium | Low — writer/reader can use `Result<(), anyhow::Error>` or a dedicated `WorkerError` to avoid pulling in all of `AnvilError` variants | Use `anvilml_core::error::AnvilError` via `map_err`; the existing framing functions already return this type |
| Cross-platform python path resolution: Windows path uses backslash, Unix uses forward slash | Low | Low — use `std::path::Path` which handles platform separators automatically; explicit suffix append for `python3` / `python.exe` | Use `Path::join()` with platform-specific final component |
| Test requires actual Python interpreter and venv to be available in CI/build environment | Medium | High — test would fail if no Python is installed | Guard the integration test behind a feature flag or skip when `which python3` fails; use `#[cfg(feature = "mock-hardware")]` which is already required by the task |
| Broadcast channel capacity (256) may be insufficient for burst events | Low | Low — 256 matches existing config default `ws_broadcast_capacity`; if needed, make it configurable later | Keep as fixed constant matching config; document in code comment |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware -- managed` exits 0 (Ping→Pong test passes)
- [ ] `cargo check --target x86_64-pc-windows-gnu -p anvilml-worker --features mock-hardware` exits 0 (Windows cross-check)
- [ ] `ManagedWorker` struct is public, exported from `lib.rs`, and has `spawn()`, `send()`, `subscribe()`, `get_status()`, and accessor methods
- [ ] Writer task sends frames via `framing::write_frame`; reader task receives via `framing::read_frame` and broadcasts events
- [ ] On EOF, worker status transitions to `Dead`
- [ ] Logging: DEBUG for each IPC message sent/received with `worker_id=` and `message_type=`/`event_type=` fields (per FORGE_AGENT_RULES §11.5)
