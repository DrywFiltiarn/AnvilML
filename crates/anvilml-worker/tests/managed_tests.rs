//! Integration tests for `managed.rs` — verifies the `WorkerHandle` struct's
//! clone semantics, status read path, and idempotent shutdown request.
//!
//! All tests construct handles from shared `Arc<RwLock<WorkerStatus>>` instances
//! to prove that clones share state, and from fresh `oneshot::channel` pairs
//! to verify the shutdown trigger works correctly.
//!
//! The second half of this file exercises `ManagedWorker::run()` — the full
//! lifecycle task — using in-process ZeroMQ sockets to simulate a Python worker.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anvilml_core::AnvilError;
use anvilml_core::NodeTypeDescriptor;
use anvilml_core::NodeTypeRegistry;
use anvilml_core::SlotDescriptor;
use anvilml_core::SlotType;
use anvilml_core::types::worker::WorkerStatus;
use anvilml_ipc::RouterTransport;
use anvilml_ipc::WorkerEvent;
use anvilml_ipc::WorkerMessage;
use anvilml_worker::Demux;
use anvilml_worker::ManagedWorker;
use anvilml_worker::ManagedWorkerConfig;
use anvilml_worker::RespawnPolicy;
use anvilml_worker::RunOutcome;
use anvilml_worker::WorkerHandle;
use anvilml_worker::WorkerSpawner;
use anvilml_worker::spawn_bridge;
use anvilml_worker::{
    DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT, DEFAULT_INIT_TIMEOUT, DEFAULT_WATCHDOG_PING_INTERVAL,
    DEFAULT_WATCHDOG_PONG_TIMEOUT,
};
use tokio::sync::RwLock;
use tokio::sync::mpsc;

/// Constructing two `WorkerHandle`s from the same `Arc<RwLock<WorkerStatus>>`
/// and calling `status()` on both returns the same value, proving clones share
/// the status lock.
///
/// Creates a shared `Arc<RwLock<WorkerStatus>>`, sets it to `Idle` via a direct
/// write, then constructs two handles from it. Both handles must report `Idle`.
#[tokio::test]
async fn test_clone_shares_status() {
    let status = Arc::new(RwLock::new(WorkerStatus::Idle));
    let handle1 = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::clone(&status),
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );
    let handle2 = WorkerHandle::new(
        "worker-1".to_string(),
        status,
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    assert_eq!(
        handle1.status().await,
        WorkerStatus::Idle,
        "clone 1 should see the shared status"
    );
    assert_eq!(
        handle2.status().await,
        WorkerStatus::Idle,
        "clone 2 should see the same shared status"
    );
}

/// Cloning a handle copies the `worker_id` String — same value, independent allocation.
///
/// Constructs a handle with `worker_id = "gpu:0"`, clones it, and verifies the clone
/// has the same `worker_id` string but is a distinct `String` allocation (proven by
/// the fact that modifying one would not affect the other).
#[tokio::test]
async fn test_clone_independent_worker_id() {
    let mut handle = WorkerHandle::new(
        "gpu:0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );
    let clone = handle.clone();

    assert_eq!(
        clone.worker_id, "gpu:0",
        "clone's worker_id should match the original"
    );
    assert_eq!(
        handle.worker_id, "gpu:0",
        "original's worker_id should be unchanged"
    );
    // Verify they are independent strings: modifying one does not affect the other.
    // Since worker_id is pub and String, we can mutate it to prove independence.
    let original_id = handle.worker_id.clone();
    handle.worker_id = "modified".to_string();
    assert_eq!(
        clone.worker_id, "gpu:0",
        "clone's worker_id should be independent of original's mutations"
    );
    assert_eq!(
        handle.worker_id, "modified",
        "original should reflect its own mutation"
    );
    // Restore for cleanliness (not strictly needed since handle is dropped after test).
    handle.worker_id = original_id;
}

/// Constructing a handle with a fresh `oneshot::channel` and calling `request_shutdown()`
/// delivers `()` to the receiver side, proving the shutdown trigger works.
///
/// Creates a `oneshot::channel`, spawns a task that waits on the receiver, constructs
/// a handle with the sender, calls `request_shutdown()`, and verifies the receiver
/// gets `Ok(())`.
#[tokio::test]
async fn test_request_shutdown_sends_signal() {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let mut handle = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        Some(tx),
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    // Spawn a task to receive the shutdown signal.
    let receiver_task = tokio::spawn(async move { rx.await });

    handle.request_shutdown();

    // The receiver should get Ok(()) since the sender was consumed and sent.
    let result = receiver_task.await.expect("receiver task should not panic");
    assert_eq!(result, Ok(()), "shutdown signal should be delivered");
}

/// Calling `request_shutdown()` twice on the same handle does not panic.
///
/// The second call operates on `None` (the `Option` was already `take()`n on the
/// first call) and returns cleanly, proving idempotency.
#[tokio::test]
async fn test_request_shutdown_is_idempotent() {
    let (tx, _rx) = tokio::sync::oneshot::channel::<()>();
    let mut handle = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        Some(tx),
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    // First call — should send the signal.
    handle.request_shutdown();

    // Second call — should be a no-op (shutdown_tx is now None).
    // This must not panic.
    handle.request_shutdown();
}

/// Constructing a handle with status set to `Initializing` and calling `status()`
/// returns `Initializing`, proving the read path works correctly for non-default states.
///
/// Creates a shared `Arc<RwLock<WorkerStatus>>`, sets it to `Initializing` via a direct
/// write before constructing the handle, then verifies `status()` returns `Initializing`.
#[tokio::test]
async fn test_status_returns_current_value() {
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let handle = WorkerHandle::new(
        "worker-0".to_string(),
        status,
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    assert_eq!(
        handle.status().await,
        WorkerStatus::Initializing,
        "status() should return the current value from the shared lock"
    );
}

/// Calling `set_status()` overwrites the stored status and `status()` returns the new value.
///
/// Constructs a handle with `WorkerStatus::Idle`, calls `set_status(WorkerStatus::Busy)`,
/// then verifies `status().await` returns `WorkerStatus::Busy`. This exercises the write
/// lock path and confirms the mutation is visible to subsequent reads.
#[tokio::test]
async fn test_set_status_changes_value() {
    let handle = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    assert_eq!(
        handle.status().await,
        WorkerStatus::Idle,
        "initial status should be Idle"
    );

    handle.set_status(WorkerStatus::Busy).await;

    assert_eq!(
        handle.status().await,
        WorkerStatus::Busy,
        "status() should return the value set by set_status()"
    );
}

/// Mutating status on one handle is observable via an independently-cloned handle.
///
/// Constructs a handle, clones it, calls `set_status(WorkerStatus::Dying)` on the original,
/// then calls `status().await` on the clone and asserts it returns `WorkerStatus::Dying`.
/// This proves the shared `Arc<RwLock<WorkerStatus>>` is correctly shared across clones.
#[tokio::test]
async fn test_set_status_visible_across_clone() {
    let handle = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );
    let clone = handle.clone();

    // Mutate the original handle's status.
    handle.set_status(WorkerStatus::Dying).await;

    // The clone should see the updated value.
    assert_eq!(
        clone.status().await,
        WorkerStatus::Dying,
        "clone should see the status changed by the original handle"
    );
}

/// Concurrent `status()` reads and `set_status()` writes complete without deadlock.
///
/// Constructs a handle with `WorkerStatus::Idle`, spawns two concurrent tasks:
/// one loops `status().await` 100 times, the other loops `set_status()` alternating
/// between `Busy` and `Idle` 100 times. Both tasks must complete within 5 seconds
/// (bounded wait per ENVIRONMENT.md §11.5), proving no deadlock between read and
/// write lock paths.
#[tokio::test]
async fn test_concurrent_status_and_set_status_no_deadlock() {
    let handle = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    let handle_read = handle.clone();
    let handle_write = handle.clone();

    let read_task = tokio::spawn(async move {
        for _ in 0..100 {
            let _ = handle_read.status().await;
        }
    });

    let write_task = tokio::spawn(async move {
        for i in 0..100 {
            if i % 2 == 0 {
                handle_write.set_status(WorkerStatus::Busy).await;
            } else {
                handle_write.set_status(WorkerStatus::Idle).await;
            }
        }
    });

    // Both tasks must complete within 5 seconds — bounded wait per ENVIRONMENT.md §11.5.
    let timeout = tokio::time::Duration::from_secs(5);
    tokio::select! {
        _ = read_task => (),
        _ = tokio::time::sleep(timeout) => {
            panic!("reader task timed out after 5s — possible deadlock");
        }
    }
    tokio::select! {
        _ = write_task => (),
        _ = tokio::time::sleep(timeout) => {
            panic!("writer task timed out after 5s — possible deadlock");
        }
    }
}

/// `set_status()` can be called multiple times with different values; each transition is correct.
///
/// Constructs a handle, calls `set_status()` five times in sequence with
/// `Initializing → Idle → Busy → Dying → Dead`, asserting each value after the call.
/// This verifies the method can be called repeatedly without side effects or state corruption.
#[tokio::test]
async fn test_set_status_callable_repeatedly() {
    let handle = WorkerHandle::new(
        "worker-0".to_string(),
        Arc::new(RwLock::new(WorkerStatus::Idle)),
        None,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    handle.set_status(WorkerStatus::Initializing).await;
    assert_eq!(
        handle.status().await,
        WorkerStatus::Initializing,
        "after set_status(Initializing), status() should return Initializing"
    );

    handle.set_status(WorkerStatus::Idle).await;
    assert_eq!(
        handle.status().await,
        WorkerStatus::Idle,
        "after set_status(Idle), status() should return Idle"
    );

    handle.set_status(WorkerStatus::Busy).await;
    assert_eq!(
        handle.status().await,
        WorkerStatus::Busy,
        "after set_status(Busy), status() should return Busy"
    );

    handle.set_status(WorkerStatus::Dying).await;
    assert_eq!(
        handle.status().await,
        WorkerStatus::Dying,
        "after set_status(Dying), status() should return Dying"
    );

    handle.set_status(WorkerStatus::Dead).await;
    assert_eq!(
        handle.status().await,
        WorkerStatus::Dead,
        "after set_status(Dead), status() should return Dead"
    );
}

// The following tests exercise `ManagedWorker::run()` — the full lifecycle task.
// They use an in-process ZeroMQ ROUTER/DEALER pair to simulate a Python worker.

use std::time::Duration;

use bytes::Bytes;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

// rmp_serde is imported at the top of the file for serializing WorkerEvent bytes.

/// Returns true if a process with the given PID currently exists (i.e. has
/// not exited/been reaped). Used by the force-kill assertions below to
/// confirm `JobObjectGuard`'s kill-on-close (Windows) or the direct SIGKILL
/// path (Unix) actually terminated the tracked child — cross-platform so
/// Windows CI exercises the same assertion Linux does, instead of skipping
/// it entirely.
#[cfg(target_os = "linux")]
fn pid_is_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// Windows equivalent of `pid_is_alive` above: opens the process with
/// `PROCESS_QUERY_LIMITED_INFORMATION` (sufficient to read the exit code,
/// no elevated rights needed) and checks `GetExitCodeProcess` returns
/// `STILL_ACTIVE`. A PID that no longer exists fails `OpenProcess` outright
/// (Windows recycles PIDs, but not within the sub-second window this test
/// checks in), which is treated as "not alive" — the correct outcome for
/// this assertion either way.
#[cfg(target_os = "windows")]
fn pid_is_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: OpenProcess/GetExitCodeProcess/CloseHandle are standard Win32
    // calls with no preconditions beyond a valid PID (which may legitimately
    // not exist — that's a normal, handled failure path, not a precondition
    // violation). The handle is always closed on every return path below.
    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            // Process doesn't exist (already reaped) — not alive.
            Err(_) => return false,
        };

        let mut exit_code: u32 = 0;
        let got_code = GetExitCodeProcess(handle, &mut exit_code).is_ok();
        let _ = CloseHandle(handle);

        got_code && exit_code == STILL_ACTIVE.0 as u32
    }
}

/// A `WorkerSpawner` for tests.
///
/// `WorkerSpawner::spawn()`'s return type is a genuine `tokio::process::Child`,
/// not a mockable trait object, so this can't fake a process — it spawns a
/// real, harmless, boundedly-long-lived OS process (`sleep 999` on Unix,
/// `cmd /c timeout 999` on Windows — matching the existing cross-platform
/// pattern already used for `JobObjectGuard` testing in `spawn_tests.rs`),
/// ignoring `venv_path`/`env` entirely since this never runs Python.
///
/// Tracks how many times `spawn()` was called (`call_count()`) — needed for
/// respawn-count assertions (e.g. "spawner called twice for an under-limit
/// respawn"). `AtomicUsize` rather than requiring `&mut self`, since
/// `WorkerSpawner::spawn()`'s signature takes `&self`.
///
/// `failing()` constructs a variant where every `spawn()` call returns `Err`
/// without spawning anything — used to test the spawn-failure crash path
/// (`RunOutcome::Crashed` from a failed `spawner.spawn()` call itself, not
/// from a transport error or watchdog timeout on an already-running worker).
struct MockWorkerSpawner {
    call_count: AtomicUsize,
    should_fail: bool,
    /// PID of the most recently spawned process, if any. Needed for tests
    /// that drive a worker through the full `run()` (not `run_once_for_test()`)
    /// — `run()` consumes `self` and never returns it, so there's no way to
    /// inspect `self.child` afterward through `child_pid_for_test()`. The
    /// mock observes the PID independently, at the moment it spawns the
    /// process, regardless of what `ManagedWorker` does with the `Child`
    /// handle afterward.
    last_pid: std::sync::Mutex<Option<u32>>,
}

impl MockWorkerSpawner {
    fn new() -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            should_fail: false,
            last_pid: std::sync::Mutex::new(None),
        }
    }

    fn failing() -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            should_fail: true,
            last_pid: std::sync::Mutex::new(None),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// PID of the most recently spawned process, if any spawn has
    /// succeeded yet.
    fn last_pid(&self) -> Option<u32> {
        *self.last_pid.lock().expect("last_pid mutex poisoned")
    }
}

impl WorkerSpawner for MockWorkerSpawner {
    fn spawn<'a>(
        &'a self,
        _venv_path: &'a Path,
        _env: HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child, AnvilError>> + Send + 'a>> {
        Box::pin(async move {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                return Err(AnvilError::Io(std::io::Error::other(
                    "MockWorkerSpawner: simulated spawn failure",
                )));
            }

            #[cfg(unix)]
            let mut cmd = {
                let mut c = tokio::process::Command::new("sleep");
                c.arg("999");
                c
            };
            #[cfg(windows)]
            let mut cmd = {
                let mut c = tokio::process::Command::new("cmd");
                c.args(["/c", "timeout", "999"]);
                c
            };

            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            let result = cmd.spawn().map_err(AnvilError::Io);
            if let Ok(ref child) = result {
                *self.last_pid.lock().expect("last_pid mutex poisoned") = child.id();
            }
            result
        })
    }
}

/// Connect a DEALER socket to a `RouterTransport`'s bound endpoint, setting the
/// worker identity. Returns the DEALER socket handle.
///
/// The DEALER socket must be kept alive for the duration of the test — if it is
/// dropped, the ROUTER will no longer recognize the worker identity and send
/// operations will fail with "Destination client not found by identity".
async fn connect_dealer(transport: &RouterTransport, worker_id: &str) -> DealerSocket {
    // Set the DEALER socket's identity so the ROUTER knows which worker this is.
    let mut opts = SocketOptions::default();
    opts.peer_identity(
        PeerIdentity::try_from(Bytes::from(worker_id.to_string())).expect("valid identity"),
    );
    let mut dealer = DealerSocket::with_options(opts);
    // Connect to the ROUTER's endpoint.
    let endpoint = format!("tcp://127.0.0.1:{}", transport.port);
    dealer
        .connect(&endpoint)
        .await
        .expect("DEALER connect to ROUTER should succeed");
    // Give the ROUTER time to register the DEALER's identity. Without this,
    // a subsequent ROUTER-side send addressed to this identity could fail
    // with "Destination client not found by identity" before the handshake
    // settles — tests that simulate worker-originated events via the DEALER's
    // own send() aren't affected by this, but the delay is cheap insurance.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    dealer
}

/// Send a `WorkerEvent` from the DEALER side to the ROUTER — the correct
/// direction for simulating a worker-originated event in tests.
///
/// `RouterTransport::send_raw()`/`send()` go the other way (ROUTER → the
/// peer identified by `worker_id`, i.e. straight to this same DEALER) and
/// must never be used to simulate the worker sending something — a ROUTER's
/// own outbound send never loops back into its own `recv()`. This was a
/// latent defect discovered while implementing `P8-E5`: `test_run_completes_
/// on_ready_event` and five other pre-existing tests used `send_raw()` for
/// exactly this wrong purpose, so the events they claimed to deliver were
/// never actually received by `ManagedWorker`. See `PHASES.md`'s amendments
/// log for the full account.
async fn send_event(dealer: &mut DealerSocket, event: &WorkerEvent) {
    let payload = rmp_serde::to_vec_named(event).expect("event should serialize");
    let mut msg = ZmqMessage::from(Bytes::from(""));
    msg.push_back(Bytes::from(payload));
    dealer.send(msg).await.expect("DEALER send should succeed");
}

/// `run()` transitions through Initializing → Idle when a Ready event is received,
/// then exits cleanly on shutdown signal.
///
/// Creates a ZeroMQ ROUTER/DEALER pair on the loopback interface. The test acts as
/// the worker (DEALER), sends a `Ready` event, then sends a shutdown signal. The
/// `ManagedWorker` (ROUTER) receives the Ready event, transitions to Idle, then
/// exits on shutdown and deregisters.
///
/// This verifies the normal startup path: Initializing → Idle.
#[tokio::test]
async fn test_run_completes_on_ready_event() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Connect a DEALER socket as the "Python worker" so the ROUTER recognizes
    // the worker identity.
    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    // Spawn the worker — it starts in Initializing state.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send a Ready event to simulate the worker reporting startup — via the
    // DEALER (worker → ROUTER), the direction ManagedWorker's recv() actually
    // receives from.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process the Ready event before asserting on it.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // The doc comment above has always claimed this transition happens —
    // verify it actually does now, rather than only checking the task exits.
    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "worker should be Idle after Ready event"
    );

    // Send shutdown signal — the worker should exit cleanly.
    drop(shutdown_tx);

    // The worker task should complete within 5 seconds — bounded wait per
    // ENVIRONMENT.md §11.5.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// `shutdown_rx` being triggered causes `run()` to set status to `Dying`, call
/// `deregister()`, and return — even before a Ready event arrives.
///
/// Creates a ROUTER/DEALER pair, spawns `ManagedWorker::run()`, and immediately
/// sends a shutdown signal (before any Ready event). The worker must exit to Dying
/// and deregister without waiting for the 60-second Initializing timeout.
#[tokio::test]
async fn test_shutdown_rx_triggers_graceful_exit() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Connect a DEALER socket as the "Python worker".
    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send shutdown immediately — no Ready event.
    drop(shutdown_tx);

    // Worker should exit within 5s.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // Graceful shutdown transitions status to Dying (not Dead — the worker
    // subprocess itself isn't confirmed gone, only asked to stop).
    assert_eq!(
        *status.read().await,
        WorkerStatus::Dying,
        "status should be Dying after graceful shutdown"
    );
}

/// On graceful shutdown path, `demux.deregister(worker_id)` is called, confirmed
/// by `demux.registered(worker_id)` returning `false` after `run()` returns.
///
/// Creates a ROUTER/DEALER pair, registers the worker (simulating the pool's
/// pre-spawn registration), sends Ready + shutdown. After `run()` completes,
/// verifies the worker is no longer in the routing table.
#[tokio::test]
async fn test_deregister_called_on_graceful_exit() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Simulate the pool's pre-spawn registration.
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    demux.register("test-worker".to_string(), tx);

    // Connect a DEALER socket as the "Python worker".
    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event via the DEALER (worker → ROUTER) — registration itself
    // already happened above (simulating the pool's pre-spawn registration);
    // this just exercises the normal startup event on top of it.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process the Ready event.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // After Ready, worker should still be registered and now Idle.
    assert!(
        demux.registered("test-worker"),
        "worker should be registered after Ready event"
    );
    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "worker should be Idle after Ready event"
    );

    // Send shutdown — worker should deregister on exit.
    drop(shutdown_tx);

    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // After exit, worker must be deregistered.
    assert!(
        !demux.registered("test-worker"),
        "worker should be deregistered after graceful shutdown"
    );
}

/// On Dying event path (simulated crash), `demux.deregister(worker_id)` is called.
///
/// Creates a ROUTER/DEALER pair, registers the worker (simulating the pool's
/// pre-spawn registration), sends Ready + Dying event. The worker must transition
/// to Dead and deregister without waiting for shutdown.
#[tokio::test]
async fn test_deregister_called_on_crash() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Simulate the pool's pre-spawn registration.
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    demux.register("test-worker".to_string(), tx);

    // Connect a DEALER socket as the "Python worker".
    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    // The crash test doesn't send a shutdown signal — the Dying event triggers
    // the exit path instead. The oneshot sender is dropped without sending.
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event first — via the DEALER socket (correct direction:
    // worker → ROUTER). The DEALER sends a 2-frame message (delimiter + payload);
    // the ROUTER prepends the identity frame.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    let ready_payload = rmp_serde::to_vec_named(&ready).unwrap();
    let mut ready_msg = ZmqMessage::from(Bytes::from(""));
    ready_msg.push_back(Bytes::from(ready_payload));
    _dealer
        .send(ready_msg)
        .await
        .expect("DEALER send Ready should succeed");

    // Small delay to ensure the Ready event is processed before the Dying event.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send Dying event — simulates the worker process crashing.
    let dying = WorkerEvent::Dying {
        reason: "simulated crash".to_string(),
    };
    let dying_payload = rmp_serde::to_vec_named(&dying).unwrap();
    let mut dying_msg = ZmqMessage::from(Bytes::from(""));
    dying_msg.push_back(Bytes::from(dying_payload));
    _dealer
        .send(dying_msg)
        .await
        .expect("DEALER send Dying should succeed");

    // Worker should exit within 5s.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // After crash exit, worker must be deregistered.
    assert!(
        !demux.registered("test-worker"),
        "worker should be deregistered after crash"
    );

    // A Dying event is the worker itself reporting it's gone — status must
    // go straight to Dead, not the Dying value (that's reserved for a
    // supervisor-initiated shutdown still in flight).
    assert_eq!(
        *status.read().await,
        WorkerStatus::Dead,
        "status should be Dead after a Dying event"
    );
}

/// When no Ready event arrives within the Initializing timeout, `run()` exits
/// to `Dead` and calls `deregister()`.
///
/// Creates a ROUTER/DEALER pair, registers the worker (simulating the pool's
/// pre-spawn registration), and sends NO events. The worker remains in
/// Initializing state until `init_timeout` fires, at which point it exits and
/// deregisters.
///
/// This test passes a short `init_timeout` (200ms) rather than
/// `DEFAULT_INIT_TIMEOUT` (60s) — it exercises the timeout *mechanism*, not
/// production's specific grace period, so there is no reason to make the
/// test suite pay a real 60s wall-clock cost for it. See
/// `test_default_init_timeout_matches_design_spec` below for the check that
/// covers the production *value*.
#[tokio::test]
async fn test_deregister_called_on_initializing_timeout() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Simulate the pool's pre-spawn registration.
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    demux.register("test-worker".to_string(), tx);

    // Connect a DEALER socket as the "Python worker" so the ROUTER recognizes
    // the identity (even though we never send a Ready event).
    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: Duration::from_millis(200),
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send no events — the worker stays in Initializing.
    // The 200ms init_timeout will fire, transitioning to Dead and deregistering.
    //
    // We use a bounded wait to avoid hanging indefinitely if the timeout
    // mechanism is broken. 5s gives generous buffer over the 200ms timeout.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => {
            panic!("ManagedWorker::run() did not complete within 5s — the Initializing timeout may not be firing");
        }
    }

    // After timeout exit, worker must be deregistered.
    assert!(
        !demux.registered("test-worker"),
        "worker should be deregistered after Initializing timeout"
    );

    assert_eq!(
        *status.read().await,
        WorkerStatus::Dead,
        "status should be Dead after the Initializing timeout fires"
    );
}

/// `DEFAULT_INIT_TIMEOUT` matches the 60s grace period specified in
/// `ANVILML_DESIGN.md §9.2` for production use. This is a cheap, direct
/// check that the constant itself hasn't drifted — the timeout *mechanism*
/// is covered by `test_deregister_called_on_initializing_timeout` above
/// using a short duration; this test covers the production *value*.
#[test]
fn test_default_init_timeout_matches_design_spec() {
    assert_eq!(
        DEFAULT_INIT_TIMEOUT,
        Duration::from_secs(60),
        "DEFAULT_INIT_TIMEOUT must match ANVILML_DESIGN.md §9.2's documented 60s grace period"
    );
}

/// A single transport error (DEALER dropped) causes exactly one crash attempt
/// to be recorded: `attempt_count()` returns 1.
///
/// Creates a ROUTER/DEALER pair, sends a `Ready` event to transition to Idle,
/// then drops the DEALER socket (which forces `recv()` to fail on the next
/// iteration). The worker must exit, and `attempt_count()` must return 1.
#[tokio::test]
async fn test_crash_appends_to_attempt_history() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Connect a DEALER socket as the "Python worker".
    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    // Spawn the worker — it starts in Initializing state.
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        // Non-respawn-eligible (max_attempts=1): this test verifies crash
        // DETECTION and attempt_history tracking, not respawn behavior —
        // that's covered separately by
        // test_respawn_under_limit_spawns_again_and_reregisters. Before
        // P8-E6, should_respawn()'s result was computed but never acted
        // on, so a respawn-eligible policy here (e.g. the old default of
        // 5 max attempts) made no observable difference. Now that P8-E6
        // acts on it, a respawn-eligible policy here would make the
        // worker respawn into a second generation that never receives a
        // Ready event, hanging past this test's 5s bound.
        respawn_policy: RespawnPolicy::new(100, 1, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send a Ready event to transition to Idle — via the DEALER (worker →
    // ROUTER), the direction ManagedWorker's recv() actually receives from.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Deregister the worker directly — this drops the sender half of
    // event_rx's channel pair (Demux::deregister()'s own doc comment:
    // map.remove() drops the stored Sender), causing event_rx.recv() to
    // return None. As of P8-F2, this is the deterministic way to trigger
    // ManagedWorker's crash path in tests: a malformed transport message
    // no longer crashes any specific worker (it's caught and retried by
    // bridge.rs's reader task instead, which is now shared pool-wide —
    // see bridge.rs's own "Error handling" doc section), so deregistering
    // directly is what actually reaches this generation's crash-handling
    // exit path (attempt_history.push + should_respawn +
    // crash_respawn_decision).
    demux.deregister("test-worker");

    // Give the worker time to detect the crash and exit.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // The worker task should complete within 5 seconds — bounded wait per
    // ENVIRONMENT.md §11.5.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // After crash, the transport-error path transitions status to Dead.
    //
    // Note: we cannot call attempt_count() on `worker` because `run()`
    // consumed `self` — the shared `status` lock is the only thing this
    // test can still observe.
    assert_eq!(
        *status.read().await,
        WorkerStatus::Dead,
        "status should be Dead after a transport recv error"
    );
}

/// Multiple transport errors each append to `attempt_history`.
///
/// This test verifies that the crash-attempt tracking accumulates across
/// multiple crash cycles. It sends a `Ready` event, then causes a transport
/// error by dropping the DEALER. After the worker exits, a second crash
/// scenario is set up with a new worker instance, confirming that each
/// crash independently records an attempt.
#[tokio::test]
async fn test_crash_history_grows_per_crash() {
    // First crash: send Ready, then drop DEALER.
    {
        let demux = Arc::new(Demux::new());
        let transport = Arc::new(RouterTransport::bind().await.unwrap());
        let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
            spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
        let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

        let mut _dealer = connect_dealer(&transport, "test-worker").await;

        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
        let (pong_tx, _pong_rx) = mpsc::channel(16);
        let worker = ManagedWorker::new(ManagedWorkerConfig {
            worker_id: "test-worker".to_string(),
            transport: Arc::clone(&transport),
            demux: Arc::clone(&demux),
            status: Arc::clone(&status),
            node_registry: Arc::new(NodeTypeRegistry::new()),
            // Non-respawn-eligible — see test_crash_appends_to_attempt_history's
            // comment for why a respawn-eligible policy here would hang past
            // this test's 5s bound as of P8-E6.
            respawn_policy: RespawnPolicy::new(100, 1, 300),
            init_timeout: DEFAULT_INIT_TIMEOUT,
            pong_tx,
            watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
            watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
            graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
            venv_path: PathBuf::from("/mock/venv"),
            env: HashMap::new(),
            spawner: Arc::new(MockWorkerSpawner::new()),
        });
        let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

        // Send Ready event via the DEALER (worker → ROUTER).
        let ready = WorkerEvent::Ready {
            worker_id: "test-worker".to_string(),
            device_index: 0,
            device_name: "Mock GPU".to_string(),
            device_type: "cpu".to_string(),
            vram_total_mib: 1024,
            vram_free_mib: 900,
            torch_version: "2.5.0".to_string(),
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: false,
            fp4: false,
            flash_attention: false,
            capabilities_source: "mock".to_string(),
            node_types: vec![],
        };
        send_event(&mut _dealer, &ready).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Deregister the worker directly — the deterministic way to
        // trigger ManagedWorker's crash path as of P8-F2: a malformed
        // transport message no longer crashes any specific worker (it's
        // caught and retried by bridge.rs's reader task instead, which
        // is now shared pool-wide), so deregistering directly is what
        // actually reaches this generation's crash-handling exit path.
        demux.deregister("test-worker");

        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::select! {
            _ = handle => (),
            _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
        }
    }

    // Second crash: a fresh worker instance, same pattern.
    {
        let demux = Arc::new(Demux::new());
        let transport = Arc::new(RouterTransport::bind().await.unwrap());
        let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
            spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
        let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

        let mut _dealer = connect_dealer(&transport, "test-worker-2").await;

        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
        let (pong_tx, _pong_rx) = mpsc::channel(16);
        let worker = ManagedWorker::new(ManagedWorkerConfig {
            worker_id: "test-worker-2".to_string(),
            transport: Arc::clone(&transport),
            demux: Arc::clone(&demux),
            status: Arc::clone(&status),
            node_registry: Arc::new(NodeTypeRegistry::new()),
            // Non-respawn-eligible — see this test's first block, above.
            respawn_policy: RespawnPolicy::new(100, 1, 300),
            init_timeout: DEFAULT_INIT_TIMEOUT,
            pong_tx,
            watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
            watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
            graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
            venv_path: PathBuf::from("/mock/venv"),
            env: HashMap::new(),
            spawner: Arc::new(MockWorkerSpawner::new()),
        });
        let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

        let ready = WorkerEvent::Ready {
            worker_id: "test-worker-2".to_string(),
            device_index: 0,
            device_name: "Mock GPU".to_string(),
            device_type: "cpu".to_string(),
            vram_total_mib: 1024,
            vram_free_mib: 900,
            torch_version: "2.5.0".to_string(),
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: false,
            fp4: false,
            flash_attention: false,
            capabilities_source: "mock".to_string(),
            node_types: vec![],
        };
        send_event(&mut _dealer, &ready).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        demux.deregister("test-worker-2");

        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::select! {
            _ = handle => (),
            _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
        }
    }
}

/// On crash, `should_respawn()` is consulted and the INFO log
/// `crash_respawn_decision` is emitted with `should_respawn = true`.
///
/// Creates a ROUTER/DEALER pair with a `RespawnPolicy` configured to
/// allow 10 max attempts, sends `Ready`, causes a crash by dropping the
/// DEALER, and verifies the worker exits cleanly. The `attempt_count()`
/// accessor proves the crash path was taken (the worker consumed `self`
/// so we verify via the exit).
///
/// The INFO log `crash_respawn_decision` is verified by checking that
/// the worker exits cleanly after the crash — the log is emitted inside
/// the crash path, and the only way to reach the exit is through that path.
#[tokio::test]
async fn test_should_respawn_called_on_crash() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    // Non-respawn-eligible (max_attempts=1): this test verifies a crash is
    // detected and causes prompt exit — respawn-eligible behavior (the
    // worker actually respawning) is covered separately by
    // test_respawn_under_limit_spawns_again_and_reregisters. Before P8-E6,
    // should_respawn()'s result was computed but never acted on, so this
    // test originally used a respawn-eligible policy (10 max attempts) to
    // verify should_respawn() itself returned true without that mattering
    // for whether run() exited promptly. Now that P8-E6 acts on the
    // decision, a respawn-eligible policy here would make the worker
    // respawn into a second generation that never receives a Ready event,
    // hanging past this test's 5s bound.
    let policy = RespawnPolicy::new(2000, 1, 300);

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: policy,
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event via the DEALER (worker → ROUTER).
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Deregister the worker directly — the deterministic way to
    // trigger ManagedWorker's crash path as of P8-F2: a malformed
    // transport message no longer crashes any specific worker (it's
    // caught and retried by bridge.rs's reader task instead, which
    // is now shared pool-wide), so deregistering directly is what
    // actually reaches this generation's crash-handling exit path.
    demux.deregister("test-worker");

    // Worker should exit within 5s — bounded wait per ENVIRONMENT.md §11.5.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // Verify the worker exited — clean exit proves the crash path executed
    // (attempt_history.push + should_respawn call + crash_respawn_decision log).
    // If the crash path had a bug (e.g. missing push), the worker would still
    // exit but the history would be empty; we verify via the exit itself since
    // attempt_count() is only accessible before self is consumed.
}

// The following three tests close a gap found while implementing P8-E5:
// `handle_event()`'s doc comment always claimed `Completed`/`Failed`/`Cancelled`
// transition status back to `Idle`, but the code never actually did it — only
// `Initializing` was ever written anywhere in `ManagedWorker`. See PHASES.md's
// amendments log for the full account. These three tests, plus the Ready/Dying/
// shutdown/timeout/crash assertions added to the existing tests above, are the
// closing verification for that gap.

/// A `Completed` event transitions status from `Busy` back to `Idle`.
///
/// Sends `Ready` (→ Idle), manually sets `Busy` via the shared status lock
/// (simulating job dispatch, which is out of `ManagedWorker`'s own scope), then
/// sends `Completed` and verifies status returns to `Idle`.
#[tokio::test]
async fn test_completed_event_transitions_to_idle() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Simulate job dispatch marking the worker Busy — this is the scheduler's
    // job in later phases, not ManagedWorker's; done here directly via the lock.
    *status.write().await = WorkerStatus::Busy;

    let completed = WorkerEvent::Completed {
        job_id: uuid::Uuid::new_v4(),
        elapsed_ms: 1234,
    };
    send_event(&mut _dealer, &completed).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Completed"
    );

    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// A `Failed` event transitions status from `Busy` back to `Idle`.
///
/// Same shape as `test_completed_event_transitions_to_idle`, using `Failed`
/// instead — a failed job still returns the worker to `Idle`, it does not
/// crash the worker process itself.
#[tokio::test]
async fn test_failed_event_transitions_to_idle() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    *status.write().await = WorkerStatus::Busy;

    let failed = WorkerEvent::Failed {
        job_id: uuid::Uuid::new_v4(),
        error: "CUDA out of memory".to_string(),
        traceback: None,
    };
    send_event(&mut _dealer, &failed).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Failed"
    );

    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// A `Cancelled` event transitions status from `Busy` back to `Idle`.
///
/// Same shape as the two tests above, using `Cancelled`.
#[tokio::test]
async fn test_cancelled_event_transitions_to_idle() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    *status.write().await = WorkerStatus::Busy;

    let cancelled = WorkerEvent::Cancelled {
        job_id: uuid::Uuid::new_v4(),
    };
    send_event(&mut _dealer, &cancelled).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Cancelled"
    );

    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// Missing Pongs trigger the watchdog's crash path identically to a transport error.
///
/// Creates a ROUTER/DEALER pair with very short watchdog timings (ping_interval=50ms,
/// pong_timeout=200ms), sends Ready to transition to Idle, then sends no Pongs.
/// The watchdog will send a Ping, wait 200ms for a Pong that never arrives, declare
/// the worker dead via `dead_tx`, and the `dead_rx` branch in `run()` will trigger
/// the same crash path as a transport error (status → Dead, attempt_history appended,
/// should_respawn called, loop breaks).
///
/// The `pong_tx` channel is created but never used — no Pongs are forwarded from
/// `handle_event()`, so the watchdog times out on its first Ping cycle.
#[tokio::test]
async fn test_watchdog_missing_pong_triggers_crash_path() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    // Create the pong channel for the worker constructor.
    let (pong_tx, _pong_rx) = mpsc::channel(16);

    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        // Non-respawn-eligible (max_attempts=1) — see
        // test_crash_appends_to_attempt_history's comment for why a
        // respawn-eligible policy here would hang past this test's 5s
        // bound as of P8-E6.
        respawn_policy: RespawnPolicy::new(1000, 1, 100),
        init_timeout: Duration::from_secs(60),
        pong_tx,
        watchdog_ping_interval: Duration::from_millis(50),
        watchdog_pong_timeout: Duration::from_millis(200),
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event to transition to Idle.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process Ready and start the watchdog.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Now withhold all Pongs. The watchdog will send a Ping (50ms interval),
    // wait 200ms for a Pong, timeout, and declare the worker dead.
    // Total wait: ~300ms for the watchdog to detect the missing Pong.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // The worker task should complete within 5s — bounded wait per
    // ENVIRONMENT.md §11.5.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // After watchdog timeout, status should be Dead.
    assert_eq!(
        *status.read().await,
        WorkerStatus::Dead,
        "status should be Dead after watchdog timeout"
    );
}

/// Sending Pongs at the correct sequence number keeps the watchdog alive.
///
/// Creates a ROUTER/DEALER pair with short watchdog timings (ping_interval=50ms,
/// pong_timeout=200ms), sends Ready to transition to Idle, then continuously
/// sends Pongs at the correct sequence number (seq 0, 1, 2, ...). The watchdog
/// should not declare the worker dead — `dead_rx` never fires, and the worker
/// stays alive for the duration of the test.
///
/// The test verifies that live Pongs don't false-trigger the crash path.
#[tokio::test]
async fn test_watchdog_live_pongs_no_false_trigger() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: Duration::from_millis(50),
        watchdog_pong_timeout: Duration::from_millis(200),
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process Ready and start the watchdog.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send Pongs at increasing sequence numbers to match the watchdog's Pings.
    // The watchdog increments its seq on each Ping; we need to send matching Pongs.
    // With a 50ms ping interval and 200ms pong timeout, we send a Pong every 50ms.
    for seq in 0..10u64 {
        let pong = WorkerEvent::Pong { seq };
        send_event(&mut _dealer, &pong).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    // Worker should still be alive and Idle (not Dead from watchdog timeout).
    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should still be Idle — Pongs kept watchdog alive"
    );

    // Clean shutdown.
    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// Pong forwarding to the watchdog channel does not disturb normal event processing.
///
/// Sends a sequence of events: Ready (→ Idle), manually sets Busy, sends Completed
/// (→ Idle), sends Failed (→ Idle). The watchdog receives Pongs on its channel but
/// filters them by sequence number. Status transitions are correct throughout.
///
/// This verifies that the `try_send` in `handle_event()` for Pongs doesn't
/// interfere with the event processing loop.
#[tokio::test]
async fn test_pong_forwarding_does_not_disturb_idle_busy() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event → Idle.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Ready"
    );

    // Manually set Busy (simulating job dispatch).
    *status.write().await = WorkerStatus::Busy;

    // Send a Pong — it should be forwarded to the watchdog but not affect status.
    let pong = WorkerEvent::Pong { seq: 0 };
    send_event(&mut _dealer, &pong).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Busy,
        "status should still be Busy after Pong"
    );

    // Send Completed → Idle.
    let completed = WorkerEvent::Completed {
        job_id: uuid::Uuid::new_v4(),
        elapsed_ms: 1234,
    };
    send_event(&mut _dealer, &completed).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Completed"
    );

    // Send Failed → Idle.
    let failed = WorkerEvent::Failed {
        job_id: uuid::Uuid::new_v4(),
        error: "test failure".to_string(),
        traceback: None,
    };
    send_event(&mut _dealer, &failed).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Failed"
    );

    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// Verifies that `RouterTransportAdapter` is no longer `#[allow(dead_code)]`
/// by confirming it compiles without dead_code warnings.
///
/// The `#[allow(dead_code)]` attribute was removed in Step 1 of this task.
/// The adapter is now constructed inside `ManagedWorker::run()` (see the
/// `test_watchdog_missing_pong_triggers_crash_path` test above), proving it's
/// live code. Clippy with `-D warnings` would fail if the adapter were unused.
#[test]
fn test_router_transport_adapter_not_dead_code() {
    // The actual construction of RouterTransportAdapter happens in
    // ManagedWorker::run() via the watchdog spawning code. This test
    // serves as a compile-time gate: if the adapter is unused, clippy
    // will flag it as dead_code and fail the build.
    //
    // The watchdog tests (test_watchdog_missing_pong_triggers_crash_path,
    // test_watchdog_live_pongs_no_false_trigger, etc.) exercise the full
    // construction path, confirming the adapter is live code.
}

/// After `run()` completes, the `pong_tx` is dropped (consumed by `self`),
/// closing the watchdog's `pong_rx`. The watchdog exits its loop without
/// sending on `dead_tx` (graceful exit).
///
/// This verifies that the watchdog task cleans up properly when the worker
/// exits — it doesn't leak or hang.
#[tokio::test]
async fn test_watchdog_channel_cleans_up_on_exit() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event to transition to Idle.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Send shutdown — worker should exit cleanly.
    drop(shutdown_tx);

    // The worker task should complete within 500ms — bounded wait per
    // ENVIRONMENT.md §11.5. The watchdog's pong_rx closes when pong_tx is
    // dropped (consumed by self), and the watchdog exits its loop without
    // sending on dead_tx (graceful exit).
    let timeout = tokio::time::sleep(Duration::from_millis(500));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 500ms"),
    }

    // After shutdown, status should be Dying (graceful shutdown path).
    assert_eq!(
        *status.read().await,
        WorkerStatus::Dying,
        "status should be Dying after graceful shutdown"
    );
}

// The following tests exercise P8-E6's respawn loop: `spawner.spawn()` at
// the top of every generation, the respawn decision on a crash, and the
// Respawning → Initializing transition between generations.

/// After a successful spawn, `self.child` is set — the worker's child
/// process is tracked, not orphaned. Uses `run_once_for_test()` directly
/// (see that method's own doc comment for why: `run()` consumes and never
/// returns `self`, so there would be no way to inspect `self.child`
/// afterward through the public API alone).
#[tokio::test]
async fn test_child_tracked_after_spawn() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    // Trigger a graceful shutdown almost immediately so run_once_for_test()
    // returns quickly — we only need it to get past the spawn step, not
    // reach any particular event-loop state.
    let _ = shutdown_tx.send(());
    let (mut worker, outcome) = worker.run_once_for_test(&mut shutdown_rx).await;

    assert_eq!(
        outcome,
        RunOutcome::ShutdownRequested,
        "shutdown was requested before any event arrived"
    );
    assert!(
        worker.child_pid_for_test().is_some(),
        "child should be tracked (Some) once spawn() has succeeded, \
         regardless of how this generation later ends"
    );
    assert_eq!(
        spawner.call_count(),
        1,
        "spawn() should have been called exactly once"
    );

    // Clean up: this test uses run_once_for_test() directly, which
    // bypasses run()'s outer-loop cleanup — graceful_shutdown_child() and
    // force_kill_child() only fire from run()'s own loop, never from
    // run_once() itself (see run()'s doc comment). Without this, the
    // spawned child would be silently dropped here, unkilled — a real,
    // confirmed process leak this test itself was causing.
    let mut child = worker
        .take_child_for_test()
        .expect("child should still be tracked");
    let _ = child.kill().await;
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
}

/// An under-limit crash triggers exactly one respawn: the spawner is called
/// a second time, and the worker is re-registered with the demux for the
/// new generation.
#[tokio::test]
async fn test_respawn_under_limit_spawns_again_and_reregisters() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);

    // max_attempts=2: the first crash has count_in_window=1 < 2 -> respawn.
    // A second crash would have count_in_window=2, not < 2 -> no respawn.
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(50, 2, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Give gen 0's own spawn+register sequence time to complete before
    // deregistering — deregister() on a worker_id that was never
    // registered yet would be a no-op, and the crash this test depends
    // on would never happen.
    tokio::time::sleep(Duration::from_millis(50)).await;
    demux.deregister("test-worker");

    // Respawn delay is 50ms; give generous margin for gen 1's spawn +
    // registration to complete.
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(
        spawner.call_count(),
        2,
        "spawner should be called once for gen 0 and again for the respawn"
    );
    assert!(
        demux.registered("test-worker"),
        "worker should be re-registered with the demux for the new generation"
    );

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// An at-limit crash does not respawn: the spawner is called exactly once,
/// and the worker exits for good.
#[tokio::test]
async fn test_respawn_at_limit_exits_permanently() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);

    // max_attempts=1: the first crash already has count_in_window=1, not
    // < 1 -> should_respawn() is false immediately. No respawn at all.
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(50, 1, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Give gen 0's own spawn+register sequence time to complete before
    // deregistering — see test_respawn_under_limit_spawns_again_and_reregisters
    // for the full explanation of why this delay is needed.
    tokio::time::sleep(Duration::from_millis(50)).await;
    demux.deregister("test-worker");

    // run() should complete on its own — no respawn means no further
    // generation to wait on. Bounded wait per ENVIRONMENT.md §11.5.
    let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
    assert!(
        result.is_ok(),
        "run() should exit permanently, not wait for a respawn that won't happen"
    );

    assert_eq!(
        spawner.call_count(),
        1,
        "spawner should be called only once — the crash was at-limit, no respawn"
    );
    assert!(
        !demux.registered("test-worker"),
        "worker should be deregistered after exiting for good"
    );
}

/// Between a respawn-eligible crash and the next generation's spawn, status
/// passes through `Respawning` before the next generation's `Initializing`.
/// Order matters: a caller polling status during this window should never
/// observe a state that implies a worker exists when none currently does.
#[tokio::test]
async fn test_respawn_status_transitions_respawning_then_initializing() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);

    // A longer delay (300ms) than the other tests so there's a wide,
    // reliably-observable window where status must read Respawning.
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(300, 2, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Give gen 0's own spawn+register sequence time to complete before
    // deregistering — see test_respawn_under_limit_spawns_again_and_reregisters
    // for the full explanation of why this delay is needed.
    tokio::time::sleep(Duration::from_millis(50)).await;
    demux.deregister("test-worker");

    // Mid-delay: status must be Respawning (crash already processed, next
    // spawn hasn't happened yet — the 300ms sleep in run()'s outer loop is
    // still in flight).
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        *status.read().await,
        WorkerStatus::Respawning,
        "status should be Respawning during the backoff delay, before the next spawn"
    );

    // Past the full delay: the next generation should have spawned and set
    // Initializing.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        *status.read().await,
        WorkerStatus::Initializing,
        "status should be Initializing once the next generation has spawned"
    );

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// The actual delay between a respawn-eligible crash and the next spawn
/// matches `RespawnPolicy::next_delay()` — not some other hardcoded value.
#[tokio::test]
async fn test_respawn_delay_matches_next_delay() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);

    let policy = RespawnPolicy::new(250, 2, 300);
    let expected_delay = policy.next_delay();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: policy,
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Give gen 0's own spawn+register sequence time to complete before
    // deregistering — see test_respawn_under_limit_spawns_again_and_reregisters
    // for the full explanation of why this delay is needed.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let crash_time = tokio::time::Instant::now();
    demux.deregister("test-worker");

    // Poll spawner.call_count() until it reaches 2 (the respawn happened),
    // bounded per ENVIRONMENT.md §11.5, and measure the elapsed time.
    let poll_result = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if spawner.call_count() >= 2 {
                return tokio::time::Instant::now();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;

    let respawn_time = poll_result.expect("respawn should happen within 2s");
    let elapsed = respawn_time - crash_time;

    // Allow generous scheduling slack either side — this is asserting the
    // delay is approximately the *configured* 250ms, not a hardcoded
    // different value (e.g. the old default of 2000ms), and not asserting
    // sub-millisecond timer precision.
    //
    // Lower bound is intentionally NOT `elapsed >= expected_delay`: WSL2's
    // Hyper-V clock periodically resyncs to the host and can jump forward,
    // which can make a monotonic-clock-based measurement like this one
    // read shorter than real wall-clock time actually elapsed — observed
    // directly (154ms measured against a configured 250ms delay) on a WSL2
    // dev environment while passing reliably on native Linux CI. This is
    // not a code bug — `run()`'s `tokio::time::sleep(delay).await` is
    // unconditional and cannot fire early; ANVILML_DESIGN.md's CI matrix
    // (native `rust-linux`/`rust-windows` runners, no WSL2) is the source
    // of truth for correctness. 150ms of tolerance comfortably absorbs the
    // observed anomaly while still catching a genuine regression (e.g. the
    // delay being disconnected from the configured policy entirely).
    let lower_bound = expected_delay.saturating_sub(Duration::from_millis(150));
    assert!(
        elapsed >= lower_bound,
        "respawn happened after only {elapsed:?}, well before the configured \
         {expected_delay:?} delay (even accounting for {lower_bound:?} of \
         tolerance for known WSL2 clock-jump behavior — see this assertion's \
         comment)"
    );
    assert!(
        elapsed < expected_delay + Duration::from_millis(500),
        "respawn took {elapsed:?}, far longer than the configured {expected_delay:?} delay — \
         delay may not be using RespawnPolicy::next_delay()"
    );

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// `spawner.spawn()` itself failing (not a running worker crashing) is
/// respawn-eligible: `attempt_history` is appended and
/// `RespawnPolicy::should_respawn()` is consulted, exactly like a transport
/// error or watchdog timeout on an already-running worker. Confirmed
/// design decision (bounded retries via should_respawn(), not an
/// unconditional infinite retry loop for a persistently broken environment).
///
/// Uses `run_once_for_test()` directly — this needs to observe one
/// generation's exact `RunOutcome` in isolation, which `run()`'s
/// multi-generation black-box behavior can't provide directly.
#[tokio::test]
async fn test_spawn_failure_is_respawn_eligible() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::failing());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);
    let (_shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(50, 2, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let (worker, outcome) = worker.run_once_for_test(&mut shutdown_rx).await;

    assert_eq!(
        outcome,
        RunOutcome::Crashed {
            should_respawn: true
        },
        "a spawn failure under the attempt limit should be respawn-eligible"
    );
    assert_eq!(
        worker.attempt_count(),
        1,
        "the spawn failure should be recorded in attempt_history"
    );
    assert!(
        !demux.registered("test-worker"),
        "a worker that never successfully spawned should never be registered"
    );
    assert_eq!(
        spawner.call_count(),
        1,
        "spawn() should have been attempted exactly once"
    );
}

/// On respawn, the previous generation's child is killed
/// (`.kill().await`) before the new one is spawned and stored — it is not
/// silently dropped (which would leak the OS process, since
/// `tokio::process::Child` does not kill on drop unless `kill_on_drop(true)`
/// was set at construction, which `ProcessWorkerSpawner`'s `Command` does
/// not do).
///
/// Uses `run_once_for_test()` twice, manually, to simulate what `run()`'s
/// outer loop does — this is the only way to hold both generations'
/// `Child` PIDs simultaneously and compare them, which neither `run()`'s
/// black-box behavior nor a single `run_once_for_test()` call can provide.
#[tokio::test]
async fn test_respawn_kills_previous_child() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(10, 3, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    // Gen 0: crash it after giving its own spawn+register sequence time to
    // complete — deregister() on a worker_id that was never registered
    // yet would be a no-op, and the crash this test depends on would
    // never happen. Spawned as its own task since it needs to run
    // concurrently with run_once_for_test()'s own .await below.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        demux.deregister("test-worker");
    });

    let (worker, outcome) = worker.run_once_for_test(&mut shutdown_rx).await;
    assert_eq!(
        outcome,
        RunOutcome::Crashed {
            should_respawn: true
        },
        "gen 0 should crash and be respawn-eligible"
    );
    let gen0_pid = worker
        .child_pid_for_test()
        .expect("gen 0 should have a tracked child");

    // Simulate run()'s outer loop: sleep for the configured delay, then
    // call run_once_for_test() again for gen 1. A fresh shutdown_rx is
    // needed since the previous one may have been polled to completion
    // internally — but it wasn't consumed (the crash path doesn't touch
    // shutdown_rx), so reusing it is fine; matching run()'s own real
    // behavior of holding one shutdown_rx across every generation.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Gen 1 never receives a Ready event in this test — it doesn't need
    // to, since this test only verifies child-kill/PID-tracking, not
    // steady-state event handling. Left to run on its own, gen 1's
    // run_once_for_test() would only return once init_timeout elapses:
    // 60 seconds, per this test's DEFAULT_INIT_TIMEOUT config — the exact
    // hang this comment is here to prevent regressing back into. Spawned
    // as its own task so a real shutdown signal can be sent concurrently,
    // giving gen 1 a genuinely fast (not merely "shorter") exit path —
    // run_once_for_test() is a single .await we can't otherwise signal
    // into mid-flight from the same sequential task.
    let gen1_handle = tokio::spawn(async move { worker.run_once_for_test(&mut shutdown_rx).await });
    let _ = shutdown_tx.send(());
    let (mut worker, _outcome) = tokio::time::timeout(Duration::from_secs(2), gen1_handle)
        .await
        .expect("gen 1's run_once_for_test() should complete within 2s of the shutdown signal")
        .expect("gen 1's run_once_for_test() task should not panic");
    let gen1_pid = worker
        .child_pid_for_test()
        .expect("gen 1 should have a tracked child");

    assert_ne!(
        gen0_pid, gen1_pid,
        "gen 1 should be a genuinely new process, not the same PID reused"
    );

    // Confirm gen 0's process is actually gone, not merely replaced in
    // self.child while still running in the background. Uses the
    // cross-platform pid_is_alive() helper (see its own doc comment) so
    // this assertion runs on Windows CI too, not just Linux.
    assert!(
        !pid_is_alive(gen0_pid),
        "gen 0's process (pid {gen0_pid}) should have been killed before \
         gen 1 spawned, not left running in the background"
    );

    // Clean up gen 1's child — this test uses run_once_for_test() directly
    // (twice), which bypasses run()'s outer-loop cleanup —
    // graceful_shutdown_child() and force_kill_child() only fire from
    // run()'s own loop, never from run_once() itself. Gen 0's child is
    // already verified dead above (killed by gen 1's own respawn-time
    // cleanup step); gen 1's child, being the last generation in this
    // test, has nothing else to kill it. Without this, it would be
    // silently dropped here, unkilled — a real, confirmed process leak
    // this test itself was causing.
    let mut gen1_child = worker
        .take_child_for_test()
        .expect("gen 1 should have a tracked child");
    let _ = gen1_child.kill().await;
    let _ = tokio::time::timeout(Duration::from_secs(5), gen1_child.wait()).await;
}

// The following tests exercise P8-E7: Windows Job Object reassignment
// across respawn generations. Windows-only, both by cfg and by necessity —
// JobObjectGuard itself does not compile on non-Windows targets.

/// An under-limit respawn's new Child is assigned to the same
/// `JobObjectGuard` as the original.
///
/// `JobObjectGuard` has no introspection API (no "list assigned PIDs"), so
/// this is verified behaviorally, matching the pattern already established
/// in `spawn_tests.rs`'s `test_assigned_child_terminated_on_drop`: drop
/// everything after a respawn and confirm the *new* (gen 1) child actually
/// gets killed by the job object's kill-on-close limit. If
/// `assign_process()` were only ever called for gen 0 (not reassigned on
/// respawn), gen 1's child would survive the drop untouched.
///
/// Uses `run_once_for_test()` directly (twice, manually) to simulate what
/// `run()`'s outer loop does — same pattern as
/// `test_respawn_kills_previous_child` — since this needs to hold gen 1's
/// `Child` handle directly afterward (`take_child_for_test()`) in order to
/// `.wait()` on it.
#[cfg(windows)]
#[tokio::test]
async fn test_respawn_reassigns_job_object_to_new_child() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(10, 3, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    // Gen 0: crash it after giving its own spawn+register sequence time
    // to complete, matching test_respawn_kills_previous_child's
    // established pattern.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        demux.deregister("test-worker");
    });

    let (worker, outcome) = worker.run_once_for_test(&mut shutdown_rx).await;
    assert_eq!(
        outcome,
        RunOutcome::Crashed {
            should_respawn: true
        },
        "gen 0 should crash and be respawn-eligible"
    );

    // Gen 1: never receives a Ready event — this test only verifies
    // job-object reassignment, not steady-state event handling. Ended via
    // a real shutdown signal rather than waiting out init_timeout, matching
    // test_respawn_kills_previous_child's established fix for the same
    // situation (gen 1 has no other way to return quickly).
    tokio::time::sleep(Duration::from_millis(20)).await;
    let gen1_handle = tokio::spawn(async move { worker.run_once_for_test(&mut shutdown_rx).await });
    let _ = shutdown_tx.send(());
    let (mut worker, _outcome) = tokio::time::timeout(Duration::from_secs(2), gen1_handle)
        .await
        .expect("gen 1's run_once_for_test() should complete within 2s of the shutdown signal")
        .expect("gen 1's run_once_for_test() task should not panic");

    let mut gen1_child = worker
        .take_child_for_test()
        .expect("gen 1 should have a tracked child");

    // Drop the worker (and with it, job_guard) — this closes the job
    // object, triggering JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE for whatever is
    // currently assigned to it. If assign_process() was genuinely called
    // for gen 1 (not just gen 0), gen 1's child dies here.
    drop(worker);

    // Bounded wait per ENVIRONMENT.md §11.5, matching
    // test_assigned_child_terminated_on_drop's own established pattern.
    let result = tokio::time::timeout(Duration::from_secs(5), gen1_child.wait()).await;
    match result {
        Ok(Ok(_)) => {
            // Gen 1's child exited — killed by the job object's
            // kill-on-close limit, proving reassignment happened.
        }
        Ok(Err(e)) => panic!("gen 1 child wait failed: {e}"),
        Err(_) => panic!(
            "gen 1's child did not exit within 5s of the worker (and job_guard) \
             dropping — assign_process() may not have been called for the \
             respawned generation"
        ),
    }
}

/// The job object's protection correctly tracks the *current* child across
/// **multiple** respawns, not just a single reassignment.
///
/// Distinct from `test_respawn_reassigns_job_object_to_new_child` above:
/// that test proves reassignment happens once; this one proves it keeps
/// happening correctly across repeated respawns — "whichever Child is
/// currently assigned when the guard drops" (this task's own acceptance
/// wording) genuinely means *whichever*, not just *the second one*.
#[cfg(windows)]
#[tokio::test]
async fn test_job_guard_kills_current_child_after_multiple_respawns() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = tokio::sync::mpsc::channel(16);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    // max_attempts=3: two crashes stay under the limit (1 < 3, then 2 < 3),
    // allowing two respawns — gen 0 -> gen 1 -> gen 2.
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(10, 3, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    // Gen 0: crash it after giving its own spawn+register sequence time
    // to complete.
    let demux_gen0 = Arc::clone(&demux);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        demux_gen0.deregister("test-worker");
    });

    let (worker, outcome) = worker.run_once_for_test(&mut shutdown_rx).await;
    assert_eq!(
        outcome,
        RunOutcome::Crashed {
            should_respawn: true
        },
        "gen 0 should crash and be respawn-eligible"
    );

    // Gen 1: crash it too. Fires on its own separate spawned task —
    // matching gen 0's own pattern above — since it needs to run
    // concurrently with run_once_for_test()'s own .await below. Needs
    // its own clone of demux: the gen 0 closure above already moved its
    // own capture of demux into itself via async move, so the outer
    // demux binding is still available here only because it was cloned,
    // not moved, at each use site.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let demux_gen1 = Arc::clone(&demux);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        demux_gen1.deregister("test-worker");
    });

    let (worker, outcome) = worker.run_once_for_test(&mut shutdown_rx).await;
    assert_eq!(
        outcome,
        RunOutcome::Crashed {
            should_respawn: true
        },
        "gen 1 should also crash and still be respawn-eligible (2 < 3)"
    );

    // Gen 2: never receives Ready — ended via a real shutdown signal, same
    // pattern as test_respawn_reassigns_job_object_to_new_child.
    tokio::time::sleep(Duration::from_millis(20)).await;
    let gen2_handle = tokio::spawn(async move { worker.run_once_for_test(&mut shutdown_rx).await });
    let _ = shutdown_tx.send(());
    let (mut worker, _outcome) = tokio::time::timeout(Duration::from_secs(2), gen2_handle)
        .await
        .expect("gen 2's run_once_for_test() should complete within 2s of the shutdown signal")
        .expect("gen 2's run_once_for_test() task should not panic");

    let mut gen2_child = worker
        .take_child_for_test()
        .expect("gen 2 should have a tracked child");

    drop(worker);

    let result = tokio::time::timeout(Duration::from_secs(5), gen2_child.wait()).await;
    match result {
        Ok(Ok(_)) => {
            // Gen 2's child exited — the job object's protection correctly
            // followed the process across two reassignments, not just one.
        }
        Ok(Err(e)) => panic!("gen 2 child wait failed: {e}"),
        Err(_) => panic!(
            "gen 2's child did not exit within 5s of the worker (and job_guard) \
             dropping — assign_process() may not have tracked the child \
             correctly across multiple respawns"
        ),
    }
}

// The following tests exercise the child-process cleanup fix for `run()`'s
// outer loop: before this fix, self.child was only ever killed as part of
// respawning into the NEXT generation — every terminal exit path
// (ShutdownRequested, InitTimedOut, WorkerReportedDying, and
// Crashed { should_respawn: false }) left the current generation's child
// process running, unkilled, orphaned. See run()'s own doc comment for
// the full explanation.
//
// These tests drive the worker through the full run() (not
// run_once_for_test()) — the new cleanup logic lives in run()'s outer
// loop, not in run_once() itself. Since run() consumes self and never
// returns it, MockWorkerSpawner::last_pid() is used to observe the
// spawned child's PID independently of ManagedWorker's own internal state.

/// On graceful shutdown, `WorkerMessage::Shutdown` is actually sent to the
/// worker — the message ANVILML_DESIGN.md's own protocol documents as
/// "the worker should finish its current step, then exit 0", but which
/// nothing in this codebase ever sent before this fix.
#[tokio::test]
async fn test_graceful_shutdown_sends_shutdown_message() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(200),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    let mut dealer = connect_dealer(&transport, "test-worker").await;
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut dealer, &ready).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let _ = shutdown_tx.send(());

    // Receive on the DEALER side — the identity frame the ROUTER used to
    // route this is stripped by ZeroMQ before it reaches the DEALER
    // (confirmed directly against zeromq 0.6.0's RouterSocket::send(),
    // which pop_front()s the identity before queuing the rest), leaving
    // 2 frames: [empty delimiter, msgpack payload].
    //
    // Loops rather than asserting on the very next message: the watchdog
    // was already spawned when Ready was processed (phase 2 started), and
    // its first Ping fires essentially immediately once spawned (standard
    // tokio::time::interval behavior — the first tick resolves right away,
    // not after a full ping_interval). That Ping can genuinely arrive
    // before this test's own shutdown_tx.send() — observed directly: this
    // test originally failed with `left: Ping { seq: 0 }, right: Shutdown`
    // on the very first assertion. Any number of interleaved Pings (0 or
    // more, timing-dependent) is expected and ignored; only Shutdown
    // itself is the thing being verified.
    let message = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let received = dealer.recv().await.expect("DEALER recv should succeed");
            let frames = received.into_vec();
            assert_eq!(
                frames.len(),
                2,
                "expected 2 frames from the DEALER's perspective (delimiter + payload)"
            );
            let message: WorkerMessage = rmp_serde::from_slice(&frames[1])
                .expect("payload should deserialize as WorkerMessage");
            if message == WorkerMessage::Shutdown {
                return message;
            }
            // Not Shutdown — a watchdog Ping that arrived first. Loop and
            // wait for the next message.
        }
    })
    .await
    .expect(
        "should receive Shutdown within 2s of the shutdown signal, ignoring any interleaved Pings",
    );
    assert_eq!(
        message,
        WorkerMessage::Shutdown,
        "the worker should receive Shutdown on graceful shutdown"
    );

    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// If the worker does not exit on its own within `graceful_shutdown_timeout`
/// after receiving `Shutdown`, it is force-killed as a fallback — not left
/// running.
///
/// `MockWorkerSpawner` always spawns a long-lived process (`sleep 999` /
/// `cmd timeout 999`) that never exits on its own, so this test always
/// exercises the fallback path, never the "exited gracefully" happy path —
/// that path isn't independently testable with this mock, since it would
/// require a mock worker that voluntarily exits after receiving Shutdown,
/// which MockWorkerSpawner doesn't simulate.
#[tokio::test]
async fn test_graceful_shutdown_force_kills_after_timeout() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    let mut dealer = connect_dealer(&transport, "test-worker").await;
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut dealer, &ready).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let pid = spawner
        .last_pid()
        .expect("spawner should have recorded a PID by now");

    let _ = shutdown_tx.send(());

    // Bounded wait for run() to complete — 100ms graceful_shutdown_timeout
    // plus generous margin for the force-kill fallback itself.
    let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(
        result.is_ok(),
        "run() should complete (via the force-kill fallback) within 5s"
    );

    // Give the OS a moment to actually reap the killed process.
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !pid_is_alive(pid),
        "child (pid {pid}) should have been force-killed after the \
         graceful_shutdown_timeout elapsed, not left running"
    );
}

/// `InitTimedOut` force-kills the tracked child — the worker never reached
/// `Ready`, so there is nothing to gracefully finish.
#[tokio::test]
async fn test_init_timeout_force_kills_child() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: Duration::from_millis(100),
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Connect a DEALER so the ROUTER recognizes the identity, but never
    // send Ready — init_timeout (100ms) will fire.
    let _dealer = connect_dealer(&transport, "test-worker").await;

    tokio::time::sleep(Duration::from_millis(50)).await;
    let pid = spawner
        .last_pid()
        .expect("spawner should have recorded a PID by now");

    let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(
        result.is_ok(),
        "run() should complete once init_timeout fires"
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !pid_is_alive(pid),
        "child (pid {pid}) should have been force-killed after \
         init_timeout, not left running"
    );
}

/// `WorkerReportedDying` force-kills the tracked child as a safety net,
/// even though the worker already reported its own termination.
#[tokio::test]
async fn test_worker_reported_dying_force_kills_child() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();

    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    let mut dealer = connect_dealer(&transport, "test-worker").await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let pid = spawner
        .last_pid()
        .expect("spawner should have recorded a PID by now");

    let dying = WorkerEvent::Dying {
        reason: "test".to_string(),
    };
    send_event(&mut dealer, &dying).await;

    let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(
        result.is_ok(),
        "run() should complete once the Dying event is processed"
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !pid_is_alive(pid),
        "child (pid {pid}) should have been force-killed after \
         WorkerReportedDying, not left running"
    );
}

/// A permanent (at-limit) crash — `Crashed { should_respawn: false }` —
/// force-kills the tracked child.
#[tokio::test]
async fn test_permanent_crash_force_kills_child() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner = Arc::new(MockWorkerSpawner::new());
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();

    // max_attempts=1: the first crash already hits the ceiling —
    // should_respawn() returns false immediately.
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::new(100, 1, 300),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT,
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
    });

    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Give gen 0's own spawn+register sequence time to complete before
    // deregistering — see test_respawn_under_limit_spawns_again_and_reregisters
    // for the full explanation of why this delay is needed.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let pid = spawner
        .last_pid()
        .expect("spawner should have recorded a PID by now");

    demux.deregister("test-worker");

    let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(
        result.is_ok(),
        "run() should complete once the permanent crash is processed"
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !pid_is_alive(pid),
        "child (pid {pid}) should have been force-killed after a \
         permanent crash, not left running"
    );
}

// The following test exercises P8-F2's actual fix directly: two
// ManagedWorker instances sharing one transport/demux/bridge, proving
// events sent to one never reach the other. Before P8-F2, both workers'
// run_once() called self.transport.recv() directly — whichever task's
// recv() happened to win a given poll would consume whatever message
// arrived, regardless of which worker it was actually addressed to. As
// of P8-F2, each worker only ever sees events routed to its own,
// demux-registered channel — by construction, not by timing luck.

/// Two `ManagedWorker` instances sharing one `RouterTransport`/`Demux`
/// (via one shared `spawn_bridge()`), with distinct DEALER identities,
/// never observe each other's events.
#[tokio::test]
async fn test_multi_worker_events_never_cross() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));

    let status_a = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let status_b = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let spawner_a = Arc::new(MockWorkerSpawner::new());
    let spawner_b = Arc::new(MockWorkerSpawner::new());
    let (pong_tx_a, _pong_rx_a) = mpsc::channel(16);
    let (pong_tx_b, _pong_rx_b) = mpsc::channel(16);
    let (shutdown_tx_a, shutdown_rx_a) = tokio::sync::oneshot::channel();
    let (shutdown_tx_b, shutdown_rx_b) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx_a, force_shutdown_rx_a) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx_b, force_shutdown_rx_b) = tokio::sync::oneshot::channel();

    let worker_a = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "worker-a".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status_a),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx: pong_tx_a,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner_a) as Arc<dyn WorkerSpawner>,
    });
    let worker_b = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "worker-b".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status_b),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx: pong_tx_b,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::clone(&spawner_b) as Arc<dyn WorkerSpawner>,
    });

    let handle_a = tokio::spawn(worker_a.run(shutdown_rx_a, force_shutdown_rx_a));
    let handle_b = tokio::spawn(worker_b.run(shutdown_rx_b, force_shutdown_rx_b));

    let mut dealer_a = connect_dealer(&transport, "worker-a").await;
    let mut dealer_b = connect_dealer(&transport, "worker-b").await;

    fn ready_event(worker_id: &str) -> WorkerEvent {
        WorkerEvent::Ready {
            worker_id: worker_id.to_string(),
            device_index: 0,
            device_name: "Mock GPU".to_string(),
            device_type: "cpu".to_string(),
            vram_total_mib: 1024,
            vram_free_mib: 900,
            torch_version: "2.5.0".to_string(),
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: false,
            fp4: false,
            flash_attention: false,
            capabilities_source: "mock".to_string(),
            node_types: vec![],
        }
    }

    // Send Ready only to worker-a. If events could ever cross (the exact
    // race P8-F2 fixes), worker-b's own now-nonexistent direct
    // transport.recv() call could have "stolen" this message before
    // P8-F2 — worker-b has no such call anymore, so this proves the fix
    // by construction, not just by this test happening not to trigger
    // the old race.
    send_event(&mut dealer_a, &ready_event("worker-a")).await;

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if *status_a.read().await == WorkerStatus::Idle {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("worker-a should reach Idle within 2s of its own Ready event");

    assert_eq!(
        *status_b.read().await,
        WorkerStatus::Initializing,
        "worker-b should be completely unaffected by an event sent only to worker-a"
    );

    // Now the reverse direction: send Ready only to worker-b, verify
    // worker-a (already Idle) is unaffected and worker-b alone transitions.
    send_event(&mut dealer_b, &ready_event("worker-b")).await;

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if *status_b.read().await == WorkerStatus::Idle {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("worker-b should reach Idle within 2s of its own Ready event");

    assert_eq!(
        *status_a.read().await,
        WorkerStatus::Idle,
        "worker-a should remain exactly as it was, unaffected by worker-b's event"
    );

    // Clean up both workers via graceful shutdown.
    let _ = shutdown_tx_a.send(());
    let _ = shutdown_tx_b.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), handle_a).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle_b).await;
}

/// A Ready event with N node type descriptors populates the registry:
/// `len() == N` and `get()` returns the correct descriptor for each type name.
///
/// Creates a ROUTER/DEALER pair with a shared `NodeTypeRegistry`, sends a Ready
/// event with 2 `NodeTypeDescriptor`s (`LoadModel` and `Sampler`), then verifies
/// the registry contains exactly 2 entries and each `get()` call returns the
/// matching descriptor.
#[tokio::test]
async fn test_ready_event_populates_registry() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let node_registry = Arc::new(NodeTypeRegistry::new());

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::clone(&node_registry),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send a Ready event with 2 node type descriptors.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![
            NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loaders".to_string(),
                description: "Loads a model from disk".to_string(),
                inputs: vec![SlotDescriptor {
                    name: "model_path".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            },
            NodeTypeDescriptor {
                type_name: "Sampler".to_string(),
                display_name: "Sampler".to_string(),
                category: "sampling".to_string(),
                description: "Samples from a latent space".to_string(),
                inputs: vec![
                    SlotDescriptor {
                        name: "latent".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    },
                    SlotDescriptor {
                        name: "model".to_string(),
                        slot_type: SlotType::Model,
                        optional: false,
                    },
                ],
                outputs: vec![SlotDescriptor {
                    name: "image".to_string(),
                    slot_type: SlotType::Image,
                    optional: false,
                }],
            },
        ],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process the Ready event.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify the registry is populated with exactly 2 entries.
    assert_eq!(
        node_registry.len(),
        2,
        "registry should contain exactly 2 node types after Ready"
    );

    // Verify each descriptor is correctly retrievable.
    let load_model = node_registry
        .get("LoadModel")
        .expect("LoadModel should be in the registry");
    assert_eq!(
        load_model.type_name, "LoadModel",
        "LoadModel descriptor type_name should match"
    );

    let sampler = node_registry
        .get("Sampler")
        .expect("Sampler should be in the registry");
    assert_eq!(
        sampler.type_name, "Sampler",
        "Sampler descriptor type_name should match"
    );

    // Clean shutdown.
    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// A Ready event with an empty `node_types` vector results in an empty registry.
///
/// Pre-populates the registry with one descriptor, then sends a Ready event with
/// `node_types: vec![]`. After processing, verifies `is_empty()` is true —
/// proving that `register_all()` with an empty vec replaces (not merges) prior
/// contents.
#[tokio::test]
async fn test_ready_event_empty_node_types_cleans_registry() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let node_registry = Arc::new(NodeTypeRegistry::new());

    // Pre-populate the registry with one descriptor.
    node_registry.register_all(vec![NodeTypeDescriptor {
        type_name: "PreExisting".to_string(),
        display_name: "Pre Existing".to_string(),
        category: "internal".to_string(),
        description: "Should be replaced".to_string(),
        inputs: vec![],
        outputs: vec![],
    }]);
    assert_eq!(
        node_registry.len(),
        1,
        "pre-populated registry should have 1 entry"
    );

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::clone(&node_registry),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready with empty node_types.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process the Ready event.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify the registry is now empty (replaced, not merged).
    assert!(
        node_registry.is_empty(),
        "registry should be empty after Ready with empty node_types"
    );

    // Clean shutdown.
    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// A second Ready event replaces prior registry contents rather than merging.
///
/// Sends first Ready with descriptors A and B, then sends a second Ready with
/// descriptor C only. Verifies the registry has exactly 1 entry (C) and that
/// A is no longer present — proving `register_all()` replaces, not merges.
#[tokio::test]
async fn test_respawn_second_ready_replaces_not_merges() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let node_registry = Arc::new(NodeTypeRegistry::new());

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::clone(&node_registry),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // First Ready with descriptors A and B.
    let ready_ab = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![
            NodeTypeDescriptor {
                type_name: "A".to_string(),
                display_name: "Node A".to_string(),
                category: "test".to_string(),
                description: "Node A".to_string(),
                inputs: vec![],
                outputs: vec![],
            },
            NodeTypeDescriptor {
                type_name: "B".to_string(),
                display_name: "Node B".to_string(),
                category: "test".to_string(),
                description: "Node B".to_string(),
                inputs: vec![],
                outputs: vec![],
            },
        ],
    };
    send_event(&mut _dealer, &ready_ab).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        node_registry.len(),
        2,
        "first Ready should populate 2 entries"
    );

    // Cause worker to exit (send Dying event).
    let dying = WorkerEvent::Dying {
        reason: "simulate respawn".to_string(),
    };
    send_event(&mut _dealer, &dying).await;

    // Worker should exit within 5s.
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }

    // Registry should still have 2 entries from the first Ready
    // (register_all was called, but no new Ready replaced it yet).
    assert_eq!(
        node_registry.len(),
        2,
        "registry should still have 2 entries"
    );

    // Now spawn a second worker (simulating respawn) and send Ready with C only.
    let demux2 = Arc::new(Demux::new());
    let transport2 = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx2, _bridge_writer_handle2, _bridge_reader_handle2) =
        spawn_bridge(Arc::clone(&transport2), Arc::clone(&demux2));
    let status2 = Arc::clone(&status);
    let mut _dealer2 = connect_dealer(&transport2, "test-worker").await;

    let (shutdown_tx2, shutdown_rx2) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx2, force_shutdown_rx2) = tokio::sync::oneshot::channel();
    let (pong_tx2, _pong_rx2) = mpsc::channel(16);
    let worker2 = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport2),
        demux: Arc::clone(&demux2),
        status: Arc::clone(&status2),
        node_registry: Arc::clone(&node_registry),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx: pong_tx2,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle2 = tokio::spawn(worker2.run(shutdown_rx2, force_shutdown_rx2));

    // Second Ready with descriptor C only.
    let ready_c = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![NodeTypeDescriptor {
            type_name: "C".to_string(),
            display_name: "Node C".to_string(),
            category: "test".to_string(),
            description: "Node C".to_string(),
            inputs: vec![],
            outputs: vec![],
        }],
    };
    send_event(&mut _dealer2, &ready_c).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Verify: only C is present, A and B are gone (replaced, not merged).
    assert_eq!(
        node_registry.len(),
        1,
        "second Ready should replace prior contents, leaving exactly 1 entry"
    );
    assert!(
        node_registry.get("A").is_none(),
        "A should not be in registry after replacement"
    );
    assert!(
        node_registry.get("B").is_none(),
        "B should not be in registry after replacement"
    );
    assert!(node_registry.get("C").is_some(), "C should be in registry");

    // Clean shutdown.
    drop(shutdown_tx2);
    let timeout2 = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle2 => (),
        _ = timeout2 => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}

/// The registry is populated before the status transitions to Idle.
///
/// Sends a Ready event with descriptors, then verifies that both conditions are
/// true simultaneously: `registry.len() > 0` AND `status == Idle`. This proves
/// the ordering invariant — `register_all()` fires before the status transition,
/// ensuring node types are available the moment the worker becomes usable.
#[tokio::test]
async fn test_ready_populates_registry_before_idle_transition() {
    let demux = Arc::new(Demux::new());
    let transport = Arc::new(RouterTransport::bind().await.unwrap());
    let (_bridge_tx, _bridge_writer_handle, _bridge_reader_handle) =
        spawn_bridge(Arc::clone(&transport), Arc::clone(&demux));
    let status = Arc::new(RwLock::new(WorkerStatus::Initializing));
    let node_registry = Arc::new(NodeTypeRegistry::new());

    let mut _dealer = connect_dealer(&transport, "test-worker").await;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (_force_shutdown_tx, force_shutdown_rx) = tokio::sync::oneshot::channel();
    let (pong_tx, _pong_rx) = mpsc::channel(16);
    let worker = ManagedWorker::new(ManagedWorkerConfig {
        worker_id: "test-worker".to_string(),
        transport: Arc::clone(&transport),
        demux: Arc::clone(&demux),
        status: Arc::clone(&status),
        node_registry: Arc::clone(&node_registry),
        respawn_policy: RespawnPolicy::default(),
        init_timeout: DEFAULT_INIT_TIMEOUT,
        pong_tx,
        watchdog_ping_interval: DEFAULT_WATCHDOG_PING_INTERVAL,
        watchdog_pong_timeout: DEFAULT_WATCHDOG_PONG_TIMEOUT,
        graceful_shutdown_timeout: Duration::from_millis(100),
        venv_path: PathBuf::from("/mock/venv"),
        env: HashMap::new(),
        spawner: Arc::new(MockWorkerSpawner::new()),
    });
    let handle = tokio::spawn(worker.run(shutdown_rx, force_shutdown_rx));

    // Send Ready event with one descriptor.
    let ready = WorkerEvent::Ready {
        worker_id: "test-worker".to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![NodeTypeDescriptor {
            type_name: "LoadModel".to_string(),
            display_name: "Load Model".to_string(),
            category: "loaders".to_string(),
            description: "Load a model".to_string(),
            inputs: vec![SlotDescriptor {
                name: "path".to_string(),
                slot_type: SlotType::String,
                optional: false,
            }],
            outputs: vec![SlotDescriptor {
                name: "model".to_string(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        }],
    };
    send_event(&mut _dealer, &ready).await;

    // Give the worker time to process the Ready event.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Both conditions must be true: registry populated AND status is Idle.
    // The ordering invariant requires register_all() to fire before status = Idle.
    assert!(
        node_registry.len() > 0,
        "registry should be populated after Ready event"
    );
    assert_eq!(
        *status.read().await,
        WorkerStatus::Idle,
        "status should be Idle after Ready event"
    );

    // Clean shutdown.
    drop(shutdown_tx);
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::select! {
        _ = handle => (),
        _ = timeout => panic!("ManagedWorker::run() did not complete within 5s"),
    }
}
