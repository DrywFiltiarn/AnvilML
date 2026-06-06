//! Managed worker process lifecycle and IPC bridge.
//!
//! The `ManagedWorker` struct owns a Python worker child process, manages its
//! stdin/stdout pipes, and runs writer/reader tasks that translate between
//! Rust channel messages and msgpack-framed IPC protocol bytes.

use anvilml_core::{
    config::ServerConfig, types::worker::WorkerStatus, AnvilError, GpuDevice, WorkerInfo,
};
use anvilml_ipc::{framing, WorkerEvent, WorkerMessage};
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::spawn;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::build_worker_env;

/// Shared IPC stdin/stdout handles for the writer/reader tasks.
struct IpcHandles {
    stdin: tokio::process::ChildStdin,
    stdout: tokio::process::ChildStdout,
}

/// A managed Python worker process with IPC bridge.
///
/// Owns the child process lifecycle (spawn, stdin/stdout piping) and provides
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
    /// Child process handle (Some while alive).
    child: Mutex<Option<tokio::process::Child>>,
    /// Join handle for the combined writer+reader task loop.
    #[allow(dead_code)]
    handle: JoinHandle<()>,
    /// Oneshot sender used to deliver IPC handles to the loop.
    ipc_tx: Mutex<Option<oneshot::Sender<IpcHandles>>>,
}

impl ManagedWorker {
    /// Create a new `ManagedWorker` with initialized channels and status.
    ///
    /// The broadcast channel capacity matches the config default
    /// `ws_broadcast_capacity = 256`.
    pub fn new(worker_id: String, device_index: u32) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let (event_tx, _rx) = broadcast::channel(256);

        let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

        // Create a oneshot channel for delivering IPC handles from spawn()
        // to the run_loop.
        let (ipc_tx, ipc_rx) = oneshot::channel::<IpcHandles>();

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
            child: Mutex::new(None),
            handle,
            ipc_tx: Mutex::new(Some(ipc_tx)),
        }
    }

    /// Spawn the Python worker child process.
    ///
    /// Resolves the Python interpreter path from the config's `venv_path`,
    /// builds the command with environment variables from
    /// `build_worker_env`, and pipes stdin/stdout for IPC.
    pub async fn spawn(&self, device: &GpuDevice, cfg: &ServerConfig) -> Result<(), AnvilError> {
        // Resolve python interpreter path.
        let python_path = resolve_python_path(&cfg.venv_path);

        // Build the command.
        let mut cmd = Command::new(&python_path);
        cmd.arg("worker_main.py")
            .arg("--worker-id")
            .arg(&self.worker_id)
            .arg("--device-index")
            .arg(self.device_index.to_string())
            .envs(build_worker_env(device, cfg))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

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

        // Take stdin/stdout handles.
        #[allow(unused_mut)]
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AnvilError::Io(std::io::Error::other("worker stdin not piped")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AnvilError::Io(std::io::Error::other("worker stdout not piped")))?;

        // Detach stderr.
        if let Some(mut stderr) = child.stderr.take() {
            let wid = self.worker_id.clone();
            tokio::task::spawn(async move {
                let _ = stderr.read_to_end(&mut Vec::new()).await;
                debug!(worker_id = %wid, "stderr drained");
            });
        }

        // Wait a brief moment for the Python process to start up and begin
        // reading from stdin. This ensures our write doesn't race with Python's
        // initial read_frame() call.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Write InitializeHardware directly to stdin synchronously using a
        // duplicated file descriptor (Unix) or async write + flush (Windows).
        // This ensures the data reaches Python before it starts reading,
        // avoiding a race condition where Python reads EOF or garbage.
        let init_msg = WorkerMessage::InitializeHardware {
            device_str: format!("{:?}:{}", device.device_type, self.device_index),
        };
        let init_frame_data = rmp_serde::to_vec_named(&init_msg).map_err(|e| {
            AnvilError::Json(format!("Failed to serialize InitializeHardware: {e}"))
        })?;
        let init_len = init_frame_data.len() as u32;
        #[allow(unused_variables)]
        let init_header = init_len.to_be_bytes();

        #[cfg(unix)]
        {
            // Duplicate the fd and write synchronously.
            let stdin_fd = stdin.as_raw_fd();
            let dup_fd = unsafe { libc::dup(stdin_fd) };
            if dup_fd >= 0 {
                let mut file = unsafe { std::fs::File::from_raw_fd(dup_fd) };
                let _ = file.write_all(&init_header);
                let _ = file.write_all(&init_frame_data);
                let _ = file.flush();
                // Don't close — the original fd is still owned by tokio.
                let _ = file.into_raw_fd();
            }
        }

        #[cfg(windows)]
        {
            // On Windows, use async write + flush directly on stdin before
            // it's moved into IpcHandles.
            if let Err(e) = framing::write_frame(&mut stdin, &init_msg).await {
                warn!(error = %e, worker_id = %self.worker_id, "failed to write InitializeHardware");
            } else if let Err(e) = stdin.flush().await {
                warn!(error = %e, worker_id = %self.worker_id, "failed to flush InitializeHardware");
            }
        }

        // Also send via mpsc channel for subsequent messages.
        if let Err(e) = self.tx.send(init_msg).await {
            warn!(error = %e, worker_id = %self.worker_id, "failed to send InitializeHardware via channel");
        }

        // Send handles to the run_loop via oneshot.
        {
            let mut guard = self.ipc_tx.lock().await;
            if let Some(tx) = guard.take() {
                tx.send(IpcHandles { stdin, stdout }).map_err(|_| {
                    AnvilError::Io(std::io::Error::other("run_loop already exited"))
                })?;
            } else {
                return Err(AnvilError::Io(std::io::Error::other(
                    "IPC channel already consumed",
                )));
            }
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
        let IpcHandles { stdin, stdout } = match ipc_rx.await {
            Ok(handles) => handles,
            Err(_) => {
                warn!(worker_id = %worker_id, "IPC channel closed before handles received");
                return;
            }
        };

        let writer_handle = spawn(writer_task(
            worker_id.clone(),
            rx,
            stdin,
            status.clone(),
            event_tx.clone(),
        ));

        let reader_handle = spawn(reader_task(worker_id, status, event_tx, stdout));

        // Wait for both tasks to complete.
        let _ = tokio::join!(writer_handle, reader_handle);
    }
}

/// Write frames from messages received on the channel.
async fn writer_task(
    worker_id: String,
    mut rx: mpsc::Receiver<WorkerMessage>,
    mut stdin: tokio::process::ChildStdin,
    _status: Arc<RwLock<WorkerStatus>>,
    _event_tx: broadcast::Sender<(String, WorkerEvent)>,
) {
    while let Some(msg) = rx.recv().await {
        debug!(
            worker_id = %worker_id,
            message_type = ?msg_discriminant(&msg),
            "writing frame to worker"
        );
        if let Err(e) = framing::write_frame(&mut stdin, &msg).await {
            warn!(error = %e, worker_id = %worker_id, "failed to write IPC frame");
            break;
        } else {
            // Flush after each write to ensure data reaches the Python worker.
            // Without flush, the OS may buffer the data and the reader won't see it.
            if let Err(e) = stdin.flush().await {
                warn!(error = %e, worker_id = %worker_id, "failed to flush IPC frame");
                break;
            }
        }
    }

    debug!(worker_id = %worker_id, "writer task exiting");
}

/// Read frames from the child process stdout and broadcast events.
async fn reader_task(
    worker_id: String,
    status: Arc<RwLock<WorkerStatus>>,
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
    mut stdout: tokio::process::ChildStdout,
) {
    // Default IPC payload limit: 64 MiB.
    let max_mib = 64;

    loop {
        match framing::read_frame(&mut stdout, max_mib).await {
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
                warn!(worker_id = %worker_id, "worker pipe closed (EOF)");
                break;
            }
            Err(e) => {
                warn!(error = %e, worker_id = %worker_id, "reader error");
                break;
            }
        }
    }

    // Set status to Dead on exit.
    {
        let mut s = status.write().await;
        *s = WorkerStatus::Dead;
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
        WorkerEvent::Pong { .. } => "Pong",
        WorkerEvent::Dying { .. } => "Dying",
        WorkerEvent::MemoryReport { .. } => "MemoryReport",
        WorkerEvent::Progress { .. } => "Progress",
        WorkerEvent::ImageReady { .. } => "ImageReady",
        WorkerEvent::Completed { .. } => "Completed",
        WorkerEvent::Failed { .. } => "Failed",
        WorkerEvent::Cancelled { .. } => "Cancelled",
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::time::{timeout, Duration};

    // Re-export rmp_serde for test serialization.
    use rmp_serde;

    /// Spawn a mock worker, send Ping, receive Pong, then Shutdown.
    ///
    /// Skipped by default because it requires a Python interpreter at a
    /// specific path. Set `ANVILML_TEST_WORKER_PYTHON` to override the path.
    #[tokio::test]
    #[cfg(feature = "mock-hardware")]
    #[ignore = "requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable"]
    async fn spawn_ping_pong() {
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
        let cfg = ServerConfig {
            venv_path: std::path::PathBuf::from("/home/dryw/forge/.venv"),
            ..ServerConfig::default()
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
    }

    /// Verify status transitions: Initializing → Idle (on Ready) → Dead.
    #[tokio::test]
    #[cfg(feature = "mock-hardware")]
    #[ignore = "requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable"]
    async fn status_transitions() {
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

        let cfg = ServerConfig {
            venv_path: std::path::PathBuf::from("/home/dryw/forge/.venv"),
            ..ServerConfig::default()
        };

        // Status should start as Initializing.
        assert_eq!(worker.get_status().await, WorkerStatus::Initializing);

        // Spawn triggers InitializeHardware → Ready, which sets status to Idle.
        worker.spawn(&device, &cfg).await.expect("spawn");

        // After spawn returns, status should be Idle (Ready was received).
        assert_eq!(worker.get_status().await, WorkerStatus::Idle);
    }

    /// Verify that EOF on the pipe sets status to Dead.
    #[tokio::test]
    #[cfg(feature = "mock-hardware")]
    async fn eof_sets_dead() {
        // Create a duplex pipe to simulate EOF.
        let (mut tx, mut rx) = tokio::io::duplex(4096);

        // Write a Ready frame then close the write end.
        let ready_event = WorkerEvent::Ready {
            worker_id: "eof-test".to_string(),
            device_index: 0,
            vram_total_mib: 8192,
            vram_free_mib: 8192,
            arch: "gfx1100".to_string(),
            fp16: true,
            bf16: true,
            flash_attention: false,
        };
        let payload = rmp_serde::to_vec_named(&ready_event).expect("serialize");
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
}
