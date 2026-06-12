# Plan Report: P905-A7

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A7                                           |
| Phase       | 905 — FP8 dtype support & model metadata          |
| Description | backend: fix cancel_terminal_job_returns_409 CI failure |
| Depends on  | P20-A2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T13:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Fix the `cancel_terminal_job_returns_409` test in `backend/tests/api_cancel.rs` by ensuring the `ANVILML_WORKER_MOCK=1` environment variable is set within the test's `temp_env::async_with_vars` closure and cleaned up unconditionally at function end, so the mock hardware + mock worker path is exercised consistently. Also bump the `backend` crate patch version.

## Scope

### In Scope
- `backend/tests/api_cancel.rs`: verify `("ANVILML_WORKER_MOCK", Some("1"))` is present in the `cancel_terminal_job_returns_409` test's `temp_env::async_with_vars` vars array; verify `std::env::remove_var("ANVILML_WORKER_MOCK")` is in the unconditional cleanup block at end of function.
- `backend/Cargo.toml`: bump patch version (0.1.13 → 0.1.14).
- `cargo test --features mock-hardware --test api_cancel`: verify both tests pass.

### Out of Scope
- Any other test files or source files.
- Any changes to `cancel_running_job_returns_202_and_ws_cancelled` (it already has the correct env vars).
- CI workflow file changes.
- OpenAPI drift or config surface sync changes.

## Approach

1. **Read** `backend/tests/api_cancel.rs` and inspect the `cancel_terminal_job_returns_409` function (lines 455–591).
2. **Verify** that the `temp_env::async_with_vars` call at line 465–470 includes `("ANVILML_WORKER_MOCK", Some("1"))` in its vars array. If absent, insert it after the existing `ANVILML_MOCK_VRAM_MIB` entry.
3. **Verify** that the unconditional cleanup block at the end of the function (after the `.await` on `async_with_vars`) includes `std::env::remove_var("ANVILML_WORKER_MOCK")`. If absent, add it alongside the existing `remove_var` calls.
4. **Bump** `backend/Cargo.toml` `[package] version` from `0.1.13` to `0.1.14`.
5. **Run** `cargo test --features mock-hardware --test api_cancel` and verify both tests pass (exit 0).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/tests/api_cancel.rs` | Ensure `ANVILML_WORKER_MOCK` env var is set in `cancel_terminal_job_returns_409` test and cleaned up unconditionally. |
| Modify | `backend/Cargo.toml` | Bump patch version `0.1.13` → `0.1.14`. |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `backend/tests/api_cancel.rs` | `cancel_running_job_returns_202_and_ws_cancelled` | Full cancellation flow: submit job, cancel → 202, WS `JobCancelled`, job status `Cancelled`, worker `Idle`. |
| `backend/tests/api_cancel.rs` | `cancel_terminal_job_returns_409` | Cancelling a completed job returns 409 with `job_not_cancellable` error body. |

## CI Impact

No CI workflow files are modified. The existing CI test gate (`cargo test --workspace --features mock-hardware`) already runs this test file. The fix ensures the test passes reliably in CI by guaranteeing the mock worker environment variable is set.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ANVILML_WORKER_MOCK` is already present in the vars array and cleanup block, making the change a no-op | High | None — idempotent verification is harmless | Verify presence before writing; if already present, skip to version bump. |
| The version bump conflicts with a concurrent release | Low | Medium — would require manual resolution | Patch bump is always safe; The Forge handles release coordination. |
| Test still fails due to flakiness or environment issue | Low | Medium | Diagnose root cause (parallelism, port conflict, DB state) and fix test isolation before proceeding. |

## Acceptance Criteria

- [ ] `("ANVILML_WORKER_MOCK", Some("1"))` is present in `cancel_terminal_job_returns_409`'s `temp_env::async_with_vars` vars array
- [ ] `std::env::remove_var("ANVILML_WORKER_MOCK")` is present in the unconditional cleanup block at end of `cancel_terminal_job_returns_409`
- [ ] `backend/Cargo.toml` patch version bumped from `0.1.13` to `0.1.14`
- [ ] `cargo test --features mock-hardware --test api_cancel` exits 0 with both tests passing
