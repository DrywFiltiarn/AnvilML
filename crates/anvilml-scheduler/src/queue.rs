/// In-memory FIFO job queue with O(1) cancellation.
///
/// `JobQueue` is a pure in-memory data structure that holds `Job` values in
/// insertion order (`VecDeque`) and tracks cancelled IDs in a `HashSet` for
/// O(1) lookup. `pop_front()` lazily discards cancelled entries it encounters,
/// which is what makes `cancel()` O(1) instead of requiring an O(n) scan.
///
/// This is the data structure the future dispatch loop (a later phase) will
/// pop jobs from. It does not persist to a database — that is handled by
/// `JobStore` in the `anvilml-registry` crate.
use std::collections::{HashSet, VecDeque};

use anvilml_core::Job;
use uuid::Uuid;

/// An in-memory FIFO queue of `Job` values with O(1) cancellation.
///
/// Jobs are stored in a `VecDeque` in insertion order. Cancelled job IDs are
/// tracked in a `HashSet<Uuid>` for O(1) lookup. The `pop_front()` method
/// lazily discards cancelled entries it encounters during iteration — this is
/// the mechanism that makes `cancel()` O(1) rather than requiring an O(n)
/// scan of the deque to remove the cancelled job.
///
/// An `all_ids` set tracks every job ID currently in the queue, allowing
/// `cancel()` to return `false` for IDs that are not present (unknown) rather
/// than blindly inserting them into the cancelled set.
///
/// # Cancel semantics
///
/// Calling `cancel(id)` marks the job as cancelled but does not remove it from
/// the deque. The job remains visible in `list()` and `get()` until
/// `pop_front()` encounters it and discards it. This lazy removal is what
/// gives `cancel()` its O(1) guarantee.
#[derive(Debug, Default)]
pub struct JobQueue {
    /// Jobs in FIFO (insertion) order.
    jobs: VecDeque<Job>,
    /// IDs of jobs that have been cancelled. A job remains in `jobs` until
    /// `pop_front()` encounters it and discards it.
    cancelled: HashSet<Uuid>,
    /// All job IDs currently in the queue (including cancelled ones that have
    /// not yet been discarded by `pop_front()`). Used by `cancel()` to return
    /// `false` for unknown IDs instead of blindly inserting into the cancelled
    /// set.
    all_ids: HashSet<Uuid>,
}

impl JobQueue {
    /// Create a new, empty `JobQueue`.
    ///
    /// All three collections start empty.
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
            cancelled: HashSet::new(),
            all_ids: HashSet::new(),
        }
    }

    /// Append a job to the back of the queue (FIFO order).
    ///
    /// The job is added to the back of the internal `VecDeque` and its ID is
    /// recorded in `all_ids` so that `cancel()` can distinguish unknown IDs
    /// from IDs that are actually in the queue.
    pub fn push(&mut self, job: Job) {
        self.all_ids.insert(job.id);
        self.jobs.push_back(job);
    }

    /// Remove and return the front-most non-cancelled job.
    ///
    /// If the job at the front of the queue has been cancelled (its ID is in
    /// the `cancelled` set), it is discarded and the method continues to the
    /// next entry. This loop naturally handles runs of consecutive cancelled
    /// entries. The job's ID is removed from `all_ids` when popped, keeping
    /// the set in sync with the deque.
    ///
    /// Returns `None` if the queue is empty or all remaining entries are
    /// cancelled.
    pub fn pop_front(&mut self) -> Option<Job> {
        while let Some(job) = self.jobs.front() {
            let job_id = job.id;
            // If the front job is cancelled, discard it and continue to the
            // next entry. This is the lazy removal that makes cancel() O(1).
            if self.cancelled.contains(&job_id) {
                self.jobs.pop_front();
                self.all_ids.remove(&job_id);
                continue;
            }
            // Found a non-cancelled job at the front — remove and return it.
            let job = self.jobs.pop_front().unwrap();
            self.all_ids.remove(&job.id);
            return Some(job);
        }
        None
    }

    /// Mark a job as cancelled by its ID.
    ///
    /// Returns `true` if the ID was newly marked (not previously in the set),
    /// `false` if it was already cancelled or not present in the queue.
    /// The job remains in the `VecDeque` until `pop_front()` encounters it
    /// and discards it — cancellation is O(1) because it only touches the
    /// hash set.
    pub fn cancel(&mut self, id: Uuid) -> bool {
        // Only mark as cancelled if the ID is actually in the queue.
        // This returns false for unknown IDs instead of blindly inserting
        // them into the cancelled set.
        if !self.all_ids.contains(&id) {
            return false;
        }
        self.cancelled.insert(id)
    }

    /// Return a reference to the job with the given ID, or `None`.
    ///
    /// Searches the entire queue including cancelled entries. Returns the
    /// first match found (in FIFO order).
    pub fn get(&self, id: Uuid) -> Option<&Job> {
        self.jobs.iter().find(|job| job.id == id)
    }

    /// Return references to all jobs currently in the queue.
    ///
    /// Includes cancelled entries that have not yet been discarded by
    /// `pop_front()`. The order matches the FIFO insertion order.
    pub fn list(&self) -> Vec<&Job> {
        self.jobs.iter().collect()
    }

    /// Return the number of jobs currently in the queue, including cancelled
    /// ones that have not yet been discarded by `pop_front()`.
    ///
    /// To count only non-cancelled entries, compare `self.len()` with
    /// the number of cancelled IDs that are still in the deque.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// Return `true` if the queue contains no jobs.
    ///
    /// This is the standard companion to `len()` — clippy requires both
    /// when a struct exposes a `pub fn len()`.
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}
