/// Integration tests for `JobQueue` — in-memory FIFO queue with O(1) cancel.
///
/// Each test constructs `JobQueue` and `Job` values using the public API.
/// Since `Job` derives `Clone` and has all public fields, test setup uses
/// direct struct literals with `chrono::Utc::now()` for timestamps.
use anvilml_core::{Job, JobSettings, JobStatus};
use anvilml_scheduler::JobQueue;
use chrono::Utc;
use uuid::Uuid;

/// Helper to construct a minimal `Job` for tests.
///
/// Uses `Uuid::new_v4()` for the ID and `Utc::now()` for `created_at`.
/// All optional fields are `None`. The status is `JobStatus::Queued`.
fn make_job(id: Uuid) -> Job {
    Job {
        id,
        status: JobStatus::Queued,
        graph: serde_json::json!({"nodes": []}),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        worker_id: None,
        error: None,
        queue_position: None,
    }
}

/// Test that jobs are returned in push order (FIFO).
///
/// Pushes two jobs with distinct UUIDs, then calls `pop_front()` twice.
/// The first pop must return the first job pushed, the second pop must
/// return the second job.
#[test]
fn test_fifo_order() {
    let mut queue = JobQueue::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let job_a = make_job(id_a);
    let job_b = make_job(id_b);

    queue.push(job_a);
    queue.push(job_b);

    let first = queue.pop_front();
    assert!(first.is_some());
    assert_eq!(first.unwrap().id, id_a, "First pop must return job A");

    let second = queue.pop_front();
    assert!(second.is_some());
    assert_eq!(second.unwrap().id, id_b, "Second pop must return job B");
}

/// Test that `pop_front()` skips cancelled jobs (lazy removal).
///
/// Pushes two jobs, cancels the first, then pops. The first pop must
/// return the second job (the first was cancelled and skipped). The
/// second pop must return `None` (queue is now empty).
#[test]
fn test_cancel_then_pop_front_skips() {
    let mut queue = JobQueue::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let job_a = make_job(id_a);
    let job_b = make_job(id_b);

    queue.push(job_a);
    queue.push(job_b);
    queue.cancel(id_a);

    let first = queue.pop_front();
    assert!(first.is_some());
    assert_eq!(
        first.unwrap().id,
        id_b,
        "Cancelled job A must be skipped, B returned"
    );

    let second = queue.pop_front();
    assert!(
        second.is_none(),
        "Queue must be empty after skipping cancelled job"
    );
}

/// Test that cancelling a new (previously unseen) ID returns `true`.
///
/// Pushes no jobs. Cancels a freshly-generated UUID. The return value
/// must be `true` because `HashSet::insert()` returns `true` when the
/// key was not previously present — this is the "newly marked" signal.
#[test]
fn test_cancel_new_id_returns_true() {
    let mut queue = JobQueue::new();
    let new_id = Uuid::new_v4();
    let result = queue.cancel(new_id);
    assert!(
        result,
        "Cancelling a new ID must return true (newly marked)"
    );
}

/// Test that cancelling an already-cancelled ID returns `false`.
///
/// Pushes a job, cancels it, then cancels it again. The second cancel
/// call must return `false` because the ID was already in the set.
#[test]
fn test_cancel_already_cancelled_returns_false() {
    let mut queue = JobQueue::new();
    let id = Uuid::new_v4();
    let job = make_job(id);

    queue.push(job);
    let first_cancel = queue.cancel(id);
    assert!(first_cancel, "First cancel must return true (newly marked)");

    let second_cancel = queue.cancel(id);
    assert!(
        !second_cancel,
        "Second cancel of same ID must return false (already marked)"
    );
}

/// Test that `get()` finds a job by its UUID.
///
/// Pushes a job, then calls `get()` with its ID. Must return `Some(&Job)`
/// with a matching ID.
#[test]
fn test_get_returns_job_by_id() {
    let mut queue = JobQueue::new();
    let id = Uuid::new_v4();
    let job = make_job(id);

    queue.push(job);
    let found = queue.get(id);
    assert!(found.is_some(), "get() must find the job by ID");
    assert_eq!(found.unwrap().id, id, "Found job must have matching ID");
}

/// Test that `get()` returns `None` for an unknown ID.
///
/// Pushes no jobs. Calls `get()` with a freshly-generated UUID. Must return `None`.
#[test]
fn test_get_unknown_id_returns_none() {
    let queue = JobQueue::new();
    let unknown_id = Uuid::new_v4();
    let result = queue.get(unknown_id);
    assert!(result.is_none(), "get() must return None for unknown ID");
}

/// Test that `list()` returns references to all jobs in the queue.
///
/// Pushes three jobs, then calls `list()`. Must return a `Vec<&Job>` of
/// length 3 with IDs matching the pushed jobs in FIFO order.
#[test]
fn test_list_returns_all_jobs() {
    let mut queue = JobQueue::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let id_c = Uuid::new_v4();

    queue.push(make_job(id_a));
    queue.push(make_job(id_b));
    queue.push(make_job(id_c));

    let all = queue.list();
    assert_eq!(all.len(), 3, "list() must return 3 jobs");
    assert_eq!(all[0].id, id_a, "First job must be A (FIFO order)");
    assert_eq!(all[1].id, id_b, "Second job must be B");
    assert_eq!(all[2].id, id_c, "Third job must be C");
}

/// Test that `len()` is correct after push/cancel (counts all in VecDeque).
///
/// Pushes three jobs, cancels one, then calls `len()`. Must return 3
/// because cancelled jobs are still in the deque until popped.
#[test]
fn test_len_after_mixed_ops() {
    let mut queue = JobQueue::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let id_c = Uuid::new_v4();

    queue.push(make_job(id_a));
    queue.push(make_job(id_b));
    queue.push(make_job(id_c));
    queue.cancel(id_b);

    assert_eq!(
        queue.len(),
        3,
        "len() must count all jobs in deque, including cancelled ones"
    );
}

/// Test that `pop_front()` discards cancelled entries and returns remaining jobs.
///
/// Pushes three jobs (A, B, C), cancels B, then pops twice. The first pop
/// must return A (first non-cancelled), the second must return C (B was
/// skipped). The third pop must return `None`.
#[test]
fn test_pop_front_discards_cancelled_and_returns_remaining() {
    let mut queue = JobQueue::new();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let id_c = Uuid::new_v4();

    queue.push(make_job(id_a));
    queue.push(make_job(id_b));
    queue.push(make_job(id_c));
    queue.cancel(id_b);

    let first = queue.pop_front();
    assert!(first.is_some());
    assert_eq!(
        first.unwrap().id,
        id_a,
        "First pop must return A (non-cancelled)"
    );

    let second = queue.pop_front();
    assert!(second.is_some());
    assert_eq!(
        second.unwrap().id,
        id_c,
        "Second pop must return C (B was skipped)"
    );

    let third = queue.pop_front();
    assert!(third.is_none(), "Third pop must return None (queue empty)");
}
