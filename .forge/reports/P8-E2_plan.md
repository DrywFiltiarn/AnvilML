# Plan Report: P8-E2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-E2                                         |
| Phase       | 008 — IPC Stress Gate & Worker Pool         |
| Description | anvilml-worker: WorkerHandle::set_status() mutator |
| Depends on  | P8-E1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-07-01T13:30:00Z                         |
| Attempt     | 1                                             |

## Objective

Add `pub async fn set_status(&self, new: WorkerStatus)` as the one and only public mutator on `WorkerHandle`, enabling downstream crates (`anvilml-scheduler`, `anvilml-server`) to transition a worker's lifecycle state after construction. The method acquires a write lock on the shared `Arc<RwLock<WorkerStatus>>`, overwrites the stored value, and returns immediately — no error path, no side effects. Four new tests in `managed_tests.rs` verify correctness, clone visibility, concurrency, and repeatability, bringing the file's total to ≥ 8 tests.

## Scope

### In Scope
- Add `pub async fn set_status(&self, new: WorkerStatus)` to `WorkerHandle` in `crates/anvilml-worker/src/managed.rs`.
- Add ≥ 4 new tests to `crates/anvilml-worker/tests/managed_tests.rs`:
  1. `test_set_status_changes_value` — set to `Busy`, verify `status()` returns `Busy`.
  2. `test_set_status_visible_across_clone` — mutate via one handle, read via independent clone.
  3. `test_concurrent_status_and_set_status_no_deadlock` — spawn reader + writer tasks concurrently, assert both complete within timeout.
  4. `test_set_status_callable_repeatedly` — cycle through `Idle → Busy → Dying → Dead`, verify each transition.
- Bump `anvilml-worker` crate version from `0.1.6` to `0.1.7` in `Cargo.toml`.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope without deferring any functionality.

## Existing Codebase Assessment

The codebase inspection found:

(a) **What already exists:** `WorkerHandle` is fully implemented in `managed.rs` (110 lines) with three public methods: `new()`, `status()` (async read via `read().await`), and `request_shutdown()` (mutating, takes `&mut self`). The struct uses `Arc<RwLock<WorkerStatus>>` for the shared status lock. `WorkerStatus` is a `Copy` enum defined in `anvilml-core/src/types/worker.rs` with variants `Spawning`, `Idle`, `Busy`, `Dying`, `Dead`. The test file `managed_tests.rs` has 5 tests (≥ 4 from P8-E1, plus one added by P8-E1 itself), all using the `#[tokio::test]` async test pattern with `Arc<RwLock<WorkerStatus>>` construction.

(b) **Established patterns:** Tests construct `WorkerHandle` directly via `WorkerHandle::new()` with `Arc::new(RwLock::new(WorkerStatus::Idle))` and `None` for shutdown/join. Assertions use `assert_eq!` with descriptive messages. The existing `status()` method uses `self.status.read().await` — the same `RwLock` type (`tokio::sync::RwLock`) will be used for `set_status()` with `write().await`. No `#[tracing::instrument]` is used on `WorkerHandle` methods (they are trivial lock operations), consistent with the design doc's §11.6 guidance to not instrument trivial functions.

(c) **Gap between design doc and source:** The design doc (§9.1) shows `join_handle` as `tokio::task::JoinHandle<()>` but the actual code wraps it in `Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>` — this difference does not affect `set_status()` since the method only touches the `status` field. The design doc does not explicitly name `set_status()` as a method, but the task context and TASKS_PHASE008.md §Group E confirm it was deferred from P8-E1 and must be implemented now.

## Resolved Dependencies

| Type   | Name           | Version verified | MCP source     | Feature flags confirmed |
|--------|----------------|-----------------|----------------|------------------------|
| crate  | tokio          | 1.52.3          | rust-docs MCP  | rt, sync, time, process |
| crate  | anvilml-core   | 0.1.x (workspace path) | N/A (local) | n/a |

No new external crates are introduced. `set_status()` uses only `self.status.write().await` from the already-declared `tokio::sync::RwLock`. `WorkerStatus` is imported from the existing `anvilml-core` path dependency.

## Approach

1. **Add `set_status()` to `WorkerHandle`** in `crates/anvilml-worker/src/managed.rs`, after the `status()` method (line 95). The implementation:
   ```rust
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
   ```
   Rationale: `tokio::sync::RwLock` supports concurrent readers and a single writer. `status()` uses `read().await` (shared lock) while `set_status()` uses `write().await` (exclusive lock). They are independent acquisitions — a reader never blocks a writer and vice versa, satisfying the task requirement. The method returns `()` (no error) because `write().await` only fails if the lock is poisoned, which cannot happen with `RwLock<WorkerStatus>` since `WorkerStatus` is `Copy` and never panics during assignment.

2. **Add 4 new tests** to `crates/anvilml-worker/tests/managed_tests.rs`, after the existing 5 tests:
   - **`test_set_status_changes_value`**: Construct a handle with `WorkerStatus::Idle`, call `set_status(WorkerStatus::Busy)`, assert `status().await == WorkerStatus::Busy`. This verifies the basic mutation path.
   - **`test_set_status_visible_across_clone`**: Construct a handle, create a clone via `.clone()`, call `set_status(WorkerStatus::Dying)` on the original, then call `status().await` on the clone and assert it returns `WorkerStatus::Dying`. This verifies the shared lock is correctly shared across clones.
   - **`test_concurrent_status_and_set_status_no_deadlock`**: Construct a handle with `WorkerStatus::Idle`, spawn two concurrent tasks: one loops `status().await` 100 times, the other loops `set_status(WorkerStatus::Busy)` / `set_status(WorkerStatus::Idle)` 100 times alternating. Both tasks must complete within 5 seconds (bounded wait per ENVIRONMENT.md §11.5). This verifies no deadlock between read and write paths.
   - **`test_set_status_callable_repeatedly`**: Construct a handle, call `set_status()` four times in sequence with `Spawning → Idle → Busy → Dying`, asserting each value after the call. This verifies the method can be called repeatedly without side effects or state corruption.

3. **Bump `anvilml-worker` crate version** from `0.1.6` to `0.1.7` in `crates/anvilml-worker/Cargo.toml` per ENVIRONMENT.md §12.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `pub async fn set_status` | `anvilml-worker::WorkerHandle` | `pub async fn set_status(&self, new: WorkerStatus)` |

No other public items are added or modified. The existing `new()`, `status()`, and `request_shutdown()` remain unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `set_status()` method to `WorkerHandle` impl |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Add 4 new tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `managed_tests.rs` | `test_set_status_changes_value` | `set_status()` overwrites the stored status and `status()` returns the new value | Handle constructed with `Idle` via `WorkerHandle::new()` | `set_status(WorkerStatus::Busy)` | `status().await == WorkerStatus::Busy` | `cargo test -p anvilml-worker --test managed_tests -- test_set_status_changes_value` exits 0 |
| `managed_tests.rs` | `test_set_status_visible_across_clone` | Mutating status on one handle is observable via an independently-cloned handle | Two handles from same `Arc<RwLock<WorkerStatus>>` | `handle.set_status(Dying)` on original, `clone.status()` | `clone.status().await == WorkerStatus::Dying` | `cargo test -p anvilml-worker --test managed_tests -- test_set_status_visible_across_clone` exits 0 |
| `managed_tests.rs` | `test_concurrent_status_and_set_status_no_deadlock` | Concurrent `status()` reads and `set_status()` writes complete without deadlock within bounded timeout | Handle with `Idle` | 100 iterations of reads + alternating writes in separate tasks | Both tasks complete within 5s | `cargo test -p anvilml-worker --test managed_tests -- test_concurrent_status_and_set_status_no_deadlock` exits 0 |
| `managed_tests.rs` | `test_set_status_callable_repeatedly` | `set_status()` can be called multiple times with different values, each transition is correct | Handle with `Idle` | Sequential: `Spawning → Idle → Busy → Dying` | Each `status()` call after `set_status()` returns the expected value | `cargo test -p anvilml-worker --test managed_tests -- test_set_status_callable_repeatedly` exits 0 |

## CI Impact

No CI changes required. The new tests are in the existing `managed_tests.rs` file under `crates/anvilml-worker/tests/`, which is already collected by `cargo test -p anvilml-worker` (the workspace test command in ENVIRONMENT.md §6 Step 6). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. `WorkerStatus` is a `Copy` enum with no platform-specific behavior. `tokio::sync::RwLock` is cross-platform. The method does not touch file paths, sockets, or subprocesses. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::sync::RwLock` write lock contention with existing `status()` readers — a writer could briefly block readers, but this is the intended behavior of `RwLock` and matches the task requirement of "independent lock-acquisition paths." The `RwLock` already handles this; no code change needed. | Low | Low | The task explicitly requires independent lock paths, which `RwLock` provides. Verified by `test_concurrent_status_and_set_status_no_deadlock`. |
| Test `test_concurrent_status_and_set_status_no_deadlock` could be flaky if the 5-second timeout is too tight on a slow CI runner. | Low | Medium | The test does 100 iterations of simple lock operations — each takes microseconds, not milliseconds. 5 seconds is orders of magnitude more than needed. If CI is slow, increase to 10s. The bounded-wait pattern (ENVIRONMENT.md §11.5) prevents indefinite hangs. |
| `WorkerStatus::Respawning` variant does not exist yet — the test `test_set_status_callable_repeatedly` cycles through `Spawning → Idle → Busy → Dying`, which are all valid variants. No risk of referencing a non-existent variant. | None | None | Verified by reading `crates/anvilml-core/src/types/worker.rs` — only 5 variants exist, all used in the test plan are present. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_set_status_changes_value` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_set_status_visible_across_clone` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_concurrent_status_and_set_status_no_deadlock` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests -- test_set_status_callable_repeatedly` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests` exits 0 with ≥ 8 total tests (5 existing + 4 new)
- [ ] `wc -l crates/anvilml-worker/tests/managed_tests.rs` shows ≥ 195 lines (155 existing + ~45 new test lines)
