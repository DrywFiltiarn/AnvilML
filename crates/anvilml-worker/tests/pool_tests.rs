//! Integration tests for `WorkerPool` (P8-G1).
//!
//! Uses a local `MockWorkerSpawner` (matching `managed_tests.rs`'s own
//! established pattern — each integration test file is its own separate
//! crate, so these can't be shared without an explicit `tests/common/`
//! module, which this codebase doesn't already use) since
//! `WorkerPool::spawn_all()`'s real path always constructs a
//! `ProcessWorkerSpawner`, which launches a real Python interpreter from a
//! real virtualenv — nothing a test environment has. `spawn_all_with_spawner()`
//! (`test-utils`-gated) is what these tests actually call.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anvilml_core::types::hardware::{
    CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps,
};
use anvilml_core::{AnvilError, ServerConfig};
use anvilml_ipc::{RouterTransport, WorkerEvent};
use anvilml_worker::{WorkerPool, WorkerSpawner};
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

/// Minimal `GpuDevice`, differing only by `index` — no test here inspects
/// any other field, so the rest are filled with simple, valid placeholder
/// values.
fn mock_device(index: u32) -> GpuDevice {
    GpuDevice {
        index,
        name: format!("Mock GPU {index}"),
        device_type: DeviceType::Cpu,
        vram_total_mib: 1024,
        vram_free_mib: 900,
        driver_version: "0.0.0".to_string(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: None,
        caps: InferenceCaps::default(),
        enumeration_source: EnumerationSource::Mock,
        capabilities_source: CapabilitySource::Fallback,
    }
}

/// Spawns a real, long-lived, harmless process (`sleep 999` / `cmd timeout
/// 999`) — matching `managed_tests.rs`'s own established `MockWorkerSpawner`
/// in spirit (tracking spawned PIDs for observation independently of
/// whatever `ManagedWorker`/`WorkerPool` do with the `Child` handle
/// afterward), but tracking every PID via `pids()` rather than only the
/// most recent one via `last_pid()` — tests here share one spawner across
/// multiple devices, so only the most recent PID would leave earlier
/// devices' processes unverifiable.
struct MockWorkerSpawner {
    call_count: AtomicUsize,
    /// Every PID this spawner has produced, in spawn order. Not just the
    /// most recent one: tests here share one spawner across multiple
    /// devices, so a single `Option<u32>` would only ever retain the
    /// last device's PID, silently losing the ability to verify earlier
    /// devices' processes actually died.
    pids: std::sync::Mutex<Vec<u32>>,
}

impl MockWorkerSpawner {
    fn new() -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            pids: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    /// Every PID spawned so far, in spawn order.
    fn pids(&self) -> Vec<u32> {
        self.pids.lock().expect("pids mutex poisoned").clone()
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
            if let Ok(ref child) = result
                && let Some(pid) = child.id()
            {
                self.pids.lock().expect("pids mutex poisoned").push(pid);
            }
            result
        })
    }
}

#[cfg(target_os = "linux")]
fn pid_is_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(target_os = "windows")]
fn pid_is_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut exit_code = 0u32;
        let alive = GetExitCodeProcess(handle, &mut exit_code).is_ok()
            && exit_code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(handle);
        alive
    }
}

/// Connect a DEALER socket to a `RouterTransport`'s bound endpoint —
/// matching `managed_tests.rs`'s own established `connect_dealer` helper.
async fn connect_dealer(transport: &RouterTransport, worker_id: &str) -> DealerSocket {
    let mut opts = SocketOptions::default();
    opts.peer_identity(
        PeerIdentity::try_from(bytes::Bytes::from(worker_id.to_string())).expect("valid identity"),
    );
    let mut dealer = DealerSocket::with_options(opts);
    let endpoint = format!("tcp://127.0.0.1:{}", transport.port);
    dealer
        .connect(&endpoint)
        .await
        .expect("DEALER connect to ROUTER should succeed");
    tokio::time::sleep(Duration::from_millis(50)).await;
    dealer
}

async fn send_event(dealer: &mut DealerSocket, event: &WorkerEvent) {
    let payload = rmp_serde::to_vec_named(event).expect("event should serialize");
    let mut msg = ZmqMessage::from(bytes::Bytes::from(""));
    msg.push_back(bytes::Bytes::from(payload));
    dealer.send(msg).await.expect("DEALER send should succeed");
}

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

/// `WorkerPool::new()` produces an empty pool — no workers exist until
/// `spawn_all()`/`spawn_all_with_spawner()` is called.
#[tokio::test]
async fn test_new_creates_empty_pool() {
    let pool = WorkerPool::new().await.expect("new() should succeed");
    assert!(
        pool.handles().is_empty(),
        "a freshly-constructed pool should have no worker handles yet"
    );
}

/// `spawn_all_with_spawner()` creates exactly one `WorkerHandle` per
/// device, with `worker_id` matching each device's index.
#[tokio::test]
async fn test_spawn_all_creates_one_handle_per_device() {
    let mut pool = WorkerPool::new().await.expect("new() should succeed");
    let spawner = Arc::new(MockWorkerSpawner::new());
    let devices = vec![mock_device(0), mock_device(1), mock_device(2)];
    let cfg = ServerConfig::default();
    let node_registry = Arc::new(anvilml_core::NodeTypeRegistry::new());

    pool.spawn_all_with_spawner(
        &devices,
        &cfg,
        Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
        Arc::clone(&node_registry),
    )
    .await
    .expect("spawn_all_with_spawner() should succeed");

    assert_eq!(
        pool.handles().len(),
        3,
        "should have exactly one handle per device"
    );

    // spawn_all_with_spawner() only schedules each worker's run() task via
    // tokio::spawn() — it does not itself wait for that task to actually
    // be polled. The real spawner.spawn() call happens asynchronously
    // inside run_once(), once the runtime gets around to running that
    // specific task, which can genuinely be after this method's own
    // .await has already resolved. Poll with a bound rather than
    // asserting immediately — matching the same established pattern used
    // elsewhere in this codebase for identical async-timing situations.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if spawner.call_count() >= 3 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("all 3 devices should have called spawn() within 2s");

    assert_eq!(
        spawner.call_count(),
        3,
        "spawn() should have been called exactly once per device"
    );

    // handles() now returns an owned Vec<WorkerHandle> (P18-D2/D3 gave
    // WorkerPool's handles field interior mutability, which means it can
    // no longer hand back a borrow tied to `pool`'s own lifetime — see
    // that method's own doc comment). Collecting owned Strings here
    // (rather than the original &str borrowed from that temporary Vec)
    // is the minimal adjustment this forces: a &str borrowed from
    // handles()'s temporary can't outlive the statement that creates it,
    // but worker_ids is used in the next statement (`.sort()`) below.
    let mut worker_ids: Vec<String> = pool.handles().iter().map(|h| h.worker_id.clone()).collect();
    worker_ids.sort();
    assert_eq!(
        worker_ids,
        vec!["0", "1", "2"],
        "each handle's worker_id should be its device's index"
    );

    // Clean up the long-lived mock processes this test spawned, via the
    // pool's own shutdown_all() (tested more thoroughly on its own below).
    pool.shutdown_all(Duration::from_millis(200)).await;
}

/// All workers spawned by one `spawn_all_with_spawner()` call share the
/// same `RouterTransport`/bridge — proven by successfully routing a
/// `Ready` event to each of two distinct workers over the pool's single
/// shared transport.
#[tokio::test]
async fn test_spawn_all_shares_one_bridge() {
    let mut pool = WorkerPool::new().await.expect("new() should succeed");
    let spawner = Arc::new(MockWorkerSpawner::new());
    let devices = vec![mock_device(0), mock_device(1)];
    let cfg = ServerConfig::default();
    let node_registry = Arc::new(anvilml_core::NodeTypeRegistry::new());

    pool.spawn_all_with_spawner(
        &devices,
        &cfg,
        spawner as Arc<dyn WorkerSpawner>,
        Arc::clone(&node_registry),
    )
    .await
    .expect("spawn_all_with_spawner() should succeed");

    let transport = Arc::clone(pool.transport());
    let mut dealer0 = connect_dealer(&transport, "0").await;
    let mut dealer1 = connect_dealer(&transport, "1").await;

    send_event(&mut dealer0, &ready_event("0")).await;
    send_event(&mut dealer1, &ready_event("1")).await;

    for handle in pool.handles() {
        let worker_id = handle.worker_id.clone();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if handle.status().await == anvilml_core::types::worker::WorkerStatus::Idle {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("worker {worker_id} should reach Idle within 2s"));
    }

    pool.shutdown_all(Duration::from_millis(200)).await;
}

/// `shutdown_all()` awaits every worker's exit — after it returns, no
/// spawned child process should still be running.
#[tokio::test]
async fn test_shutdown_all_awaits_exit() {
    let mut pool = WorkerPool::new().await.expect("new() should succeed");
    let spawner = Arc::new(MockWorkerSpawner::new());
    let devices = vec![mock_device(0), mock_device(1)];
    let cfg = ServerConfig::default();
    let node_registry = Arc::new(anvilml_core::NodeTypeRegistry::new());

    pool.spawn_all_with_spawner(
        &devices,
        &cfg,
        Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
        Arc::clone(&node_registry),
    )
    .await
    .expect("spawn_all_with_spawner() should succeed");

    // Give both workers' own spawn+register sequences time to complete
    // before requesting shutdown.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let pids = spawner.pids();
    assert_eq!(pids.len(), 2, "both devices should have spawned a process");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        pool.shutdown_all(Duration::from_secs(2)),
    )
    .await;
    assert!(
        result.is_ok(),
        "shutdown_all() should complete within its own bounded timeout"
    );

    // Give the OS a moment to actually reap the killed processes —
    // Child::kill() sends the signal but doesn't itself wait for the
    // process to fully terminate, matching the same grace period already
    // established in managed_tests.rs for identical checks.
    tokio::time::sleep(Duration::from_millis(100)).await;

    for pid in pids {
        assert!(
            !pid_is_alive(pid),
            "child (pid {pid}) should have exited once shutdown_all() returned, \
             not been left running"
        );
    }
}

/// A worker whose own `graceful_shutdown_child()` hasn't finished within
/// `shutdown_all()`'s pool-level timeout is force-killed (aborted) rather
/// than blocking the rest of the pool's shutdown.
///
/// `MockWorkerSpawner`'s spawned process never exits on its own, so with
/// the production `DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT` (30s) still in
/// effect for the worker itself, its own internal graceful-shutdown wait
/// is still in progress well past a much shorter pool-level timeout —
/// exactly the straggler scenario this test targets, with no need for any
/// per-device timeout override.
#[tokio::test]
async fn test_shutdown_all_force_kills_straggler() {
    let mut pool = WorkerPool::new().await.expect("new() should succeed");
    let spawner = Arc::new(MockWorkerSpawner::new());
    let devices = vec![mock_device(0)];
    let cfg = ServerConfig::default();
    let node_registry = Arc::new(anvilml_core::NodeTypeRegistry::new());

    pool.spawn_all_with_spawner(
        &devices,
        &cfg,
        Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
        Arc::clone(&node_registry),
    )
    .await
    .expect("spawn_all_with_spawner() should succeed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let pids = spawner.pids();
    assert_eq!(
        pids.len(),
        1,
        "the one device should have spawned a process"
    );
    let pid = pids[0];

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        pool.shutdown_all(Duration::from_millis(200)),
    )
    .await;
    assert!(
        result.is_ok(),
        "shutdown_all() should complete promptly (via the force-shutdown \
         fallback) rather than waiting out the worker's own much longer \
         graceful_shutdown_timeout"
    );

    // Give the OS a moment to actually reap the killed process — same
    // reasoning as test_shutdown_all_awaits_exit's own identical sleep.
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        !pid_is_alive(pid),
        "child (pid {pid}) should have been force-killed via the \
         force_shutdown() signal, not left running — the whole point of \
         this test is proving the straggler's process actually dies, not \
         merely that shutdown_all() returns promptly"
    );
}
