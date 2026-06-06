# Plan Report: P9-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A5                                       |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: WorkerPool spawn_all + list + acquire/set status |
| Depends on  | P9-A4 (ManagedWorker spawn + IPC bridge)    |
| Project     | anvilml                                     |
| Planned at  | 2026-06-06T11:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `src/pool.rs` — the `WorkerPool` struct for `anvilml-worker` that manages a collection of `ManagedWorker` instances. It provides lifecycle orchestration: spawning workers per detected device (or one CPU worker as fallback), listing workers, acquiring idle workers for job dispatch, updating busy/idle status, subscribing to IPC events, and sending messages. On `Ready` event it sets the worker status to `Idle` and merges authoritative capabilities (arch, fp16, bf16, flash_attention, vram) into the matching `GpuDevice` with `capabilities_source = Worker`, per design §5.4.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/pool.rs` with `WorkerPool` struct and methods:
  - `spawn_all(hw: &HardwareInfo, cfg: &ServerConfig) -> Self` — spawn one worker per GPU device; one CPU worker if no GPUs detected. On each `Ready` event: set status to Idle AND merge caps into the matching `GpuDevice`.
  - `list(&self) -> Vec<WorkerInfo>` — return current info for all workers.
  - `acquire_idle(&self, device_index: Option<u32>) -> Option<Arc<ManagedWorker>>` — find an idle worker matching the optional device index.
  - `set_busy(&self, worker_id: &str, job_id: Uuid)` — mark a worker busy with the given job ID.
  - `set_idle(&self, worker_id: &str)` — mark a worker idle.
  - `subscribe_events(&self) -> broadcast::Receiver<(String, WorkerEvent)>` — subscribe to pooled events.
  - `send(&self, worker_id: &str, msg: WorkerMessage) -> impl Future<Output=Result<(), AnvilError>>` — route a message to the named worker.
  - Internal event listener task that processes events from each worker's broadcast channel, updating status and merging capabilities on Ready.
- Re-export `WorkerPool` from `lib.rs`.
- Add test module: spawn_all with mock-hardware creates one CPU worker; verify it reaches Idle after Ready event (mocked).

### Out of Scope
- `shutdown_all()` / `restart(worker_id)` — deferred to P9-A6 or later.
- Keepalive ping/pong watchdog — deferred to P9-A6 or later.
- Hardware capability table updates in SQLite — handled by server layer, not pool.
- Worker respawn logic (Dead → Respawning → Initializing) — deferred to P9-A6 or later.
- VramLedger integration with MemoryReport events — scheduler's concern.

## Approach

1. **Create `crates/anvilml-worker/src/pool.rs`** with the following structure:

   ```rust
   pub struct WorkerPool {
       workers: Vec<Arc<ManagedWorker>>,
       event_tx: broadcast::Sender<(String, WorkerEvent)>,
       device_map: Arc<RwLock<HashMap<u32, usize>>>,  // device_index → worker index
       hardware: Arc<Mutex<HardwareInfo>>,             // mutable copy for capability merging
   }
   ```

2. **Implement `spawn_all(hw: &HardwareInfo, cfg: &ServerConfig) -> Self`**:
   - Create broadcast channel (capacity 256, matching existing ManagedWorker).
   - Clone hardware info into `Arc<Mutex<HardwareInfo>>`.
   - For each `GpuDevice` in `hw.gpus`: create a `ManagedWorker::new("worker-{i}", i)`, then `spawn(&device, cfg)`. Store index mapping.
   - If `hw.gpus.is_empty()`: create one CPU worker at index 0 with a synthetic `GpuDevice { device_type: Cpu, ... }`.
   - Spawn an internal event listener task that subscribes to each worker's broadcast channel and processes events:
     - On `WorkerEvent::Ready{arch, fp16, bf16, flash_attention, vram_total_mib, vram_free_mib, device_index}`: find the matching `GpuDevice` in the hardware copy, update its caps (fp16, bf16, flash_attention), arch, vram fields, set `capabilities_source = Worker`, and log at INFO level.
   - Return the pool.

3. **Implement status/query methods**:
   - `list()`: iterate workers, call `worker.info()` for each, collect into `Vec<WorkerInfo>`.
   - `acquire_idle(device_index)`: lock device_map or iterate workers; find one with `status == Idle` matching the optional index filter.
   - `set_busy(worker_id, job_id)`: find worker by ID, acquire write lock on status, set to Busy; update `current_job_id` in the WorkerInfo context (via internal tracking).
   - `set_idle(worker_id)`: find worker, set status back to Idle, clear current_job_id.

4. **Implement event routing**:
   - `subscribe_events()`: return `event_tx.subscribe()`.
   - `send(worker_id, msg)`: iterate workers, compare `worker.worker_id()` with the target, call `worker.send(msg)`. Return error if not found.

5. **Re-export from `lib.rs`**: Add `pub mod pool;` and `pub use pool::WorkerPool;`.

6. **Add test** in `pool.rs`:
   - `spawn_all_creates_cpu_worker_when_no_gpus`: with mock-hardware, pass a `HardwareInfo { gpus: [], ... }`, call `spawn_all`, verify exactly one worker exists, and its status is Idle (mock Ready event injected via the reader task's broadcast).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-worker/src/pool.rs` | WorkerPool struct + spawn_all + list + acquire/set status + subscribe_events + send |
| Modify | `crates/anvilml-worker/src/lib.rs` | Add `pub mod pool;` and `pub use pool::WorkerPool;` re-export |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/pool.rs` (mod tests) | `spawn_all_creates_cpu_worker_when_no_gpus` | With empty GPU list, spawn_all creates exactly one CPU worker; status transitions to Idle after Ready event is broadcast |

## CI Impact

No CI changes required. The existing `cargo test --workspace --features mock-hardware` gate will automatically include the new pool module tests. No new dependencies are added — all used crates (tokio, anvilml-core, anvilml-ipc) are already in the worker crate's Cargo.toml. No platform cross-check changes needed since pool.rs uses only cross-platform Rust (no `#[cfg(unix)]` or `#[cfg(windows)]`).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Ready event capability merging needs access to hardware info that pool doesn't own | High | Medium | Pool owns a mutable copy of HardwareInfo (Arc<Mutex<HardwareInfo>>). The merge updates the copy in-place. Server layer can read from this or maintain its own — P9-A5 only needs the pool's view. |
| Test cannot spawn real Python worker for integration test | Certain | Low | Use mock-hardware feature; inject a Ready event directly into the broadcast channel to simulate the handshake without requiring a live Python process. The existing managed.rs tests already use this pattern (see `eof_sets_dead`). |
| Worker ID matching in `send()` is O(n) | Low | Low — worker count is small (≤ devices + 1) | Linear scan over workers is acceptable for MVP; N is bounded by GPU count (typically ≤ 8). No optimization needed. |
| Capability merge on Ready could conflict with pre-spawn device table data | Low | Low — design §5.4 explicitly says worker caps are authoritative and overwrite pre-spawn values | Follow the spec exactly: overwrite arch, caps fields, set capabilities_source = Worker. This is the intended behavior. |

## Acceptance Criteria

- [ ] `crates/anvilml-worker/src/pool.rs` exists with `WorkerPool` struct implementing all specified methods
- [ ] `lib.rs` re-exports `WorkerPool` as `pub use pool::WorkerPool;`
- [ ] `spawn_all` creates one worker per GPU device, or one CPU worker if no GPUs
- [ ] On `Ready` event, worker status transitions to Idle AND caps (arch/fp16/bf16/flash_attention/vram) merge into matching GpuDevice with `capabilities_source = Worker`
- [ ] `list()` returns accurate `Vec<WorkerInfo>` for all workers
- [ ] `acquire_idle(Some(n))` finds idle worker at device index n; `acquire_idle(None)` finds any idle worker
- [ ] `set_busy` / `set_idle` correctly transition worker status and job tracking
- [ ] `subscribe_events()` returns a broadcast receiver that delivers events from all workers
- [ ] `send(worker_id, msg)` routes messages to the correct worker by ID
- [ ] Test `spawn_all_creates_cpu_worker_when_no_gpus` passes with `cargo test -p anvilml-worker --features mock-hardware -- pool`
- [ ] `cargo clippy --package anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check)
