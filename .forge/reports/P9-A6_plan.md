# Plan Report: P9-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A6                                         |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: pool.rs WorkerPool managing Vec<ManagedWorker> |
| Depends on  | P9-A1, P9-A2, P9-A3, P9-A4, P9-A5            |
| Project     | anvilml                                       |
| Planned at  | 2026-06-16T23:00:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-worker/src/pool.rs` implementing `WorkerPool`, a struct that owns a `Vec<Arc<ManagedWorker>>` and the shared `Arc<RouterTransport>`, and provides methods to spawn workers for all detected devices, retrieve worker info snapshots, and broadcast status changes as `WsEvent::WorkerStatusChanged`. The acceptance criterion is that spawning N mock workers results in N workers in `Idle` status, verified by `cargo test -p anvilml-worker --features mock-hardware -- pool` exiting 0.

## Scope

### In Scope
- **CREATE** `crates/anvilml-worker/src/pool.rs` — `WorkerPool` struct and methods:
  - `spawn_all(cfg: &ServerConfig, devices: &[GpuDevice], transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> impl Future<Output = Result<Self, AnvilError>>`
  - `get_worker_infos(&self) -> Vec<WorkerInfo>`
  - `broadcaster(&self) -> &Arc<EventBroadcaster>`
- **MODIFY** `crates/anvilml-worker/src/lib.rs` — add `pub mod pool;` and `pub use pool::WorkerPool;`
- **MODIFY** `crates/anvilml-worker/Cargo.toml` — bump patch version `0.1.5 → 0.1.6`; add `anvilml-server` dependency for `EventBroadcaster`
- **CREATE** `crates/anvilml-worker/tests/pool_tests.rs` — tests for spawn, get_worker_infos, and broadcaster

### Out of Scope
- Actual subprocess spawning in tests (uses `ManagedWorker::new()` with pre-built channels)
- Dispatch routing logic (belongs in the scheduler, not the pool)
- Worker respawn logic (belongs in `managed.rs` / future respawn integration)
- Any changes to `managed.rs`, `bridge.rs`, `keepalive.rs`, `spawn.rs`, or `env.rs`

## Existing Codebase Assessment

The `anvilml-worker` crate already contains five of the six modules specified in Phase 009: `managed.rs` (ManagedWorker with status state machine), `bridge.rs` (IPC bridge tasks), `keepalive.rs` (heartbeat), `spawn.rs` (subprocess construction), and `env.rs` (env var builder). The `managed.rs` module exposes `get_status() -> Arc<RwLock<WorkerStatus>>` for external status inspection, and its `spawn()` method takes `(cfg, device, transport, worker_id)` and returns `Result<Self, AnvilError>`.

The `RouterTransport` (from `anvilml-ipc`) is already used by `ManagedWorker::spawn()` and the bridge tasks. It wraps an `Arc<Mutex<RouterSocket>>` with a `port: u16` field and provides `send(&[u8], &WorkerMessage)` and `recv() -> Result<(String, WorkerEvent)>`.

The `EventBroadcaster` (from `anvilml-server`) wraps `tokio::sync::broadcast::Sender<WsEvent>` with capacity 1024 and provides `send(WsEvent)` and `subscribe() -> Receiver<WsEvent>`. The pool will hold an `Arc<EventBroadcaster>` to emit `WsEvent::WorkerStatusChanged` when worker statuses change.

The test pattern in `managed_tests.rs` uses `ManagedWorker::new()` with pre-built channels (bypassing subprocess spawning), then spawns `run()` and sends events through the broadcast channel. The pool tests will follow this same pattern.

There is one architectural note: `EventBroadcaster` lives in `anvilml-server`, which normally cannot be depended on by `anvilml-worker` (per the dependency graph). This task adds `anvilml-server` as a dependency of `anvilml-worker`, which is a deviation from the established graph. The ACT agent should consider whether `EventBroadcaster` should be defined at a lower level (e.g., `anvilml-core`) in a follow-up refactor.

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source     | Feature flags confirmed |
|--------|---------------|-----------------|----------------|------------------------|
| crate  | zeromq        | 0.6.0           | workspace Cargo.toml | tokio-runtime, tcp-transport |
| crate  | tokio         | 1.52.3          | workspace Cargo.toml | full |
| crate  | tracing       | 0.1.44          | workspace Cargo.toml | std, attributes |
| crate  | rmp-serde     | 1.3.1           | workspace Cargo.toml | n/a |
| crate  | anvilml-server| path dep        | workspace Cargo.toml | (path dependency, no version) |

All external crate versions are verified from the workspace `Cargo.toml` `[workspace.dependencies]` section. The `anvilml-server` is a path dependency (`../anvilml-server`) — no version lookup needed.

## Approach

1. **Add `anvilml-server` dependency to `anvilml-worker/Cargo.toml`** and bump version `0.1.5 → 0.1.6`.
   - Add `anvilml-server = { path = "../anvilml-server" }` to `[dependencies]`.
   - Note: This introduces a dependency from `anvilml-worker` to `anvilml-server`, which is a deviation from the established dependency graph (ARCHITECTURE.md §3). The `EventBroadcaster` type is defined in `anvilml-server/src/ws/broadcaster.rs`. A follow-up refactor should consider moving `EventBroadcaster` to `anvilml-core`.

2. **Create `crates/anvilml-worker/src/pool.rs`** with the following structure:
   - Import types: `Arc`, `Vec`, `anvilml_core::{GpuDevice, ServerConfig, WorkerInfo, WorkerStatus}`, `anvilml_ipc::RouterTransport`, `anvilml_server::ws::broadcaster::EventBroadcaster`, `tokio::sync::RwLock`.
   - Define `WorkerPool` struct:
     ```rust
     pub struct WorkerPool {
         workers: Vec<Arc<ManagedWorker>>,
         transport: Arc<RouterTransport>,
         broadcaster: Arc<EventBroadcaster>,
     }
     ```
     Each worker is paired with its device index (stored as a separate field alongside the worker Arc in the pool, or retrieved from the `WorkerInfo` construction context). Since `ManagedWorker` does not expose `device_index`, the pool will store `(Arc<ManagedWorker>, u32)` tuples internally (where `u32` is the device index), or store a parallel `Vec<u32>` of device indices.
   - Actually, looking at `managed.rs` more carefully, the pool needs to construct `WorkerInfo` which requires `device_index`. Since `ManagedWorker` doesn't expose `device_index`, the pool will store a parallel `Vec<u32>` of device indices alongside the workers.

   - Implement `WorkerPool`:
     - `pub async fn spawn_all(cfg: &ServerConfig, devices: &[GpuDevice], transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> Result<Self, AnvilError>`
       - Iterate over `devices.iter().enumerate()`, generating `worker_id = format!("worker-{}", i)`.
       - For each device, call `ManagedWorker::spawn(cfg, device, transport.clone(), worker_id)`.
       - Collect `(Arc::new(worker), device.index)` tuples.
       - Store the shared transport and broadcaster.
       - Log at INFO: `tracing::info!(worker_count = %devices.len(), "worker pool spawned")` — mandatory log point per ENVIRONMENT.md §9 (Workers subsystem).
     - `pub async fn get_worker_infos(&self) -> Vec<WorkerInfo>`
       - Iterate over `(worker, device_index)` tuples.
       - For each, read `*worker.status.read().await` to get `WorkerStatus`.
       - Get `worker.worker_id` — but `worker_id` is private in `ManagedWorker`. We need a getter or store the worker_id in the pool.
       - Actually, the pool knows the worker_id because it generated it. Store `(Arc<ManagedWorker>, String, u32)` tuples (worker, worker_id, device_index).
       - Construct `WorkerInfo { id: worker_id, device_index, device_name: worker.device_name.clone(), status, current_job_id: None, vram_used_mib: None }`.
       - Wait — `device_name` is also private in `ManagedWorker`. We need to either add a getter or store the device name in the pool.
       - The cleanest approach: store `(Arc<ManagedWorker>, String, String)` tuples where the second String is the worker_id and the third is the device_name (from `GpuDevice::name`). This avoids needing getters on `ManagedWorker`.
     - `pub fn broadcaster(&self) -> &Arc<EventBroadcaster>`
       - Return a reference to the stored broadcaster.

   - Implement `WorkerStatus` change notification in `spawn_all`:
     - After spawning each worker, spawn a background task that monitors status changes.
     - The task reads the status periodically (or uses a broadcast channel from the worker).
     - Actually, the simplest approach: `get_worker_infos` doesn't need to trigger broadcasts. The broadcasts should happen when the status *changes*. Since the pool doesn't own the status (the `ManagedWorker::run()` loop does), the pool needs a mechanism to observe changes.
     - The cleanest approach: each `ManagedWorker` has an `event_tx: broadcast::Sender<(String, WorkerEvent)>`. The pool could subscribe to these events and emit `WsEvent::WorkerStatusChanged` when it sees status-relevant events.
     - But `event_tx` is private in `ManagedWorker`. The pool can't access it.
     - Alternative: the pool spawns a monitoring task per worker that polls the status Arc with a small interval (e.g., 100ms) and broadcasts on change. This is simple and doesn't require changes to `ManagedWorker`.
     - Actually, looking at the task description again: "On worker status change: send WsEvent::WorkerStatusChanged". This implies the pool should detect and broadcast changes. The polling approach is the simplest that doesn't require modifying `ManagedWorker`.
     - Polling approach: in `spawn_all`, after spawning each worker, spawn a tokio task that:
       1. Reads the current status
       2. Loops: sleep 100ms, read status again, if changed from previous, broadcast `WsEvent::WorkerStatusChanged`, update previous
       3. This task runs alongside the worker's lifecycle

     - Actually, there's a simpler approach that avoids polling: since the pool owns the `Arc<RwLock<WorkerStatus>>` (via `ManagedWorker::get_status()`), and the `ManagedWorker::run()` loop writes to it, we can use a `tokio::sync::Notify` or a broadcast channel to signal changes. But this requires modifying `ManagedWorker` to include a notification mechanism.

     - The simplest approach that doesn't require modifying `ManagedWorker`: after spawning each worker, spawn a monitoring task that polls the status. This is acceptable for a Phase 9 implementation — the monitoring task is lightweight and the 100ms poll interval is reasonable for status change detection.

     - Actually, re-reading the task description: "On worker status change: send WsEvent::WorkerStatusChanged". The task doesn't specify *how* the pool detects changes. The polling approach is fine for this phase.

   - For the tests, since we use `ManagedWorker::new()` (not `spawn()`), there's no subprocess or `run()` loop. The test will manually set the status to `Idle` via the `RwLock` and verify the pool reports `Idle`.

3. **Update `crates/anvilml-worker/src/lib.rs`**:
   - Add `pub mod pool;` after `pub mod respawn;`
   - Add `pub use pool::WorkerPool;` to the re-exports

4. **Create `crates/anvilml-worker/tests/pool_tests.rs`**:
   - Test `test_spawn_all_workers_idle`: Create a `RouterTransport`, a mock `EventBroadcaster`, spawn N workers using `ManagedWorker::new()`, verify `get_worker_infos()` returns N workers all with `status: Idle`.
   - Test `test_broadcaster_returns_reference`: Verify `pool.broadcaster()` returns a non-null reference.
   - Test `test_pool_broadcasts_status_change`: Set a worker's status to `Busy` via the status RwLock, verify the broadcaster received a `WsEvent::WorkerStatusChanged`.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| struct | `anvilml_worker::pool::WorkerPool` | `pub struct WorkerPool { workers: Vec<Arc<ManagedWorker>>, transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster> }` |
| fn | `anvilml_worker::pool::WorkerPool::spawn_all` | `pub async fn spawn_all(cfg: &ServerConfig, devices: &[GpuDevice], transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> Result<Self, AnvilError>` |
| fn | `anvilml_worker::pool::WorkerPool::get_worker_infos` | `pub async fn get_worker_infos(&self) -> Vec<WorkerInfo>` |
| fn | `anvilml_worker::pool::WorkerPool::broadcaster` | `pub fn broadcaster(&self) -> &Arc<EventBroadcaster>` |
| re-export | `anvilml_worker` | `pub use pool::WorkerPool;` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/pool.rs` | WorkerPool struct with spawn_all, get_worker_infos, broadcaster methods |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `pub mod pool;` and `pub use pool::WorkerPool;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6; add `anvilml-server` dependency |
| CREATE | `crates/anvilml-worker/tests/pool_tests.rs` | Integration tests for WorkerPool |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/pool_tests.rs` | `test_spawn_all_workers_idle` | Spawning N workers results in N Idle workers | RouterTransport bound, EventBroadcaster created | N=3 mock workers via `ManagedWorker::new()` | `get_worker_infos()` returns 3 workers, all with `status: Idle` | `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_spawn_all_workers_idle` exits 0 |
| `tests/pool_tests.rs` | `test_broadcaster_returns_reference` | `broadcaster()` returns a valid reference to the stored EventBroadcaster | WorkerPool created | Pool with EventBroadcaster | `pool.broadcaster()` is non-null, same Arc as passed to spawn_all | `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_broadcaster_returns_reference` exits 0 |
| `tests/pool_tests.rs` | `test_pool_broadcasts_status_change` | Status change triggers WsEvent::WorkerStatusChanged broadcast | WorkerPool with monitoring task running, worker in Idle | Set worker status to Busy via RwLock | Broadcaster received `WsEvent::WorkerStatusChanged{worker_id, status: Busy, device_index}` | `cargo test -p anvilml-worker --features mock-hardware -- pool_tests::test_pool_broadcasts_status_change` exits 0 |

## CI Impact

No CI changes required. The task adds a new test module (`pool_tests.rs`) under `crates/anvilml-worker/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` (the rust-linux and rust-windows CI jobs). The new `anvilml-server` dependency of `anvilml-worker` is a path dependency — no external package resolution changes.

## Platform Considerations

None identified. The `WorkerPool` struct and its methods are platform-neutral — they use only `Arc`, `Vec`, `RwLock`, and the existing `ManagedWorker`/`RouterTransport`/`EventBroadcaster` types. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `EventBroadcaster` in `anvilml-server` creates a dependency from `anvilml-worker` to `anvilml-server`, violating the established dependency graph (ARCHITECTURE.md §3). This may cause compilation issues if `anvilml-server` has its own dependency on `anvilml-worker`. | Medium | High | Verify at session start that `anvilml-server` does not transitively depend on `anvilml-worker`. If it does, define a minimal `EventBroadcaster` trait in `anvilml-core` or use `tokio::sync::broadcast::Sender<WsEvent>` directly in the pool. |
| `ManagedWorker` fields (`worker_id`, `device_name`) are private, requiring the pool to store parallel tracking data. If the ACT agent adds getters to `ManagedWorker`, it changes the public API surface of P9-A5. | Low | Medium | The plan stores `(Arc<ManagedWorker>, String, u32)` tuples in the pool, avoiding the need for getters. If the ACT agent adds getters, it must be minimal and not change existing signatures. |
| Status change detection via polling introduces a small delay (100ms) between status change and broadcast. In production, a more efficient notification mechanism may be needed. | Low | Low | Acceptable for Phase 9. The monitoring task is lightweight. A follow-up task can replace polling with a proper notification channel. |
| Test uses `ManagedWorker::new()` which bypasses subprocess spawning. The test verifies pool logic but not the full spawn pipeline (which is tested in managed_tests.rs). | Low | Low | This is by design — the pool tests verify pool behavior, not subprocess behavior. The spawn pipeline is covered by P9-A5 tests. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- pool` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (full crate test suite)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings in modified files)
- [ ] `cargo fmt --all -- --check` exits 0 (formatted code)
- [ ] `head -1 .forge/reports/P9-A6_plan.md` prints `# Plan Report: P9-A6`
- [ ] `grep "^## " .forge/reports/P9-A6_plan.md` shows all 12 section headings
- [ ] `wc -l .forge/reports/P9-A6_plan.md` outputs > 40 lines
