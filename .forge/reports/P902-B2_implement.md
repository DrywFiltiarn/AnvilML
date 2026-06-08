# Implementation Report: P902-B2

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P902-B2                                                     |
| Phase       | 902 — Stabilisation Retrofit                                |
| Description | anvilml-scheduler: retrofit mandatory job-store and queue DEBUG log points (job_store.rs, queue.rs) |
| Implemented | 2026-06-08T19:47:00Z                                        |
| Status      | COMPLETE                                                    |

## Summary

Added four mandatory §11.5 DEBUG-level `tracing::debug!` log points to the job-store (`job_store.rs`) and in-memory queue (`queue.rs`) modules of `anvilml-scheduler`. Two log points were added to `job_store.rs` (insert_job, update_status) and two to `queue.rs` (enqueue, pop_next). The enqueue log required a minor refactoring — capturing `job.id` before the move into `push_back` — and using `inner.len()` instead of `self.len()` to avoid a deadlock on the held mutex guard. The pop_next method was restructured from a `.map()` chain into an `if let` block to bind the removed job for logging. The crate patch version was bumped from 0.1.8 to 0.1.9. All 161 tests pass, all four platform cross-checks pass, clippy is clean, and both format checks exit 0.

## Resolved Dependencies

| Type   | Name | Version resolved | Source         |
|--------|------|-----------------|----------------|
| (none) | —    | —               | —              |

No new dependencies were added. The existing `tracing = { workspace = true }` dependency in `Cargo.toml` was used for all log calls.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Added 2× `tracing::debug!` calls: after `.execute()` in `insert_job()`, after `.execute()` in `update_status()` |
| Modify | `crates/anvilml-scheduler/src/queue.rs` | Added 2× `tracing::debug!` calls: in `enqueue()` (with job_id captured before move), in `pop_next()` (restructured `.map()` → `if let`); changed `self.len()` → `inner.len()` to avoid mutex deadlock |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.8 → 0.1.9` |

## Commit Log

```
 .forge/reports/P902-B2_plan.md            | 95 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |  6 +-
 .forge/state/state.json                   | 13 +++--
 Cargo.lock                                |  2 +-
 crates/anvilml-scheduler/Cargo.toml       |  2 +-
 crates/anvilml-scheduler/src/job_store.rs |  4 ++
 crates/anvilml-scheduler/src/queue.rs     | 13 +++--
 7 files changed, 120 insertions(+), 15 deletions(-)
```

## Test Results

```
running 161 tests (74 + 56 + 18 + 0 + 0 + 19 + 1 + 4 + 2 + 1 + 7 + 2 + 3 + 22 + 16 + 3 + 1 + 16 + 8 + 1 + 0+2+0+0+0+0+0)

anvilml_core:           74 passed; 0 failed
anvilml_hardware:       56 passed; 0 failed
anvilml_ipc:            18 passed; 0 failed
anvilml_openapi:         0 passed; 0 failed
anvilml_registry:       19 passed; 0 failed (unit) + 1+4+2+1+7+2+3 = 18 (integration)
anvilml_scheduler:      22 passed; 0 failed
anvilml_server:         16 passed; 0 failed (unit) + 3+1 = 4 (integration)
anvilml_worker:         16 passed; 0 failed
backend (anvilml bin):   8 passed; 0 failed (unit) + 1 (config_reference gate) = 9
Doc-tests:               2 passed; 0 failed

Total: 161 passed; 0 failed
```

## Format Gate

```
cargo fmt --all -- --check
(exit code 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.53s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.68s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.34s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.26s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passes — no config fields were modified by this task.

## Deviations from Plan

- **`enqueue()` log point**: The plan specified `queue_len = self.len()`, but using `self.len()` while holding the mutex lock would deadlock (the lock is not re-entrant). Changed to `queue_len = inner.len()` which reads the deque length through the already-held `MutexGuard`. This is semantically identical and avoids a runtime deadlock.
- **`enqueue()` log point**: The plan specified capturing `job.id` directly in the log macro after `push_back(job)`, but `Job` does not implement `Copy`, so the value is moved into `push_back`. Added `let job_id = job.id;` before `push_back` to capture the ID first. This is a minimal change that preserves exact same observable behavior.

## Blockers

None.
