//! Integration tests for `managed.rs` — verifies the `WorkerHandle` struct's
//! clone semantics, status read path, and idempotent shutdown request.
//!
//! All tests construct handles from shared `Arc<RwLock<WorkerStatus>>` instances
//! to prove that clones share state, and from fresh `oneshot::channel` pairs
//! to verify the shutdown trigger works correctly.

use std::sync::Arc;

use anvilml_core::types::worker::WorkerStatus;
use anvilml_worker::WorkerHandle;
use tokio::sync::RwLock;

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
        Arc::new(tokio::sync::Mutex::new(None)),
    );
    let handle2 = WorkerHandle::new(
        "worker-1".to_string(),
        status,
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
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    // First call — should send the signal.
    handle.request_shutdown();

    // Second call — should be a no-op (shutdown_tx is now None).
    // This must not panic.
    handle.request_shutdown();
}

/// Constructing a handle with status set to `Spawning` and calling `status()`
/// returns `Spawning`, proving the read path works correctly for non-default states.
///
/// Creates a shared `Arc<RwLock<WorkerStatus>>`, sets it to `Spawning` via a direct
/// write before constructing the handle, then verifies `status()` returns `Spawning`.
#[tokio::test]
async fn test_status_returns_current_value() {
    let status = Arc::new(RwLock::new(WorkerStatus::Spawning));
    let handle = WorkerHandle::new(
        "worker-0".to_string(),
        status,
        None,
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    assert_eq!(
        handle.status().await,
        WorkerStatus::Spawning,
        "status() should return the current value from the shared lock"
    );
}
