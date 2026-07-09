//! Demultiplexes incoming `WorkerEvent`s to per-worker channels, and
//! additionally fans them out to any number of pool-wide subscribers.
//!
//! Maps `worker_id → tokio::sync::mpsc::Sender<WorkerEvent>` so that the IPC bridge
//! task can route each inbound event to the correct primary consumer without
//! blocking on concurrent senders.
//!
//! See `ANVILML_DESIGN.md §9.4` for the mandatory register/deregister pairing rule:
//! every `register()` call must have a matching `deregister()` call on every exit
//! path (graceful shutdown, crash, timeout).
//!
//! See `ANVILML_DESIGN.md §9.8` (`docs/ADDENDUM_DEMUX_FANOUT.md`) for the
//! `subscribe()`/`unsubscribe()` fan-out mechanism: this is the *only*
//! sanctioned way for a second subsystem (e.g. `anvilml-scheduler`'s event
//! loop) to observe `WorkerEvent`s without racing `bridge.rs`'s `reader_task`
//! for `RouterTransport::recv()` on the shared, pool-wide socket.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use anvilml_core::AnvilError;
use anvilml_ipc::WorkerEvent;
use tokio::sync::mpsc::{Receiver, Sender, channel};

/// Bounded channel capacity for each fan-out subscriber created via
/// `subscribe()`. Sized generously above normal per-tick event volume (a
/// handful of `Progress`/terminal events per job) so a subscriber only ever
/// hits the bound under a genuine stall, not ordinary bursts. Not exposed as
/// a constructor parameter — no caller has needed a different value yet, and
/// `ANVILML_DESIGN.md §9.8` doesn't specify one; add a parameter if that
/// changes rather than guessing a second value now.
const SUBSCRIBER_CHANNEL_CAPACITY: usize = 256;

/// Opaque handle returned by `Demux::subscribe()`, used to later remove that
/// subscription via `Demux::unsubscribe()`. Not `Clone` — a subscriber owns
/// exactly one id for its own subscription's lifetime.
pub type SubscriptionId = u64;

/// Demultiplexes `WorkerEvent`s to per-worker channels, and fans the same
/// events out to any number of independent, pool-wide subscribers.
///
/// Holds two independently-locked tables:
/// - `inner`: the primary per-worker routing table. Each `register()` call
///   inserts (or overwrites, if the worker ID already exists) a sender. Each
///   `deregister()` call removes the entry. Unchanged from the original
///   single-consumer design (`ANVILML_DESIGN.md §9.4`).
/// - `subscribers`: an independent, pool-wide fan-out list, added per
///   `ANVILML_DESIGN.md §9.8`. Every event passed to `route()` is cloned and
///   best-effort delivered to every entry here, in addition to the primary
///   delivery `inner` already performs. A subscriber is not tied to any one
///   `worker_id` — it receives events from every worker.
///
/// The `route()` method is async because it awaits on the primary channel's
/// `send()`. It clones the sender (cheap — just an Arc increment) before
/// unlocking the mutex, so the mutex hold time is bounded and does not block
/// other workers. Fan-out to subscribers uses `try_send()` (non-blocking) —
/// see `route()`'s own doc comment for why blocking there would be unsafe.
pub struct Demux {
    /// Maps worker_id to the channel sender for that worker.
    ///
    /// Protected by a `Mutex` — the lock is held only for the map lookup/insert
    /// and is released before any `.await` point, per the async discipline in
    /// `ANVILML_DESIGN.md §4.7`.
    inner: Mutex<HashMap<String, Sender<WorkerEvent>>>,

    /// Maps subscription id to that subscriber's channel sender. See this
    /// struct's own doc comment and `ANVILML_DESIGN.md §9.8`.
    ///
    /// Same locking discipline as `inner`: the lock is held only for the
    /// map operation itself, never across an `.await` point. Fan-out uses
    /// `try_send()`, which doesn't await at all, so in practice the lock
    /// here is held only for the lookup/clone, exactly like `inner`.
    subscribers: Mutex<HashMap<SubscriptionId, Sender<(String, WorkerEvent)>>>,

    /// Monotonically increasing counter for allocating fresh
    /// `SubscriptionId`s. `Relaxed` ordering is sufficient — this only needs
    /// to hand out distinct values, not synchronize any other memory access.
    next_subscription_id: AtomicU64,
}

impl Default for Demux {
    fn default() -> Self {
        Self::new()
    }
}

impl Demux {
    /// Creates a new empty `Demux`.
    ///
    /// The primary routing table starts empty; workers must `register()`
    /// before events can be routed to them. The subscriber list also starts
    /// empty; callers must `subscribe()` before receiving fan-out events.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            subscribers: Mutex::new(HashMap::new()),
            next_subscription_id: AtomicU64::new(0),
        }
    }

    /// Registers a new pool-wide fan-out subscriber, independent of the
    /// per-worker `register()`/`route()` table.
    ///
    /// Returns the new subscription's id (pass to `unsubscribe()` to remove
    /// it) and the receiving half of a bounded channel. From this call
    /// onward, until `unsubscribe()` is called (or this `Demux` itself is
    /// dropped), every event passed to `route()` — for every `worker_id` —
    /// is cloned and best-effort delivered here, tagged with the source
    /// `worker_id` since a subscriber is not scoped to any single worker.
    ///
    /// # Returns
    /// A `(SubscriptionId, Receiver<(String, WorkerEvent)>)` pair.
    pub fn subscribe(&self) -> (SubscriptionId, Receiver<(String, WorkerEvent)>) {
        let (tx, rx) = channel(SUBSCRIBER_CHANNEL_CAPACITY);
        // `Relaxed` is sufficient: this only needs uniqueness across calls,
        // not ordering relative to any other atomic or memory operation.
        let id = self.next_subscription_id.fetch_add(1, Ordering::Relaxed);
        let mut map = self
            .subscribers
            .lock()
            .expect("mutex poisoned — this should never happen");
        map.insert(id, tx);
        (id, rx)
    }

    /// Removes a fan-out subscription created by `subscribe()`.
    ///
    /// Safe to call with an id that was already removed (e.g. by a previous
    /// `unsubscribe()` call, or never existed) — this is a no-op in that
    /// case, matching `deregister()`'s existing idempotency convention.
    ///
    /// # Arguments
    /// * `id` — The subscription id returned by `subscribe()`.
    pub fn unsubscribe(&self, id: SubscriptionId) {
        let mut map = self
            .subscribers
            .lock()
            .expect("mutex poisoned — this should never happen");
        map.remove(&id);
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

    /// Routes an event to the worker identified by `worker_id`, and
    /// best-effort fans a clone out to every active `subscribe()`r.
    ///
    /// Looks up the worker in the primary routing table, clones the sender, unlocks
    /// the mutex, then awaits on `tx.send(event)`. If the send fails (receiver
    /// dropped), returns an `Ipc` error. If the worker is not found, returns
    /// `WorkerNotFound`. This primary-delivery behavior is unchanged from before
    /// `ANVILML_DESIGN.md §9.8`'s fan-out addition.
    ///
    /// Subscriber fan-out (`ANVILML_DESIGN.md §9.8`) happens first, via
    /// `fan_out_to_subscribers()`, and is entirely independent of primary
    /// delivery's outcome: subscribers still see the event even if no worker
    /// is currently `register()`ed for `worker_id` (e.g. between a crash and
    /// respawn), and a failure fanning out to subscribers never affects the
    /// `Result` this method returns.
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
        // Fan out to subscribers before attempting primary delivery — see
        // this method's own doc comment for why the two are independent.
        // Cloning the event once per subscriber here (rather than sharing
        // one clone) keeps each subscriber's channel fully decoupled from
        // the others: one subscriber's `try_send` outcome can never affect
        // what another subscriber, or the primary consumer below, receives.
        self.fan_out_to_subscribers(worker_id, &event);

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

    /// Best-effort delivers a clone of `event` to every active `subscribe()`r,
    /// tagged with `worker_id`. Never blocks and never returns an error to the
    /// caller — see this struct's own doc comment and `ANVILML_DESIGN.md §9.8`
    /// for why a slow or dead subscriber must not be able to stall `route()`.
    ///
    /// Clones each subscriber's `Sender` while holding the lock, then drops
    /// the lock before calling `try_send()` on any of them — the same
    /// clone-before-send discipline `route()` itself uses for the primary
    /// table, applied here even though `try_send()` doesn't await, purely for
    /// consistency and to keep the lock's hold time minimal regardless of how
    /// many subscribers are currently registered.
    fn fan_out_to_subscribers(&self, worker_id: &str, event: &WorkerEvent) {
        let senders: Vec<(SubscriptionId, Sender<(String, WorkerEvent)>)> = {
            let map = self
                .subscribers
                .lock()
                .expect("mutex poisoned — this should never happen");
            map.iter().map(|(id, tx)| (*id, tx.clone())).collect()
        };

        for (id, tx) in senders {
            // try_send() is non-blocking: a full channel (slow subscriber) or
            // a closed channel (dropped subscriber that never called
            // unsubscribe()) both simply drop this one event for that one
            // subscriber, logged at WARN — never propagated as an error from
            // route(), and never allowed to delay delivery to any other
            // subscriber or to the primary consumer.
            if let Err(err) = tx.try_send((worker_id.to_string(), event.clone())) {
                tracing::warn!(
                    subscription_id = id,
                    worker_id = %worker_id,
                    error = %err,
                    "demux fan-out: event dropped for subscriber (full or closed channel)"
                );
            }
        }
    }
}
