# Tasks: Phase 904 — Test Isolation Hardening

| Field | Value |
|-------|-------|
| Phase | 904 |
| Name | Test Isolation Hardening |
| Milestone group | Correction (retrofit) |
| Depends on phases | 18 |
| Task file | `.forge/tasks/tasks_phase904.json` |
| Tasks | 3 |

## Overview

Phase 904 is a retrofit correction phase inserted between Phase 18 and Phase 19. It resolves two classes of test isolation bug that cause the CI `rust-linux` and `rust-windows` jobs to hang with tests exceeding the 60-second timeout.

**Root cause 1 — `sqlite::memory:` multi-connection pool.** `anvilml-scheduler`'s `setup_pool()` helper calls `SqlitePool::connect("sqlite::memory:")`, which creates a pool with the default `max_connections` (10). Each SQLite `:memory:` URL is a separate, independent database per connection. The `CREATE TABLE` DDL executes on connection 0; subsequent queries are dispatched to connections 1–9, which see an empty schema. The result is either a hard SQL error or an indefinite hang waiting for a lock that spans across connection slots.

**Root cause 2 — `#[serial_test::serial]` + `#[tokio::test]` deadlock.** `serial_test` 3.x enforces serialisation by acquiring a `parking_lot::Mutex` on the OS thread for the duration of each annotated test. `#[tokio::test]` (default flavor: `current_thread`) drives the runtime via `Runtime::block_on(...)` on that same thread. Any test that spawns a background `tokio::task` (dispatch loop, axum server, broadcast subscriber) requires the tokio thread to poll those tasks — but the thread is blocked in `block_on` holding the serial lock. Nothing can make progress. The deadlock manifests across two crates:

- `anvilml-scheduler` — all `job_store` and `scheduler` tests carry `#[serial]`.
- `backend` — four integration test files (`preflight_check.rs`, `api_ws_lifecycle.rs`, `api_cancel.rs`, `api_delete.rs`) all carry `#[serial]` and each spawns an axum server via `tokio::spawn`.

The fix is uniform: remove `#[serial]` everywhere it is not needed (tests with no shared state), fix the `sqlite::memory:` pool to `max_connections(1)`, and switch tests that require background tasks to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`, which does not block the spawning thread and allows background tasks to be polled on worker threads.

`serial_test` remains a legitimate dependency in `anvilml-hardware`, where it serialises sync tests that mutate `ANVILML_MOCK_DEVICE_TYPE` / `ANVILML_MOCK_VRAM_MIB` environment variables. The workspace dependency is retained; only the per-crate dev-dependency entries for `anvilml-scheduler` and `backend` are removed.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Test isolation fixes | P904-A1 … P904-A3 | Fix scheduler pool + serial, fix backend serial, verify workspace |

## Prerequisites

- P18-A4 must be complete (final task of Phase 18).
- The workspace must compile clean under `cargo check --workspace --features mock-hardware` before this phase begins.

## Interfaces and Contracts

No production interfaces change. All modifications are confined to `#[cfg(test)]` code and `[dev-dependencies]` entries.

## Task Descriptions

### Group A — Test Isolation Fixes

#### P904-A1: anvilml-scheduler: fix test isolation (pool max_connections, serial removal, multi_thread runtime)

**Goal:** Eliminate the two interacting causes of scheduler test hangs.

**Files to modify:**
- `crates/anvilml-scheduler/src/job_store.rs`
- `crates/anvilml-scheduler/src/scheduler.rs`
- `crates/anvilml-scheduler/Cargo.toml`

**job_store.rs changes:**
- In `setup_pool()`, replace `SqlitePool::connect("sqlite::memory:")` with `SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(":memory:").create_if_missing(true)).await`. This ensures the schema-creation DDL and all subsequent queries execute against the same in-memory database connection.
- Remove `use serial_test::serial`.
- Remove all 6 `#[serial]` attributes from the `job_store` tests. Each test already creates its own pool — there is no shared mutable state to protect.

**scheduler.rs changes:**
- For pure logic tests that call `select_worker()` directly with no async spawning (`test_select_auto_single_idle`, `test_select_auto_all_busy`, `test_select_auto_ranked_by_free_mib`, `test_select_auto_tie_break_device_index`, `test_select_cpu`, `test_select_cpu_not_available`, `test_select_preference_idle`, `test_select_preference_busy`, `test_select_preference_not_found`): remove `#[serial]` only; leave `#[tokio::test]` unchanged.
- For all remaining scheduler tests that call `make_scheduler()` or `start_dispatch_loop()` (any test that touches a broadcast channel or spawns a background task): replace `#[serial]` + `#[tokio::test]` with `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.

**Cargo.toml changes:**
- Remove `serial_test = { workspace = true }` from `[dev-dependencies]`.

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with 0 failed and no test exceeding 10 seconds.

---

#### P904-A2: backend: fix test isolation (serial removal, multi_thread runtime, temp_env cleanup)

**Goal:** Eliminate the `#[serial]` + `current_thread` deadlock across all four backend integration test files.

**Files to modify:**
- `backend/tests/api_ws_lifecycle.rs`
- `backend/tests/api_cancel.rs`
- `backend/tests/api_delete.rs`
- `backend/tests/preflight_check.rs`
- `backend/Cargo.toml`

**api_ws_lifecycle.rs, api_cancel.rs, api_delete.rs changes (identical pattern):**
- Remove `use serial_test::serial;` (or `use serial_test;`).
- Remove all `#[serial]` attributes from every test function.
- Change `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` on every test function. These tests all spawn an axum server via `tokio::spawn` and drive a hyper or tungstenite client concurrently — they require a multi-thread runtime to make progress.
- Env isolation already in place (`temp_env::async_with_vars` + unconditional `remove_var` after): leave unchanged.

**preflight_check.rs changes:**
- Same `#[serial]` removal and `multi_thread` runtime change as above.
- Additionally, in `job_submit_rejected_when_preflight_fails`: the test body currently opens with a bare `std::env::remove_var("ANVILML_WORKER_MOCK")` with no save/restore. Replace this with a `temp_env::async_with_vars([("ANVILML_WORKER_MOCK", None::<&str>)], async { <rest of body> }).await` wrapper so the env state is properly scoped rather than unconditionally stripped.

**Cargo.toml changes:**
- Remove `serial_test = { workspace = true }` from `[dev-dependencies]`. The `temp-env` dev-dependency already present is sufficient for all env isolation needs.

**Acceptance criterion:** `cargo test -p backend --features mock-hardware` exits 0 with 0 failed and no test exceeding 30 seconds.

---

#### P904-A3: anvilml: verify full workspace test suite green after P904 isolation fixes

**Goal:** Confirm that P904-A1 and P904-A2 have not introduced regressions elsewhere, and that all six CI gates pass clean.

**No source changes permitted** unless a test failure is directly and demonstrably caused by the P904-A1/A2 changes (e.g. a previously-masked test failure now surfaced). Any such fix must be documented clearly in the implement report with a root-cause explanation.

**Gates to run (all must exit 0):**

```bash
cargo test --workspace --features mock-hardware
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo clippy --bin anvilml -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

**Implement report must include:** per-crate test counts from the full workspace run, confirming zero failures and no timeouts.

**Acceptance criterion:** All six commands exit 0. This task gates P19-A1.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware         # 0 failed, no timeouts
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo clippy --bin anvilml -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```

All six commands must exit 0.

## Known Constraints and Gotchas

- **`serial_test` is retained in `anvilml-hardware`.** The hardware crate uses `#[serial]` on sync tests that mutate `ANVILML_MOCK_DEVICE_TYPE` and `ANVILML_MOCK_VRAM_MIB`. This is a legitimate use (sync tests, no tokio runtime, genuine global env-var mutation). Do not touch that crate.
- **`serial_test` workspace dependency is retained.** Only the per-crate `[dev-dependencies]` entries in `anvilml-scheduler/Cargo.toml` and `backend/Cargo.toml` are removed. The workspace root `Cargo.toml` entry remains because `anvilml-hardware` still uses it.
- **`worker_threads = 2` is the minimum for dispatch-loop tests.** The scheduler dispatch loop is a single `tokio::spawn` task; 2 worker threads (the spawning thread + 1 worker) is sufficient. Using a higher count is harmless but unnecessary.
- **P19-A1 prereq update.** P19-A1 currently prereqs `P18-A4`. After P904 is authored, update P19-A1's prereqs to `["P904-A3"]` so Phase 19 cannot begin until the workspace is verified green.