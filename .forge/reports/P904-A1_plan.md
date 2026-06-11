# Plan Report: P904-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A1                                           |
| Phase       | 904 — Test Isolation Hardening                    |
| Description | Fix scheduler pool max_connections, serial removal, multi_thread runtime |
| Depends on  | P18-A4                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T06:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Eliminate two interacting causes of scheduler test hangs: (1) `SqlitePool::connect("sqlite::memory:")` creating a pool with `max_connections=10`, where DDL runs on connection 0 but queries hit connections 1–9 that see an empty schema; and (2) `#[serial_test::serial]` + `#[tokio::test]` (current_thread) deadlocking when tests spawn background tokio tasks. Fix the pool to `max_connections(1)`, remove all `#[serial]` attributes, and switch tests that spawn background tasks to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.

## Scope

### In Scope
- `crates/anvilml-scheduler/src/job_store.rs` — fix `setup_pool()`, remove `use serial_test::serial`, remove 6 `#[serial]` attributes
- `crates/anvilml-scheduler/src/scheduler.rs` — remove `use serial_test::serial`, remove 19 `#[serial]` attributes, add `multi_thread` flavor to 10 tests that spawn background tasks
- `crates/anvilml-scheduler/Cargo.toml` — remove `serial_test = { workspace = true }` from `[dev-dependencies]`

### Out of Scope
- `backend/Cargo.toml` and `backend/tests/*.rs` (handled by P904-A2)
- `anvilml-hardware` crate (retains `serial_test` legitimately)
- Any production code changes
- Workspace-level `Cargo.toml` changes
- Version bumps (handled by ACT agent per FORGE_AGENT_RULES §12)

## Approach

1. **job_store.rs — Fix `setup_pool()`**: Replace `SqlitePool::connect("sqlite::memory:")` with `SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").create_if_missing(true)).await`. This ensures all DDL and queries execute on the same in-memory database.

2. **job_store.rs — Remove serial_test**: Remove `use serial_test::serial;` from the test module. Remove `#[serial]` from all 6 test functions. Each test creates its own pool, so there is no shared mutable state to protect.

3. **job_store.rs — Change test runtime**: Replace `#[tokio::test]` with `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` on all 6 test functions (each creates its own pool via `setup_pool()`).

4. **scheduler.rs — Remove serial_test**: Remove `use serial_test::serial;` from the test module.

5. **scheduler.rs — Keep `#[tokio::test]` for select_worker-only tests** (9 tests that call `select_worker()` directly with no async spawning): `test_select_auto_single_idle`, `test_select_auto_all_busy`, `test_select_auto_ranked_by_free_mib`, `test_select_auto_tie_break_device_index`, `test_select_cpu`, `test_select_cpu_not_available`, `test_select_preference_idle`, `test_select_preference_busy`, `test_select_preference_not_found`. Remove `#[serial]` only.

6. **scheduler.rs — Add `multi_thread` flavor for tests that spawn background tasks** (10 tests): `test_submit_valid_job`, `test_submit_invalid_graph`, `test_submit_broadcasts_event`, `test_submit_persists_settings`, `test_dispatch_sends_execute`, `test_complete`, `test_image_ready_broadcasts_event`, `test_progress_broadcasts_event`, `test_cancel_broadcasts_event`, `test_cancel_queued`, `test_cancel_running`. Replace `#[serial]` + `#[tokio::test]` with `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.

7. **Cargo.toml — Remove serial_test dev-dependency**: Delete `serial_test = { workspace = true }` from `[dev-dependencies]`.

8. **Pre-Stop Verification**: Run the three required checks against the written report file.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Fix `setup_pool()` to use `max_connections(1)`; remove `serial_test` import and 6 `#[serial]` attrs; change all 6 tests to `multi_thread` runtime |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Remove `serial_test` import; remove 19 `#[serial]` attrs; add `multi_thread` flavor to 11 tests that spawn background tasks; keep `#[tokio::test]` for 9 select_worker-only tests |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Remove `serial_test = { workspace = true }` from `[dev-dependencies]` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| job_store.rs | `test_insert_and_get` | Insert + retrieve a job round-trips correctly |
| job_store.rs | `test_list_jobs_all` | List returns all inserted jobs |
| job_store.rs | `test_list_jobs_status_filter` | Status filter returns only matching jobs |
| job_store.rs | `test_list_jobs_limit` | Limit parameter caps result count |
| job_store.rs | `test_list_jobs_before_cursor` | Before-cursor filter returns only earlier jobs |
| job_store.rs | `test_update_status` | Status transition Queued → Running with timestamps |
| scheduler.rs | `test_submit_valid_job` | Valid job persists as Queued + enqueued + returns response |
| scheduler.rs | `test_submit_invalid_graph` | Invalid graph returns `InvalidGraph` error |
| scheduler.rs | `test_submit_broadcasts_event` | `job.queued` event sent on broadcast channel |
| scheduler.rs | `test_submit_persists_settings` | Custom settings round-trip through DB |
| scheduler.rs | `test_dispatch_sends_execute` | Dispatch loop transitions job to Running + worker Busy |
| scheduler.rs | `test_complete` | Completed event transitions job to Completed + worker Idle |
| scheduler.rs | `test_image_ready_broadcasts_event` | ImageReady triggers artifact save + broadcast |
| scheduler.rs | `test_progress_broadcasts_event` | Progress event triggers JobProgress broadcast |
| scheduler.rs | `test_cancel_broadcasts_event` | Cancelled event triggers broadcast + DB update + worker Idle |
| scheduler.rs | `test_cancel_queued` | `cancel()` on queued job removes from queue + DB update |
| scheduler.rs | `test_cancel_running` | `cancel()` on running job sends IPC + DB update + worker Idle |
| scheduler.rs | `test_select_auto_single_idle` | Auto mode picks the only idle worker |
| scheduler.rs | `test_select_auto_all_busy` | Auto mode returns None when all busy |
| scheduler.rs | `test_select_auto_ranked_by_free_mib` | Auto mode ranks by free VRAM |
| scheduler.rs | `test_select_auto_tie_break_device_index` | Tie-break by lowest device_index |
| scheduler.rs | `test_select_cpu` | Force-CPU picks the CPU worker |
| scheduler.rs | `test_select_cpu_not_available` | Force-CPU returns None without CPU worker |
| scheduler.rs | `test_select_preference_idle` | Device preference selects idle worker at index |
| scheduler.rs | `test_select_preference_busy` | Device preference returns None when target busy |
| scheduler.rs | `test_select_preference_not_found` | Device preference returns None for out-of-range index |

## CI Impact

The CI gate `cargo test --workspace --features mock-hardware` will exercise these tests. Removing `serial_test` from the dev-dependencies means that crate no longer needs to be resolved for the scheduler build. The `mock-hardware` feature is already used in CI and is unaffected. No CI workflow files are modified. The change should improve CI reliability by eliminating the deadlock that previously caused test timeouts.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `multi_thread` runtime changes test execution order, exposing a latent data race | Low | Medium | Each test creates its own independent pool via `setup_pool()` — no shared state between tests |
| `SqlitePoolOptions::new().max_connections(1).connect_with(...)` API shape differs from current | Low | Medium | Verified against sqlx documentation: `SqlitePoolOptions`, `SqliteConnectOptions`, `connect_with()` are all stable APIs |
| Removing `serial_test` dev-dependency causes unresolved import if some test still references it | Low | High | All `#[serial]` attributes and the `use serial_test::serial` import are removed in the same change |
| `worker_threads = 2` insufficient for dispatch loop tests | Low | Low | Per TASKS_PHASE904.md, 2 worker threads is the documented minimum (spawning thread + 1 worker); dispatch loop runs as a single `tokio::spawn` task |
| `test_cancel_queued` uses `broadcast::channel` but no `start_dispatch_loop` — still needs `multi_thread` because `submit()` spawns tasks internally via the scheduler's internal notify mechanism | Medium | Low | The test uses `make_scheduler` which creates a scheduler with a `Notify` handle; even without explicit dispatch loop, the broadcast channel subscription requires multi-thread runtime to avoid blocking |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with all tests passing and no test exceeding 10 seconds
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (no transitive breakage)
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] No `serial_test` import remains in `anvilml-scheduler` source files
- [ ] No `#[serial]` attribute remains in `anvilml-scheduler` test modules
- [ ] `serial_test` entry removed from `crates/anvilml-scheduler/Cargo.toml` `[dev-dependencies]`
