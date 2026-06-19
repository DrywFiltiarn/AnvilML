//! Tests for `queue.rs` — `JobQueue` FIFO queue with O(1) cancel.
//!
//! Each test constructs a `JobQueue`, performs operations, and asserts
//! on the results. All tests use synchronous code — no `#[tokio::test]`
//! needed since `JobQueue` is pure synchronous logic.

use anvilml_core::{Job, JobSettings, JobStatus};
use anvilml_scheduler::queue::JobQueue;
use chrono::Utc;
use uuid::Uuid;

/// Helper to create a test job with a given UUID.
///
/// Uses the current UTC time for `created_at` and defaults all
/// optional fields to `None` except `status` which defaults to
/// `Queued`. This keeps test code concise and readable.
fn make_job(id: Uuid) -> Job {
    Job {
        id,
        status: JobStatus::Queued,
        graph: serde_json::json!({ "nodes": [] }),
        settings: JobSettings::default(),
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        worker_id: None,
        error: None,
        queue_position: None,
    }
}

/// Push three jobs in order, then pop three times and verify FIFO order.
///
/// The first pushed job must be the first popped, the second pushed
/// must be the second popped, and the third pushed must be the last
/// popped. This verifies the core FIFO invariant.
#[test]
fn test_push_pop_fifo_order() {
    let mut queue = JobQueue::new();

    let job_a = make_job(Uuid::new_v4());
    let job_b = make_job(Uuid::new_v4());
    let job_c = make_job(Uuid::new_v4());

    queue.push(job_a.clone());
    queue.push(job_b.clone());
    queue.push(job_c.clone());

    // FIFO order: first in, first out.
    assert_eq!(queue.pop_front().unwrap().id, job_a.id);
    assert_eq!(queue.pop_front().unwrap().id, job_b.id);
    assert_eq!(queue.pop_front().unwrap().id, job_c.id);
}

/// Popping from an empty queue returns None.
///
/// This is the base case — a freshly constructed queue has no jobs,
/// so `pop_front` must return `None` without panicking.
#[test]
fn test_pop_empty_returns_none() {
    let mut queue = JobQueue::new();
    assert!(queue.pop_front().is_none());
}

/// Cancelling an existing job returns true and removes it from the queue.
///
/// After cancellation, `get()` for the cancelled ID must return `None`,
/// `len()` must have decreased by one, and the remaining jobs must
/// still be accessible and in valid state.
#[test]
fn test_cancel_returns_true_and_removes() {
    let mut queue = JobQueue::new();

    let job = make_job(Uuid::new_v4());
    queue.push(job.clone());

    assert_eq!(queue.len(), 1);
    assert!(queue.cancel(job.id));

    // Job should be gone.
    assert!(queue.get(&job.id).is_none());
    assert_eq!(queue.len(), 0);
    assert!(queue.pop_front().is_none());
}

/// Cancelling a non-existent job returns false and leaves the queue unchanged.
///
/// This tests the negative path: an ID that was never pushed (or was
/// already cancelled) must not cause any state mutation.
#[test]
fn test_cancel_returns_false_for_missing_id() {
    let mut queue = JobQueue::new();

    let existing = make_job(Uuid::new_v4());
    let existing_id = existing.id;
    queue.push(existing);

    let missing = Uuid::new_v4();
    assert!(!queue.cancel(missing));

    // Queue must be unchanged.
    assert_eq!(queue.len(), 1);
    assert!(queue.get(&existing_id).is_some());
}

/// `get()` returns the correct job reference by UUID.
///
/// Verifies that the index map correctly maps a UUID to the job's
/// position in the deque, and that the returned reference has the
/// correct fields.
#[test]
fn test_get_returns_job_by_id() {
    let mut queue = JobQueue::new();

    let job = make_job(Uuid::new_v4());
    queue.push(job.clone());

    let found = queue.get(&job.id);
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, job.id);
    assert_eq!(found.status, JobStatus::Queued);
}

/// `list()` returns all jobs in FIFO push order.
///
/// Push three jobs, call `list()`, and verify the returned `Vec`
/// contains all three jobs in the same order they were pushed.
#[test]
fn test_list_returns_all_jobs_in_order() {
    let mut queue = JobQueue::new();

    let job_a = make_job(Uuid::new_v4());
    let job_b = make_job(Uuid::new_v4());
    let job_c = make_job(Uuid::new_v4());

    queue.push(job_a.clone());
    queue.push(job_b.clone());
    queue.push(job_c.clone());

    let list = queue.list();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].id, job_a.id);
    assert_eq!(list[1].id, job_b.id);
    assert_eq!(list[2].id, job_c.id);
}

/// `len()` correctly tracks push, pop, and cancel operations.
///
/// Exercises all three mutating operations and verifies `len()` reflects
/// the correct count at each step. This is the comprehensive integration
/// test for the queue's size tracking.
#[test]
fn test_len_after_operations() {
    let mut queue = JobQueue::new();

    assert_eq!(queue.len(), 0);

    let job_a = make_job(Uuid::new_v4());
    let job_b = make_job(Uuid::new_v4());
    let job_c = make_job(Uuid::new_v4());

    queue.push(job_a.clone());
    assert_eq!(queue.len(), 1);

    queue.push(job_b.clone());
    assert_eq!(queue.len(), 2);

    queue.push(job_c.clone());
    assert_eq!(queue.len(), 3);

    // Pop one — len should decrease.
    queue.pop_front();
    assert_eq!(queue.len(), 2);

    // Cancel job_b by ID — len should decrease again.
    // After the first pop, job_b is now at position 0 and job_c at position 1.
    // Cancelling job_b swaps job_c into position 0.
    queue.cancel(job_b.id);
    assert_eq!(queue.len(), 1);

    // Only job_c remains (it was displaced to position 0 by the cancel).
    assert_eq!(queue.pop_front().unwrap().id, job_c.id);
    assert_eq!(queue.len(), 0);
}

/// Cancelling the last item in the queue (swap-remove with self-swap).
///
/// When the cancelled item is the last one in the deque, swap with
/// itself is a no-op. This tests the edge case where index == last_index
/// in the cancel method.
#[test]
fn test_cancel_last_item() {
    let mut queue = JobQueue::new();

    let job_a = make_job(Uuid::new_v4());
    let job_b = make_job(Uuid::new_v4());

    queue.push(job_a.clone());
    queue.push(job_b.clone());

    // Cancel the last item (job_b is at index 1, last_index is 1).
    assert!(queue.cancel(job_b.id));
    assert_eq!(queue.len(), 1);
    assert!(queue.get(&job_b.id).is_none());
    assert!(queue.get(&job_a.id).is_some());
}

/// Cancelling the first item in a multi-item queue (swap-remove with displacement).
///
/// When cancelling the front item, the last item is swapped to the
/// front. This tests that the displaced item's index is correctly
/// updated in `by_id`.
#[test]
fn test_cancel_first_item_with_displacement() {
    let mut queue = JobQueue::new();

    let job_a = make_job(Uuid::new_v4());
    let job_b = make_job(Uuid::new_v4());
    let job_c = make_job(Uuid::new_v4());

    queue.push(job_a.clone());
    queue.push(job_b.clone());
    queue.push(job_c.clone());

    // Cancel the first item — job_c should move to position 0.
    assert!(queue.cancel(job_a.id));
    assert_eq!(queue.len(), 2);

    // job_c should now be at the front.
    let popped = queue.pop_front().unwrap();
    assert_eq!(popped.id, job_c.id);

    // job_b should be next.
    let popped = queue.pop_front().unwrap();
    assert_eq!(popped.id, job_b.id);

    assert_eq!(queue.len(), 0);
}

/// Multiple cancellations preserve remaining queue integrity.
///
/// Push five jobs, cancel three of them (non-sequential IDs), then
/// verify the remaining two are still accessible and in valid state.
#[test]
fn test_multiple_cancellations() {
    let mut queue = JobQueue::new();

    let jobs: Vec<_> = (0..5).map(|_| make_job(Uuid::new_v4())).collect();

    for job in &jobs {
        queue.push(job.clone());
    }

    assert_eq!(queue.len(), 5);

    // Cancel indices 1, 2, 3 (the middle three).
    assert!(queue.cancel(jobs[1].id));
    assert!(queue.cancel(jobs[2].id));
    assert!(queue.cancel(jobs[3].id));

    assert_eq!(queue.len(), 2);

    // Jobs 0 and 4 should remain.
    assert!(queue.get(&jobs[0].id).is_some());
    assert!(queue.get(&jobs[4].id).is_some());
    assert!(queue.get(&jobs[1].id).is_none());
    assert!(queue.get(&jobs[2].id).is_none());
    assert!(queue.get(&jobs[3].id).is_none());
}
