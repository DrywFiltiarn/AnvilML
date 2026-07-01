# Implementation Report: P8-B3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P8-B3                           |
| Phase         | 008 — IPC Stress Gate & Worker Pool |
| Description   | anvilml-worker: job_object.rs Windows orphan-cleanup wrapper |
| Implemented   | 2026-07-01T09:15:00Z           |
| Status        | COMPLETE                        |

## Summary

Implemented `crates/anvilml-worker/src/job_object.rs`, a Windows-only module that wraps Win32 Job Objects to prevent orphaned Python worker subprocesses when the supervisor process dies. The module provides `JobObjectGuard::new()` to create a job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and `JobObjectGuard::assign_process()` to assign a spawned child process to the job. Added three `#[cfg(windows)]`-gated integration tests and updated the test catalogue.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         |
|--------|----------|-----------------|----------------|
| crate  | windows  | 0.58.0          | rust-docs MCP  |

Note: The MCP resolved `windows` 0.62.2 as latest, but the plan specified `0.58` (caret) to maintain compatibility with the workspace lockfile's existing `windows 0.58.0` transitive dependency. The caret range `>=0.58.0, <0.59.0` resolves to `0.58.0` from the lockfile. The API shape (`CreateJobObjectW`, `AssignProcessToJobObject`, `DuplicateHandle`, `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`) is stable across versions.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/job_object.rs` | Windows Job Object orphan-cleanup wrapper (182 lines) |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Added `windows` 0.58 target-conditional dependency with `Win32_Foundation`, `Win32_Security`, `Win32_System_JobObjects`, `Win32_System_Threading` features; bumped version 0.1.1 → 0.1.2 |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Added `#[cfg(windows)] mod job_object;` and `#[cfg(windows)] pub use job_object::JobObjectGuard;` |
| MODIFY | `crates/anvilml-worker/tests/spawn_tests.rs` | Added 3 `#[cfg(windows)]`-gated integration tests for orphan-cleanup |
| MODIFY | `docs/TESTS.md` | Added entries for 3 new tests |
| MODIFY | `Cargo.lock` | Updated with new `windows` dependency for `anvilml-worker` |

## Commit Log

```
 .forge/reports/P8-B3_plan.md               | 138 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 ++-
 Cargo.lock                                 |   3 +-
 crates/anvilml-worker/Cargo.toml           |  10 +-
 crates/anvilml-worker/src/job_object.rs    | 182 +++++++++++++++++++++++++++++
 crates/anvilml-worker/src/lib.rs           |   5 +
 crates/anvilml-worker/tests/spawn_tests.rs | 143 ++++++++++++++++++++++-
 docs/TESTS.md                              |  36 ++++++
 9 files changed, 523 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/spawn_tests.rs (target/debug/deps/spawn_tests-e46d9c68b2201952)

running 4 tests
test test_env_vars_applied ... ok
test test_interpreter_path_unix ... ok
test test_stdio_piped ... ok
test test_worker_script_arg ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Note: The 3 Windows-specific tests (`test_job_object_creation_succeeds`, `test_assigned_child_terminated_on_drop`, `test_double_assignment_fails_cleanly`) are gated `#[cfg(windows)]` and are not collected on Linux. They will be exercised on the `rust-windows` CI job and on Windows targets via `cargo test --target x86_64-pc-windows-gnu`.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware → Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.97s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.02s

# 3. Real-hardware Linux
cargo check --bin anvilml → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.33s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
cargo check --bin anvilml --target x86_64-pc-windows-gnu → Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.37s
```

All four cross-checks exit 0.

## Project Gates

Gate 1 (Config Surface Sync) — Not triggered: this task does not modify `ServerConfig` or any nested config struct.
Gate 2 (OpenAPI Drift) — Not triggered: this task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields.
Gate 3 (Node Parity) — Not triggered: this task does not add/remove/rename node types.
Gate 4 (Mock/Real Parity Markers) — Not triggered: this task does not modify `execute()`, `load()`, `sample()`, `decode()`, or `compute_latent_shape()` functions.

## Public API Delta

```
+pub use job_object::JobObjectGuard;
```

New public items:
- `pub struct JobObjectGuard` — `crates/anvilml-worker/src/job_object.rs` (struct, private fields)
- `pub fn JobObjectGuard::new() -> Result<Self, AnvilError>` — creates job object with kill-on-close
- `pub fn JobObjectGuard::assign_process(&self, child: &tokio::process::Child) -> Result<(), AnvilError>` — assigns child to job

These match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

1. **Dependency version**: The plan specified `windows = { version = "0.58", ... }`. The MCP resolved `0.62.2` as latest, but the caret range `0.58` (`>=0.58.0, <0.59.0`) resolves to `0.58.0` from the workspace lockfile, which is the existing transitive dependency version. This is consistent with the plan's intent.

2. **API substitutions in `windows` crate 0.58**:
   - `DuplicateHandle` is in `Win32::Foundation` (not `Win32::Security` as the plan implied). Added `Win32_Security` feature for `CreateJobObjectW` which requires `SECURITY_ATTRIBUTES`.
   - `CreateJobObjectW` returns `windows_core::Result<HANDLE>` (not `Option<HANDLE>`). Changed error handling to use `map_err`.
   - `as_raw_handle()` is unstable in Rust 1.96 (nightly-only feature). Used `raw_handle()` which is stable and returns `Option<*mut c_void>`.
   - `windows::core::Error::from_last_error()` does not exist in `windows` 0.58. Used `windows::core::Error::from_win32()` and converted to `std::io::Error` manually.
   - `BOOL::as_bool()` does not exist in `windows` 0.58. Used `Result::is_ok()` on the Win32 API return values.
   - `PROCESS_ALL_ACCESS` is `PROCESS_ACCESS_RIGHTS` (newtype), not `u32`. Used `.0` to extract the inner value.
   - `AssignProcessToJobObject` is `unsafe` in `windows` 0.58. Added `unsafe` block.

3. **Test approach**: The plan's `test_assigned_child_terminated_on_drop` and `test_double_assignment_fails_cleanly` use `tokio::runtime::Builder::new_current_thread()` to create a blocking runtime for spawning processes and waiting for them. The original plan's suggestion of using `std::process::Command` was changed to `tokio::process::Command` to get a `tokio::process::Child` which is the type expected by `assign_process()`.

## Blockers

None.
