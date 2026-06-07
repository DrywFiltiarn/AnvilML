//! In-memory job queue backed by a `Mutex<VecDeque<Job>>`.
//!
//! Provides thread-safe FIFO enqueue, cancellation of queued jobs, and
//! pop-next operations that skip already-cancelled entries.

use std::collections::VecDeque;
use std::sync::Mutex;

use anvilml_core::types::job::{Job, JobStatus};
use uuid::Uuid;

/// A thread-safe in-memory job queue.
///
/// Jobs are stored in a `VecDeque` behind a `Mutex`, providing FIFO ordering.
/// `pop_next` removes cancelled entries during iteration so that only
/// active (Queued) jobs are returned.
pub struct JobQueue {
    inner: Mutex<VecDeque<Job>>,
}

impl JobQueue {
    /// Create a new, empty `JobQueue`.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
        }
    }

    /// Enqueue a job at the back of the queue.
    pub fn enqueue(&self, job: Job) {
        let mut inner = self.inner.lock().expect("JobQueue mutex poisoned");
        inner.push_back(job);
    }

    /// Cancel a queued job identified by `id`.
    ///
    /// Iterates the deque in order and finds the first entry whose `id`
    /// matches and whose `status` is `Queued`.  Sets that entry to
    /// `Cancelled` and returns `true`.  If no matching job is found or
    /// it is already in a non-Queued state, returns `false`.
    pub fn cancel_queued(&self, id: Uuid) -> bool {
        let mut inner = self.inner.lock().expect("JobQueue mutex poisoned");
        for job in inner.iter_mut() {
            if job.id == id && job.status == JobStatus::Queued {
                job.status = JobStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Pop the next queued job from the front of the queue.
    ///
    /// Iterates from the front, removing any entry with `status ==
    /// Cancelled`.  Returns the first entry with `status == Queued`
    /// (also removed), wrapped in `Some`.  If no queued entry remains,
    /// returns `None`.
    pub fn pop_next(&self) -> Option<Job> {
        let mut inner = self.inner.lock().expect("JobQueue mutex poisoned");

        // Collect indices of cancelled entries to remove (reverse order so
        // removals don't shift remaining indices).
        let mut cancelled_indices: Vec<usize> = Vec::new();
        for (i, job) in inner.iter().enumerate() {
            if job.status == JobStatus::Cancelled {
                cancelled_indices.push(i);
            }
        }

        // Remove cancelled entries from back to front.
        for idx in cancelled_indices.into_iter().rev() {
            inner.remove(idx);
        }

        // Find and return the first queued entry.
        inner
            .iter()
            .position(|j| j.status == JobStatus::Queued)
            .map(|pos| inner.remove(pos).expect("position found"))
    }

    /// Return the total number of entries in the queue (including cancelled).
    pub fn len(&self) -> usize {
        let inner = self.inner.lock().expect("JobQueue mutex poisoned");
        inner.len()
    }

    /// Return `true` if the queue contains no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Enqueue three jobs, pop three times — verify FIFO order by ID.
    #[test]
    fn test_enqueue_pop_order() {
        let queue = JobQueue::new();
        let job1 = Job {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: anvilml_core::types::job::JobSettings::default(),
            device_index: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let job2 = Job {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: anvilml_core::types::job::JobSettings::default(),
            device_index: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let job3 = Job {
            id: Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: anvilml_core::types::job::JobSettings::default(),
            device_index: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };

        queue.enqueue(job1.clone());
        queue.enqueue(job2.clone());
        queue.enqueue(job3.clone());

        let popped1 = queue.pop_next().expect("first pop must return Some");
        assert_eq!(popped1.id, job1.id);

        let popped2 = queue.pop_next().expect("second pop must return Some");
        assert_eq!(popped2.id, job2.id);

        let popped3 = queue.pop_next().expect("third pop must return Some");
        assert_eq!(popped3.id, job3.id);

        // Queue should be empty now.
        assert!(queue.pop_next().is_none());
    }

    /// Enqueue three jobs, cancel the middle one, pop twice — verify that
    /// `pop_next` skips the cancelled job and returns job #1 then #3.
    #[test]
    fn test_cancel_skipped_on_pop() {
        let queue = JobQueue::new();
        let id1 = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let id2 = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
        let id3 = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();

        queue.enqueue(Job {
            id: id1,
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: anvilml_core::types::job::JobSettings::default(),
            device_index: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        });
        queue.enqueue(Job {
            id: id2,
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: anvilml_core::types::job::JobSettings::default(),
            device_index: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        });
        queue.enqueue(Job {
            id: id3,
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": [], "edges": []}),
            settings: anvilml_core::types::job::JobSettings::default(),
            device_index: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        });

        // Cancel the middle job (id2).
        assert!(queue.cancel_queued(id2));

        // Pop twice: should get id1 then id3.
        let popped1 = queue.pop_next().expect("first pop must return Some");
        assert_eq!(popped1.id, id1);

        let popped2 = queue.pop_next().expect("second pop must return Some");
        assert_eq!(popped2.id, id3);

        // Queue should be empty now.
        assert!(queue.pop_next().is_none());
    }
}
