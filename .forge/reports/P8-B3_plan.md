# Plan Report: P8-B3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-B3                                       |
| Phase       | 008 — IPC Stress Gate & Worker Pool         |
| Description | anvilml-worker: job_object.rs Windows orphan-cleanup wrapper |
| Depends on  | P8-B2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-01T06:20:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-worker/src/job_object.rs`, a Windows-only module that wraps Win32 Job Objects to prevent orphaned Python worker subprocesses when the supervisor process dies unexpectedly. The module provides `JobObjectGuard::new()` to create a job object with the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` limit, and `JobObjectGuard::assign_process()` to assign a spawned child process to the job. On Linux this module is not compiled (`#[cfg(windows)]`), leaving a documented gap for future Linux orphan-cleanup work.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/job_object.rs` with `JobObjectGuard` struct and its methods, gated `#[cfg(windows)]`.
- Implement `JobObjectGuard::new() -> Result<Self, AnvilError>`: creates a Win32 Job Object via `CreateJobObjectW`, configures `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` via `SetInformationJobObject`.
- Implement `JobObjectGuard::assign_process(&self, child: &tokio::process::Child) -> Result<(), AnvilError>`: extracts the raw Windows HANDLE from the child via `std::os::windows::process::ChildExt`, duplicates it with `DuplicateHandle`, assigns it to the job object via `AssignProcessToJobObject`.
- Add `#[cfg(windows)] mod job_object;` and `pub use job_object::JobObjectGuard;` to `lib.rs` (gated so it does not appear on non-Windows builds).
- Add `windows` crate as a `[target.'cfg(windows)'.dependencies]` in `Cargo.toml` with the `Win32_System_JobObjects` and `Win32_System_Threading` feature flags.
- Write >=3 integration tests in `crates/anvilml-worker/tests/spawn_tests.rs`, gated `#[cfg(windows)]` at the file level, covering: job object creation succeeds, an assigned child is terminated when the handle drops, double-assignment errors cleanly.

### Out of Scope
- Linux orphan cleanup (PR_SET_PDEATHSIG or equivalent). This task explicitly does not implement any Linux mechanism; it is a documented gap.
- Any changes to `spawn.rs` itself — that module handles command construction and spawning; job-object assignment is a caller-side concern wired by the next task in this group (P8-G1's `WorkerPool::spawn_all()`).
- Dual-mode parity markers (REAL_PATH_VERIFIED/MOCK_PATH_VERIFIED) — these apply to node `execute()` and arch-module `load()`/`sample()`/`decode()` functions in the Python worker, not to a Windows process-management utility.

defers_to (from JSON): []

## Existing Codebase Assessment

The `anvilml-worker` crate currently has two source modules: `env.rs` (environment variable builder) and `spawn.rs` (subprocess `Command` construction and `spawn_worker()`). The `lib.rs` is a minimal re-export stub (7 lines). The `spawn.rs` module already has `#[cfg(windows)]` / `#[cfg(unix)]` platform-specific branches for interpreter path selection, establishing the pattern this task follows.

The `spawn.rs` module uses `tokio::process::Command` and `tokio::process::Child` (from the `process` feature of tokio 1.52.3). The `AnvilError` type from `anvilml-core` is already imported and used — the `Io` variant wraps `std::io::Error`, which is the natural source for Win32 API errors via `windows::core::Error::into_io_error()`.

The test file `tests/spawn_tests.rs` currently has 5 tests, all gated or platform-neutral. The `#[cfg(windows)]` test `test_interpreter_path_windows` exists at line 38. This task adds additional `#[cfg(windows)]`-gated tests to the same file.

No dual-mode parity markers apply to this task — the convention covers Python node/arch-module functions, not Rust process-management utilities.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed                              |
|--------|----------|-----------------|----------------|------------------------------------------------------|
| crate  | windows  | 0.62.2          | rust-docs MCP  | Win32_System_JobObjects, Win32_System_Threading, Win32_Foundation |

Note: The workspace Cargo.lock pins `windows` 0.58.0 as a transitive dependency. This task introduces `windows` as a direct dependency of `anvilml-worker`. The API shape (`CreateJobObjectW`, `AssignProcessToJobObject`, `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`) is stable across 0.58–0.62 and was confirmed in the 0.62.2 README. If the resolver picks 0.58.0 instead, the API is identical — no code change needed.

## Approach

1. **Add `windows` dependency to `Cargo.toml`.** In `crates/anvilml-worker/Cargo.toml`, add a target-conditional dependency:
   ```toml
   [target.'cfg(windows)'.dependencies]
   windows = { version = "0.58", features = [
       "Win32_Foundation",
       "Win32_System_JobObjects",
       "Win32_System_Threading",
   ]}
   ```
   Rationale: Using `0.58` (caret) keeps the version compatible with the workspace lockfile's existing 0.58.0 while allowing patch updates. The feature flags are minimal — only what's needed for job objects, plus `Win32_Foundation` (required transitively for `HANDLE` and `BOOL` types).

2. **Create `crates/anvilml-worker/src/job_object.rs`.** The module contains:
   - `JobObjectGuard` struct holding the `HANDLE` to the job object.
   - `impl JobObjectGuard`:
     - `pub fn new() -> Result<Self, AnvilError>`: Creates a named or anonymous job object via `CreateJobObjectW(None, 0)`. If creation fails, converts the `windows::core::Error` to `AnvilError::Io`. Configures the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` limit by populating a `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` struct (setting `BasicLimitInformation.LimitFlags` to `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), then calling `SetInformationJobObject` with `JobObjectExtendedLimitInformation`. Emits a DEBUG log on success.
     - `pub fn assign_process(&self, child: &tokio::process::Child) -> Result<(), AnvilError>`: Uses `std::os::windows::process::ChildExt::as_raw_handle()` to get the child's raw Windows HANDLE. Duplicates the handle via `DuplicateHandle` (current process → current process, because `AssignProcessToJobObject` requires a handle with `PROCESS_ALL_ACCESS` and the raw handle from `as_raw_handle()` may not have sufficient access). Calls `AssignProcessToJobObject` with the duplicated handle and the job object's `HANDLE`. Closes the duplicated handle via `CloseHandle` regardless of success/failure (to avoid handle leaks). On error, converts to `AnvilError::Io`. Emits a DEBUG log on success.
   - The entire module is gated with `#[cfg(windows)]` at the top of the file.
   - Inline comments explain: why `DuplicateHandle` is needed (Win32 job objects require a process handle with sufficient access rights), and why the duplicated handle is closed after assignment (the job object holds its own reference).

3. **Wire `job_object` module into `lib.rs`.** Add `#[cfg(windows)] mod job_object;` and `#[cfg(windows)] pub use job_object::JobObjectGuard;` after the existing `mod spawn;` / `pub use spawn` lines. Keep `lib.rs` under 80 lines (currently 7 lines, adding ~4 more is fine).

4. **Write integration tests in `tests/spawn_tests.rs`.** Add three `#[cfg(windows)]`-gated tests:
   - `test_job_object_creation_succeeds`: Calls `JobObjectGuard::new()` and asserts `Ok`. Verifies the job object was created without error.
   - `test_assigned_child_terminated_on_drop`: Creates a long-running child process (`cmd /c timeout 999` on Windows), assigns it to the job object, then drops the guard. Asserts the child process has exited within a bounded timeout (5 seconds). If the timeout fires, captures the child's exit status and includes it in the failure message. This tests the core orphan-prevention guarantee.
   - `test_double_assignment_fails_cleanly`: Creates a job object, assigns one child, then attempts to assign a second child to the same job. Asserts the second assignment returns an error (Win32 `AssignProcessToJobObject` returns `ERROR_ACCESS_DENIED` when a process is already in another job). Verifies the error is converted to `AnvilError::Io` cleanly — no panic, no resource leak.

5. **Verify compilation.** Run `cargo check -p anvilml-worker --features mock-hardware --target x86_64-pc-windows-gnu` to confirm the `#[cfg(windows)]` module compiles on the cross-check target (per ENVIRONMENT.md §7).

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `struct JobObjectGuard` | `crates/anvilml-worker/src/job_object.rs` | `pub struct JobObjectGuard { /* private */ }` |
| `JobObjectGuard::new` | `crates/anvilml-worker/src/job_object.rs` | `pub fn new() -> Result<Self, AnvilError>` |
| `JobObjectGuard::assign_process` | `crates/anvilml-worker/src/job_object.rs` | `pub fn assign_process(&self, child: &tokio::process::Child) -> Result<(), AnvilError>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/job_object.rs` | Windows Job Object orphan-cleanup wrapper |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `#[cfg(windows)] mod job_object;` and re-export |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Add `windows` target-conditional dependency with feature flags |
| MODIFY | `crates/anvilml-worker/tests/spawn_tests.rs` | Add >=3 `#[cfg(windows)]`-gated integration tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_job_object_creation_succeeds` | `JobObjectGuard::new()` creates a job object without error | Windows target, `windows` crate available | None | `Ok(JobObjectGuard { … })` | `cargo test -p anvilml-worker --test spawn_tests --features mock-hardware -- --include-ignored` exits 0 (on Windows) |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_assigned_child_terminated_on_drop` | A child assigned to a job object is killed when the guard drops | Windows target, child process can be spawned | `cmd /c timeout 999` subprocess | Child process exits within 5s of guard drop | `cargo test -p anvilml-worker --test spawn_tests --features mock-hardware --test-threads=1` exits 0 (on Windows) |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_double_assignment_fails_cleanly` | Assigning a second child to the same job object returns an error cleanly (no panic, no leak) | Windows target, two child processes spawnable | Two `cmd /c timeout 999` subprocesses | Second `assign_process()` returns `Err(AnvilError::Io(_))` | `cargo test -p anvilml-worker --test spawn_tests --features mock-hardware --test-threads=1` exits 0 (on Windows) |

Note: Tests use `--test-threads=1` because they spawn long-running subprocesses that must not race. The `test_assigned_child_terminated_on_drop` test uses a bounded 5-second wait on the subprocess exit — per ENVIRONMENT.md §11.5's bounded-wait requirement, this prevents indefinite hangs if the child fails to start or the job object mechanism behaves unexpectedly.

## CI Impact

No CI changes required. The tests are gated `#[cfg(windows)]` at the test file level, so they are only collected and run on Windows CI jobs (`rust-windows`). The Linux CI job (`rust-linux`) will compile the crate successfully because the `job_object` module is behind `#[cfg(windows)]` — the `#[cfg(unix)]` compilation path has zero references to `JobObjectGuard`. The Windows cross-check (`cargo check --target x86_64-pc-windows-gnu`) in ENVIRONMENT.md §7 already exercises this code path.

## Platform Considerations

This task is Windows-only. The `job_object.rs` module is gated `#[cfg(windows)]` at the file level. The `lib.rs` mod declaration is also `#[cfg(windows)]`. On non-Windows targets, the module is entirely absent from the compiled binary — no dead code, no unresolved symbols.

No `#[cfg(unix)]` code is introduced. Linux orphan cleanup via `PR_SET_PDEATHSIG` is explicitly out of scope and documented as a gap.

The `windows` crate dependency is target-conditional (`[target.'cfg(windows)'.dependencies]`), so it does not affect Linux/macOS builds at all.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `as_raw_handle()` returns a HANDLE that cannot be duplicated because it lacks `PROCESS_DUP_HANDLE` access. The `DuplicateHandle` call fails with `ERROR_ACCESS_DENIED`. | Medium | High | Use `OpenProcess(PROCESS_ALL_ACCESS, FALSE, child.id())` to get a fresh handle with full access before duplicating. The `child.id()` method (available on all platforms via `std::process::Child::id()`) returns the OS process ID, which `OpenProcess` can use to obtain a full-access handle. This avoids the `as_raw_handle()` access-rights issue entirely. |
| The `windows` crate's `Win32_System_JobObjects` feature flag name changed between versions. The MCP-confirmed name for 0.62.2 is `Win32_System_JobObjects`; if 0.58.0 uses a different name, the build fails on Windows. | Low | High | The windows-rs crate has used consistent feature flag names across 0.57–0.62 for core Win32 subsystems. If the flag name differs, the ACT agent resolves via MCP at session start and adjusts. |
| Test `test_assigned_child_terminated_on_drop` hangs because the child process does not exit even after the job object kills it. The bounded wait prevents the test from hanging forever, but the captured stderr may be empty if Windows kills the process silently. | Low | Medium | Use `child.wait().timeout(Duration::from_secs(5))` with a clear failure message that includes the child's exit status (if available) and any captured output. On Windows, `TerminateProcess` by job objects does not produce stdout/stderr — the empty output is expected. |
| `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` struct layout differs between Windows versions. | Low | Low | The struct is a standard Win32 API struct with stable layout since Windows Vista. The `windows` crate generates bindings from the latest SDK metadata, ensuring correct layout. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-worker --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `cargo test -p anvilml-worker --test spawn_tests --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (on Windows, all 3+ tests pass)
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` prints a number ≤ 80
- [ ] `grep -c "^## " .forge/reports/P8-B3_plan.md` prints 12
- [ ] `head -1 .forge/reports/P8-B3_plan.md` prints `# Plan Report: P8-B3`
- [ ] `wc -l .forge/reports/P8-B3_plan.md` prints a number > 40
