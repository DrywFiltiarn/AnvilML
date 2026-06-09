//! Managed worker process lifecycle and IPC bridge.
//!
//! The `ManagedWorker` struct owns a Python worker child process, manages its
//! Unix socket / Windows named pipe IPC, and runs writer/reader tasks that
//! translate between Rust channel messages and msgpack-framed IPC protocol bytes.

use anvilml_core::{
    config::ServerConfig, types::worker::WorkerStatus, AnvilError, GpuDevice, WorkerInfo,
};
use anvilml_ipc::{framing, WorkerEvent, WorkerMessage};
use interprocess::local_socket::tokio::prelude::*;
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
use interprocess::local_socket::{ListenerOptions, ToFsName};
#[cfg(windows)]
use interprocess::os::windows::local_socket::NamedPipe;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::build_worker_env;

/// Shared IPC read/write handles for the writer/reader tasks.
#[derive(Debug)]
struct IpcHandles {
    reader: interprocess::local_socket::tokio::RecvHalf,
    writer: interprocess::local_socket::tokio::SendHalf,
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
    tx: mpsc::Sender<WorkerMessage>,
    /// Broadcast sender for events emitted by the reader task.
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
    /// Child process handle (Some while alive), wrapped in Arc for shared access
    /// by both the spawn path and the keepalive watchdog task.
    child: Arc<Mutex<Option<tokio::process::Child>>>,
    /// Join handle for the combined writer+reader task loop.
    #[allow(dead_code)]
    handle: JoinHandle<()>,
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
    /// IPC socket path used for logging and cleanup.
    ipc_socket_path: Arc<std::sync::Mutex<String>>,
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
            handle,
            ipc_tx: std::sync::Mutex::new(Some(ipc_tx)),
            ping_interval,
            pong_timeout,
            respawn_delay_ms,
            generation: Arc::new(AtomicU64::new(0)),
            ipc_socket_path: Arc::new(std::sync::Mutex::new(String::new())),
        }
    }

    /// Spawn the Python worker child process.
    ///
    /// Resolves the Python interpreter path from the config's `venv_path`,
    /// builds the command with environment variables from
    /// `build_worker_env`, and connects to the worker via a Unix socket
    /// (Linux/macOS) or Windows named pipe.
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

        // Build socket path for IPC.
        let socket_path = build_socket_path(self.device_index, self.worker_id.clone());
        let socket_path_str = socket_path.to_string_lossy().into_owned();
        *self.ipc_socket_path.lock().unwrap() = socket_path_str.clone();

        // Build the command.
        let mut cmd = Command::new(&python_path);
        cmd.arg("worker/worker_main.py")
            .arg("--worker-id")
            .arg(&self.worker_id)
            .arg("--device-index")
            .arg(self.device_index.to_string())
            .current_dir(_repo_root_for_worker())
            .envs(build_worker_env(device, cfg, &socket_path_str))
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

        // Create parent directory for Unix socket (Windows named pipe path
        // doesn't require a parent directory).
        #[cfg(unix)]
        {
            if let Some(parent) = socket_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    AnvilError::Io(std::io::Error::other(format!(
                        "failed to create socket dir: {e}"
                    )))
                })?;
            }
        }

        // Bind the local socket listener.
        let name = to_socket_name(&socket_path).map_err(AnvilError::Io)?;
        let listener = ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| {
                AnvilError::Io(std::io::Error::other(format!(
                    "failed to bind IPC socket: {e}"
                )))
            })?;

        info!(socket_path = %socket_path_str, "bound IPC socket");

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

        // Accept the worker's connection with a 10s timeout.
        let stream = tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
            .await
            .map_err(|_| {
                warn!(
                    worker_id = %self.worker_id,
                    socket_path = %socket_path_str,
                    "IPC accept timed out — worker did not connect"
                );
                AnvilError::Io(std::io::Error::other("IPC accept timed out"))
            })?
            .map_err(|e| {
                warn!(
                    worker_id = %self.worker_id,
                    error = %e,
                    "failed to accept IPC connection"
                );
                AnvilError::Io(e)
            })?;

        debug!(
            worker_id = %self.worker_id,
            socket_path = %socket_path_str,
            "IPC connection accepted"
        );

        let (reader, writer) = stream.split();

        // Deliver IPC handles to run_loop immediately — before any data is sent.
        // This ensures the reader task registers the socket for polling (Linux) or
        // I/O completion ports (Windows) before InitializeHardware arrives,
        // preventing missed edge-triggered wakeups.
        {
            let mut guard = self.ipc_tx.lock().unwrap();
            if let Some(tx) = guard.take() {
                tx.send(IpcHandles { reader, writer }).map_err(|_| {
                    AnvilError::Io(std::io::Error::other("run_loop already exited"))
                })?;
            } else {
                return Err(AnvilError::Io(std::io::Error::other(
                    "IPC channel already consumed",
                )));
            }
        }

        // Send InitializeHardware through the mpsc channel. The writer_task
        // inside run_loop will serialize and write it to the socket after the
        // reader_task has already registered the socket for polling. This avoids
        // the race where data arrives before epoll registration.
        let init_msg = WorkerMessage::InitializeHardware {
            device_str: format!("{:?}:{}", device.device_type, self.device_index),
        };
        if let Err(e) = self.tx.send(init_msg).await {
            warn!(error = %e, worker_id = %self.worker_id, "failed to send InitializeHardware");
            return Err(AnvilError::Io(std::io::Error::other(
                "mpsc channel closed before InitializeHardware could be sent",
            )));
        }

        // Yield multiple times so writer_task can process the message, write it to
        // the socket, and reader_task can read the Ready response and update status.
        // Without these yields, spawn() would block the event loop in its polling
        // loop before writer_task/reader_task get scheduled.
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }

        // Wait for status to transition from Initializing to Idle.
        let timeout_duration = std::time::Duration::from_secs(10);
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
        self.tx.send(msg).await.map_err(|e| {
            warn!(error = %e, worker_id = %self.worker_id, "worker channel closed");
            AnvilError::WorkerDead(format!("send failed: {}", e))
        })
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
        let tx = self.tx.clone();
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
    /// Delivers the provided reader/writer handles to the run_loop via the oneshot
    /// channel, bypassing the actual child process spawn. The caller is responsible
    /// for ensuring the handles are compatible with the framing protocol.
    #[cfg(test)]
    pub async fn inject_handles_for_test(
        &self,
        reader: interprocess::local_socket::tokio::RecvHalf,
        writer: interprocess::local_socket::tokio::SendHalf,
    ) {
        let mut guard = self.ipc_tx.lock().unwrap();
        if let Some(tx) = guard.take() {
            tx.send(IpcHandles { reader, writer })
                .expect("run_loop alive");
        }
    }

    /// Reset the IPC oneshot channel for respawn.
    ///
    /// Creates a fresh `(oneshot::Sender<IpcHandles>, oneshot::Receiver<IpcHandles>)`
    /// pair and stores the sender. The old receiver (if any was consumed) is gone;
    /// the run_loop will wait for the new one.
    #[allow(dead_code)]
    fn reset_ipc_tx(&mut self) {
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

        self.tx = tx;
        *self.ipc_tx.lock().unwrap() = Some(ipc_tx);
        self.handle = new_handle;
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
        self.reset_ipc_tx();

        // Spawn the Python worker child process.
        self.spawn(device, cfg).await?;

        info!(
            worker_id = %self.worker_id,
            "worker respawned"
        );

        Ok(())
    }

    /// The main event loop: waits for IPC handles via oneshot, then runs
    /// writer and reader tasks concurrently.
    async fn run_loop(
        worker_id: String,
        rx: mpsc::Receiver<WorkerMessage>,
        event_tx: broadcast::Sender<(String, WorkerEvent)>,
        status: Arc<RwLock<WorkerStatus>>,
        ipc_rx: oneshot::Receiver<IpcHandles>,
    ) {
        // Wait for IPC handles from spawn().
        let IpcHandles { reader, writer } = match ipc_rx.await {
            Ok(handles) => handles,
            Err(_) => {
                warn!(worker_id = %worker_id, "IPC channel closed before handles received");
                return;
            }
        };

        let writer_handle = spawn(writer_task(
            worker_id.clone(),
            rx,
            writer,
            status.clone(),
            event_tx.clone(),
        ));

        let reader_handle = spawn(reader_task(worker_id, status, event_tx, reader));

        // Wait for both tasks to complete.
        let _ = tokio::join!(writer_handle, reader_handle);
    }
}

/// Write frames from messages received on the channel.
async fn writer_task(
    worker_id: String,
    mut rx: mpsc::Receiver<WorkerMessage>,
    mut writer: interprocess::local_socket::tokio::SendHalf,
    _status: Arc<RwLock<WorkerStatus>>,
    _event_tx: broadcast::Sender<(String, WorkerEvent)>,
) {
    while let Some(msg) = rx.recv().await {
        debug!(
            worker_id = %worker_id,
            message_type = ?msg_discriminant(&msg),
            "writing frame to worker"
        );
        if let Err(e) = framing::write_frame(&mut writer, &msg).await {
            warn!(error = %e, worker_id = %worker_id, "failed to write IPC frame");
            break;
        } else {
            // Flush after each write to ensure data reaches the Python worker.
            // Without flush, the OS may buffer the data and the reader won't see it.
            if let Err(e) = writer.flush().await {
                warn!(error = %e, worker_id = %worker_id, "failed to flush IPC frame");
                break;
            }
        }
    }

    debug!(worker_id = %worker_id, "writer task exiting");
}

/// Read frames from the worker connection and broadcast events.
async fn reader_task(
    worker_id: String,
    status: Arc<RwLock<WorkerStatus>>,
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
    mut reader: interprocess::local_socket::tokio::RecvHalf,
) {
    // Default IPC payload limit: 64 MiB.
    let max_mib = 64;

    loop {
        match framing::read_frame(&mut reader, max_mib).await {
            Ok(event) => {
                debug!(
                    worker_id = %worker_id,
                    event_type = ?event_discriminant(&event),
                    "received event from worker"
                );

                // Update status based on the event.
                update_status_from_event(&status, &event).await;

                let _ = event_tx.send((worker_id.clone(), event));
            }
            Err(AnvilError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                warn!(worker_id = %worker_id, "worker connection closed (EOF)");
                // Broadcast status change before exiting.
                let _ = event_tx.send((
                    worker_id.clone(),
                    WorkerEvent::WorkerStatusChanged {
                        status: WorkerStatus::Dead,
                    },
                ));
                break;
            }
            Err(e) => {
                warn!(error = %e, worker_id = %worker_id, "reader error");
                break;
            }
        }
    }

    warn!(worker_id = %worker_id, "reader task exiting — worker is Dead");
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

/// Build the IPC socket path for a worker.
///
/// On Unix: `{temp_dir}/anvilml-{pid}/worker-{index}-{uid}.sock`
/// On Windows: `\\.\pipe\anvilml-worker-{worker_id}-{index}-{pid}-{uid}`
///
/// A process-global atomic counter (`uid`) is appended to every path so that
/// each call produces a name that has never been used before in this process.
/// This prevents `CreateNamedPipe` / `bind` conflicts when a prior instance's
/// handles have not yet been fully released by the OS (common on Windows after
/// a worker death and respawn cycle).
fn build_socket_path(device_index: u32, worker_id: String) -> std::path::PathBuf {
    static PIPE_COUNTER: AtomicU64 = AtomicU64::new(0);
    let uid = PIPE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    if cfg!(windows) {
        std::path::PathBuf::from(format!(
            r"\\.\pipe\anvilml-worker-{worker_id}-{device_index}-{pid}-{uid}"
        ))
    } else {
        let dir = std::env::temp_dir().join(format!("anvilml-{pid}"));
        dir.join(format!("worker-{device_index}-{uid}.sock"))
    }
}

/// Convert a socket path to an `interprocess` local socket name.
///
/// On Unix, socket paths are filesystem paths and use `GenericFilePath`.
/// On Windows, socket paths are named pipes and use `GenericNamespaced`.
fn to_socket_name(
    path: &std::path::Path,
) -> Result<interprocess::local_socket::Name<'_>, std::io::Error> {
    #[cfg(unix)]
    {
        path.to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        path.to_fs_name::<NamedPipe>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::time::{timeout, Duration};

    // Re-export rmp_serde for test serialization.
    use rmp_serde;

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

    /// Verify that EOF on the pipe sets status to Dead.
    #[tokio::test]
    #[cfg(feature = "mock-hardware")]
    async fn eof_sets_dead() {
        // Create a duplex pipe to simulate EOF.
        let (mut tx, mut rx) = tokio::io::duplex(4096);

        // Write a Ready frame using flat dict format (same as Python worker).
        let ready_json: serde_json::Map<String, serde_json::Value> = [
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
        let payload = rmp_serde::to_vec_named(&ready_json).expect("serialize");
        let len = payload.len() as u32;
        tx.write_all(&len.to_be_bytes())
            .await
            .expect("write header");
        tx.write_all(&payload).await.expect("write payload");
        drop(tx); // Close write side → EOF.

        // Read the Ready event and verify status transitioned to Idle.
        let event = timeout(Duration::from_secs(2), framing::read_frame(&mut rx, 64))
            .await
            .expect("read Ready frame")
            .expect("frame ok");

        assert!(matches!(event, WorkerEvent::Ready { .. }));
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
    #[tokio::test]
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
                let mut worker = ManagedWorker::new("respawn-test".to_string(), 0);

                // Build platform-appropriate socket paths for mock IPC handles.
                // On Windows a plain filesystem path is not a valid named pipe path;
                // \\.\pipe\ prefix is required. On Unix use the temp directory.
                #[cfg(windows)]
                let socket_path = std::path::PathBuf::from(format!(
                    r"\\.\pipe\anvilml-test-{}-respawn",
                    std::process::id()
                ));
                #[cfg(unix)]
                let socket_path = {
                    let p = std::env::temp_dir()
                        .join(format!("anvilml-test-{}-respawn", std::process::id()));
                    let _ = tokio::fs::remove_file(&p).await;
                    if let Some(dir) = p.parent() {
                        let _ = tokio::fs::create_dir_all(dir).await;
                    }
                    p
                };

                let name = to_socket_name(&socket_path).expect("convert socket path to name");
                let listener = ListenerOptions::new()
                    .name(name)
                    .create_tokio()
                    .expect("bind test socket");
                let (server_stream, _client) = tokio::join!(
                    async {
                        let stream =
                            tokio::time::timeout(Duration::from_secs(5), listener.accept())
                                .await
                                .expect("accept")
                                .expect("accept ok");
                        stream
                    },
                    async {
                        tokio::time::timeout(
                            Duration::from_secs(5),
                            LocalSocketStream::connect(
                                to_socket_name(&socket_path).expect("convert to name"),
                            ),
                        )
                        .await
                        .expect("connect")
                        .expect("connect ok")
                    },
                );

                let (reader, writer) = server_stream.split();

                // Inject mock handles so the run_loop can start.
                worker.inject_handles_for_test(reader, writer).await;

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
                #[cfg(windows)]
                let socket_path2 = std::path::PathBuf::from(format!(
                    r"\\.\pipe\anvilml-test-{}-respawn2",
                    std::process::id()
                ));
                #[cfg(unix)]
                let socket_path2 = {
                    let p = std::env::temp_dir()
                        .join(format!("anvilml-test-{}-respawn2", std::process::id()));
                    let _ = tokio::fs::remove_file(&p).await;
                    if let Some(dir) = p.parent() {
                        let _ = tokio::fs::create_dir_all(dir).await;
                    }
                    p
                };

                let name2 = to_socket_name(&socket_path2).expect("convert socket path to name");
                let listener2 = ListenerOptions::new()
                    .name(name2)
                    .create_tokio()
                    .expect("bind respawn socket");
                let (server_stream2, _client2) = tokio::join!(
                    async {
                        let stream =
                            tokio::time::timeout(Duration::from_secs(5), listener2.accept())
                                .await
                                .expect("accept")
                                .expect("accept ok");
                        stream
                    },
                    async {
                        tokio::time::timeout(
                            Duration::from_secs(5),
                            LocalSocketStream::connect(
                                to_socket_name(&socket_path2).expect("convert to name"),
                            ),
                        )
                        .await
                        .expect("connect")
                        .expect("connect ok")
                    },
                );

                let (reader2, writer2) = server_stream2.split();

                // Reset IPC channel and inject fresh handles.
                worker.reset_ipc_tx();
                worker.inject_handles_for_test(reader2, writer2).await;

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
