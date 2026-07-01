# Plan Report: P8-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-B2                                         |
| Phase       | 8 — IPC Stress Gate & Worker Pool            |
| Description | anvilml-worker: spawn.rs subprocess Command construction |
| Depends on  | P8-B1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-07-01T07:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-worker/src/spawn.rs` implementing `spawn_worker()` — the function that constructs a `tokio::process::Command` targeting the correct Python interpreter in the worker venv (platform-specific path per ENVIRONMENT.md §5), running `worker/worker_main.py`, with environment variables injected from `WorkerEnv::build()`, and stdout/stderr piped so the supervisor reads them. This is the subprocess construction layer that `WorkerPool::spawn_all()` will call to launch each Python worker process.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/spawn.rs` with `spawn_worker(venv_path: &Path, env: HashMap<String, String>) -> Result<tokio::process::Child, AnvilError>`
- Platform-specific interpreter path: `{venv_path}/bin/python3` on Unix (`#[cfg(unix)]`), `{venv_path}\Scripts\python.exe` on Windows (`#[cfg(windows)]`)
- Worker script argument: `worker/worker_main.py` (relative to cwd)
- Apply env map via `Command::envs()`
- Set stdout and stderr to `Stdio::piped()`
- Declare `mod spawn; pub use spawn::spawn_worker;` in `lib.rs`
- Add `tokio` dependency with `process` feature to `anvilml-worker/Cargo.toml`
- Create `crates/anvilml-worker/tests/spawn_tests.rs` with ≥4 tests
- Bump `anvilml-worker` patch version in `Cargo.toml`

### Out of Scope
- Windows Job Object wrapping — deferred to P8-B3 (`defers_to: []` is empty, but this scope is explicitly deferred by the task context to a separate task, not to another task in `defers_to`; no deferral bullet is needed here because the task's own context states this)
- Actual process execution — tests verify Command construction only, not real subprocess spawning (worker_main.py does not exist until Phase 9)
- Error handling for interpreter not found — that is a runtime concern for `WorkerPool::spawn_all()` which wraps the spawn in a retry/respawn loop

defers_to (from JSON): []

## Existing Codebase Assessment

The `anvilml-worker` crate currently has two files: `lib.rs` (4 lines, re-exporting `WorkerEnv`) and `src/env.rs` (77 lines, the `WorkerEnv::build()` function). The crate has no tokio dependency yet — it only depends on `anvilml-ipc`, `anvilml-hardware`, and `anvilml-core` via path dependencies.

The established patterns are:
- **Error handling:** `AnvilError` from `anvilml-core` is used throughout; `std::io::Error` maps automatically via `#[from]` on `AnvilError::Io`.
- **Naming:** Functions use `snake_case`; structs are `PascalCase`; modules are `snake_case`.
- **Test style:** Tests go in `crates/{name}/tests/` as separate test crate files (not inline `#[cfg(test)]` blocks). Tests import via `use anvilml_worker::...` and use the public API only. Each test has a doc comment explaining what it verifies.
- **Logging:** `#[tracing::instrument]` is applied to meaningful async functions; structured field notation is used.

No gap between design doc and current source affects this task — the design doc (§9.3) already specifies `spawn.rs` as the module for subprocess construction, and the crate's current state (minimal, only env.rs) matches the expected starting point for this group of tasks.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | process                  |

The workspace lockfile pins tokio at 1.52.3 (confirmed via `Cargo.lock`). The `process` feature is required for `tokio::process::Command` and `tokio::process::Child`. This feature is confirmed via `rust-docs_get_crate_features` on tokio 1.52.3.

## Approach

### Step 1: Add tokio dependency to Cargo.toml

Add `tokio = { version = "1.52.3", features = ["process"] }` to `[dependencies]` in `crates/anvilml-worker/Cargo.toml`. This is the minimal feature set needed — only `process` is required for `Command` and `Child`. The `process` feature transitively brings in `bytes`, `libc`, `mio`, and `signal-hook-registry` on Unix.

### Step 2: Create `crates/anvilml-worker/src/spawn.rs`

Implement two items:

**`pub fn build_command(venv_path: &Path, env: HashMap<String, String>) -> tokio::process::Command`**

This is a helper function that constructs and configures the `Command` without spawning it. It is public so that tests can inspect the Command's configuration without actually spawning a subprocess (which would fail since `worker_main.py` does not exist yet).

The function:
1. Determines the interpreter path using `#[cfg(unix)]` / `#[cfg(windows)]`:
   - Unix: `venv_path.join("bin/python3")`
   - Windows: `venv_path.join("Scripts\\python.exe")`
2. Calls `.arg("worker/worker_main.py")` to set the script argument.
3. Calls `.envs(env)` to apply all environment variables from the map.
4. Calls `.stdout(Stdio::piped())` and `.stderr(Stdio::piped())` to pipe both streams.
5. Returns the configured `Command`.

**`pub async fn spawn_worker(venv_path: &Path, env: HashMap<String, String>) -> Result<tokio::process::Child, AnvilError>`**

This is the public spawn function. It:
1. Calls `build_command(venv_path, env)` to get the configured `Command`.
2. Calls `.spawn()` on the `Command`, which returns `Result<tokio::process::Child, std::io::Error>`.
3. Maps the `std::io::Error` to `AnvilError::Io` via `thiserror::Error`'s automatic `From` impl.
4. Returns `Result<tokio::process::Child, AnvilError>`.

Logging: Apply `#[tracing::instrument]` to `spawn_worker` with span name `spawn_worker` (lowercase snake_case per FORGE_AGENT_RULES §11.6). Add a DEBUG log at the entry point with `venv_path` field.

### Step 3: Update `lib.rs`

Add `mod spawn;` and `pub use spawn::spawn_worker;` to `crates/anvilml-worker/src/lib.rs`. Keep the existing `mod env; pub use env::WorkerEnv;` line. The file will be ~6 lines, well within the 80-line hard cap.

### Step 4: Create `crates/anvilml-worker/tests/spawn_tests.rs`

Write ≥4 integration tests that call `build_command()` (not `spawn_worker()`, since spawning would require `worker_main.py` to exist) and verify the Command's configuration:

1. **`test_interpreter_path_unix`** (cfg-gated for unix): Verifies the interpreter path is `{venv_path}/bin/python3` on Unix platforms. Uses `cfg(unix)` attribute to only run on Unix.

2. **`test_interpreter_path_windows`** (cfg-gated for windows): Verifies the interpreter path is `{venv_path}\Scripts\python.exe` on Windows platforms. Uses `cfg(windows)` attribute.

3. **`test_worker_script_arg`**: Verifies the command has exactly one argument: `worker/worker_main.py`.

4. **`test_env_vars_applied`**: Verifies that all env vars from the `HashMap` are present on the Command by checking `Command::get_envs()` (available via `std::process::Command` API).

5. **`test_stdio_piped`**: Verifies that `stdout` and `stderr` are both set to `Stdio::piped()` by checking `Command::get_stdout()` and `Command::get_stderr()`.

Each test has a doc comment describing what it verifies, following the project's test documentation obligation (ENVIRONMENT.md §11.4).

### Step 5: Bump crate version

Increment `anvilml-worker`'s patch version from `0.1.0` to `0.1.1` in `crates/anvilml-worker/Cargo.toml`, per the crate version bump convention (ENVIRONMENT.md §12).

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `build_command` | `anvilml-worker/src/spawn.rs` | `pub fn build_command(venv_path: &Path, env: HashMap<String, String>) -> tokio::process::Command` |
| `spawn_worker` | `anvilml-worker/src/spawn.rs` | `pub async fn spawn_worker(venv_path: &Path, env: HashMap<String, String>) -> Result<tokio::process::Child, AnvilError>` |

Re-exported in `lib.rs`: `pub use spawn::spawn_worker;`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | crates/anvilml-worker/Cargo.toml | Add tokio dependency with `process` feature; bump patch version 0.1.0 → 0.1.1 |
| CREATE | crates/anvilml-worker/src/spawn.rs | `build_command()` and `spawn_worker()` implementations |
| MODIFY | crates/anvilml-worker/src/lib.rs | Add `mod spawn; pub use spawn::spawn_worker;` |
| CREATE | crates/anvilml-worker/tests/spawn_tests.rs | ≥4 integration tests for spawn.rs |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| tests/spawn_tests.rs | test_interpreter_path_unix (cfg: unix) | The interpreter path on Unix platforms is `{venv_path}/bin/python3` | None | venv_path = `/tmp/test_venv` | Command args[0] ends with `bin/python3` | `cargo test -p anvilml-worker --test spawn_tests test_interpreter_path_unix` exits 0 |
| tests/spawn_tests.rs | test_interpreter_path_windows (cfg: windows) | The interpreter path on Windows is `{venv_path}\Scripts\python.exe` | None | venv_path = `C:\test_venv` | Command args[0] ends with `Scripts\python.exe` | `cargo test -p anvilml-worker --test spawn_tests test_interpreter_path_windows` exits 0 |
| tests/spawn_tests.rs | test_worker_script_arg | The command has exactly one argument: `worker/worker_main.py` | None | Any venv_path, any env map | Command args = `["worker/worker_main.py"]` | `cargo test -p anvilml-worker --test spawn_tests test_worker_script_arg` exits 0 |
| tests/spawn_tests.rs | test_env_vars_applied | All env vars from the HashMap are present on the Command | None | Full env map from WorkerEnv::build() | All keys in env map appear in Command::get_envs() | `cargo test -p anvilml-worker --test spawn_tests test_env_vars_applied` exits 0 |
| tests/spawn_tests.rs | test_stdio_piped | stdout and stderr are both Stdio::piped() | None | Any inputs | Command::get_stdout() and get_stderr() return Some(Stdio) with piped config | `cargo test -p anvilml-worker --test spawn_tests test_stdio_piped` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/spawn_tests.rs` is automatically picked up by `cargo test -p anvilml-worker` (the existing CI step in ENVIRONMENT.md §6 Step 6). The tokio dependency addition is within the workspace dependency graph and does not change any CI job structure.

## Platform Considerations

This task introduces `#[cfg(unix)]` / `#[cfg(windows)]` conditional compilation for the interpreter path selection:
- `#[cfg(unix)]`: uses `{venv_path}/bin/python3` (covers both Linux and macOS)
- `#[cfg(windows)]`: uses `{venv_path}\Scripts\python.exe`

The test `test_interpreter_path_unix` is gated with `#[cfg(unix)]` and `test_interpreter_path_windows` with `#[cfg(windows)]`, so only the relevant test runs on each platform.

The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) will exercise the `#[cfg(windows)]` path during compilation even on Linux, confirming the code compiles for the Windows target.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Command::get_envs()`, `Command::get_stdout()`, `Command::get_stderr()` API may not exist or may have different signatures in tokio 1.52.3's `tokio::process::Command` (which is a re-export of `std::process::Command`). If so, tests cannot inspect the Command's internal configuration without spawning it. | Medium | High | Verify the API shape via rust-docs MCP before writing the plan. If the methods don't exist, fall back to testing `spawn_worker()` with a mock subprocess that exits immediately, or test via a helper that returns the Command and use `std::process::Command` methods (which `tokio::process::Command` wraps). |
| The `process` feature of tokio pulls in `libc` and `mio` which may cause compilation issues on the Windows cross-check target (`x86_64-pc-windows-gnu`). | Low | Medium | The `process` feature is confirmed to support Windows via rust-docs MCP. The Windows cross-check (ENVIRONMENT.md §7 check 2) will surface any issues. |
| `worker/worker_main.py` does not exist yet (Phase 9), so any test that calls `spawn_worker()` directly will fail with a "file not found" error. | High | Low | Tests use `build_command()` which constructs the Command without spawning. `spawn_worker()` is tested indirectly by verifying `build_command()` produces a valid Command. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-worker --test spawn_tests` exits 0
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` prints a number ≤ 80
- [ ] `grep -c '^## ' .forge/reports/P8-B2_plan.md` prints 12 (all 12 mandatory sections present)
- [ ] `head -1 .forge/reports/P8-B2_plan.md` prints `# Plan Report: P8-B2`
