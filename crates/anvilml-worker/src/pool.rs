//! `WorkerPool`: owns every `ManagedWorker` in the process, the shared
//! `RouterTransport`, and the pool-wide IPC bridge (P8-F1).
//!
//! Per `ANVILML_DESIGN.md §9.2`: "`WorkerPool` owns `Vec<WorkerHandle>` and
//! the shared `Arc<RouterTransport>`."
//!
//! # Construction shape
//!
//! This task's own spec states `spawn_all(&mut self, devices, cfg)` — a
//! method on an already-existing `&mut self` — while also describing
//! `spawn_all()` itself as the place `bridge::spawn_bridge()` gets called
//! "once for the pool." Those two things don't fully reconcile: something
//! has to construct `self` (with its non-`Option` `transport` and
//! `bridge_handles` fields) *before* `spawn_all()` can ever run on it.
//!
//! Resolved as: `WorkerPool::new()` binds a fresh `RouterTransport` and
//! spawns the bridge immediately — neither depends on `devices` or `cfg`,
//! both are pool-wide resources that exist independently of which workers
//! end up in the pool. `spawn_all()` then only handles the per-device
//! worker-spawning loop, reusing what `new()` already set up. This keeps
//! the struct's fields exactly as specified (no `Option` wrapping needed,
//! since `new()` always fully populates them) and is still consistent with
//! "the pool's bridge exists by the time `spawn_all()` has run," just not
//! literally inside `spawn_all()`'s own function body.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::types::worker::WorkerStatus;
use anvilml_core::{AnvilError, GpuDevice, ServerConfig};
use anvilml_ipc::{RouterTransport, WorkerMessage};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::bridge::spawn_bridge;
use crate::demux::Demux;
use crate::env::WorkerEnv;
use crate::managed::{
    DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT, DEFAULT_INIT_TIMEOUT, DEFAULT_WATCHDOG_PING_INTERVAL,
    DEFAULT_WATCHDOG_PONG_TIMEOUT, ManagedWorker, ManagedWorkerConfig, WorkerHandle,
};
use crate::respawn::RespawnPolicy;
use crate::spawn::{ProcessWorkerSpawner, WorkerSpawner};

/// Owns every `ManagedWorker` in the process, the shared `RouterTransport`
/// all of them communicate over, and the pool-wide IPC bridge (P8-F1) that
/// routes each worker's events to its own, individually-registered demux
/// channel — see `bridge.rs`'s own doc comment for why one shared bridge,
/// not one per worker.
pub struct WorkerPool {
    handles: Vec<WorkerHandle>,
    transport: Arc<RouterTransport>,
    demux: Arc<Demux>,
    /// Sender half of the bridge's writer channel. See `bridge_sender()`'s
    /// own doc comment for the full explanation of its current (unused)
    /// status and why it's kept regardless.
    bridge_tx: mpsc::Sender<(String, WorkerMessage)>,
    /// (writer task, reader task) — see `bridge::spawn_bridge()`'s own
    /// doc comment for what each does.
    bridge_handles: (JoinHandle<()>, JoinHandle<()>),
}

impl WorkerPool {
    /// Construct an empty pool: binds a fresh `RouterTransport`, creates a
    /// fresh `Demux`, and spawns the pool-wide bridge (P8-F1) against
    /// them. No workers exist yet — call `spawn_all()` to populate the
    /// pool from a device list.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Ipc` if `RouterTransport::bind()` fails (e.g.
    /// no available ports).
    pub async fn new() -> Result<Self, AnvilError> {
        let transport = Arc::new(
            RouterTransport::bind()
                .await
                .map_err(|e| AnvilError::Ipc(e.to_string()))?,
        );
        let demux = Arc::new(Demux::new());
        let (bridge_tx, writer_handle, reader_handle) =
            spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

        Ok(Self {
            handles: Vec::new(),
            transport,
            demux,
            bridge_tx,
            bridge_handles: (writer_handle, reader_handle),
        })
    }

    /// The pool's currently-tracked worker handles.
    ///
    /// Empty immediately after `new()`; populated by `spawn_all()`, one
    /// handle per device.
    pub fn handles(&self) -> &[WorkerHandle] {
        &self.handles
    }

    /// The pool-wide `RouterTransport` every worker in this pool shares.
    pub fn transport(&self) -> &Arc<RouterTransport> {
        &self.transport
    }

    /// The pool-wide IPC bridge's writer channel — queuing a `(worker_id,
    /// message)` pair here sends it via `RouterTransport::send()`, the
    /// same way `bridge.rs`'s own writer task always has (see
    /// `bridge::spawn_bridge()`'s own doc comment).
    ///
    /// Not consumed by anything in `anvilml-worker` itself as of this
    /// task — every current outgoing message
    /// (`graceful_shutdown_child()`'s `WorkerMessage::Shutdown`,
    /// `KeepaliveWatchdog`'s Pings) goes through a `ManagedWorker`'s own
    /// direct `transport.send()` call instead, not through this pool-wide
    /// channel. Exposed regardless, matching this task's own description
    /// of `spawn_all()` "keeping its writer" — a future caller with a
    /// genuine pool-wide send need (e.g. broadcasting to every worker at
    /// once) can use this rather than re-deriving a sender from
    /// `transport()`/re-spawning a second bridge.
    pub fn bridge_sender(&self) -> &mpsc::Sender<(String, WorkerMessage)> {
        &self.bridge_tx
    }

    /// Spawn one `ManagedWorker` per device, registering a `WorkerHandle`
    /// for each into this pool. Thin wrapper around `spawn_all_impl()`
    /// using the real `ProcessWorkerSpawner` — see that method's own doc
    /// comment for the full behavior. Calls `spawn_all_impl()` directly
    /// rather than `spawn_all_with_spawner()`: the latter is
    /// `test-utils`-gated, so a plain, ungated build (e.g. `cargo clippy
    /// --workspace` without that feature) would fail to compile this
    /// method at all if it called through the gated wrapper.
    pub async fn spawn_all(
        &mut self,
        devices: &[GpuDevice],
        cfg: &ServerConfig,
    ) -> Result<(), AnvilError> {
        self.spawn_all_impl(devices, cfg, Arc::new(ProcessWorkerSpawner))
            .await
    }

    /// Spawn one `ManagedWorker` per device, using the given `spawner`
    /// rather than always constructing a `ProcessWorkerSpawner`.
    ///
    /// `spawn_all()` itself always passes a real `ProcessWorkerSpawner` —
    /// this is a `test-utils`-gated variant, not a general-purpose public
    /// API, needed because `ProcessWorkerSpawner` launches a real Python
    /// interpreter from a real virtualenv, which no test environment has.
    /// Matches the same test-only-injectable-dependency pattern already
    /// established in `managed.rs` (`run_once_for_test()`,
    /// `child_pid_for_test()`, `take_child_for_test()`) for the identical
    /// reason: `#[cfg(test)]` doesn't work for code called from
    /// integration tests in `tests/`, since those compile as separate
    /// crates.
    ///
    /// `ProcessWorkerSpawner` is stateless (a unit struct), so production's
    /// own `spawn_all()` shares one `Arc` across every device rather than
    /// constructing a fresh instance per worker — matching what this
    /// method does for whatever `spawner` a caller (test or otherwise)
    /// provides.
    ///
    /// `log_level` is read from `ANVILML_LOG`, falling back to `RUST_LOG`,
    /// falling back to `"info"` — matching `ENVIRONMENT.md`'s own
    /// documented precedence for the supervisor's own log level. Neither
    /// env var is a `ServerConfig` field, so this can't come from `cfg`
    /// directly; reading the same variables the supervisor itself would
    /// use means every worker logs at the same level as the process
    /// supervising it, which is the reasonable default absent any
    /// per-worker override mechanism.
    ///
    /// `mock` (whether to inject `ANVILML_WORKER_MOCK`, per
    /// `WorkerEnv::build()`'s own doc comment) is derived from the
    /// `mock-hardware` cargo feature at compile time via `cfg!(...)`, not
    /// read from `cfg: &ServerConfig` — matching `env.rs`'s own doc
    /// comment describing this exact parameter.
    ///
    /// Each worker gets `RespawnPolicy::default()` (`ANVILML_DESIGN.md
    /// §19.4`'s documented defaults: 2s delay, 5 max attempts, 5-minute
    /// window) and the production `DEFAULT_*` timeout constants — this
    /// task's own spec doesn't mention per-device timeout overrides, and
    /// none of the config types checked (`ServerConfig`, `GpuDevice`)
    /// carry any.
    #[cfg(feature = "test-utils")]
    pub async fn spawn_all_with_spawner(
        &mut self,
        devices: &[GpuDevice],
        cfg: &ServerConfig,
        spawner: Arc<dyn WorkerSpawner>,
    ) -> Result<(), AnvilError> {
        self.spawn_all_impl(devices, cfg, spawner).await
    }

    /// Shared implementation for `spawn_all()` and
    /// `spawn_all_with_spawner()` — see either's own doc comment for the
    /// full behavior. Private: not `test-utils`-gated, since it isn't
    /// itself part of any public contract, only reached through the two
    /// gated/ungated public wrappers above.
    async fn spawn_all_impl(
        &mut self,
        devices: &[GpuDevice],
        cfg: &ServerConfig,
        spawner: Arc<dyn WorkerSpawner>,
    ) -> Result<(), AnvilError> {
        let log_level = std::env::var("ANVILML_LOG")
            .or_else(|_| std::env::var("RUST_LOG"))
            .unwrap_or_else(|_| "info".to_string());
        let mock = cfg!(feature = "mock-hardware");

        for device in devices {
            let worker_id = device.index.to_string();

            let env = WorkerEnv::build(
                self.transport.port,
                &worker_id,
                device.index,
                device.device_type,
                mock,
                &log_level,
                cfg.max_ipc_payload_mib,
            );

            let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
            let (pong_tx, _pong_rx) = mpsc::channel(16);
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            let (force_shutdown_tx, force_shutdown_rx) = oneshot::channel();

            let worker = ManagedWorker::new(ManagedWorkerConfig {
                worker_id: worker_id.clone(),
                transport: Arc::clone(&self.transport),
                demux: Arc::clone(&self.demux),
                status: Arc::clone(&status),
                respawn_policy: RespawnPolicy::default(),
                init_timeout: DEFAULT_INIT_TIMEOUT,
                pong_tx,
                watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
                watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
                graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
                venv_path: cfg.venv_path.clone(),
                env,
                spawner: Arc::clone(&spawner),
            });

            let join_handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));
            let join_handle = Arc::new(tokio::sync::Mutex::new(Some(join_handle)));
            let handle = WorkerHandle::new(
                worker_id,
                status,
                Some(shutdown_tx),
                Some(force_shutdown_tx),
                join_handle,
            );
            self.handles.push(handle);
        }

        Ok(())
    }

    /// Gracefully shut down every worker in the pool, per
    /// `ANVILML_DESIGN.md §19.3` steps 2-4.
    ///
    /// 1. `request_shutdown()` on every handle — each worker's own
    ///    `ManagedWorker::run()` then runs its own graceful-then-force-kill
    ///    sequence internally (`graceful_shutdown_child()`, added in the
    ///    orphan-cleanup work preceding this task).
    /// 2. Await every handle's `run()` task, bounded by **one shared**
    ///    `timeout` for the whole pool — not `timeout` per worker,
    ///    sequentially, which would let total wall-clock shutdown time
    ///    scale with worker count instead of staying bounded at `timeout`
    ///    overall. Matches §19.3's own framing: "server waits up to 30
    ///    seconds for workers \[plural\] to exit" describes one shared
    ///    window, not one per worker. Each handle is cloned into its own
    ///    spawned task so all of them run concurrently — cloning is fine
    ///    for *this* phase specifically, since `await_exit()` only needs
    ///    `&self`.
    /// 3. Any handle that didn't exit within `timeout` is a straggler.
    ///    Stragglers are force-shut-down via `force_shutdown()` — **on
    ///    their original handle**, not a clone (`oneshot::Sender` isn't
    ///    `Clone`, matching `shutdown_tx`'s own, already-established
    ///    pattern — see `WorkerHandle`'s `Clone` impl) — then given a
    ///    short, bounded grace period for the force-kill itself to
    ///    resolve. `force_kill_child()` (what actually runs once
    ///    `force_shutdown()`'s signal reaches
    ///    `graceful_shutdown_child()`) is fast — a direct `.kill().await`,
    ///    no internal bounded wait of its own — so this grace period
    ///    exists only to avoid hanging forever if something is more
    ///    deeply wrong, not because force-killing is expected to be slow.
    ///    Only if even *that* bound elapses does this fall back to
    ///    `abort()` — a true last resort at that point, since something
    ///    has gone wrong beyond what this method can reasonably account
    ///    for. See `WorkerHandle::abort()`'s own doc comment for why
    ///    externally cancelling the task is not, on its own, an adequate
    ///    substitute for the `force_shutdown()` path above.
    /// 4. The bridge's own reader and writer tasks are aborted last, after
    ///    every worker's `run()` task has exited or been aborted — nothing
    ///    should still need `transport.send()` (`KeepaliveWatchdog`) or the
    ///    bridge's own event routing by that point.
    pub async fn shutdown_all(&mut self, timeout: Duration) {
        for handle in &mut self.handles {
            handle.request_shutdown();
        }

        // Phase 1: await every handle concurrently, bounded by one shared
        // timeout. Report stragglers back by index rather than acting on
        // them directly inside each spawned task — force_shutdown() needs
        // the ORIGINAL handle's own force_shutdown_tx, which a clone
        // doesn't have.
        let (report_tx, mut report_rx) = mpsc::channel(self.handles.len().max(1));
        for (index, handle) in self.handles.iter().cloned().enumerate() {
            let report_tx = report_tx.clone();
            tokio::spawn(async move {
                if !handle.await_exit(timeout).await {
                    let _ = report_tx.send(index).await;
                }
            });
        }
        drop(report_tx);
        let mut stragglers = Vec::new();
        while let Some(index) = report_rx.recv().await {
            stragglers.push(index);
        }

        // Phase 2: force-shut-down every straggler via its ORIGINAL
        // handle. Sending the signal itself is fast (no .await inside
        // force_shutdown() at all), so doing this sequentially across
        // stragglers is fine — the actual waiting happens concurrently
        // next.
        for &index in &stragglers {
            let handle = &mut self.handles[index];
            tracing::warn!(
                worker_id = %handle.worker_id,
                "worker did not exit within shutdown_all()'s timeout — forcing"
            );
            handle.force_shutdown();
        }

        // Phase 3: await every straggler's force-kill concurrently,
        // bounded by one shared grace period — same reasoning as phase 1
        // for why this must be concurrent, not sequential.
        let mut force_tasks = Vec::with_capacity(stragglers.len());
        for &index in &stragglers {
            let handle = self.handles[index].clone();
            force_tasks.push(tokio::spawn(async move {
                if !handle.await_exit(Duration::from_secs(5)).await {
                    tracing::error!(
                        worker_id = %handle.worker_id,
                        "worker still did not exit after force_shutdown() — \
                         aborting task as a last resort (may leak its child process)"
                    );
                    handle.abort().await;
                }
            }));
        }
        for task in force_tasks {
            let _ = task.await;
        }

        self.bridge_handles.0.abort();
        self.bridge_handles.1.abort();
    }
}
