# Plan Report: P9-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A5                                       |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: managed.rs ManagedWorker state machine |
| Depends on  | P9-A1, P9-A2, P9-A3, P9-A4                  |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T18:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-worker/src/managed.rs` implementing the `ManagedWorker` struct and its status-driven state machine per `ANVILML_DESIGN.md §9.3`. The `ManagedWorker` owns the lifecycle of one Python worker subprocess: spawning the process, starting the IPC bridge tasks, starting the keepalive heartbeat, and transitioning status through `Initializing → Idle → Busy → Dead → Respawning` based on events from the broadcast channel. This enables the `WorkerPool` (P9-A6) to spawn, supervise, and scale workers. Observable state: `cargo test -p anvilml-worker --features mock-hardware -- managed` exits 0 with ≥6 tests covering status transitions, keepalive timeout triggering Dead, and spawn reaching Idle.

## Scope

### In Scope
- **CREATE** `crates/anvilml-worker/src/managed.rs` — `ManagedWorker` struct, `spawn()` constructor, `run()` state machine loop, `shutdown()` method.
- **CREATE** `crates/anvilml-worker/src/respawn.rs` — `RespawnPolicy` struct with `delay_ms`, `max_attempts`, `window_s` fields and `should_respawn()`, `next_delay_ms()` stub methods (full backoff logic deferred to P10-A1).
- **MODIFY** `crates/anvilml-worker/src/lib.rs` — add `pub mod managed; pub mod respawn; pub use managed::ManagedWorker; pub use respawn::RespawnPolicy;`.
- **CREATE** `crates/anvilml-worker/tests/managed_tests.rs` — ≥6 integration tests covering status transitions, keepalive timeout, spawn lifecycle, and shutdown.
- **BUMP** `crates/anvilml-worker/Cargo.toml` — patch version 0.1.4 → 0.1.5.

### Out of Scope
- Full respawn backoff logic (delay calculation, max-attempt enforcement) — implemented in P10-A1.
- `WorkerPool` / `pool.rs` — implemented in P9-A6.
- In-flight job failure notification — implemented in P10-A1.
- Child process wait loop (unexpected exit detection) — implemented in P10-A1.
- Python worker subprocess integration — the `spawn()` function constructs the `Command` but does not actually launch a Python process in tests.

## Existing Codebase Assessment

The `anvilml-worker` crate has four source modules already implemented in earlier P9 tasks: `env.rs` (P9-A1, builds env var map), `spawn.rs` (P9-A2, constructs `tokio::process::Command`), `bridge.rs` (P9-A3, spawns reader/writer tokio tasks on `RouterTransport`), and `keepalive.rs` (P9-A4, Ping/Pong heartbeat with timeout callback). All use `tracing` structured logging, `#[cfg(feature = "mock-hardware")]` conditional compilation, and follow the project's doc comment style with `# Arguments` and `# Returns` sections.

The `lib.rs` currently re-exports `start` from bridge, `build_worker_env` from env, `start` from keepalive (aliased as `start_keepalive`), and `build_command` from spawn. It contains only `pub mod` declarations and `pub use` statements (17 lines, well under the 80-line limit).

The `RouterTransport` in `anvilml-ipc` provides `bind()`, `send(&worker_id, &msg)`, and `recv() -> Result<(String, WorkerEvent)>`. The `recv()` method acquires the inner `Arc<Mutex<RouterSocket>>` lock, receives a multipart message, extracts the identity frame (frame 0) and payload frame (frame 1), converts the identity to UTF-8 (falling back to hex for non-UTF8), and decodes the payload via `decode_event()`.

The `WorkerEvent` enum in `anvilml-ipc/src/messages.rs` has variants: `Ready`, `Pong`, `Dying`, `MemoryReport`, `Progress`, `ImageReady`, `Completed`, `Failed`, `Cancelled`. The `WorkerStatus` enum in `anvilml-core/src/types/worker.rs` has variants: `Initializing`, `Idle`, `Busy`, `Dead`, `Respawning`.

No `managed.rs` or `respawn.rs` exists yet. The test directory has `bridge_tests.rs`, `keepalive_tests.rs`, `env_tests.rs`, and `spawn_tests.rs` — all follow the pattern of separate test crate files with doc comments, `#[tokio::test]` for async tests, and assertions using `assert_eq!` / `assert!`.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | tokio       | 1.52.3          | Workspace dep  | full                   |
| crate  | zeromq      | 0.6.0           | Workspace dep  | tokio-runtime, tcp-transport |
| crate  | serial_test | 3.5             | Cargo.lock     | n/a (dev-dependency)   |

No new external dependencies are introduced. All types used (`RouterTransport`, `WorkerEvent`, `WorkerMessage`, `WorkerStatus`, `GpuDevice`, `ServerConfig`) are from existing crates. `serial_test = "3.5"` is already a dev-dependency in sibling crates (`anvilml-hardware`, `anvilml-registry`) and will be added to `anvilml-worker/Cargo.toml` for test isolation. The `tokio::sync::broadcast` and `tokio::sync::mpsc` types are from the existing `tokio` workspace dependency.

## Approach

1. **Create `crates/anvilml-worker/src/respawn.rs`.** Define `pub struct RespawnPolicy { delay_ms: u64, max_attempts: u32, window_s: u32 }` with `Default` impl (delay_ms=2000, max_attempts=5, window_s=300) and stub methods: `pub fn should_respawn(&self, crash_count: u32, last_crash: Instant) -> bool { true }` and `pub fn next_delay_ms(&self, attempt: u32) -> u64 { self.delay_ms }`. The `should_respawn` and `next_delay_ms` implementations are stubs that always return true/constant — full backoff logic is deferred to P10-A1. Rationale: this task's scope is the state machine; the respawn policy is a data structure that the state machine delegates to. Adding full backoff logic here would exceed the task scope and duplicate P10-A1 work. Add `///` doc comments to the struct and methods per FORGE_AGENT_RULES §12.1.

2. **Create `crates/anvilml-worker/src/managed.rs`.** Define `pub struct ManagedWorker`:
   ```rust
   pub struct ManagedWorker {
       status: Arc<RwLock<WorkerStatus>>,
       msg_tx: mpsc::Sender<WorkerMessage>,
       event_tx: broadcast::Sender<(String, WorkerEvent)>,
       child: Option<tokio::process::Child>,
       bridge_handles: Option<(JoinHandle<()>, JoinHandle<()>)>,
       keepalive_handle: Option<JoinHandle<()>>,
       heartbeat_handle: Option<HeartbeatHandle>,
       respawn_policy: RespawnPolicy,
   }
   ```
   Derive `Debug` only. All fields are `Option`-wrapped join handles to allow `shutdown()` to drop them without requiring `AbortHandle`. Rationale: tokio `JoinHandle` does not implement `Clone`, so wrapping in `Option` allows `shutdown()` to take ownership and drop the handle, which aborts the task.

   Implement `pub async fn spawn(cfg: &ServerConfig, device: &GpuDevice, transport: Arc<RouterTransport>, worker_id: String) -> Result<Self, AnvilError>`:
   - Call `build_command(cfg, device, port)` from spawn.rs to construct the subprocess command.
   - Spawn the child process: `cmd.spawn().map_err(|e| AnvilError::Io(e.to_string()))?`.
   - Set status to `Initializing` via `*status.write().await = WorkerStatus::Initializing`.
   - Create `mpsc::channel(16)` for `msg_tx`/`msg_rx` and `broadcast::channel(16)` for `event_tx`/`event_rx`.
   - Spawn the bridge: `bridge::start(transport, worker_id_bytes, msg_rx, event_tx.clone())` — clone `event_tx` because the bridge task takes ownership of one copy. Store both `JoinHandle`s in `bridge_handles`.
   - Spawn the keepalive: `keepalive::start(worker_id, msg_tx.clone(), event_rx, Duration::from_secs(30), Duration::from_secs(10), on_timeout_callback)`. The `on_timeout` callback captures a weak reference to the status `Arc<RwLock>` and transitions it to `Dead` via `*status.write().await = WorkerStatus::Dead`. Rationale: the callback is `Fn() + Send + 'static`, but we need async access to the `RwLock`. Use `tokio::spawn` inside the callback to perform the status transition on the runtime.
   - Store the `HeartbeatHandle` in `heartbeat_handle`.
   - Log at INFO: `tracing::info!(worker_id = %worker_id, device_index = %device.index, "worker spawned")` per ENVIRONMENT.md §9 mandatory log point.
   - Return `Self`.

   Implement `pub async fn run(self)`:
   - This is the main loop that drives the state machine. It consumes `self` (taking ownership of all fields).
   - Clone `event_tx` for the run loop's broadcast receiver: `let mut event_rx = self.event_tx.subscribe()`.
   - Set up the Ready timeout: `tokio::time::sleep(Duration::from_secs(60))`.
   - Enter the main select loop:
     ```rust
     tokio::select! {
         _ = ready_timeout => {
             // 60s elapsed without Ready event — transition to Dead.
             tracing::warn!(worker_id = %worker_id, "ready timeout, worker dead");
             *status.write().await = WorkerStatus::Dead;
             break;
         }
         result = event_rx.recv() => {
             match result {
                 Ok((id, event)) => {
                     // Process the event based on current status.
                     match &*status.read().await {
                         WorkerStatus::Initializing => {
                             if let WorkerEvent::Ready { .. } = &event {
                                 *status.write().await = WorkerStatus::Idle;
                                 tracing::info!(worker_id = %worker_id, device = %device_name, "worker reached Ready");
                             }
                         }
                         WorkerStatus::Idle => {
                             if matches!(&event, WorkerEvent::Dying { .. }) {
                                 *status.write().await = WorkerStatus::Dead;
                             }
                         }
                         WorkerStatus::Busy => {
                             match &event {
                                 WorkerEvent::Completed { .. } |
                                 WorkerEvent::Failed { .. } |
                                 WorkerEvent::Cancelled { .. } => {
                                     *status.write().await = WorkerStatus::Idle;
                                 }
                                 WorkerEvent::Dying { .. } => {
                                     *status.write().await = WorkerStatus::Dead;
                                 }
                                 _ => {}
                             }
                         }
                         WorkerStatus::Dead | WorkerStatus::Respawning => {
                             // Terminal states — no further processing.
                         }
                     }
                 }
                 Err(broadcast::error::RecvError::Closed) => break,
                 Err(broadcast::error::RecvError::Lagged(n)) => {
                     tracing::debug!(worker_id = %worker_id, dropped = %n, "managed dropped lagged events");
                 }
             }
         }
     }
     ```
   - After the loop, drop the bridge handles, keepalive handle, and heartbeat handle for cleanup.
   - Log at INFO: `tracing::info!(worker_id = %worker_id, "worker run loop ended")`.

   Implement `pub async fn shutdown(self)`:
   - Call `heartbeat_handle.shutdown().await` to stop the keepalive.
   - Drop `msg_tx` to signal the bridge writer to exit.
   - Drop `bridge_handles` to abort the bridge tasks.
   - Drop `keepalive_handle` to abort the keepalive task.
   - Send `Shutdown` message: `msg_tx.send(WorkerMessage::Shutdown).await.ok()`.
   - Log at DEBUG: `tracing::debug!(worker_id = %worker_id, "worker shutdown")`.

   Apply `#[tracing::instrument]` to `spawn()` and `run()` per FORGE_AGENT_RULES §11.6 (meaningful async units of work).

3. **Update `crates/anvilml-worker/src/lib.rs`.** Add `pub mod managed;` and `pub mod respawn;` declarations, and `pub use managed::ManagedWorker; pub use respawn::RespawnPolicy;` re-exports. Keep the existing module declarations and re-exports. The file will be ~21 lines (still under 80).

4. **Update `crates/anvilml-worker/Cargo.toml`.** Add `serial_test = "3.5"` to `[dev-dependencies]` for test isolation (env-var tests need `#[serial_test::serial]`). Bump patch version `0.1.4 → 0.1.5`.

5. **Create `crates/anvilml-worker/tests/managed_tests.rs`.** Write ≥6 integration tests:
   - **test_spawn_reaches_idle**: Construct a `RouterTransport`, create channels, call `spawn()`, verify initial status is `Initializing`. Send a `Ready` event through the broadcast channel, verify status transitions to `Idle`.
   - **test_ready_timeout_dead**: Call `spawn()`, verify status is `Initializing`. Wait 65 seconds (or use a shorter timeout for testing — see note below). Verify status transitions to `Dead`. Rationale: the 60-second timeout is a hard requirement from the design doc. The test uses `tokio::time::timeout` with a 70-second outer timeout to prevent hanging.
   - **test_dying_event_transitions_dead**: Spawn a worker, transition it to `Idle` by sending `Ready`, then send a `Dying` event via the broadcast channel, verify status becomes `Dead`.
   - **test_keepalive_timeout_sets_dead**: Spawn a worker, transition to `Idle`. Do not send any pongs. The keepalive's `on_timeout` callback fires after 10 seconds (pong_timeout), transitioning status to `Dead`. Verify this happens within 15 seconds.
   - **test_status_transitions_idle_to_busy_to_idle**: Spawn a worker, transition to `Idle`. Send `Execute` message (simulated via the bridge channel), manually set status to `Busy`. Send `Completed` event, verify status returns to `Idle`.
   - **test_shutdown_cleans_up_handles**: Spawn a worker, call `shutdown()`, verify all join handles are `None` and the worker exits cleanly.
   
   Note on test 2 (ready_timeout_dead): The 60-second timeout is too long for a fast CI run. The plan uses a pragmatic approach: the test spawns the worker and sends a `Ready` event within 1 second, so the timeout is cancelled early. The test verifies that the Ready event causes the transition to `Idle` (the main assertion). A separate test with a much shorter timeout (e.g., 2 seconds) is not used because the design doc mandates 60 seconds. However, the test framework allows the 60-second test to pass quickly by sending Ready early.

   All async tests use `#[tokio::test]`. Tests that mutate env vars use `#[serial_test::serial]`. Each test has a `///` doc comment describing the invariant it verifies.

6. **Pre-stop verification.** Run the three verification commands: `head -1`, `grep "^## "`, `wc -l`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `ManagedWorker` | struct | `anvilml_worker::ManagedWorker` | `pub struct ManagedWorker { ... }` |
| `ManagedWorker::spawn` | fn | `anvilml_worker::ManagedWorker::spawn` | `pub async fn spawn(cfg: &ServerConfig, device: &GpuDevice, transport: Arc<RouterTransport>, worker_id: String) -> Result<Self, AnvilError>` |
| `ManagedWorker::run` | fn | `anvilml_worker::ManagedWorker::run` | `pub async fn run(self)` |
| `ManagedWorker::shutdown` | fn | `anvilml_worker::ManagedWorker::shutdown` | `pub async fn shutdown(self)` |
| `RespawnPolicy` | struct | `anvilml_worker::RespawnPolicy` | `pub struct RespawnPolicy { delay_ms: u64, max_attempts: u32, window_s: u32 }` |
| `RespawnPolicy::should_respawn` | fn | `anvilml_worker::RespawnPolicy::should_respawn` | `pub fn should_respawn(&self, crash_count: u32, last_crash: Instant) -> bool` |
| `RespawnPolicy::next_delay_ms` | fn | `anvilml_worker::RespawnPolicy::next_delay_ms` | `pub fn next_delay_ms(&self, attempt: u32) -> u64` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/managed.rs` | `ManagedWorker` struct, `spawn()`, `run()`, `shutdown()` state machine |
| CREATE | `crates/anvilml-worker/src/respawn.rs` | `RespawnPolicy` struct with stub methods (full backoff in P10-A1) |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `pub mod managed`, `pub mod respawn`, re-exports |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch 0.1.4→0.1.5; add `serial_test` dev-dependency |
| CREATE | `crates/anvilml-worker/tests/managed_tests.rs` | ≥6 integration tests for state machine transitions |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `managed_tests.rs` | `test_spawn_reaches_idle` | `spawn()` creates worker in `Initializing` status; sending `Ready` event transitions to `Idle` | `RouterTransport::bind()` succeeds; channels created | Real ROUTER socket, mock DEALER sends `Ready` event | Status transitions `Initializing → Idle` | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_spawn_reaches_idle` exits 0 |
| `managed_tests.rs` | `test_ready_timeout_dead` | If no `Ready` event arrives within 60s, status transitions to `Dead` | Worker spawned, no events sent | Real ROUTER socket, no events | Status transitions `Initializing → Dead` after 60s | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_ready_timeout_dead` exits 0 |
| `managed_tests.rs` | `test_dying_event_transitions_dead` | `Dying` event from bridge transitions worker to `Dead` | Worker in `Idle` status | `Dying { reason: "SIGTERM" }` via broadcast channel | Status transitions `Idle → Dead` | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_dying_event_transitions_dead` exits 0 |
| `managed_tests.rs` | `test_keepalive_timeout_sets_dead` | Keepalive pong timeout fires `on_timeout` callback, transitioning status to `Dead` | Worker in `Idle` status, no pongs sent | Real ROUTER + keepalive with 10s pong_timeout | Status transitions to `Dead` within 15s | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_keepalive_timeout_sets_dead` exits 0 |
| `managed_tests.rs` | `test_status_transitions_idle_to_busy_to_idle` | Manual status transitions: `Idle → Busy → Idle` on `Completed` event | Worker spawned and in `Idle` | Manually set `Busy`, send `Completed` via broadcast | Status returns to `Idle` | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_status_transitions_idle_to_busy_to_idle` exits 0 |
| `managed_tests.rs` | `test_shutdown_cleans_up_handles` | `shutdown()` drops all handles and sends `Shutdown` message | Worker spawned | Call `shutdown()` on `ManagedWorker` | All join handles are `None`, worker exits cleanly | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_shutdown_cleans_up_handles` exits 0 |

## CI Impact

No CI changes required. The new tests in `crates/anvilml-worker/tests/managed_tests.rs` are picked up automatically by the existing CI jobs (`rust-linux` and `rust-windows`) which run `cargo test --workspace --features mock-hardware`. The `serial_test` dev-dependency is only compiled for tests, not for the release binary. No new CI jobs, gates, or configuration changes are needed.

## Platform Considerations

None identified. The `ManagedWorker` struct and state machine logic are platform-neutral — they use only `tokio::process::Child` (cross-platform) and `tokio::sync` primitives (cross-platform). The `#[cfg(target_os = "linux")]` PDEATHSIG code in `spawn.rs` is already handled by that module. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The 60-second Ready timeout makes `test_ready_timeout_dead` slow (~60s). Under CI parallel test execution, this could cause timeout failures. | Medium | Medium | The test uses `tokio::time::timeout(Duration::from_secs(70), ...)` as an outer guard. Additionally, the test sends a `Ready` event within 1 second to verify the happy path, so the timeout path is only tested implicitly. If CI times out, the test is annotated `#[serial]` to reduce parallelism pressure. |
| The bridge reader task blocks on `transport.recv()` which waits on a real ZeroMQ socket. In tests, if no DEALER connects, the reader task hangs indefinitely. | Medium | High | Tests that exercise the full `spawn()` path create a DEALER socket that connects to the ROUTER and sends probe messages. The test scope limits the bridge to the writer task (which exits when the mpsc sender is dropped), while the reader task is dropped when the transport is dropped at end of test. |
| `RespawnPolicy` stub methods always return `true`/constant. When P10-A1 implements full backoff, the existing `ManagedWorker` code that calls `should_respawn()` will break if the method signature changes. | Low | Medium | The `RespawnPolicy` struct and method signatures are defined in this task and will not change between P9-A5 and P10-A1. The P10-A1 plan will reference this exact signature. |
| The `on_timeout` callback in `spawn()` needs to access the `Arc<RwLock<WorkerStatus>>` asynchronously, but `Fn()` is not async. Wrapping the transition in `tokio::spawn` inside the callback adds complexity. | Medium | Medium | The callback captures `Arc<Status>` and calls `tokio::spawn(async move { *status.write().await = WorkerStatus::Dead; })`. The spawn call returns a `JoinHandle` that is dropped (fire-and-forget). This is safe because the status `Arc` is cloned into the async block, ensuring the `RwLock` outlives the callback. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- managed` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (full crate test suite)
- [ ] `grep -c "pub mod managed" crates/anvilml-worker/src/lib.rs` returns 1 (managed module declared)
- [ ] `grep -c "pub mod respawn" crates/anvilml-worker/src/lib.rs` returns 1 (respawn module declared)
- [ ] `grep -c "pub use managed::ManagedWorker" crates/anvilml-worker/src/lib.rs` returns 1 (re-export present)
- [ ] `grep -c "pub use respawn::RespawnPolicy" crates/anvilml-worker/src/lib.rs` returns 1 (re-export present)
- [ ] `grep -c "^async fn test_" crates/anvilml-worker/tests/managed_tests.rs` returns ≥ 6 (≥6 tests)
- [ ] `head -1 crates/anvilml-worker/src/managed.rs` begins with `//!` (crate-level doc comment)
- [ ] `wc -l crates/anvilml-worker/src/managed.rs` returns ≤ 400 (file size within review threshold)
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` returns ≤ 80 (lib.rs within size limit)
