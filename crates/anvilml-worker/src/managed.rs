//! A cheap, `Clone`-able handle for interacting with a worker's lifecycle.
//!
//! Each `WorkerHandle` owns an `Arc`-reference to the worker's status lock,
//! a `oneshot::Sender` for requesting shutdown, and an `Arc<Mutex<Option<JoinHandle>>>`
//! for tracking the worker task. Cloning a handle produces a new handle that shares
//! the same status lock and join handle — both clones observe the same status and can
//! request the same shutdown. The `worker_id` field is copied (not shared) across clones.
//!
//! This handle does **not** own the `ManagedWorker` struct itself — it is a lightweight
//! view into the worker's shared state, designed to be freely shared across tasks and
//! API handlers without `Arc`-wrapping the full worker.

use std::sync::Arc;

use tokio::sync::RwLock;

use anvilml_core::types::worker::WorkerStatus;

/// A cheap, `Clone`-able handle for interacting with a worker's lifecycle.
///
/// Each `WorkerHandle` owns an `Arc`-reference to the worker's status lock,
/// a `oneshot::Sender` for requesting shutdown, and an `Arc<Mutex<Option<JoinHandle>>>`
/// for tracking the worker task. Cloning a handle produces a new handle that shares
/// the same status lock and join handle — both clones observe the same status and can
/// request the same shutdown. The `worker_id` field is copied (not shared) across clones.
///
/// This handle does **not** own the `ManagedWorker` struct itself — it is a lightweight
/// view into the worker's shared state, designed to be freely shared across tasks and
/// API handlers without `Arc`-wrapping the full worker.
pub struct WorkerHandle {
    /// Stable worker identity — the bare device index as a string (e.g. `"0"`).
    /// Copied, not shared, across clones.
    pub worker_id: String,

    /// Shared status lock — all clones read from the same lock.
    /// Private: consumers must use `status()` to read the current state.
    status: Arc<RwLock<WorkerStatus>>,

    /// Optional shutdown trigger — `take()`n on the first call to `request_shutdown()`,
    /// making the operation idempotent (second call is a no-op).
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// Shared join handle wrapper — allows the pool to extract and await the handle
    /// during shutdown with a bounded timeout.
    join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

// `shutdown_tx` (oneshot::Sender) is not Clone — clones cannot request shutdown.
// Only the original handle retains the ability to trigger shutdown.
impl Clone for WorkerHandle {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            status: Arc::clone(&self.status),
            // Clone cannot take the sender — the clone loses the ability to
            // request shutdown, preserving the invariant that only the original
            // handle can trigger it.
            shutdown_tx: None,
            join_handle: Arc::clone(&self.join_handle),
        }
    }
}

impl WorkerHandle {
    /// Construct a new `WorkerHandle` from its component parts.
    ///
    /// # Arguments
    ///
    /// * `worker_id` — Stable worker identity string (e.g. `"0"`). Copied into the handle.
    /// * `status` — Shared `Arc<RwLock<WorkerStatus>>` for the worker's lifecycle state.
    ///   All clones of this handle share this same lock.
    /// * `shutdown_tx` — Optional `oneshot::Sender` used to signal the worker to shut down.
    ///   If `None`, `request_shutdown()` becomes a no-op.
    /// * `join_handle` — Shared `Arc<Mutex<Option<JoinHandle>>>` for tracking the worker task.
    pub fn new(
        worker_id: String,
        status: Arc<RwLock<WorkerStatus>>,
        shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
        join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    ) -> Self {
        Self {
            worker_id,
            status,
            shutdown_tx,
            join_handle,
        }
    }

    /// Read the worker's current lifecycle state.
    ///
    /// Acquires a read lock on the shared status, copies the `Copy` value,
    /// and releases the lock. The caller never holds the lock.
    pub async fn status(&self) -> WorkerStatus {
        *self.status.read().await
    }

    /// Set the worker's lifecycle state.
    ///
    /// Acquires a write lock on the shared status, overwrites the stored value,
    /// and releases the lock. This is the only public mutator on `WorkerHandle`.
    ///
    /// # Arguments
    ///
    /// * `new` — The new `WorkerStatus` value to set.
    pub async fn set_status(&self, new: WorkerStatus) {
        *self.status.write().await = new;
    }

    /// Request the worker to shut down gracefully.
    ///
    /// Takes the `oneshot::Sender` from `shutdown_tx` and sends `()` to the receiver.
    /// If `shutdown_tx` is already `None` (already called), this is a no-op — making
    /// the method idempotent. The result of `tx.send(())` is ignored because the
    /// receiver may already have been dropped (worker already exited).
    pub fn request_shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            // Ignore the result — the receiver may already be dropped if the worker
            // has already exited. This makes request_shutdown idempotent.
            let _ = tx.send(());
        }
    }
}
