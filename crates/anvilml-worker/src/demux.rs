//! Demultiplexes incoming `WorkerEvent`s to per-worker channels.
//!
//! Maps `worker_id → tokio::sync::mpsc::Sender<WorkerEvent>` so that the IPC bridge
//! task can route each inbound event to the correct consumer without blocking on
//! concurrent senders.
//!
//! See `ANVILML_DESIGN.md §9.4` for the mandatory register/deregister pairing rule:
//! every `register()` call must have a matching `deregister()` call on every exit
//! path (graceful shutdown, crash, timeout).

use std::collections::HashMap;
use std::sync::Mutex;

use anvilml_core::AnvilError;
use anvilml_ipc::WorkerEvent;
use tokio::sync::mpsc::Sender;

/// Demultiplexes `WorkerEvent`s to per-worker channels.
///
/// Holds a mutex-protected map from worker ID to channel sender. Each `register()`
/// call inserts (or overwrites, if the worker ID already exists) a sender. Each
/// `deregister()` call removes the entry.
///
/// The `route()` method is async because it awaits on the channel `send()`. It
/// clones the sender (cheap — just an Arc increment) before unlocking the mutex,
/// so the mutex hold time is bounded and does not block other workers.
pub struct Demux {
    /// Maps worker_id to the channel sender for that worker.
    ///
    /// Protected by a `Mutex` — the lock is held only for the map lookup/insert
    /// and is released before any `.await` point, per the async discipline in
    /// `ANVILML_DESIGN.md §4.7`.
    inner: Mutex<HashMap<String, Sender<WorkerEvent>>>,
}

impl Default for Demux {
    fn default() -> Self {
        Self::new()
    }
}

impl Demux {
    /// Creates a new empty `Demux`.
    ///
    /// The routing table starts empty; workers must `register()` before events
    /// can be routed to them.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Registers a worker with its channel sender.
    ///
    /// If a worker with the same `worker_id` is already registered, the old sender
    /// is replaced by the new one. This is idempotent and safe for respawn scenarios:
    /// the old sender's channel will eventually drain and close, which is harmless.
    ///
    /// # Arguments
    /// * `worker_id` — Stable worker identity (e.g. `"0"`, `"1"`).
    /// * `tx` — Channel sender for receiving events from this worker.
    pub fn register(&self, worker_id: String, tx: Sender<WorkerEvent>) {
        // Lock the mutex, insert or overwrite the entry.
        // The Mutex is short-lived — we only touch the HashMap, no .await inside.
        let mut map = self
            .inner
            .lock()
            .expect("mutex poisoned — this should never happen");
        map.insert(worker_id, tx);
    }

    /// Deregisters a worker, removing its entry from the routing table.
    ///
    /// Returns `true` if the worker was present and removed, `false` if it was
    /// not found. Safe to call on absent entries — returns `false` without error.
    ///
    /// This is the mandatory deregistration path required by `ANVILML_DESIGN.md §9.4`.
    /// `ManagedWorker::run()` must call this on every exit path.
    ///
    /// # Arguments
    /// * `worker_id` — The worker identity to remove.
    ///
    /// # Returns
    /// `true` if an entry was removed, `false` if the worker was not registered.
    pub fn deregister(&self, worker_id: &str) -> bool {
        let mut map = self
            .inner
            .lock()
            .expect("mutex poisoned — this should never happen");
        map.remove(worker_id).is_some()
    }

    /// Queries whether a worker is currently registered in the routing table.
    ///
    /// This is a read-only check — it does not insert or modify any entry.
    /// Used by tests and the pool to verify deregistration after `run()` exits.
    ///
    /// # Arguments
    /// * `worker_id` — The worker identity to look up.
    ///
    /// # Returns
    /// `true` if a sender for this worker_id exists in the map, `false` otherwise.
    pub fn registered(&self, worker_id: &str) -> bool {
        let map = self
            .inner
            .lock()
            .expect("mutex poisoned — this should never happen");
        map.contains_key(worker_id)
    }

    /// Routes an event to the worker identified by `worker_id`.
    ///
    /// Looks up the worker in the routing table, clones the sender, unlocks the
    /// mutex, then awaits on `tx.send(event)`. If the send fails (receiver dropped),
    /// returns an `Ipc` error. If the worker is not found, returns `WorkerNotFound`.
    ///
    /// The clone-before-send pattern ensures the mutex is not held across the
    /// `.await`, preventing deadlock against concurrent `register()`/`deregister()`
    /// calls from other tasks.
    ///
    /// # Arguments
    /// * `worker_id` — The target worker identity.
    /// * `event` — The event to deliver.
    ///
    /// # Errors
    /// Returns `AnvilError::WorkerNotFound` if no worker with that ID is registered.
    /// Returns `AnvilError::Ipc` if the channel send fails (receiver was dropped).
    pub async fn route(&self, worker_id: &str, event: WorkerEvent) -> Result<(), AnvilError> {
        // Clone the sender while holding the lock, then drop the lock before
        // awaiting the channel send. The block scope ensures the MutexGuard
        // is dropped before the `.await` point, per async discipline.
        let tx = {
            let map = self
                .inner
                .lock()
                .expect("mutex poisoned — this should never happen");
            map.get(worker_id)
                .cloned()
                .ok_or_else(|| AnvilError::WorkerNotFound(worker_id.to_string()))?
        };

        // Send the event. If the receiver is gone (worker died), return an IPC error.
        tx.send(event)
            .await
            .map_err(|_| AnvilError::Ipc(format!("send failed for worker {worker_id}")))
    }
}
