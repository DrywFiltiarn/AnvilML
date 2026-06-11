# Implementation Report: P904-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A1                                           |
| Phase       | 904 — Test Isolation Hardening                    |
| Description | Fix scheduler pool max_connections, serial removal, multi_thread runtime |
| Implemented | 2026-06-11T09:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Fixed two interacting causes of scheduler test hangs: (1) `SqlitePool::connect("sqlite::memory:")` creating a pool with `max_connections=10`, where DDL runs on connection 0 but queries hit connections 1–9 that see an empty schema; and (2) `#[serial_test::serial]` + `#[tokio::test]` (current_thread) deadlocking when tests spawn background tokio tasks. Replaced the pool with `SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").create_if_missing(true))`, removed all `#[serial]` attributes and the `serial_test` dev-dependency, and switched tests that spawn background tasks to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. All 43 scheduler tests and the full workspace test suite (246+ tests) pass with zero failures.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|------------|------------------|----------------|
| remove | serial_test| (removed)        | N/A            |

No new dependencies added. `serial_test` dev-dependency removed from `crates/anvilml-scheduler/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Remove `serial_test = { workspace = true }` from `[dev-dependencies]`; bump version 0.1.17 → 0.1.18 |
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Fix `setup_pool()` to use `SqlitePoolOptions::new().max_connections(1).connect_with(...)`; move `SqliteConnectOptions`/`SqlitePoolOptions`/`Path` imports into test module; remove `use serial_test::serial`; remove `#[serial]` from all 6 tests; change all 6 tests to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Remove `use serial_test::serial`; remove `#[serial]` from all 19 tests; add `multi_thread` flavor to 11 tests that spawn background tasks; keep `#[tokio::test]` for 9 select_worker-only tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |  6 ++---
 .forge/state/state.json                   | 13 +++++-----
 .forge/tasks/tasks_phase019.json          |  3 ++-
 Cargo.lock                                |  3 +--
 crates/anvilml-scheduler/Cargo.toml       |  3 +--
 crates/anvilml-scheduler/src/job_store.rs | 28 ++++++++++----------
 crates/anvilml-scheduler/src/scheduler.rs | 43 ++++++++-----------------------
 7 files changed, 39 insertions(+), 60 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-93fac82b827ddd80)

running 43 tests
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_cycle_detected_2node ... ok
test dag::tests::test_unknown_node_ref ... ok
test dag::tests::test_unknown_output_slot ... ok
test dag::tests::test_valid_edge_references ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_valid_graph ... ok
test dag::tests::test_valid_zit_5node_passes ... ok
test ledger::tests::test_free_mib_unknown_device ... ok
test ledger::tests::test_init_from ... ok
test ledger::tests::test_update ... ok
test ledger::tests::test_would_fit_true ... ok
test ledger::tests::test_would_fit_false ... ok
test nodes::tests::test_all_nine_types_present ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test queue::tests::test_cancel_skipped_on_pop ... ok
test queue::tests::test_enqueue_pop_order ... ok
test scheduler::tests::test_select_auto_ranked_by_free_mib ... ok
test scheduler::tests::test_select_auto_tie_break_device_index ... ok
test scheduler::tests::test_select_cpu ... ok
test scheduler::tests::test_select_cpu_not_available ... ok
test scheduler::tests::test_select_preference_busy ... ok
test scheduler::tests::test_select_preference_not_found ... ok
test job_store::tests::test_insert_and_get ... ok
test job_store::tests::test_list_jobs_before_cursor ... ok
test scheduler::tests::test_select_auto_single_idle ... ok
test job_store::tests::test_list_jobs_all ... ok
test job_store::tests::test_update_status ... ok
test scheduler::tests::test_submit_invalid_graph ... ok
test job_store::tests::test_list_jobs_limit ... ok
test scheduler::tests::test_submit_valid_job ... ok
test job_store::tests::test_list_jobs_status_filter ... ok
test scheduler::tests::test_cancel_queued ... ok
test scheduler::tests::test_select_auto_all_busy ... ok
test scheduler::tests::test_select_preference_idle ... ok
test scheduler::tests::test_submit_broadcasts_event ... ok
test scheduler::tests::test_submit_persists_settings ... ok
test scheduler::tests::test_dispatch_sends_execute ... ok
test scheduler::tests::test_cancel_running ... ok
test scheduler::tests::test_complete ... ok
test scheduler::tests::test_cancel_broadcasts_event ... ok
test scheduler::tests::test_image_ready_broadcasts_event ... ok
test scheduler::tests::test_progress_broadcasts_event ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
```

## Format Gate

```
cargo fmt --all -- --check
```
Exit code 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.07s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.46s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.56s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.68s
```

All four platform cross-checks exit 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
cargo test -p backend --features mock-hardware -- config_reference
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

No OpenAPI drift gate required — no handler/schema changes in this task.

## Deviations from Plan

- The plan listed 10 tests for `multi_thread` flavor but enumerated 11 test names. All 11 tests that spawn background tasks (dispatch loop via `start_dispatch_loop()`) were given `multi_thread` flavor: `test_submit_valid_job`, `test_submit_invalid_graph`, `test_submit_broadcasts_event`, `test_submit_persists_settings`, `test_dispatch_sends_execute`, `test_complete`, `test_image_ready_broadcasts_event`, `test_progress_broadcasts_event`, `test_cancel_broadcasts_event`, `test_cancel_queued`, `test_cancel_running`.
- The clippy lint required moving `SqliteConnectOptions`, `SqlitePoolOptions`, and `Path` imports from the module level into the test module, since they are only used in test code. The plan did not specify import placement.

## Blockers

None.
