//! WorkerPool — manages a collection of running `ManagedWorker` event loops.
//!
//! `WorkerPool` owns the shared `RouterTransport`, the shared
//! `EventBroadcaster`, and the single demux task (`crate::demux`) that reads
//! every worker's events off that transport. It provides methods to spawn
//! workers for all detected devices, retrieve worker info snapshots, broadcast
//! status changes, and request worker restarts.
//!
//! `WorkerPool` does not hold `ManagedWorker` values directly. `ManagedWorker::run()`
//! consumes the worker for the worker's entire lifetime, so the pool instead
//! holds, per worker, exactly what it needs after spawning: a clone of the
//! status `Arc` (for status snapshots and the polling monitor below), the
//! `oneshot::Sender` half of the worker's shutdown signal (to request a
//! graceful stop), the `watch::Sender` half of the worker's restart-generation
//! counter (to request an unconditional restart — see `restart_worker()`), and
//! the `run()` task's `JoinHandle` (to await that stop actually completing).
//! See `WorkerHandle`.
//!
//! A background monitoring task polls each worker's status at a 100ms
//! interval and broadcasts `WorkerStatusChanged` events when the status
//! changes.
//!
//! **Hard constraints:** Contain only pool management and status
//! broadcasting. No job dispatch logic — that belongs in the scheduler.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{
    AnvilError, GpuDevice, NodeTypeRegistry, ServerConfig, WorkerInfo, WorkerStatus,
};
use anvilml_ipc::{EventBroadcaster, RouterTransport};
use tracing;

use crate::demux::{self, RouteTable};
use crate::managed::ManagedWorker;

/// Outer bound on how long `shutdown_all()` waits for a single worker's
/// `run()` task to finish its shutdown sequence, on top of that sequence's
/// own internal timeouts (2s bridge writer + 5s child process, ≈7s worst
/// case). See `shutdown_all()`'s doc comment for why this exists as a
/// defensive measure rather than the primary bound.
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// Everything the pool needs to retain about one running worker after
/// `ManagedWorker::run()` has taken ownership of the worker itself.
///
/// `ManagedWorker` does not expose `worker_id` or `device_name` publicly,
/// so the pool stores these values here too, alongside the handles it
/// actually needs to observe and stop the worker.
struct WorkerHandle {
    /// A clone of the worker's status `Arc`, for status snapshots
    /// (`get_worker_infos()`) and the polling monitor task — neither needs
    /// the `ManagedWorker` itself, only this.
    status: Arc<tokio::sync::RwLock<WorkerStatus>>,

    /// The sending half of this worker's graceful-shutdown signal. Firing it
    /// (via `.send(())`) wakes the dedicated shutdown arm in `run()`'s
    /// `select!` loop, which performs a full teardown sequence (bridge
    /// Shutdown message, subprocess kill, demux deregistration) and does
    /// NOT attempt a respawn afterward. `Option`-wrapped because
    /// `oneshot::Sender::send` consumes `self` — `shutdown_all()` needs to
    /// take ownership of this out of the handle to fire it.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// The sending half of this worker's restart-generation counter. Sending
    /// any new value (e.g. incrementing a generation counter) wakes
    /// `run()`'s restart arm, which unconditionally force-kills the
    /// subprocess and respawns it — bypassing `RespawnPolicy` entirely.
    ///
    /// Backed by `tokio::sync::watch` (not `oneshot`) so the same sender
    /// remains valid across any number of respawns: `ManagedWorker::run()`
    /// clones its `watch::Receiver` into each respawned worker rather than
    /// constructing a new pair, meaning WorkerPool's sender here is the
    /// single-source-of-truth restart signal for the worker's entire
    /// supervised lifetime.
    restart_tx: tokio::sync::watch::Sender<u64>,

    /// The `JoinHandle` for the task running this worker's `run()` loop.
    /// Awaited (with a bound) in `shutdown_all()` to confirm the worker's
    /// full shutdown sequence — not just the signal send — has completed.
    run_handle: tokio::task::JoinHandle<()>,

    /// The worker's stable display identity (e.g. `"worker-0"`).
    worker_id: String,

    /// The GPU device name, captured at spawn time. See `get_worker_infos()`
    /// for why this is a static label rather than read back from the worker.
    device_name: String,
}

/// A pool of managed workers, one per GPU device.
///
/// # Construction
///
/// Use `spawn_all()` to create a pool from a `ServerConfig` and a list of
/// `GpuDevice` values. Each device gets its own worker with a generated
/// identity (`"worker-0"`, `"worker-1"`, etc.).
pub struct WorkerPool {
    /// Wrapped in a `tokio::sync::Mutex` (rather than a bare `Vec`) so that
    /// `shutdown_all(&self)` and `restart_worker(&self)` can access the vec
    /// through a shared reference — `WorkerPool` is held behind an `Arc`
    /// shared with `AppState` and the system stats tick task, so no method
    /// on this struct can ever take `self` by value.
    workers: tokio::sync::Mutex<Vec<WorkerHandle>>,

    /// Stored for future dispatch routing logic (belongs in the scheduler).
    #[allow(dead_code)] // reserved for future dispatch routing
    transport: Arc<RouterTransport>,

    /// The shared event broadcaster used to notify WebSocket clients
    /// about worker status changes and other system events.
    broadcaster: Arc<EventBroadcaster>,

    /// The pool-wide demux task's handle (see `crate::demux`), guarded by a
    /// `Mutex` for the same reason `workers` is: `shutdown_all` only has
    /// `&self` and must still be able to take ownership of the handle to
    /// abort it. `None` only in pools built via `new()` for tests that
    /// never start a demux task at all.
    demux_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl WorkerPool {
    /// Spawn a managed worker for each device, start each one's `run()`
    /// loop as a background task, and return a `WorkerPool`.
    ///
    /// # Arguments
    ///
    /// * `cfg` — The server configuration (provides venv path and IPC payload cap).
    /// * `devices` — The list of GPU devices to spawn workers for.
    /// * `transport` — The shared `RouterTransport` for IPC communication.
    /// * `broadcaster` — The shared `EventBroadcaster` for WebSocket events.
    /// * `node_registry` — The node type registry. Each worker's `Ready`
    ///   event node types are forwarded into this registry, via
    ///   `ManagedWorker::spawn()`, so the scheduler can learn which node
    ///   types are available at runtime. `WorkerPool` does not retain this
    ///   `Arc` itself — only `AppState` (see `anvilml-server`) holds a
    ///   long-lived reference for `GET /v1/nodes` — `spawn_all` exists
    ///   purely to thread the same `Arc` the caller already owns into
    ///   every worker it spawns.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if any worker fails to spawn.
    ///
    /// # Background monitoring
    ///
    /// After spawning each worker, a background task is launched that polls
    /// the worker's status every 100ms. When a status change is detected,
    /// a `WsEvent::WorkerStatusChanged` event is broadcast to all connected
    /// WebSocket clients.
    ///
    /// # Demux startup ordering
    ///
    /// The demux task is started once, before any device is spawned, with
    /// an empty routing table — not after the loop, with a fully-built one.
    /// `ManagedWorker::spawn()` starts a worker's keepalive (and so its
    /// first ping) before returning, so building the table only after every
    /// device has spawned would leave every worker's first ping/pong racing
    /// the demux task's own startup. `ManagedWorker::spawn()` registers its
    /// own route against the already-running demux task before it returns
    /// — see that method's doc comment for why registration moved there.
    ///
    /// # Run-loop startup
    ///
    /// Each worker's `run()` is spawned as its own task immediately after
    /// `ManagedWorker::spawn()` returns, in the same loop iteration — this
    /// is the fix for `run()` never being invoked at all in earlier
    /// revisions of this method, which left every worker stuck in
    /// `Initializing` forever since nothing was driving its event loop.
    #[tracing::instrument(skip(cfg, devices, transport, broadcaster, node_registry), fields(worker_count = %devices.len()))]
    pub async fn spawn_all(
        cfg: &ServerConfig,
        devices: &[GpuDevice],
        transport: Arc<RouterTransport>,
        broadcaster: Arc<EventBroadcaster>,
        node_registry: Arc<NodeTypeRegistry>,
    ) -> Result<Self, AnvilError> {
        let mut workers = Vec::with_capacity(devices.len());

        // Empty at this point — see "Demux startup ordering" above for why
        // the task must exist before any worker does, not after all of them.
        let routes: RouteTable =
            Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
        let demux_handle = demux::start(transport.clone(), routes.clone());

        for (i, device) in devices.iter().enumerate() {
            // Matches the naming convention the scheduler and worker
            // subprocess environment variables also use.
            let worker_id = format!("worker-{i}");

            // Build the restart watch channel once per worker. The sender
            // stays here in WorkerHandle; the receiver is passed into
            // spawn() and cloned through every subsequent respawn — see
            // ManagedWorker's restart_rx field doc for why watch persists
            // across respawns without needing replacement on our end.
            let (restart_tx, restart_rx) = tokio::sync::watch::channel(0u64);

            let worker = ManagedWorker::spawn(
                cfg,
                device,
                transport.clone(),
                worker_id.clone(),
                routes.clone(),
                restart_rx,
                Arc::clone(&node_registry),
            )
            .await?;

            // GpuDevice's name is captured here rather than read back from
            // ManagedWorker, since device_name is private and may anyway be
            // overwritten by the Ready event later — the pool only needs a
            // stable label for WorkerInfo snapshots, not the live value.
            let device_name = device.name.clone();

            let status = worker.get_status();
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

            // run() is spawned immediately — this is the fix for the
            // run/shutdown conflict: the pool never holds an owned
            // ManagedWorker past this point, only the handles below, so
            // there is nothing left for a later method to try to reclaim
            // by value.
            let run_handle = tokio::spawn(worker.run(shutdown_rx));

            workers.push(WorkerHandle {
                status,
                shutdown_tx: Some(shutdown_tx),
                restart_tx,
                run_handle,
                worker_id,
                device_name,
            });
        }

        // One task per worker, polling rather than event-driven: the
        // alternative (subscribing to each worker's own event_tx) would
        // need its own demux-style fan-out, for a status display that
        // tolerates being up to 100ms stale.
        for handle in &workers {
            let broadcaster = Arc::clone(&broadcaster);
            let worker_id = handle.worker_id.clone();
            let device_index = devices
                .iter()
                .position(|d| d.name == handle.device_name)
                .unwrap_or(0) as u32;
            let status = Arc::clone(&handle.status);

            // Seeded with the worker's actual starting status so the first
            // poll iteration doesn't broadcast a spurious "change" from some
            // arbitrary default into Initializing.
            let initial_status = *status.read().await;
            let mut previous_status = initial_status;

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let current_status = *status.read().await;

                    if current_status != previous_status {
                        tracing::debug!(
                            worker_id = %worker_id,
                            old_status = ?previous_status,
                            new_status = ?current_status,
                            "worker status changed, broadcasting"
                        );

                        broadcaster.send(anvilml_core::types::WsEvent::WorkerStatusChanged {
                            worker_id: worker_id.clone(),
                            status: current_status,
                            device_index,
                        });

                        previous_status = current_status;
                    }
                }
            });
        }

        // Mandatory INFO log point per ENVIRONMENT.md §9; worker_count is
        // indexed by log aggregators.
        tracing::info!(
            worker_count = %devices.len(),
            "worker pool spawned"
        );

        Ok(Self {
            workers: tokio::sync::Mutex::new(workers),
            transport,
            broadcaster,
            demux_handle: tokio::sync::Mutex::new(Some(demux_handle)),
        })
    }

    /// Create a pool from pre-built status handles (for testing), with no
    /// demux task and no background monitoring — tests that need either
    /// can start them manually against the same transport.
    ///
    /// Unlike `spawn_all()`, this does not spawn any `run()` task — there
    /// is no real `ManagedWorker` behind these entries, only a status
    /// `Arc` a test can write to directly to simulate state transitions.
    /// Each entry therefore gets an inert `shutdown_tx`/`run_handle` pair:
    /// a oneshot whose receiver is immediately dropped (so firing it later
    /// is a harmless no-op) and a task that exits immediately. Tests that
    /// need `shutdown_all()` to do real work against a real `run()` loop
    /// should drive that through `ManagedWorker::spawn` and `tokio::spawn`
    /// directly rather than through this constructor.
    ///
    /// # Arguments
    ///
    /// * `workers` — Pre-built `(status, worker_id, device_name)` triples.
    /// * `transport` — The shared `RouterTransport` for IPC communication.
    /// * `broadcaster` — The shared `EventBroadcaster` for WebSocket events.
    pub fn new(
        workers: Vec<(Arc<tokio::sync::RwLock<WorkerStatus>>, String, String)>,
        transport: Arc<RouterTransport>,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Self {
        let workers = workers
            .into_iter()
            .map(|(status, worker_id, device_name)| {
                let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
                let (restart_tx, _restart_rx) = tokio::sync::watch::channel(0u64);
                let run_handle = tokio::spawn(async {});
                WorkerHandle {
                    status,
                    shutdown_tx: Some(shutdown_tx),
                    restart_tx,
                    run_handle,
                    worker_id,
                    device_name,
                }
            })
            .collect();

        Self {
            workers: tokio::sync::Mutex::new(workers),
            transport,
            broadcaster,
            demux_handle: tokio::sync::Mutex::new(None),
        }
    }

    /// Retrieve a snapshot of all worker info.
    ///
    /// # Fields
    ///
    /// - `current_job_id`: Always `None` — the pool does not track job assignments
    ///   (that belongs in the scheduler).
    /// - `vram_used_mib`: Always `None` — VRAM usage is reported by the worker
    ///   via the `MemoryReport` event, which the pool does not currently track.
    pub async fn get_worker_infos(&self) -> Vec<WorkerInfo> {
        // Held only for this method's duration; shutdown_all is the only
        // other place touching self.workers, and shutdown happens once,
        // after the server has stopped accepting requests, so the two
        // are not expected to contend in practice.
        let workers = self.workers.lock().await;
        let mut infos = Vec::with_capacity(workers.len());

        for handle in workers.iter() {
            let status = *handle.status.read().await;

            // Reconstructed by name match rather than stored directly,
            // since WorkerHandle doesn't carry device_index and adding it
            // would duplicate what's already recoverable from `devices`
            // at spawn time.
            let device_index = workers
                .iter()
                .position(|h| h.device_name == handle.device_name)
                .unwrap_or(0) as u32;

            infos.push(WorkerInfo {
                id: handle.worker_id.clone(),
                device_index,
                device_name: handle.device_name.clone(),
                status,
                current_job_id: None,
                vram_used_mib: None,
            });
        }

        infos
    }

    /// Return a reference to the shared event broadcaster, so callers like
    /// the system stats tick can send their own events on the same bus.
    pub fn broadcaster(&self) -> &Arc<EventBroadcaster> {
        &self.broadcaster
    }

    /// Request an unconditional restart of the worker identified by
    /// `worker_id`.
    ///
    /// Signals `run()`'s restart arm by incrementing the restart-generation
    /// counter on the worker's `watch` channel. The arm force-kills the
    /// subprocess (if still alive), sets `Dead`, transitions to `Respawning`,
    /// then calls `ManagedWorker::spawn()` to bring up a fresh worker — all
    /// without consulting `RespawnPolicy`. Valid from any current state,
    /// including an already-`Dead` worker whose automatic respawn attempts
    /// were exhausted.
    ///
    /// Returns immediately after signalling — it does not wait for the
    /// respawn to complete. The caller can poll `GET /v1/workers` or watch
    /// for `WorkerStatusChanged` WebSocket events to observe the transition
    /// through `Dead` → `Respawning` → `Initializing` → `Idle`.
    ///
    /// # Errors
    ///
    /// * `AnvilError::WorkerNotFound(worker_id)` — no worker with the
    ///   given id is registered in this pool.
    /// * `AnvilError::Internal(...)` — the worker's `run()` task has
    ///   already exited (its `restart_rx` was dropped), so the signal
    ///   could not be delivered. This is expected if the worker crashed
    ///   unrecoverably and its loop exited before this call arrived.
    pub async fn restart_worker(&self, worker_id: &str) -> Result<(), AnvilError> {
        let workers = self.workers.lock().await;

        let handle = workers
            .iter()
            .find(|h| h.worker_id == worker_id)
            .ok_or_else(|| AnvilError::WorkerNotFound(worker_id.to_string()))?;

        // Increment the generation counter. watch::Sender::send_modify
        // allows an in-place mutation without needing to borrow the old
        // value separately, and always succeeds even if there are no
        // receivers (which would mean run() already exited — detected by
        // is_closed() below).
        handle.restart_tx.send_modify(|gen| *gen += 1);

        // A closed watch channel means all receivers (i.e. run()'s
        // restart_rx clone inside the worker's select loop) have been
        // dropped — the worker task has already exited. The signal was
        // sent but will never be received. Surface this as an error so
        // the caller (the HTTP handler) can return a meaningful response
        // rather than silently 202-ing a no-op.
        if handle.restart_tx.is_closed() {
            return Err(AnvilError::Internal(format!(
                "worker {} run() task has already exited; restart signal was not delivered",
                worker_id
            )));
        }

        tracing::info!(
            worker_id = %worker_id,
            "restart signal sent to worker"
        );

        Ok(())
    }

    /// Shut down every worker in the pool, then the demux task.
    ///
    /// This is the fix for workers surviving a supervisor Ctrl+C: previously
    /// nothing in `main.rs` ever drove a worker's `run()` loop at all (see
    /// `spawn_all`'s doc comment), so the Python subprocesses were simply
    /// abandoned when the process exited. `with_graceful_shutdown` only
    /// drains in-flight HTTP connections — it has no knowledge of worker
    /// subprocesses.
    ///
    /// Takes `&self` because `WorkerPool` lives behind an `Arc` shared with
    /// `AppState` and the system stats tick task, so it can never be moved
    /// out at the call site. Unlike the previous `Arc<ManagedWorker>` +
    /// `Arc::try_unwrap` design, this no longer needs exclusive ownership of
    /// anything: each `WorkerHandle`'s `shutdown_tx` is fired to request a
    /// graceful stop, and the corresponding `run_handle` is awaited to
    /// confirm that `run()`'s full shutdown sequence (see that method's doc
    /// comment) actually completed — not just that the signal was sent.
    ///
    /// Workers are shut down sequentially rather than concurrently. This
    /// pool is sized to one worker per GPU (typically 1–8), so sequential
    /// shutdown completes well within the per-worker grace period used by
    /// `run()`'s shutdown sequence and keeps the shutdown log easy to read.
    ///
    /// The demux task is stopped last, after every worker, rather than
    /// first — each worker's own shutdown sequence still needs *something*
    /// notionally listening on its event channel right up until that
    /// worker's writer task delivers its `Shutdown` message, even though in
    /// practice nothing further arrives once a worker stops responding.
    /// Stopping demux first would not break shutdown, but would leave a
    /// brief window where the routing table still exists with no consumer
    /// reading it — stopping it last avoids reasoning about that window at all.
    ///
    /// # Per-worker timeout
    ///
    /// `run()`'s own shutdown arm already bounds its internal steps (a 2s
    /// cap on the bridge writer, a 5s cap on the child process), so
    /// `run_handle` is expected to resolve within roughly 7s of the signal
    /// being sent. The `WORKER_SHUTDOWN_TIMEOUT` await here is a defensive
    /// outer bound on top of that, not a replacement for it — covering the
    /// case where a worker is wedged somewhere `run()`'s own timeouts don't
    /// reach (e.g. the keepalive shutdown handshake, or the task simply
    /// never being scheduled). On expiry, `run_handle.abort()` is called
    /// and a WARN is logged; the loop still proceeds to the next worker
    /// rather than stalling the whole shutdown sequence on one
    /// uncooperative worker. Aborting does not guarantee the Python
    /// subprocess itself is reaped — only that this task stops waiting on
    /// it — which is why the warning explicitly calls that out.
    ///
    /// # Returns
    ///
    /// Nothing. Failures to cleanly shut down an individual worker are
    /// logged at WARN rather than surfaced as an error — shutdown must
    /// proceed for all workers even if one is uncooperative.
    #[tracing::instrument(skip(self))]
    pub async fn shutdown_all(&self) {
        // shutdown_all is expected to run exactly once, at process exit,
        // so leaving the pool empty afterward is correct, not a leak.
        let drained = std::mem::take(&mut *self.workers.lock().await);

        for mut handle in drained {
            let worker_id = handle.worker_id.clone();

            // take() rather than a bare field access: oneshot::Sender::send
            // consumes self, and shutdown_tx's Option wrapper exists
            // precisely so this method (the only caller that ever fires
            // it) can move it out.
            let Some(shutdown_tx) = handle.shutdown_tx.take() else {
                // Structurally unreachable today — nothing else ever takes
                // this field — but treated as a recoverable warning rather
                // than a panic, consistent with the rest of this method's
                // "shutdown must proceed even if one worker misbehaves"
                // posture.
                tracing::warn!(
                    worker_id = %worker_id,
                    "worker handle already had no shutdown sender; skipping signal"
                );
                handle.run_handle.abort();
                continue;
            };

            // A Err here means run()'s task has already exited on its own
            // (e.g. ready timeout, or the worker crashed and exhausted its
            // respawn attempts) — the receiver was dropped along with it.
            // That's not a failure to act on; the run_handle await just
            // below will resolve immediately.
            let _ = shutdown_tx.send(());

            // Awaiting `&mut handle.run_handle` rather than the owned
            // JoinHandle by value: tokio::time::timeout takes its future
            // by value and drops it on expiry, which for an owned
            // JoinHandle would only detach this task's interest in the
            // result — the spawned task itself keeps running orphaned,
            // not aborted. Borrowing keeps the JoinHandle alive past the
            // timeout so the Err(_) branch below can call abort() on it
            // for real.
            match tokio::time::timeout(WORKER_SHUTDOWN_TIMEOUT, &mut handle.run_handle).await {
                Ok(Ok(())) => {
                    tracing::debug!(worker_id = %worker_id, "worker shut down cleanly");
                }
                Ok(Err(join_err)) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        error = %join_err,
                        "worker's run() task panicked or was cancelled during shutdown"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        worker_id = %worker_id,
                        timeout_secs = %WORKER_SHUTDOWN_TIMEOUT.as_secs(),
                        "worker did not shut down within the timeout, aborting its \
                         run() task; worker subprocess may remain running"
                    );
                    handle.run_handle.abort();
                }
            }
        }

        // Aborted rather than awaited: by this point every worker has
        // already been told to shut down, so there is nothing left for the
        // demux task to usefully deliver — waiting for transport.recv() to
        // itself error out would just add latency to shutdown for no benefit.
        if let Some(handle) = self.demux_handle.lock().await.take() {
            handle.abort();
        }
    }
}
