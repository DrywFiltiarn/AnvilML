# Implementation Report: P13-A1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P13-A1                                            |
| Phase         | 013 — Job Queue & Persistence                     |
| Description   | anvilml-scheduler: queue.rs JobQueue FIFO with O(1) cancel |
| Implemented   | 2026-06-19T21:05:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created the `JobQueue` data structure in `crates/anvilml-scheduler/src/queue.rs` — a FIFO queue backed by `VecDeque` with a `HashMap<Uuid, usize>` index map that enables O(1) cancellation via swap-remove. Implemented all six public methods (`new`, `push`, `pop_front`, `cancel`, `get`, `list`, `len`, plus `is_empty` required by clippy). Added 10 comprehensive tests in `tests/queue_tests.rs` covering FIFO ordering, empty queue, cancel success/failure, get by ID, list order, len tracking, edge cases (cancel last item, cancel first with displacement), and multiple cancellations. Modified `lib.rs` to expose the module and bumped the crate version from 0.1.4 to 0.1.5.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source          |
|--------|------------|------------------|-----------------|
| crate  | uuid       | 1.23.3           | workspace Cargo |
| crate  | chrono     | 0.4.45           | workspace Cargo |

Note: `uuid` was added as a direct workspace dependency to `anvilml-scheduler/Cargo.toml` because the `cancel` method signature requires `Uuid` as a parameter type and `anvilml_core` does not re-export it. `chrono` was added as a dev-dependency for test fixtures (creating `Job` values with `Utc::now()` timestamps). No other new dependencies were introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/queue.rs` | `JobQueue` struct with `items: VecDeque<Job>` and `by_id: HashMap<Uuid, usize>`, plus all six public methods with doc comments and inline comments |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Added `pub mod queue;` after `pub mod dag;` |
| CREATE | `crates/anvilml-scheduler/tests/queue_tests.rs` | 10 integration tests for `JobQueue` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bumped patch version 0.1.4 → 0.1.5; added `uuid = { workspace = true }` and `chrono` dev-dependency |
| MODIFY | `docs/TESTS.md` | Added 10 entries for new queue tests |

## Commit Log

```
 .forge/reports/P13-A1_plan.md                 | 193 ++++++++++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +-
 Cargo.lock                                    |   6 +-
 crates/anvilml-scheduler/Cargo.toml           |   6 +-
 crates/anvilml-scheduler/src/lib.rs           |   1 +
 crates/anvilml-scheduler/src/queue.rs         | 192 ++++++++++++++++++
 crates/anvilml-scheduler/tests/queue_tests.rs | 270 ++++++++++++++++++++++++++
 docs/TESTS.md                                 |  90 +++++++++
 9 files changed, 765 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/queue_tests.rs (target/debug/deps/queue_tests-351f2a5d99f4f0f5)

running 10 tests
test test_cancel_first_item_with_displacement ... ok
test test_cancel_last_item ... ok
test test_cancel_returns_false_for_missing_id ... ok
test test_cancel_returns_true_and_removes ... ok
test test_get_returns_job_by_id ... ok
test test_len_after_operations ... ok
test test_list_returns_all_jobs_in_order ... ok
test test_multiple_cancellations ... ok
test test_pop_empty_returns_none ... ok
test test_push_pop_fifo_order ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 170 workspace tests pass (including 10 new queue tests). No failures, no ignored tests.

## Format Gate

```
cargo fmt --all -- --check
# exited 0 — no output (clean)
```

## Platform Cross-Check

```
# Check 1 — mock-hardware Linux:
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.03s
# OK

# Check 2 — mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.26s
# OK

# Check 3 — real-hardware Linux:
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.95s
# OK

# Check 4 — real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.16s
# OK
```

All four platform cross-checks exit 0.

## Project Gates

```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
# OK — config surface sync verified
```

Gate 1 (config_reference) passes. No config fields were added or modified by this task.
Gate 2 (openapi_drift) — not applicable; no handler signatures or ToSchema derives changed.
Gate 3 (node_parity) — not applicable; no node types added or modified.

## Public API Delta

```
pub mod queue;
pub struct JobQueue { items: VecDeque<Job>, by_id: HashMap<Uuid, usize> }
pub fn JobQueue::new() -> Self
pub fn JobQueue::push(&mut self, job: Job)
pub fn JobQueue::pop_front(&mut self) -> Option<Job>
pub fn JobQueue::cancel(&mut self, id: Uuid) -> bool
pub fn JobQueue::get(&self, id: &Uuid) -> Option<&Job>
pub fn JobQueue::list(&self) -> Vec<&Job>
pub fn JobQueue::len(&self) -> usize
pub fn JobQueue::is_empty(&self) -> bool
```

All new pub items match the plan's Public API Surface table, plus `is_empty` (required by clippy's `len_without_is_empty` lint).

## Deviations from Plan

- **Added `uuid` as a direct workspace dependency** to `anvilml-scheduler/Cargo.toml`. The plan stated "No new external dependencies are needed" but the `cancel` method signature requires `Uuid` as a parameter type, and `anvilml_core` does not re-export `Uuid`. This is a minimal one-line addition.
- **Added `chrono` as a dev-dependency** for test fixtures. The test file creates `Job` values with `created_at: Utc::now()`, which requires the `chrono` crate. Added as a workspace dependency with `dev-dependencies` scope.
- **Added `is_empty()` method** alongside `len()` as required by clippy's `len_without_is_empty` lint (`-D warnings`).
- **Implemented `rebuild_indices()` private method** and called it from `pop_front()`. The plan's `VecDeque` + `HashMap<usize>` index approach is fundamentally incompatible with `pop_front` because `VecDeque::pop_front` shifts all remaining elements' indices. The `rebuild_indices` method restores correctness by rebuilding all indices after each `pop_front`. This makes `pop_front` O(n) for the rebuild but O(1) for the removal itself. Since `pop_front` is called once per job dispatch (relatively infrequent) compared to `push` and `cancel`, this trade-off is acceptable.
- **Added `Default` impl for `JobQueue`** implementing `Default` via `Self::new()`, following the pattern used in other crates in this workspace.

## Blockers

None.
