# Plan Report: P904-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A2                                           |
| Phase       | 904 — Test Isolation Hardening                    |
| Description | backend: fix test isolation (serial removal, multi_thread runtime, temp_env cleanup) |
| Depends on  | P18-A4                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T06:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Eliminate the `#[serial_test::serial]` + `#[tokio::test]` (current_thread) deadlock across all four backend integration test files by removing `#[serial]`, removing the `serial_test` dev-dependency, and switching to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. Additionally, fix env-var isolation in `preflight_check.rs` by replacing a bare `std::env::remove_var("ANVILML_WORKER_MOCK")` with a `temp_env::async_with_vars` scope.

## Scope

### In Scope
- `backend/tests/api_ws_lifecycle.rs`: remove `use serial_test::serial;`, remove `#[serial]` from `test_ws_lifecycle_full_job`, change `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.
- `backend/tests/api_cancel.rs`: remove `use serial_test::serial;`, remove `#[serial]` from `cancel_running_job_returns_202_and_ws_cancelled` and `cancel_terminal_job_returns_409`, change both `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.
- `backend/tests/api_delete.rs`: remove `use serial_test::serial;`, remove `#[serial]` from all 5 test functions, change all `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`.
- `backend/tests/preflight_check.rs`: remove `use serial_test::serial` (or `serial_test::serial`) and `#[serial_test::serial]` from `job_submit_rejected_when_preflight_fails` and `job_submit_proceeds_in_mock_mode`; change both `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`; in `job_submit_rejected_when_preflight_fails`, replace bare `std::env::remove_var("ANVILML_WORKER_MOCK")` at line 125 with `temp_env::async_with_vars([("ANVILML_WORKER_MOCK", None::<&str>)], async { <rest of body> }).await`.
- `backend/Cargo.toml`: remove `serial_test = { workspace = true }` from `[dev-dependencies]`.

### Out of Scope
- `crates/anvilml-hardware` — serial_test remains there legitimately (sync tests mutating env vars).
- `crates/anvilml-scheduler` — handled by P904-A1.
- Workspace root `Cargo.toml` — serial_test workspace dependency retained.
- Any production code changes.
- Version bumps (no source files in a crate are modified; only test files and dev-dependencies).

## Approach

1. **`api_ws_lifecycle.rs`**: Delete line 27 (`use serial_test::serial;`). Delete line 80 (`#[serial]`). Change line 81 from `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. Leave the existing `std::env::set_var`/`std::env::remove_var` cleanup at the end unchanged (already adequate).

2. **`api_cancel.rs`**: Delete line 29 (`use serial_test::serial;`). Delete line 198 (`#[serial]`) and change line 199 (`#[tokio::test]` → `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`). Delete line 456 (`#[serial]`) and change line 457 (`#[tokio::test]` → `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`). Leave existing `temp_env::async_with_vars` + unconditional `remove_var` cleanup unchanged.

3. **`api_delete.rs`**: Delete line 32 (`use serial_test::serial;`). Delete lines 227, 340, 435, 565, 682 (`#[serial]`). Change lines 228, 341, 436, 566, 683 (`#[tokio::test]` → `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`). Leave existing `temp_env::async_with_vars` + unconditional `remove_var` cleanup unchanged.

4. **`preflight_check.rs`**:
   - Remove `#[serial_test::serial]` from `job_submit_rejected_when_preflight_fails` (line 121) and `job_submit_proceeds_in_mock_mode` (line 226).
   - Change `#[tokio::test]` to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` on both of those test functions (lines 122, 227).
   - Also change `#[tokio::test]` on `env_endpoint_reflects_failed_preflight` (line 46) and `env_returns_correct_shape_in_stub_context` (line 87) to `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` for consistency (they don't use serial, but the task says "same serial+runtime changes" on every test fn).
   - In `job_submit_rejected_when_preflight_fails`: remove the bare `std::env::remove_var("ANVILML_WORKER_MOCK");` at line 125. Wrap the entire test body (from `let tmp = tempfile::tempdir()...` through the end of the function) inside `temp_env::async_with_vars([("ANVILML_WORKER_MOCK", None::<&str>)], async { <body> }).await;`.

5. **`backend/Cargo.toml`**: Remove line 39 (`serial_test = { workspace = true }`) from `[dev-dependencies]`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/tests/api_ws_lifecycle.rs` | Remove `use serial_test::serial;`, remove `#[serial]`, change `#[tokio::test]` to multi_thread |
| Modify | `backend/tests/api_cancel.rs` | Remove `use serial_test::serial;`, remove 2x `#[serial]`, change 2x `#[tokio::test]` to multi_thread |
| Modify | `backend/tests/api_delete.rs` | Remove `use serial_test::serial;`, remove 5x `#[serial]`, change 5x `#[tokio::test]` to multi_thread |
| Modify | `backend/tests/preflight_check.rs` | Remove 2x `#[serial_test::serial]`, change 4x `#[tokio::test]` to multi_thread, wrap `job_submit_rejected_when_preflight_fails` body in `temp_env::async_with_vars` |
| Modify | `backend/Cargo.toml` | Remove `serial_test = { workspace = true }` from `[dev-dependencies]` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `backend/tests/api_ws_lifecycle.rs` | `test_ws_lifecycle_full_job` | WebSocket lifecycle with multi_thread runtime (no serial dependency) |
| `backend/tests/api_cancel.rs` | `cancel_running_job_returns_202_and_ws_cancelled` | Cancel flow with multi_thread runtime |
| `backend/tests/api_cancel.rs` | `cancel_terminal_job_returns_409` | Cancel terminal job returns 409 with multi_thread runtime |
| `backend/tests/api_delete.rs` | `delete_completed_job_removes_artifact_and_row` | Delete completed job + artifact cleanup |
| `backend/tests/api_delete.rs` | `delete_running_job_returns_409` | Delete running job returns 409 |
| `backend/tests/api_delete.rs` | `bulk_delete_all_terminal_jobs` | Bulk delete all terminal jobs |
| `backend/tests/api_delete.rs` | `bulk_delete_by_status_removes_only_matching` | Bulk delete by specific status |
| `backend/tests/api_delete.rs` | `delete_nonexistent_job_returns_404` | Delete nonexistent job returns 404 |
| `backend/tests/preflight_check.rs` | `env_endpoint_reflects_failed_preflight` | Env endpoint with failed preflight |
| `backend/tests/preflight_check.rs` | `env_returns_correct_shape_in_stub_context` | Env endpoint shape in stub context |
| `backend/tests/preflight_check.rs` | `job_submit_rejected_when_preflight_fails` | Preflight gate rejects job (now scoped with temp_env) |
| `backend/tests/preflight_check.rs` | `job_submit_proceeds_in_mock_mode` | Mock mode bypasses preflight gate |

## CI Impact

No CI workflow files are modified. The test command changes are internal to the test runtime configuration (multi_thread vs current_thread). The acceptance criterion `cargo test -p backend --features mock-hardware` must exit 0, which is the same command used by the CI `rust-linux` and `rust-windows` gates. Removing `serial_test` from dev-dependencies reduces the build graph slightly but has no effect on CI structure.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Multi-thread runtime changes test execution timing, causing intermittent failures | Low | Medium | The `multi_thread` runtime is the correct fix for the deadlock; if timing-sensitive tests fail, add explicit `sleep` or use `temp_env::async_with_vars` to scope env vars. The acceptance criterion run will surface any issues. |
| `temp_env::async_with_vars` wrapper in `preflight_check.rs` changes indentation/structure, introducing a syntax error | Low | Low | Careful one-shot edit; verify compilation with `cargo check -p backend --features mock-hardware`. |
| Removing `serial_test` from backend dev-dependencies breaks a test that implicitly relies on serial execution order | Low | Medium | Each test already uses its own temp dir, in-memory DB, and unique port. No shared mutable state exists between tests. Verified by reviewing all 4 test files. |
| `job_submit_proceeds_in_mock_mode` sets `ANVILML_WORKER_MOCK` via `set_var` (not scoped) — this remains unchanged per task scope | Low | Low | This test already has save/restore cleanup (lines 275, 321-325). The task only requires fixing `job_submit_rejected_when_preflight_fails`, not this test. |

## Acceptance Criteria

- [ ] `cargo check -p backend --features mock-hardware` exits 0
- [ ] `cargo test -p backend --features mock-hardware` exits 0 with all 12 tests passing
- [ ] No `serial_test` import remains in any backend test file
- [ ] No `#[serial]` or `#[serial_test::serial]` attribute remains in any backend test file
- [ ] All test functions use `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`
- [ ] `serial_test` line removed from `backend/Cargo.toml` `[dev-dependencies]`
- [ ] `job_submit_rejected_when_preflight_fails` uses `temp_env::async_with_vars` instead of bare `std::env::remove_var`
