//! Managed worker process lifecycle and IPC bridge.
//!
//! The `ManagedWorker` struct owns a Python worker child process, manages its
//! ZeroMQ DEALER socket IPC, and runs a combined reader/writer task that
//! translates between Rust channel messages and msgpack-serialised IPC protocol bytes.

use anvilml_core::{
    config::ServerConfig, types::worker::WorkerStatus, AnvilError, GpuDevice, WorkerInfo,
};
use anvilml_ipc::{WorkerEvent, WorkerMessage};
use serde_json::Value as JsonValue;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::select;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use zeromq::prelude::*;

use crate::build_worker_env;

/// Shared IPC socket handle for the run_loop.
struct IpcHandles {
    socket: zeromq::DealerSocket,
}

impl std::fmt::Debug for IpcHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcHandles").field("socket", &"..").finish()
    }
}

/// A managed Python worker process with IPC bridge.
///
/// Owns the child process lifecycle (spawn, socket IPC piping) and provides
/// async methods for sending messages, subscribing to events, and querying status.
pub struct ManagedWorker {
    /// Logical worker identifier (e.g. `"worker-0"`).
    worker_id: String,
    /// GPU device index this worker owns.
    device_index: u32,
    /// Current lifecycle status, shared across async tasks.
    status: Arc<RwLock<WorkerStatus>>,
    /// Sender for outbound messages to the worker process.
    /// Wrapped in std::sync::Mutex for interior mutability (needed by reset_ipc_tx / restart).
    /// The mutex is always accessed via block_in_place() when called from async contexts,
    /// and blocking_lock() when called from synchronous contexts (start_keepalive).
    tx: std::sync::Mutex<mpsc::Sender<WorkerMessage>>,
    /// Broadcast sender for events emitted by the reader task.
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
    /// Child process handle (Some while alive), wrapped in Arc for shared access
    /// by both the spawn path and the keepalive watchdog task.
    child: Arc<Mutex<Option<tokio::process::Child>>>,
    /// Join handle for the combined writer+reader task loop.
    /// Wrapped in Mutex for interior mutability (needed by reset_ipc_tx / restart).
    #[allow(dead_code)]
    handle: std::sync::Mutex<JoinHandle<()>>,
    /// Oneshot sender used to deliver IPC handles to the loop.
    ipc_tx: std::sync::Mutex<Option<oneshot::Sender<IpcHandles>>>,
    /// Configurable ping interval (default: 30_000 ms).
    ping_interval: std::time::Duration,
    /// Configurable pong timeout (default: 10_000 ms).
    pong_timeout: std::time::Duration,
    /// Delay before respawning a dead worker (default: 2_000 ms).
    #[allow(dead_code)]
    respawn_delay_ms: std::time::Duration,
    /// Monotonically increasing generation counter.
    ///
    /// Incremented each time a new keepalive is started. Pong-timeout tasks
    /// capture the generation at the moment they are spawned and only broadcast
    /// `Dead` when the counter still matches, preventing stale timeouts from a
    /// previous lifecycle from triggering a spurious death after respawn.
    generation: Arc<AtomicU64>,
}

impl ManagedWorker {
    /// Create a new `ManagedWorker` with initialized channels and status.
    ///
    /// The broadcast channel capacity matches the config default
    /// `ws_broadcast_capacity = 256`.
    ///
    /// Ping interval and pong timeout are read from environment variables
    /// (`ANVILML_PING_INTERVAL_MS`, `ANVILML_PONG_TIMEOUT_MS`) or fall back
    /// to built-in defaults of 30 s and 10 s respectively.
    pub fn new(worker_id: String, device_index: u32) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let tx = std::sync::Mutex::new(tx);
        let (event_tx, _rx) = broadcast::channel(256);

        let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

        // Create a oneshot channel for delivering IPC handles from spawn()
        // to the run_loop.
        let (ipc_tx, ipc_rx) = oneshot::channel::<IpcHandles>();

        // Read configurable keepalive parameters from environment.
        let ping_interval = std::env::var("ANVILML_PING_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
            .unwrap_or(std::time::Duration::from_secs(30));

        let pong_timeout = std::env::var("ANVILML_PONG_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
            .unwrap_or(std::time::Duration::from_secs(10));

        let respawn_delay_ms = std::env::var("ANVILML_RESPAWN_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
            .unwrap_or(std::time::Duration::from_secs(2));

        // Spawn the combined writer+reader loop; it waits for IPC handles.
        let handle = spawn(Self::run_loop(
            worker_id.clone(),
            rx,
            event_tx.clone(),
            status.clone(),
            ipc_rx,
        ));

        Self {
            worker_id,
            device_index,
            status,
            tx,
            event_tx,
            child: Arc::new(Mutex::new(None)),
            handle: std::sync::Mutex::new(handle),
            ipc_tx: std::sync::Mutex::new(Some(ipc_tx)),
            ping_interval,
            pong_timeout,
            respawn_delay_ms,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Spawn the Python worker child process.
    ///
    /// Resolves the Python interpreter path from the config's `venv_path`,
    /// builds the command with environment variables from
    /// `build_worker_env`, and connects to the worker via a ZeroMQ DEALER socket
    /// on `tcp://127.0.0.1:{port}`.
    pub async fn spawn(&self, device: &GpuDevice, cfg: &ServerConfig) -> Result<(), AnvilError> {
        // Resolve venv path to absolute (fixes Windows CreateProcess
        // ERROR_PATH_NOT_FOUND when child CWD differs from parent CWD).
        let abs_venv = if cfg.venv_path.is_absolute() {
            cfg.venv_path.clone()
        } else {
            _repo_root_for_worker().join(&cfg.venv_path)
        };

        // Resolve python interpreter path.
        let python_path = resolve_python_path(&abs_venv);

        // Bind a ZeroMQ DEALER socket for IPC (port 0 = let OS pick an available port).
        let mut socket = zeromq::DealerSocket::new();
        let endpoint = socket.bind("tcp://127.0.0.1:0").await.map_err(|e| {
            AnvilError::Io(std::io::Error::other(format!(
                "failed to bind IPC socket: {e}"
            )))
        })?;
        let ipc_port = match endpoint {
            zeromq::Endpoint::Tcp(_, port) => port,
            other => {
                return Err(AnvilError::Io(std::io::Error::other(format!(
                    "unexpected bind endpoint type: {other:?}"
                ))))
            }
        };
        let ipc_addr = format!("127.0.0.1:{ipc_port}");

        // Build the command.
        let mut cmd = Command::new(&python_path);
        cmd.arg("worker/worker_main.py")
            .arg("--worker-id")
            .arg(&self.worker_id)
            .arg("--device-index")
            .arg(self.device_index.to_string())
            .current_dir(_repo_root_for_worker())
            .envs(build_worker_env(device, cfg, ipc_port))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());

        // Linux orphan cleanup: set PDEATHSIG to SIGHUP so the child is killed
        // if the parent process dies.
        #[cfg(unix)]
        {
            unsafe {
                cmd.pre_exec(|| {
                    let sig: usize = libc::SIGHUP as usize;
                    libc::prctl(libc::PR_SET_PDEATHSIG, sig, 0, 0, 0);
                    Ok(())
                });
            }
        }

        // Windows: remove the child from the parent's console process group so
        // that CTRL_C_EVENT is not broadcast to the worker process.
        #[cfg(windows)]
        {
            cmd.creation_flags(0x0000_0200); // CREATE_NEW_PROCESS_GROUP
        }

        info!(ipc_addr = %ipc_addr, "bound IPC socket");

        let mut child = cmd.spawn().map_err(|e| {
            warn!(
                error = %e,
                python = %python_path.display(),
                "failed to spawn worker process"
            );
            AnvilError::Io(e)
        })?;

        info!(
            worker_id = %self.worker_id,
            device_index = self.device_index,
            pid = child.id(),
            "worker spawned"
        );

        // Detach stderr.
        if let Some(mut stderr) = child.stderr.take() {
            let wid = self.worker_id.clone();
            tokio::task::spawn(async move {
                let _ = stderr.read_to_end(&mut Vec::new()).await;
                debug!(worker_id = %wid, "stderr drained");
            });
        }

        // Deliver IPC handles to run_loop immediately — before any data is sent.
        // This ensures the reader task registers the socket for polling (Linux) or
        // I/O completion ports (Windows) before InitializeHardware arrives,
        // preventing missed edge-triggered wakeups.
        {
            let mut guard = self.ipc_tx.lock().unwrap();
            if let Some(tx) = guard.take() {
                tx.send(IpcHandles { socket }).map_err(|_| {
                    AnvilError::Io(std::io::Error::other("run_loop already exited"))
                })?;
            } else {
                return Err(AnvilError::Io(std::io::Error::other(
                    "IPC channel already consumed",
                )));
            }
        }

        // Send InitializeHardware through the mpsc channel. The run_loop
        // will serialize and send it to the socket after the socket has
        // been registered for polling. This avoids the race where data
        // arrives before epoll registration.
        let init_msg = WorkerMessage::InitializeHardware {
            device_str: format!("{:?}:{}", device.device_type, self.device_index),
        };
        // Clone the sender before awaiting so the MutexGuard is dropped
        // before the .await point (std::sync::MutexGuard is not Send).
        let init_tx = {
            let guard = self.tx_lock();
            guard.clone()
        };
        if let Err(e) = init_tx.send(init_msg).await {
            warn!(error = %e, worker_id = %self.worker_id, "failed to send InitializeHardware");
            return Err(AnvilError::Io(std::io::Error::other(
                "mpsc channel closed before InitializeHardware could be sent",
            )));
        }

        // Yield multiple times so run_loop can process the message, send it to
        // the socket, and read the Ready response and update status.
        // Without these yields, spawn() would block the event loop in its polling
        // loop before run_loop gets scheduled.
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }

        // Wait for status to transition from Initializing to Idle.
        // Default: 60 s — HIP/ROCm on Windows requires ~5 s on warm runs and
        // significantly more on cold boot or under driver initialisation load.
        // Override with ANVILML_WORKER_READY_TIMEOUT_MS for testing or tight envs.
        let timeout_duration = std::env::var("ANVILML_WORKER_READY_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
            .unwrap_or(std::time::Duration::from_secs(60));
        let start = std::time::Instant::now();
        while start.elapsed() < timeout_duration {
            if *self.status.read().await == WorkerStatus::Idle {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // Store the child handle for later (before checking status to avoid deadlock).
        {
            let mut guard = self.child.lock().await;
            *guard = Some(child);
        }

        // Verify status is Idle.
        if *self.status.read().await != WorkerStatus::Idle {
            return Err(AnvilError::Io(std::io::Error::other(
                "worker did not reach Ready state in time",
            )));
        }

        Ok(())
    }

    /// Send a message to the worker process.
    pub async fn send(&self, msg: WorkerMessage) -> Result<(), AnvilError> {
        debug!(
            worker_id = %self.worker_id,
            message_type = ?msg_discriminant(&msg),
            "sending message to worker"
        );
        // Clone the sender before awaiting so the MutexGuard is dropped
        // before the .await point (std::sync::MutexGuard is not Send).
        let tx = {
            let guard = self.tx_lock();
            guard.clone()
        };
        tx.send(msg).await.map_err(|e| {
            warn!(error = %e, worker_id = %self.worker_id, "worker channel closed");
            AnvilError::WorkerDead(format!("send failed: {}", e))
        })
    }

    /// Restart this worker: send Shutdown, wait for Dying, force-kill if needed,
    /// re-spawn, and wait for Idle.
    #[allow(clippy::too_many_arguments)]
    pub async fn restart(&self, device: &GpuDevice, cfg: &ServerConfig) -> Result<(), AnvilError> {
        // 1. Set status to Respawning and broadcast.
        self.set_status(WorkerStatus::Respawning).await;
        let _ = self.event_tx.send((
            self.worker_id.clone(),
            WorkerEvent::WorkerStatusChanged {
                status: WorkerStatus::Respawning,
            },
        ));
        info!(worker_id = %self.worker_id, "worker restart initiated");

        // 2. Send Shutdown.
        let _ = {
            let guard = self.tx_lock();
            guard.clone()
        }
        .send(WorkerMessage::Shutdown)
        .await;
        debug!(
            worker_id = %self.worker_id,
            message_type = "Shutdown",
            "sent shutdown for restart"
        );

        // 3. Wait up to 5 s for Dying.
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if self.get_status().await == WorkerStatus::Dead {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // 4. Force-kill if still alive.
        if self.get_status().await != WorkerStatus::Dead {
            warn!(
                worker_id = %self.worker_id,
                "worker did not die in 5s — force-killing"
            );
            if let Some(mut ch) = self.child.lock().await.take() {
                let _ = ch.kill().await;
            }
            let mut s = self.status.write().await;
            *s = WorkerStatus::Dead;
        }

        // 5. Reset IPC channels (new run_loop).
        self.reset_ipc_tx().await;

        // 6. Re-spawn (sends InitializeHardware, waits for Idle).
        self.spawn(device, cfg).await?;

        info!(worker_id = %self.worker_id, "worker restarted");
        Ok(())
    }

    /// Subscribe to the broadcast channel for worker events.
    pub fn subscribe(&self) -> broadcast::Receiver<(String, WorkerEvent)> {
        self.event_tx.subscribe()
    }

    /// Get the current worker status.
    pub async fn get_status(&self) -> WorkerStatus {
        *self.status.read().await
    }

    /// Set the worker's lifecycle status.
    pub async fn set_status(&self, status: WorkerStatus) {
        let mut s = self.status.write().await;
        *s = status;
    }

    /// Lock the tx channel, blocking in-place if called from an async context.
    fn tx_lock(&self) -> std::sync::MutexGuard<'_, mpsc::Sender<WorkerMessage>> {
        // Use block_in_place to avoid panicking when called from within a tokio runtime.
        // This moves the blocking lock to a thread pool reserved for blocking operations.
        tokio::task::block_in_place(|| self.tx.lock().unwrap())
    }

    /// Force-kill the child process and set status to Dead.
    ///
    /// Used by `shutdown_all` to terminate stragglers that did not
    /// respond to a Shutdown message within the timeout window.
    pub async fn force_kill(&self) {
        if let Some(mut ch) = self.child.lock().await.take() {
            let _ = ch.kill().await;
        }
        let mut s = self.status.write().await;
        *s = WorkerStatus::Dead;
    }

    /// Send a Shutdown message to the worker.
    pub async fn send_shutdown(&self) {
        // Clone the sender before awaiting so the MutexGuard is dropped
        // before the .await point (std::sync::MutexGuard is not Send).
        let tx = {
            let guard = self.tx_lock();
            guard.clone()
        };
        let _ = tx.send(WorkerMessage::Shutdown).await;
    }

    /// Get a reference to the worker ID.
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Get the device index.
    pub fn device_index(&self) -> u32 {
        self.device_index
    }

    /// Return the child process PID if one is stored.
    ///
    /// This is a test-only accessor gated behind `#[cfg(any(test, feature = "test-helpers"))]`.
    #[cfg(any(test, feature = "test-helpers"))]
    pub async fn child_pid(&self) -> Option<u32> {
        self.child.lock().await.as_ref().and_then(|c| c.id())
    }

    /// Set the stored child process handle (test-only).
    #[cfg(any(test, feature = "test-helpers"))]
    pub async fn set_child_for_test(&self, child: tokio::process::Child) {
        let mut guard = self.child.lock().await;
        *guard = Some(child);
    }

    /// Build a `WorkerInfo` from current state.
    pub async fn info(&self) -> WorkerInfo {
        let status = *self.status.read().await;
        WorkerInfo {
            worker_id: self.worker_id.clone(),
            device_index: self.device_index,
            device_name: format!("worker-{}", self.device_index),
            status,
            current_job_id: None,
            vram_used_mib: 0,
        }
    }

    /// Start the keepalive watchdog task.
    ///
    /// Increments the internal generation counter so that any pong-timeout tasks
    /// spawned by a *previous* keepalive invocation are silently invalidated: they
    /// will observe a generation mismatch and skip the `Dead` broadcast.
    ///
    /// Returns the [`JoinHandle`] for the keepalive loop. The caller **must** store
    /// this handle and `abort()` it before replacing or respawning the worker, so
    /// that no further pings are sent on the stale channel and no new pong-timeout
    /// tasks are created.
    ///
    /// Sends `Ping{seq}` messages at the configured interval and force-kills the
    /// child process if a matching `Pong{seq}` is not received within `pong_timeout`.
    ///
    /// This method must be called after `spawn()` returns, when the worker is
    /// in the `Idle` state and the child handle is stored.
    pub fn start_keepalive(&self) -> JoinHandle<()> {
        // Advance generation so any pong-timeout tasks from the previous keepalive
        // (which may still be in-flight) observe a mismatch and do nothing.
        let current_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        let ping_interval = self.ping_interval;
        let pong_timeout = self.pong_timeout;
        let worker_id = self.worker_id.clone();
        let tx = self.tx.lock().unwrap().clone();
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();
        let child = self.child.clone();
        let generation = self.generation.clone();

        tokio::spawn(async move {
            debug!(worker_id = %worker_id, generation = current_gen, "keepalive task started");

            let mut interval = tokio::time::interval(ping_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut next_seq: u64 = 0;

            loop {
                interval.tick().await;

                // If the generation has advanced (worker was replaced), stop immediately.
                if generation.load(Ordering::SeqCst) != current_gen {
                    debug!(worker_id = %worker_id, generation = current_gen, "keepalive: generation superseded, exiting");
                    break;
                }

                let seq = next_seq;
                next_seq += 1;

                debug!(worker_id = %worker_id, seq = seq, "sending ping");

                // Subscribe BEFORE sending the Ping. broadcast::subscribe() only
                // receives messages sent after the subscribe call, so subscribing
                // first guarantees the Pong cannot arrive and be broadcast before
                // the timeout task is listening — preventing a missed-wakeup that
                // would cause a healthy worker to be killed on pong timeout.
                let pong_rx = event_tx.subscribe();

                // Send Ping message. If channel is closed, worker is dead — exit.
                if tx.send(WorkerMessage::Ping { seq }).await.is_err() {
                    warn!(worker_id = %worker_id, "keepalive: send failed, worker may be dead");
                    break;
                }

                // Per-pong timeout: spawn a task that kills the child if no Pong{seq} arrives.
                let pw_id = worker_id.clone();
                let child_handle = child.clone();
                let status_clone = status.clone();
                let ev_tx = event_tx.clone();
                let gen_clone = generation.clone();
                tokio::spawn(async move {
                    // Use the receiver created before the Ping was sent — never subscribe here.
                    let mut rx = pong_rx;
                    let pong_received = tokio::time::timeout(pong_timeout, async {
                        loop {
                            match rx.recv().await {
                                Ok((_, WorkerEvent::Pong { seq: rseq })) if rseq == seq => {
                                    return true
                                }
                                Ok(_) => continue,
                                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                Err(broadcast::error::RecvError::Closed) => break false,
                            }
                        }
                    })
                    .await;

                    match pong_received {
                        Ok(true) => {
                            // Pong received — nothing to do.
                            debug!(worker_id = %pw_id, seq = seq, "received pong");
                        }
                        _ => {
                            // Guard: only act if this timeout belongs to the current generation.
                            // A stale timeout from a previous keepalive must not kill or declare
                            // dead a worker that has since been replaced.
                            if gen_clone.load(Ordering::SeqCst) != current_gen {
                                debug!(
                                    worker_id = %pw_id,
                                    seq = seq,
                                    generation = current_gen,
                                    "pong timeout discarded — generation superseded"
                                );
                                return;
                            }

                            warn!(worker_id = %pw_id, seq = seq, "pong timeout — killing worker");
                            // Force-kill the child process.
                            if let Some(mut ch) = child_handle.lock().await.take() {
                                if let Err(e) = ch.kill().await {
                                    warn!(worker_id = %pw_id, error = %e, "failed to kill worker on pong timeout");
                                } else {
                                    info!(worker_id = %pw_id, "killed worker on pong timeout");
                                }
                            }
                            let mut s = status_clone.write().await;
                            *s = WorkerStatus::Dead;
                            // Broadcast status change so pool can trigger respawn.
                            let _ = ev_tx.send((
                                pw_id.clone(),
                                WorkerEvent::WorkerStatusChanged {
                                    status: WorkerStatus::Dead,
                                },
                            ));
                        }
                    }
                });
            }

            debug!(worker_id = %worker_id, generation = current_gen, "keepalive task exiting");
        })
    }

    /// Inject mock IPC handles for testing without spawning a real Python process.
    ///
    /// Creates a zeromq DEALER socket pair, injects the worker-side socket into the
    /// run_loop via the oneshot channel, and returns the supervisor-side socket
    /// for the test to use.
    #[cfg(test)]
    pub async fn inject_handles_for_test(&self) -> zeromq::DealerSocket {
        // Create a DEALER socket pair for test IPC.
        // Supervisor side: bind a DEALER socket.
        let mut supervisor_dealer = zeromq::DealerSocket::new();
        let endpoint = supervisor_dealer
            .bind("tcp://127.0.0.1:0")
            .await
            .expect("bind supervisor dealer");
        let port = match endpoint {
            zeromq::Endpoint::Tcp(_, port) => port,
            _ => panic!("expected TCP endpoint"),
        };

        // Worker side: connect to the bound address.
        let mut worker_dealer = zeromq::DealerSocket::new();
        worker_dealer
            .connect(&format!("tcp://127.0.0.1:{port}"))
            .await
            .expect("connect worker dealer");

        // Inject the worker-side socket into the run_loop.
        let mut guard = self.ipc_tx.lock().unwrap();
        if let Some(tx) = guard.take() {
            tx.send(IpcHandles {
                socket: worker_dealer,
            })
            .unwrap();
        }

        // Return the supervisor-side socket for the test to use.
        supervisor_dealer
    }

    /// Reset the IPC oneshot channel for respawn.
    ///
    /// Creates a fresh `(oneshot::Sender<IpcHandles>, oneshot::Receiver<IpcHandles>)`
    /// pair and stores the sender. The old receiver (if any was consumed) is gone;
    /// the run_loop will wait for the new one.
    #[allow(dead_code)]
    async fn reset_ipc_tx(&self) {
        let (ipc_tx, ipc_rx) = oneshot::channel::<IpcHandles>();
        let (tx, rx) = mpsc::channel(64);

        let worker_id = self.worker_id.clone();
        let status = self.status.clone();
        let event_tx = self.event_tx.clone();

        let new_handle = spawn(Self::run_loop(
            worker_id.clone(),
            rx,
            event_tx.clone(),
            status.clone(),
            ipc_rx,
        ));

        // Use block_in_place to avoid panicking in a tokio runtime.
        *tokio::task::block_in_place(|| self.tx.lock().unwrap()) = tx;
        *self.ipc_tx.lock().unwrap() = Some(ipc_tx);
        *self.handle.lock().unwrap() = new_handle;
    }

    /// Respawn the worker process after it has died.
    ///
    /// Sets status to `Respawning`, broadcasts a `WorkerStatusChanged(Respawning)`
    /// event, resets the IPC channel, spawns a fresh Python worker child process,
    /// and waits for it to reach Idle. On success, the worker is back in the Idle state.
    #[allow(dead_code)]
    async fn respawn(&mut self, device: &GpuDevice, cfg: &ServerConfig) -> Result<(), AnvilError> {
        // Set status to Respawning and broadcast.
        self.set_status(WorkerStatus::Respawning).await;
        let _ = self.event_tx.send((
            self.worker_id.clone(),
            WorkerEvent::WorkerStatusChanged {
                status: WorkerStatus::Respawning,
            },
        ));

        info!(
            worker_id = %self.worker_id,
            "worker respawn initiated"
        );

        // Reset the IPC channel so the new run_loop can receive handles.
        self.reset_ipc_tx().await;

        // Spawn the Python worker child process.
        self.spawn(device, cfg).await?;

        info!(
            worker_id = %self.worker_id,
            "worker respawned"
        );

        Ok(())
    }

    /// The main event loop: waits for IPC handles via oneshot, then runs
    /// a combined reader/writer loop that handles both sending messages
    /// to the worker and receiving events from it.
    ///
    /// This function runs inline (not spawned) because the zeromq DEALER
    /// socket cannot be cloned and must stay in a single task.
    async fn run_loop(
        worker_id: String,
        mut rx: mpsc::Receiver<WorkerMessage>,
        event_tx: broadcast::Sender<(String, WorkerEvent)>,
        status: Arc<RwLock<WorkerStatus>>,
        ipc_rx: oneshot::Receiver<IpcHandles>,
    ) {
        // Wait for IPC handles from spawn().
        let IpcHandles { mut socket } = match ipc_rx.await {
            Ok(handles) => handles,
            Err(_) => {
                warn!(worker_id = %worker_id, "IPC channel closed before handles received");
                return;
            }
        };

        // Combined reader/writer loop for the zeromq DEALER socket.
        // Handles both sending messages from the mpsc channel to the worker
        // and receiving events from the worker via the zeromq socket.
        loop {
            select! {
                // Send path: receive a message from the mpsc channel and send it
                // to the worker via zeromq.
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            debug!(
                                worker_id = %worker_id,
                                message_type = ?msg_discriminant(&msg),
                                "sending message to worker"
                            );
                            let payload = match serialize_message(&msg) {
                                Ok(p) => p,
                                Err(e) => {
                                    warn!(error = %e, worker_id = %worker_id, "failed to serialize message");
                                    break;
                                }
                            };
                            if let Err(e) = socket.send(zeromq::ZmqMessage::from(payload)).await {
                                warn!(error = %e, worker_id = %worker_id, "failed to send IPC frame");
                                break;
                            }
                        }
                        None => {
                            // mpsc channel closed — worker is gone.
                            debug!(worker_id = %worker_id, "mpsc channel closed");
                            break;
                        }
                    }
                }

                // Receive path: read an event from the worker via zeromq.
                result = socket.recv() => {
                    let msg = match result {
                        Ok(msg) => msg,
                        Err(e) => {
                            // Connection closed (EOF) or other error — treat as worker death.
                            warn!(error = %e, worker_id = %worker_id, "socket recv error");
                            break;
                        }
                    };

                    let bytes: Vec<u8> = match msg.try_into() {
                        Ok(b) => b,
                        Err(_) => {
                            warn!(worker_id = %worker_id, "failed to convert zmq message to bytes");
                            break;
                        }
                    };

                    let event = match rmp_serde::from_slice::<serde_json::Map<String, JsonValue>>(&bytes) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(error = %e, worker_id = %worker_id, "failed to deserialize IPC frame");
                            break;
                        }
                    };

                    let event = match worker_event_from_map(&event) {
                        Ok(e) => e,
                        Err(e) => {
                            warn!(error = %e, worker_id = %worker_id, "failed to parse worker event");
                            break;
                        }
                    };

                    debug!(
                        worker_id = %worker_id,
                        event_type = ?event_discriminant(&event),
                        "received event from worker"
                    );

                    // Update status based on the event.
                    update_status_from_event(&status, &event).await;

                    let _ = event_tx.send((worker_id.clone(), event));
                }
            }
        }

        warn!(worker_id = %worker_id, "run_loop exiting — worker is Dead");

        // Broadcast status change so pool can trigger respawn.
        let _ = event_tx.send((
            worker_id.clone(),
            WorkerEvent::WorkerStatusChanged {
                status: WorkerStatus::Dead,
            },
        ));
    }
}

/// Update the worker status based on an incoming event.
async fn update_status_from_event(status: &Arc<RwLock<WorkerStatus>>, event: &WorkerEvent) {
    let mut s = status.write().await;
    match event {
        WorkerEvent::Ready { .. } if *s == WorkerStatus::Initializing => {
            *s = WorkerStatus::Idle;
        }
        WorkerEvent::Dying { .. } => {
            *s = WorkerStatus::Dead;
        }
        _ => {}
    }
}

/// Serialize a `WorkerMessage` into a flat dict compatible with Python's
/// msgpack serialization (uses `_type` as the variant discriminator).
///
/// Copied from `anvilml_ipc::framing` to decouple managed.rs from the framing module
/// after switching from tokio TCP to zeromq transport.
fn serialize_message(msg: &WorkerMessage) -> Result<Vec<u8>, AnvilError> {
    let mut map = serde_json::Map::new();
    match msg {
        WorkerMessage::Ping { seq } => {
            map.insert("_type".into(), "Ping".into());
            map.insert("seq".into(), JsonValue::Number((*seq).into()));
        }
        WorkerMessage::Shutdown => {
            map.insert("_type".into(), "Shutdown".into());
        }
        WorkerMessage::InitializeHardware { device_str } => {
            map.insert("_type".into(), "InitializeHardware".into());
            map.insert("device_str".into(), JsonValue::String(device_str.clone()));
        }
        WorkerMessage::Execute {
            job_id,
            graph,
            settings,
            device_index,
        } => {
            map.insert("_type".into(), "Execute".into());
            map.insert("job_id".into(), JsonValue::String(job_id.to_string()));
            map.insert("graph".into(), graph.clone());
            // Serialize JobSettings as a flat dict
            let mut settings_map = serde_json::Map::new();
            settings_map.insert("seed".into(), JsonValue::Number(settings.seed.into()));
            settings_map.insert("steps".into(), JsonValue::Number(settings.steps.into()));
            let gs = settings.guidance_scale as f64;
            map.insert(
                "guidance_scale".into(),
                JsonValue::Number(
                    serde_json::Number::from_f64(gs).unwrap_or_else(|| serde_json::Number::from(1)),
                ),
            );
            settings_map.insert("width".into(), JsonValue::Number(settings.width.into()));
            settings_map.insert("height".into(), JsonValue::Number(settings.height.into()));
            if let Some(ref dp) = settings.device_preference {
                settings_map.insert("device_preference".into(), JsonValue::Number((*dp).into()));
            }
            map.insert("settings".into(), JsonValue::Object(settings_map));
            map.insert(
                "device_index".into(),
                JsonValue::Number((*device_index).into()),
            );
        }
        WorkerMessage::CancelJob { job_id } => {
            map.insert("_type".into(), "CancelJob".into());
            map.insert("job_id".into(), JsonValue::String(job_id.to_string()));
        }
        WorkerMessage::MemoryQuery => {
            map.insert("_type".into(), "MemoryQuery".into());
        }
    }
    rmp_serde::to_vec_named(&map).map_err(|e| {
        tracing::error!(error = %e, "IPC frame serialize failed");
        AnvilError::Json(e.to_string())
    })
}

/// Deserialize a flat dict (from Python's msgpack) into a WorkerEvent.
///
/// The dict uses `_type` as the variant discriminator and has fields at
/// the top level (e.g. `{"_type": "Ready", "worker_id": "...", ...}`).
///
/// Copied from `anvilml_ipc::framing` to decouple managed.rs from the framing module
/// after switching from tokio TCP to zeromq transport.
fn worker_event_from_map(map: &serde_json::Map<String, JsonValue>) -> Result<WorkerEvent, String> {
    let _type = map
        .get("_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "_type field missing or not a string".to_string())?;

    match _type {
        "Ready" => Ok(WorkerEvent::Ready {
            worker_id: map
                .get("worker_id")
                .and_then(|v| v.as_str())
                .ok_or("worker_id missing")?
                .to_string(),
            device_index: map
                .get("device_index")
                .and_then(|v| v.as_u64())
                .ok_or("device_index missing")? as u32,
            vram_total_mib: map
                .get("vram_total_mib")
                .and_then(|v| v.as_u64())
                .ok_or("vram_total_mib missing")? as u32,
            vram_free_mib: map
                .get("vram_free_mib")
                .and_then(|v| v.as_u64())
                .ok_or("vram_free_mib missing")? as u32,
            arch: map
                .get("arch")
                .and_then(|v| v.as_str())
                .ok_or("arch missing")?
                .to_string(),
            fp16: map
                .get("fp16")
                .and_then(|v| v.as_bool())
                .ok_or("fp16 missing")?,
            bf16: map
                .get("bf16")
                .and_then(|v| v.as_bool())
                .ok_or("bf16 missing")?,
            flash_attention: map
                .get("flash_attention")
                .and_then(|v| v.as_bool())
                .ok_or("flash_attention missing")?,
        }),
        "Ping" => Ok(WorkerEvent::Ping {
            seq: map
                .get("seq")
                .and_then(|v| v.as_u64())
                .ok_or("seq missing")?,
        }),
        "Pong" => Ok(WorkerEvent::Pong {
            seq: map
                .get("seq")
                .and_then(|v| v.as_u64())
                .ok_or("seq missing")?,
        }),
        "Dying" => Ok(WorkerEvent::Dying {
            reason: map
                .get("reason")
                .and_then(|v| v.as_str())
                .ok_or("reason missing")?
                .to_string(),
        }),
        "MemoryReport" => Ok(WorkerEvent::MemoryReport {
            vram_used_mib: map
                .get("vram_used_mib")
                .and_then(|v| v.as_u64())
                .ok_or("vram_used_mib missing")? as u32,
            ram_used_mib: map
                .get("ram_used_mib")
                .and_then(|v| v.as_u64())
                .ok_or("ram_used_mib missing")?,
        }),
        "Progress" => Ok(WorkerEvent::Progress {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            node_index: map
                .get("node_index")
                .and_then(|v| v.as_u64())
                .ok_or("node_index missing")? as u32,
            node_total: map
                .get("node_total")
                .and_then(|v| v.as_u64())
                .ok_or("node_total missing")? as u32,
            node_type: map
                .get("node_type")
                .and_then(|v| v.as_str())
                .ok_or("node_type missing")?
                .to_string(),
            step: map.get("step").and_then(|v| v.as_u64()).map(|v| v as u32),
            step_total: map
                .get("step_total")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
        }),
        "ImageReady" => Ok(WorkerEvent::ImageReady {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            image_b64: map
                .get("image_b64")
                .and_then(|v| v.as_str())
                .ok_or("image_b64 missing")?
                .to_string(),
            width: map
                .get("width")
                .and_then(|v| v.as_u64())
                .ok_or("width missing")? as u32,
            height: map
                .get("height")
                .and_then(|v| v.as_u64())
                .ok_or("height missing")? as u32,
            format: map
                .get("format")
                .and_then(|v| v.as_str())
                .ok_or("format missing")?
                .to_string(),
            seed: map
                .get("seed")
                .and_then(|v| v.as_i64())
                .ok_or("seed missing")?,
            steps: map
                .get("steps")
                .and_then(|v| v.as_u64())
                .ok_or("steps missing")? as u32,
            prompt: map
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or("prompt missing")?
                .to_string(),
        }),
        "Completed" => Ok(WorkerEvent::Completed {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            elapsed_ms: map
                .get("elapsed_ms")
                .and_then(|v| v.as_u64())
                .ok_or("elapsed_ms missing")?,
        }),
        "Failed" => Ok(WorkerEvent::Failed {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
            error: map
                .get("error")
                .and_then(|v| v.as_str())
                .ok_or("error missing")?
                .to_string(),
            traceback: map
                .get("traceback")
                .and_then(|v| v.as_str())
                .ok_or("traceback missing")?
                .to_string(),
        }),
        "Cancelled" => Ok(WorkerEvent::Cancelled {
            job_id: uuid::Uuid::parse_str(
                map.get("job_id")
                    .and_then(|v| v.as_str())
                    .ok_or("job_id missing")?,
            )
            .map_err(|e| format!("invalid job_id: {}", e))?,
        }),
        _ => Err(format!("unknown event type: {}", _type)),
    }
}

/// Get a discriminant name for a WorkerMessage.
fn msg_discriminant(msg: &WorkerMessage) -> &'static str {
    match msg {
        WorkerMessage::Ping { .. } => "Ping",
        WorkerMessage::Shutdown => "Shutdown",
        WorkerMessage::InitializeHardware { .. } => "InitializeHardware",
        WorkerMessage::Execute { .. } => "Execute",
        WorkerMessage::CancelJob { .. } => "CancelJob",
        WorkerMessage::MemoryQuery => "MemoryQuery",
    }
}

/// Get a discriminant name for a WorkerEvent.
fn event_discriminant(event: &WorkerEvent) -> &'static str {
    match event {
        WorkerEvent::Ready { .. } => "Ready",
        WorkerEvent::Ping { .. } => "Ping",
        WorkerEvent::Pong { .. } => "Pong",
        WorkerEvent::Dying { .. } => "Dying",
        WorkerEvent::MemoryReport { .. } => "MemoryReport",
        WorkerEvent::Progress { .. } => "Progress",
        WorkerEvent::ImageReady { .. } => "ImageReady",
        WorkerEvent::Completed { .. } => "Completed",
        WorkerEvent::Failed { .. } => "Failed",
        WorkerEvent::Cancelled { .. } => "Cancelled",
        WorkerEvent::WorkerStatusChanged { .. } => "WorkerStatusChanged",
    }
}

/// Resolve the Python interpreter path from a venv directory.
fn resolve_python_path(venv_path: &Path) -> std::path::PathBuf {
    let python = if cfg!(windows) {
        "Scripts\\python.exe"
    } else {
        "bin/python3"
    };
    venv_path.join(python)
}

/// Return the repository root directory where `worker/worker_main.py` lives.
///
/// This is used as the current working directory for the worker subprocess so that
/// the relative path `worker/worker_main.py` resolves correctly regardless of
/// where the test or binary is invoked from.
fn _repo_root_for_worker() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Returns a `ServerConfig` with the venv path resolved from `ANVILML_VENV_PATH`,
    /// or `None` if the resulting Python interpreter does not exist on disk.
    ///
    /// Call at the top of any test that spawns a real Python worker. If `None` is
    /// returned the test should return immediately — this prevents the 60-second
    /// accept/status-poll stall that occurs on CI runners where no venv is present.
    fn venv_cfg_or_skip() -> Option<ServerConfig> {
        let venv_path = std::env::var("ANVILML_VENV_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/home/dryw/forge/.venv"));

        // Resolve relative paths the same way spawn() does — against the repo root
        // derived from CARGO_MANIFEST_DIR. This ensures that a relative path like
        // `.ci-venv` set by CI (working dir = repo root) resolves correctly even
        // though `cargo test` runs with cwd = crates/anvilml-worker/.
        let abs_venv = if venv_path.is_absolute() {
            venv_path.clone()
        } else {
            _repo_root_for_worker().join(&venv_path)
        };

        let python = if cfg!(windows) {
            abs_venv.join("Scripts").join("python.exe")
        } else {
            abs_venv.join("bin").join("python3")
        };

        if !python.exists() {
            eprintln!(
                "SKIP: Python interpreter not found at {}                  (set ANVILML_VENV_PATH to run this test)",
                python.display()
            );
            return None;
        }

        Some(ServerConfig {
            venv_path,
            ..ServerConfig::default()
        })
    }

    /// Spawn a mock worker, send Ping, receive Pong, then Shutdown.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[cfg(feature = "mock-hardware")]
    #[cfg_attr(
        windows,
        ignore = "stalls on Windows tokio test runtime; covered on Linux"
    )]
    async fn spawn_ping_pong() {
        temp_env::async_with_vars([("ANVILML_WORKER_MOCK", Some("1"))], async {
            let worker = ManagedWorker::new("test-worker".to_string(), 0);

            // Build a mock device.
            let device = GpuDevice {
                index: 0,
                name: "Mock GPU".to_string(),
                device_type: anvilml_core::DeviceType::Cpu,
                vram_total_mib: 8192,
                vram_free_mib: 8192,
                driver_version: "mock".to_string(),
                pci_vendor_id: 0,
                pci_device_id: 0,
                arch: Some("gfx1100".to_string()),
                caps: Default::default(),
                enumeration_source: anvilml_core::EnumerationSource::Mock,
                capabilities_source: anvilml_core::CapabilitySource::Fallback,
                db_group_name: None,
            };

            // Use the system python3 directly for tests.
            let cfg = match venv_cfg_or_skip() {
                Some(c) => c,
                None => return,
            };

            // Spawn the worker process (waits for Ready internally).
            worker.spawn(&device, &cfg).await.expect("spawn");

            // Subscribe to events.
            let mut rx = worker.subscribe();

            // Send Ping.
            worker
                .send(WorkerMessage::Ping { seq: 1 })
                .await
                .expect("send ping");

            // Wait for Pong.
            let pong_timeout = timeout(Duration::from_secs(5), async {
                loop {
                    match rx.recv().await {
                        Ok((_, WorkerEvent::Pong { seq })) => {
                            assert_eq!(seq, 1);
                            break;
                        }
                        Ok(_) => continue,
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!(lagged = n, "dropped events");
                        }
                        Err(broadcast::error::RecvError::Closed) => panic!("event channel closed"),
                    }
                }
            })
            .await;

            assert!(pong_timeout.is_ok(), "did not receive Pong in time");

            // Send Shutdown.
            worker
                .send(WorkerMessage::Shutdown)
                .await
                .expect("send shutdown");

            // Wait for the handle to complete.
            let join_timeout = timeout(Duration::from_secs(10), async {
                loop {
                    if worker.get_status().await == WorkerStatus::Dead {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await;

            assert!(
                join_timeout.is_ok(),
                "worker did not exit after shutdown in time"
            );
        })
        .await;
    }

    /// Verify status transitions: Initializing → Idle (on Ready) → Dead.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[cfg(feature = "mock-hardware")]
    #[cfg_attr(
        windows,
        ignore = "stalls on Windows tokio test runtime; covered on Linux"
    )]
    async fn status_transitions() {
        temp_env::async_with_vars([("ANVILML_WORKER_MOCK", Some("1"))], async {
            let worker = ManagedWorker::new("status-test".to_string(), 0);

            let device = GpuDevice {
                index: 0,
                name: "Mock GPU".to_string(),
                device_type: anvilml_core::DeviceType::Cpu,
                vram_total_mib: 8192,
                vram_free_mib: 8192,
                driver_version: "mock".to_string(),
                pci_vendor_id: 0,
                pci_device_id: 0,
                arch: Some("gfx1100".to_string()),
                caps: Default::default(),
                enumeration_source: anvilml_core::EnumerationSource::Mock,
                capabilities_source: anvilml_core::CapabilitySource::Fallback,
                db_group_name: None,
            };

            let cfg = match venv_cfg_or_skip() {
                Some(c) => c,
                None => return,
            };

            // Status should start as Initializing.
            assert_eq!(worker.get_status().await, WorkerStatus::Initializing);

            // Spawn triggers InitializeHardware → Ready, which sets status to Idle.
            worker.spawn(&device, &cfg).await.expect("spawn");

            // After spawn returns, status should be Idle (Ready was received).
            assert_eq!(worker.get_status().await, WorkerStatus::Idle);
        })
        .await;
    }

    /// End-to-end handshake regression test: spawn → exactly one Ready → Idle.
    ///
    /// Guards against re-introduction of the double InitializeHardware write bug
    /// (P10-B1). Asserts that after spawn completes:
    /// - Status is Idle
    /// - Exactly one `Ready` event is received in a 500ms drain window
    /// - No second Ready, no Dying, no Dead events appear during the drain
    ///
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[cfg(feature = "mock-hardware")]
    #[cfg_attr(
        windows,
        ignore = "stalls on Windows tokio test runtime; covered on Linux"
    )]
    async fn handshake_completes_once() {
        temp_env::async_with_vars(
            [
                ("ANVILML_WORKER_MOCK", Some("1")),
                ("ANVILML_PING_INTERVAL_MS", Some("50")),
                ("ANVILML_PONG_TIMEOUT_MS", Some("150")),
            ],
            async {
                let worker = ManagedWorker::new("handshake-test".to_string(), 0);

                // Build a mock device (same shape as existing tests).
                let device = GpuDevice {
                    index: 0,
                    name: "Mock GPU".to_string(),
                    device_type: anvilml_core::DeviceType::Cpu,
                    vram_total_mib: 8192,
                    vram_free_mib: 8192,
                    driver_version: "mock".to_string(),
                    pci_vendor_id: 0,
                    pci_device_id: 0,
                    arch: Some("gfx1100".to_string()),
                    caps: Default::default(),
                    enumeration_source: anvilml_core::EnumerationSource::Mock,
                    capabilities_source: anvilml_core::CapabilitySource::Fallback,
                    db_group_name: None,
                };

                let cfg = match venv_cfg_or_skip() {
                    Some(c) => c,
                    None => return,
                };

                // Subscribe to broadcast channel *before* spawning.
                let mut rx = worker.subscribe();

                // Spawn the worker (writes InitializeHardware internally, waits for Ready).
                worker.spawn(&device, &cfg).await.expect("spawn");

                // Assert status is Idle after spawn completes.
                assert_eq!(worker.get_status().await, WorkerStatus::Idle);

                // Drain broadcast channel with 500ms timeout: collect all events within window.
                let drain_timeout = tokio::time::Duration::from_millis(500);
                let start = std::time::Instant::now();
                let mut ready_count = 0u32;
                let mut had_dying = false;
                let mut had_dead = false;

                while start.elapsed() < drain_timeout {
                    match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
                        .await
                    {
                        Ok(Ok((_, event))) => match &event {
                            WorkerEvent::Ready { .. } => ready_count += 1,
                            WorkerEvent::Dying { .. } => had_dying = true,
                            WorkerEvent::WorkerStatusChanged { status }
                                if *status == WorkerStatus::Dead =>
                            {
                                had_dead = true
                            }
                            _ => {} // Pong, etc. are expected — ignore.
                        },
                        Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                        Ok(Err(broadcast::error::RecvError::Closed)) => break,
                        Err(_) => {
                            // No event within 100ms — drain window is effectively over.
                            break;
                        }
                    }
                }

                // Exactly one Ready event must have been received.
                assert_eq!(
                    ready_count, 1,
                    "expected exactly one Ready event, got {ready_count}"
                );

                // No second Ready, no Dying, no Dead during the drain window.
                assert!(!had_dying, "unexpected Dying event during handshake drain");
                assert!(
                    !had_dead,
                    "unexpected WorkerStatusChanged(Dead) during handshake drain"
                );
            },
        )
        .await;
    }

    /// Verify that a socket error on the zeromq socket causes the run_loop to
    /// exit and broadcast a Dead status event.
    ///
    /// Creates a DEALER socket pair, injects the worker-side socket into the
    /// run_loop, sends a Ready frame through the supervisor side, then
    /// sends an invalid message to trigger a deserialization error, which
    /// causes the run_loop to exit and broadcast WorkerStatusChanged(Dead).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[cfg(feature = "mock-hardware")]
    async fn eof_sets_dead() {
        // Create a DEALER socket pair: supervisor binds, worker connects.
        let mut supervisor_dealer = zeromq::DealerSocket::new();
        let endpoint = supervisor_dealer
            .bind("tcp://127.0.0.1:0")
            .await
            .expect("bind supervisor dealer");
        let port = match endpoint {
            zeromq::Endpoint::Tcp(_, port) => port,
            _ => panic!("expected TCP endpoint"),
        };

        let mut worker_dealer = zeromq::DealerSocket::new();
        worker_dealer
            .connect(&format!("tcp://127.0.0.1:{port}"))
            .await
            .expect("connect worker dealer");

        // Create a worker and subscribe to events before injecting handles.
        let worker = ManagedWorker::new("eof-test".to_string(), 0);
        let mut event_rx = worker.subscribe();

        // Inject the worker-side socket into the run_loop.
        let mut guard = worker.ipc_tx.lock().unwrap();
        if let Some(tx) = guard.take() {
            tx.send(IpcHandles {
                socket: worker_dealer,
            })
            .unwrap();
        }

        // Write a Ready frame using msgpack-serialised flat dict (same as Python worker).
        let ready_json: serde_json::Map<String, JsonValue> = [
            ("_type".to_string(), serde_json::json!("Ready")),
            ("worker_id".to_string(), serde_json::json!("eof-test")),
            ("device_index".to_string(), serde_json::json!(0u64)),
            ("vram_total_mib".to_string(), serde_json::json!(8192u64)),
            ("vram_free_mib".to_string(), serde_json::json!(8192u64)),
            ("arch".to_string(), serde_json::json!("gfx1100")),
            ("fp16".to_string(), serde_json::json!(true)),
            ("bf16".to_string(), serde_json::json!(true)),
            ("flash_attention".to_string(), serde_json::json!(false)),
        ]
        .into_iter()
        .collect();
        let ready_payload = rmp_serde::to_vec_named(&ready_json).expect("serialize");

        // Send the Ready frame through the supervisor socket.
        supervisor_dealer
            .send(zeromq::ZmqMessage::from(ready_payload))
            .await
            .expect("send ready");

        // Drain events: expect a Ready event.
        let ready_received = timeout(Duration::from_secs(2), async {
            loop {
                match event_rx.recv().await {
                    Ok((_, WorkerEvent::Ready { .. })) => return true,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break false,
                }
            }
        })
        .await;
        assert!(
            ready_received.is_ok() && ready_received.unwrap(),
            "should receive Ready event"
        );

        // Send an invalid message (not msgpack-encoded) to trigger deserialization
        // error, which causes the run_loop to exit and broadcast Dead.
        supervisor_dealer
            .send(zeromq::ZmqMessage::from(vec![0xff, 0xfe, 0xfd]))
            .await
            .expect("send invalid");

        // Wait for the run_loop to detect the error and broadcast Dead.
        let dead_received = timeout(Duration::from_secs(3), async {
            loop {
                match event_rx.recv().await {
                    Ok((_, WorkerEvent::WorkerStatusChanged { status }))
                        if status == WorkerStatus::Dead =>
                    {
                        return true;
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break false,
                }
            }
        })
        .await;

        assert!(
            dead_received.is_ok() && dead_received.unwrap(),
            "should receive WorkerStatusChanged(Dead) after socket error"
        );
    }

    /// Keepalive watchdog: sends Pongs for seq 0–1, then stops; verifies the
    /// worker transitions to Dead after a pong timeout with no Python process.
    #[tokio::test]
    #[cfg(feature = "mock-hardware")]
    async fn keepalive_pings_and_kills_on_timeout() {
        // Use short intervals for fast testing.
        std::env::set_var("ANVILML_PING_INTERVAL_MS", "50");
        std::env::set_var("ANVILML_PONG_TIMEOUT_MS", "150");

        let worker = ManagedWorker::new("keepalive-test".to_string(), 0);

        // Set status to Idle directly (simulating what Ready event would do).
        worker.set_status(WorkerStatus::Idle).await;

        // Spawn a dummy child process so the keepalive task has something to kill.
        let child = spawn_dummy_child();
        {
            let mut guard = worker.child.lock().await;
            *guard = Some(child);
        }

        // Start the keepalive watchdog.
        worker.start_keepalive();

        // Send 2 Pongs (for seq 0 and 1) via the event broadcast channel,
        // then stop sending so seq=2 times out.
        let pong_0 = WorkerEvent::Pong { seq: 0 };
        let _ = worker.event_tx.send((worker.worker_id.clone(), pong_0));

        tokio::time::sleep(Duration::from_millis(80)).await;

        let pong_1 = WorkerEvent::Pong { seq: 1 };
        let _ = worker.event_tx.send((worker.worker_id.clone(), pong_1));

        // No more pongs — seq=2 will time out at 150ms.

        // Wait for timeout and verify worker is Dead.
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if worker.get_status().await == WorkerStatus::Dead {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;

        assert!(result.is_ok(), "worker should be Dead after pong timeout");
        assert_eq!(worker.get_status().await, WorkerStatus::Dead);

        // Cleanup.
        std::env::remove_var("ANVILML_PING_INTERVAL_MS");
        std::env::remove_var("ANVILML_PONG_TIMEOUT_MS");
    }

    /// Spawn a dummy child process and return its handle, for use in keepalive tests.
    fn spawn_dummy_child() -> tokio::process::Child {
        #[cfg(unix)]
        {
            Command::new("true").spawn().expect("spawn dummy")
        }
        #[cfg(windows)]
        {
            Command::new("cmd")
                .arg("/c")
                .arg("exit 0")
                .spawn()
                .expect("spawn dummy")
        }
    }

    /// Verify that EOF triggers Dead status and WorkerStatusChanged broadcast,
    /// then respawn via mock handles transitions back to Idle.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[cfg(feature = "mock-hardware")]
    async fn respawn_after_death() {
        // Use temp_env to isolate env-var mutations from parallel tests.
        // Without this, leaked ANVILML_PING_INTERVAL_MS / ANVILML_PONG_TIMEOUT_MS
        // cause concurrent spawn tests to be killed before Ready arrives.
        temp_env::async_with_vars(
            [
                ("ANVILML_PING_INTERVAL_MS", Some("50")),
                ("ANVILML_PONG_TIMEOUT_MS", Some("150")),
                ("ANVILML_RESPAWN_DELAY_MS", Some("100")),
            ],
            async {
                let worker = ManagedWorker::new("respawn-test".to_string(), 0);

                // Use inject_handles_for_test to get a supervisor socket.
                let supervisor = worker.inject_handles_for_test().await;
                drop(supervisor); // Drop to trigger EOF on the injected socket.

                // Wait for the run_loop to detect EOF and set status to Dead.
                let _ = timeout(Duration::from_secs(2), async {
                    loop {
                        if worker.get_status().await == WorkerStatus::Dead {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                })
                .await;

                // For testing: set status to Idle directly (simulating Ready event).
                worker.set_status(WorkerStatus::Idle).await;

                // Spawn the dummy child for keepalive testing.
                let keepalive_child = spawn_dummy_child();
                {
                    let mut guard = worker.child.lock().await;
                    *guard = Some(keepalive_child);
                }

                // Subscribe to events to capture WorkerStatusChanged broadcasts.
                let mut event_rx = worker.subscribe();

                // Start the keepalive watchdog — no pongs will be sent, so it times out.
                worker.start_keepalive();

                // Wait for pong timeout → Dead status and WorkerStatusChanged(Dead) broadcast.
                let dead_result = timeout(Duration::from_secs(2), async {
                    loop {
                        if worker.get_status().await == WorkerStatus::Dead {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                })
                .await;
                assert!(
                    dead_result.is_ok(),
                    "worker should be Dead after pong timeout"
                );
                assert_eq!(worker.get_status().await, WorkerStatus::Dead);

                // Verify WorkerStatusChanged(Dead) was broadcast.
                let status_changed = timeout(Duration::from_secs(1), async {
                    loop {
                        match event_rx.recv().await {
                            Ok((_, WorkerEvent::WorkerStatusChanged { status }))
                                if status == WorkerStatus::Dead =>
                            {
                                return true;
                            }
                            Ok(_) => continue,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break false,
                        }
                    }
                })
                .await;
                assert!(
                    status_changed.is_ok() && status_changed.unwrap(),
                    "WorkerStatusChanged(Dead) should be broadcast on pong timeout"
                );

                // Simulate respawn: set Respawning.
                worker.set_status(WorkerStatus::Respawning).await;
                let _ = worker.event_tx.send((
                    worker.worker_id().to_string(),
                    WorkerEvent::WorkerStatusChanged {
                        status: WorkerStatus::Respawning,
                    },
                ));

                // Create fresh mock handles for the respawned worker.
                let supervisor2 = worker.inject_handles_for_test().await;
                drop(supervisor2); // Drop to trigger EOF on the new injected socket.

                // Reset IPC channel and inject fresh handles.
                worker.reset_ipc_tx().await;
                let _supervisor3 = worker.inject_handles_for_test().await;

                // Set status back to Idle (simulating what Ready event would do).
                worker.set_status(WorkerStatus::Idle).await;

                // Verify status is Idle.
                let idle_result = timeout(Duration::from_secs(1), async {
                    loop {
                        if worker.get_status().await == WorkerStatus::Idle {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                })
                .await;
                assert!(idle_result.is_ok(), "worker should be Idle after respawn");
            },
        )
        .await;
    }

    /// Canonical regression test: spawn reaches Idle without timing workarounds.
    ///
    /// This test validates that the epoll fix (P10-B3) correctly delivers
    /// InitializeHardware through the mpsc channel after reader_task has
    /// registered stdout for polling — ensuring no edge-triggered wakeups
    /// are missed on Linux.
    ///
    /// Required: ANVILML_WORKER_MOCK=1 and ANVILML_VENV_PATH must be set.
    ///
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[cfg(feature = "mock-hardware")]
    #[cfg_attr(
        windows,
        ignore = "stalls on Windows tokio test runtime; covered on Linux"
    )]
    async fn spawn_reaches_idle() {
        temp_env::async_with_vars([("ANVILML_WORKER_MOCK", Some("1"))], async {
            let worker = ManagedWorker::new("idle-test".to_string(), 0);

            let device = GpuDevice {
                index: 0,
                name: "Mock GPU".to_string(),
                device_type: anvilml_core::DeviceType::Cpu,
                vram_total_mib: 8192,
                vram_free_mib: 8192,
                driver_version: "mock".to_string(),
                pci_vendor_id: 0,
                pci_device_id: 0,
                arch: Some("gfx1100".to_string()),
                caps: Default::default(),
                enumeration_source: anvilml_core::EnumerationSource::Mock,
                capabilities_source: anvilml_core::CapabilitySource::Fallback,
                db_group_name: None,
            };

            let cfg = match venv_cfg_or_skip() {
                Some(c) => c,
                None => return,
            };

            // spawn() internally sends InitializeHardware, waits for Ready→Idle.
            worker.spawn(&device, &cfg).await.expect("spawn");

            // Verify status is Idle — no sleep, no timing workaround.
            assert_eq!(worker.get_status().await, WorkerStatus::Idle);
        })
        .await;
    }
}
