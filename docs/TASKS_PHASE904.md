# Tasks: Phase 904 — Test Isolation Hardening

| Field | Value |
|-------|-------|
| Phase | 904 |
| Name | Test Isolation Hardening |
| Milestone group | Correction (retrofit) |
| Depends on phases | 18 |
| Task file | `.forge/tasks/tasks_phase904.json` |
| Tasks | 4 |

## Overview

Phase 904 is a retrofit correction phase inserted between Phase 18 and Phase 19. It resolves three classes of test bug that cause the CI `rust-linux` and `rust-windows` jobs to either hang (>60 s timeout) or panic.

**Root cause 1 — `sqlite::memory:` multi-connection pool.** `anvilml-scheduler`'s `setup_pool()` helper calls `SqlitePool::connect("sqlite::memory:")`, which creates a pool with the default `max_connections` (10). Each SQLite `:memory:` URL is a separate, independent database per connection. The `CREATE TABLE` DDL executes on connection 0; subsequent queries are dispatched to connections 1–9, which see an empty schema. The result is either a hard SQL error or an indefinite hang.

**Root cause 2 — `#[serial_test::serial]` + `#[tokio::test]` deadlock.** `serial_test` 3.x enforces serialisation by acquiring a `parking_lot::Mutex` on the OS thread for the duration of each annotated test. `#[tokio::test]` (default flavor: `current_thread`) drives the runtime via `Runtime::block_on(...)` on that same thread. Any test that spawns a background `tokio::task` (dispatch loop, axum server, broadcast subscriber) requires the tokio thread to poll those tasks — but the thread is blocked in `block_on` holding the serial lock. Nothing can make progress. This affects all scheduler tests and four backend integration test files (`preflight_check.rs`, `api_ws_lifecycle.rs`, `api_cancel.rs`, `api_delete.rs`).

**Root cause 3 — `resolve_interpreter_unix` test missing platform guard.** `backend/src/preflight.rs` contains a unit test `resolve_interpreter_unix` that asserts the Unix interpreter path (`/opt/myvenv/bin/python3`) with no `#[cfg(not(windows))]` guard. The production function `resolve_interpreter()` correctly returns the Windows path on Windows via `cfg!(windows)`, so the test panics on Windows with a path mismatch.

The fix for root causes 1 and 2 is uniform: fix the `:memory:` pool to `max_connections(1)`, remove `#[serial]` everywhere it is not needed, and switch tests that require background tasks to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. The fix for root cause 3 is a single `#[cfg(not(windows))]` attribute on one test function.

`serial_test` remains a legitimate dependency in `anvilml-hardware`, where it serialises sync tests that mutate `ANVILML_MOCK_DEVICE_TYPE` / `ANVILML_MOCK_VRAM_MIB` environment variables. The workspace dependency is retained; only the per-crate dev-dependency entries for `anvilml-scheduler` and `backend` are removed.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Test isolation fixes | P904-A1, P904-A2, P904-A2b, P904-A3 | Fix scheduler pool + serial, fix backend serial, fix preflight platform guard, verify workspace |

## Prerequisites

- P18-A4 must be complete (final task of Phase 18).
- The workspace must compile clean under `cargo check --workspace --features mock-hardware` before this phase begins.

## Interfaces and Contracts

No production interfaces change. All modifications are confined to `#[cfg(test)]` code, `[dev-dependencies]` entries, and test attribute annotations.

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
- For all remaining scheduler tests that call `make_scheduler()` or `start_dispatch_loop()`: replace `#[serial]` + `#[tokio::test]` with `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.

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
- Change `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` on every test function.
- Env isolation already in place via `temp_env::async_with_vars`: leave unchanged.

**preflight_check.rs changes:**
- Same `#[serial]` removal and `multi_thread` runtime change as above.
- Additionally, in `job_submit_rejected_when_preflight_fails`: replace the bare `std::env::remove_var("ANVILML_WORKER_MOCK")` at the top of the test body with a `temp_env::async_with_vars([("ANVILML_WORKER_MOCK", None::<&str>)], async { <rest of body> }).await` wrapper so the env state is properly scoped rather than unconditionally stripped.

**Cargo.toml changes:**
- Remove `serial_test = { workspace = true }` from `[dev-dependencies]`.

**Acceptance criterion:** `cargo test -p backend --features mock-hardware` exits 0 with 0 failed and no test exceeding 30 seconds.

---

#### P904-A2b: backend: fix resolve_interpreter_unix test running on Windows without platform guard

**Goal:** Stop `resolve_interpreter_unix` from panicking on Windows.

**File to modify:**
- `backend/src/preflight.rs`

**The bug:** `resolve_interpreter_unix` has no platform guard and asserts the Unix interpreter path (`/opt/myvenv/bin/python3`). The production function `resolve_interpreter()` correctly returns the Windows path on Windows via `cfg!(windows)`, causing the assertion to fail:

```
left:  "/opt/myvenv\\Scripts\\python.exe"
right: "/opt/myvenv/bin/python3"
```

**The fix:** Add `#[cfg(not(windows))]` to the `resolve_interpreter_unix` test function. One attribute, no logic changes:

```rust
#[test]
#[cfg(not(windows))]
fn resolve_interpreter_unix() {
    let venv = Path::new("/opt/myvenv");
    let result = resolve_interpreter(venv);
    assert_eq!(result, PathBuf::from("/opt/myvenv/bin/python3"));
}
```

The companion `resolve_interpreter_windows` test is already correctly structured with `#[cfg(windows)]` guarding its assertion body — do not change it.

No changes to production code. No changes to any other test function.

**Acceptance criterion:** `cargo test -p backend --features mock-hardware -- preflight` exits 0 on both Linux and Windows (cross-check: `cargo test -p backend --features mock-hardware --target x86_64-pc-windows-gnu -- preflight` exits 0).

---

#### P904-A3: anvilml: verify full workspace test suite green after P904 isolation fixes

**Goal:** Confirm that P904-A1, P904-A2, and P904-A2b have not introduced regressions elsewhere, and that all six CI gates pass clean.

**No source changes permitted** unless a test failure is directly and demonstrably caused by the P904 changes. Any such fix must be documented clearly in the implement report with a root-cause explanation.

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
- **`serial_test` workspace dependency is retained.** Only the per-crate `[dev-dependencies]` entries in `anvilml-scheduler/Cargo.toml` and `backend/Cargo.toml` are removed.
- **`resolve_interpreter_windows` test body is already gated.** The existing `#[cfg(windows)]` guard is inside the test body, not on the function itself. This means the function still compiles and runs on all platforms but is a no-op on non-Windows. This pattern is intentional and must not be changed.
- **`worker_threads = 2` is the minimum for dispatch-loop tests.** The scheduler dispatch loop is a single `tokio::spawn` task; 2 worker threads is sufficient.
- **P19-A1 prereq update required.** P19-A1 currently prereqs `P18-A4`. After P904 is committed, update P19-A1's prereqs to `["P904-A3"]` so Phase 19 cannot begin until the workspace is verified green.