//! Integration tests for `spawn_stats_tick()` — the periodic `SystemStats`
//! background publisher.
//!
//! All tests inject a millisecond-scale interval so they run quickly and
//! deterministically, rather than waiting on the 5-second production
//! default — the exact testability rationale `spawn_stats_tick()`'s own
//! doc comment gives for taking `interval` as a parameter.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anvilml_core::WsEvent;
use anvilml_core::types::hardware::{CapabilitySource, EnumerationSource};
use anvilml_core::types::worker::WorkerStatus;
use anvilml_core::{DeviceType, GpuDevice, InferenceCaps};
use anvilml_ipc::EventBroadcaster;
use anvilml_server::ws::spawn_stats_tick;
use anvilml_worker::{WorkerHandle, WorkerPool};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle as TokioJoinHandle;

/// Build a `GpuDevice` stub with the given index/type — the fields beyond
/// those two don't matter for these tests, only that the struct is valid.
fn make_test_device(index: u32, device_type: DeviceType) -> GpuDevice {
    GpuDevice {
        index,
        name: format!("Mock GPU {index}"),
        device_type,
        vram_total_mib: 16384,
        vram_free_mib: 16384,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: InferenceCaps::default(),
        enumeration_source: EnumerationSource::Mock,
        capabilities_source: CapabilitySource::DeviceTable,
    }
}

/// Build a `WorkerHandle` with a controllable, pre-set status. Mirrors the
/// helper pattern established in
/// `crates/anvilml-scheduler/tests/scheduler_tests.rs`.
fn make_test_handle(worker_id: &str, status: WorkerStatus) -> WorkerHandle {
    let status = Arc::new(RwLock::new(status));
    let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
    let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
    let join_handle: Arc<Mutex<Option<TokioJoinHandle<()>>>> = Arc::new(Mutex::new(None));
    WorkerHandle::new(
        worker_id.into(),
        status,
        Some(shutdown_tx),
        Some(force_shutdown_tx),
        join_handle,
    )
}

/// Construct a `WorkerPool` with no workers — sufficient for tests that
/// only care about a tick being published, not about its `workers` field.
async fn make_empty_pool() -> Arc<WorkerPool> {
    Arc::new(
        WorkerPool::new()
            .await
            .expect("WorkerPool::new() must succeed in test"),
    )
}

/// Construct a `WorkerPool` pre-populated with `(WorkerHandle, GpuDevice)`
/// pairs via `set_up_test_workers()` (`test-utils` feature) — no real
/// worker subprocess is spawned.
async fn make_pool_with_workers(workers: Vec<(WorkerHandle, GpuDevice)>) -> Arc<WorkerPool> {
    let mut pool = WorkerPool::new()
        .await
        .expect("WorkerPool::new() must succeed in test");
    pool.set_up_test_workers(workers);
    Arc::new(pool)
}

/// A tick publishes a `WsEvent::SystemStats` observable by a subscriber
/// that subscribed before the tick task was spawned.
#[tokio::test]
async fn test_tick_publishes_system_stats() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();
    let workers = make_empty_pool().await;

    let handle = spawn_stats_tick(
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
        Duration::from_millis(20),
    );

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out waiting for the first tick")
        .expect("broadcast channel closed unexpectedly");

    assert!(
        matches!(event, WsEvent::SystemStats { .. }),
        "expected WsEvent::SystemStats, got {event:?}"
    );

    handle.abort();
}

/// The published `SystemStats.workers` field reflects the pool's actual
/// worker state — not the empty placeholder `ws/handler.rs`'s (`P16-C1`)
/// per-connection initial frame always sends.
#[tokio::test]
async fn test_workers_reflect_pool_state() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();

    let workers = make_pool_with_workers(vec![
        (
            make_test_handle("0", WorkerStatus::Idle),
            make_test_device(0, DeviceType::Cuda),
        ),
        (
            make_test_handle("1", WorkerStatus::Busy),
            make_test_device(1, DeviceType::Cpu),
        ),
    ])
    .await;

    let handle = spawn_stats_tick(
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
        Duration::from_millis(20),
    );

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out waiting for the first tick")
        .expect("broadcast channel closed unexpectedly");

    let WsEvent::SystemStats {
        workers: worker_infos,
        ..
    } = event
    else {
        panic!("expected WsEvent::SystemStats");
    };

    assert_eq!(worker_infos.len(), 2, "expected both test workers listed");

    let w0 = worker_infos
        .iter()
        .find(|w| w.worker_id == "0")
        .expect("worker \"0\" must be present");
    assert_eq!(w0.status, WorkerStatus::Idle);
    assert_eq!(w0.device_index, 0);
    assert_eq!(w0.device_type, DeviceType::Cuda);

    let w1 = worker_infos
        .iter()
        .find(|w| w.worker_id == "1")
        .expect("worker \"1\" must be present");
    assert_eq!(w1.status, WorkerStatus::Busy);
    assert_eq!(w1.device_index, 1);
    assert_eq!(w1.device_type, DeviceType::Cpu);

    handle.abort();
}

/// Two consecutive ticks both publish a `SystemStats` event — proving the
/// task runs an ongoing periodic loop, not a single one-shot send.
#[tokio::test]
async fn test_two_consecutive_ticks_both_publish() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();
    let workers = make_empty_pool().await;

    let handle = spawn_stats_tick(
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
        Duration::from_millis(15),
    );

    for tick_number in 1..=2 {
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for tick {tick_number}"))
            .expect("broadcast channel closed unexpectedly");
        assert!(
            matches!(event, WsEvent::SystemStats { .. }),
            "tick {tick_number}: expected WsEvent::SystemStats, got {event:?}"
        );
    }

    handle.abort();
}

/// The injected `interval` genuinely controls tick cadence — a short
/// interval yields several ticks well within a second, which would be
/// impossible if the loop silently used a hardcoded 5-second period
/// instead of the constructor parameter.
#[tokio::test]
async fn test_interval_parameter_controls_cadence() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();
    let workers = make_empty_pool().await;

    let started = Instant::now();
    let handle = spawn_stats_tick(
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
        Duration::from_millis(10),
    );

    for _ in 1..=3 {
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for a tick")
            .expect("broadcast channel closed unexpectedly");
    }

    assert!(
        started.elapsed() < Duration::from_millis(800),
        "3 ticks at a 10ms interval took {:?} — interval parameter appears \
         to not be controlling cadence (a hardcoded 5s period would time \
         this test out well before reaching this assertion)",
        started.elapsed()
    );

    handle.abort();
}

/// The published stats are real host data, not the always-zero placeholder
/// `ws/handler.rs`'s per-connection initial frame sends (`P16-C1`) — a
/// running test process always has nonzero resident memory.
#[tokio::test]
async fn test_stats_are_real_data_not_the_c1_placeholder() {
    let broadcaster = Arc::new(EventBroadcaster::new());
    let mut rx = broadcaster.subscribe();
    let workers = make_empty_pool().await;

    let handle = spawn_stats_tick(
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
        Duration::from_millis(20),
    );

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out waiting for the first tick")
        .expect("broadcast channel closed unexpectedly");

    let WsEvent::SystemStats { ram_used_mib, .. } = event else {
        panic!("expected WsEvent::SystemStats");
    };

    assert!(
        ram_used_mib > 0,
        "a running process's host must report nonzero used RAM — got 0, \
         which matches P16-C1's placeholder rather than real sysinfo data"
    );

    handle.abort();
}
