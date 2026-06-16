# Plan Report: P9-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A2                                         |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: spawn.rs subprocess Command construction |
| Depends on  | P9-A1 (env.rs)                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T15:05:00Z                        |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-worker/src/spawn.rs` with `pub fn build_command(cfg: &ServerConfig, device: &GpuDevice, port: u16) -> tokio::process::Command` that constructs a `tokio::process::Command` to launch the Python worker subprocess. The command uses the venv Python interpreter path (platform-specific), sets `worker/worker_main.py` as the script argument, injects environment variables from `build_worker_env()`, pipes stdout/stderr for log capture, and on Unix uses `PR_SET_PDEATHSIG` via `unsafe` in a `pre_exec` closure. Tests verify the command's program path, arguments, and environment keys are correct. After this task, `cargo test -p anvilml-worker --features mock-hardware -- spawn` exits 0.

## Scope

### In Scope
- **CREATE** `crates/anvilml-worker/src/spawn.rs`: `build_command()` function that constructs a `tokio::process::Command` with platform-specific Python interpreter path, correct arguments, env injection via `build_worker_env()`, piped stdio, and Unix `PR_SET_PDEATHSIG`.
- **MODIFY** `crates/anvilml-worker/src/lib.rs`: add `pub mod spawn;` and `pub use spawn::build_command;` re-exports.
- **CREATE** `crates/anvilml-worker/tests/spawn_tests.rs`: unit tests verifying command path, args, env vars, and platform-specific behavior.
- **MODIFY** `crates/anvilml-worker/Cargo.toml`: add `libc` dependency (for `prctl` on Unix).
- **BUMP** `crates/anvilml-worker` patch version from `0.1.1` to `0.1.2`.

### Out of Scope
- Creating `worker/worker_main.py` — that is P9-B1.
- Implementing `ManagedWorker` — that is P9-A5.
- Implementing `WorkerPool` — that is P9-A6.
- Implementing the IPC bridge — that is P9-A3.
- Implementing keepalive — that is P9-A4.
- Implementing respawn policy — that is P9-A6 (respawn.rs).

## Existing Codebase Assessment

The `anvilml-worker` crate currently has two source files: `lib.rs` (11 lines, re-exports only) and `env.rs` (85 lines, implements `build_worker_env()`). The `env.rs` module already constructs the environment variable map that this task will inject into the subprocess. The `GpuDevice` and `ServerConfig` types used by both `env.rs` and the new `spawn.rs` are defined in `anvilml-core` and imported via the `anvilml_core` crate dependency.

Established patterns:
- **Error handling**: `env.rs` returns `HashMap<String, String>` directly (no `Result`), using `HashMap::insert()` which never fails. `spawn.rs` will follow the same pattern — `Command::new()` and `Command::envs()` never return `Err`, so no `Result` wrapping is needed.
- **Test style**: Tests live in `crates/anvilml-worker/tests/` as separate test crate files (e.g., `env_tests.rs`). Each test has a doc comment describing what it verifies, preconditions, inputs, and expected output. Tests construct fixtures inline (`make_device()` helper).
- **Feature-gated code**: `env.rs` uses `#[cfg(feature = "mock-hardware")]` for the `ANVILML_WORKER_MOCK` env var. `spawn.rs` will use `#[cfg(unix)]` and `#[cfg(windows)]` for platform-specific behavior.
- **Doc comments**: Every `pub fn` has a `///` doc comment describing purpose, arguments, and return value.

No discrepancies found between the design doc and current source that would affect this task's approach. The `ServerConfig` and `GpuDevice` types match the design doc specifications exactly.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | libc       | 0.2.186         | Cargo.lock     | n/a                    |
| crate  | tokio      | 1.52.3          | Workspace dep  | `full` (includes `process`) |

Notes:
- `libc` (0.2.186) is already a transitive dependency in `Cargo.lock`. This task adds it as a direct dependency of `anvilml-worker` for the `prctl` / `PR_SET_PDEATHSIG` symbols. The `prctl` function and `PR_SET_PDEATHSIG` constant are available in `libc` on all Unix targets.
- `tokio::process::Command` is part of the `process` feature, which is included in tokio's `full` feature set declared in the workspace dependencies.

## Approach

1. **Add `libc` dependency to `Cargo.toml`.** Append `libc = "0.2"` under `[dependencies]` in `crates/anvilml-worker/Cargo.toml`. This provides `libc::prctl()` and `libc::PR_SET_PDEATHSIG` for the Unix orphan cleanup path.

2. **Create `crates/anvilml-worker/src/spawn.rs`.** Implement the `build_command()` function with the following structure:

   a. **Determine Python interpreter path.** Use `cfg!(target_os = "windows")` to branch:
      - Unix: `{cfg.venv_path}/bin/python3` — the standard venv interpreter path per `ENVIRONMENT.md §5`.
      - Windows: `{cfg.venv_path}/Scripts/python.exe` — the Windows venv interpreter path per `ENVIRONMENT.md §5`.
      Use `PathBuf::join()` to construct the path from `cfg.venv_path`, then convert to `OsString` for `Command::arg()`.

   b. **Construct the Command.** Call `tokio::process::Command::new()` with the interpreter path. Set the script argument to `"worker/worker_main.py"` (relative to the working directory, which is the repository root at server startup).

   c. **Inject environment variables.** Call `Command::envs()` with the result of `build_worker_env(device, cfg, port)` from the `env` module. This injects all 6-7 `ANVILML_*` variables.

   d. **Pipe stdout/stderr.** Call `Command::stdout(Stdio::piped())` and `Command::stderr(Stdio::piped())` for log capture by the supervisor.

   e. **Unix orphan cleanup (`#[cfg(unix)]`).** Use `Command::pre_exec()` with an `unsafe` closure that calls `libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM)`. The `unsafe` block is justified because `prctl` with `PR_SET_PDEATHSIG` is a well-documented Linux syscall that sets the parent death signal — if the parent (Rust supervisor) dies, the child (Python worker) receives `SIGTERM`. This is the standard Linux orphan cleanup mechanism. The call is infallible from the caller's perspective (it returns `0` on success, non-zero on error — but we ignore the return value; if `prctl` fails, the process simply won't have orphan cleanup, which is acceptable).

   f. **Windows job object.** No explicit Windows job object setup in this task — that is deferred to a future task where `ManagedWorker` owns the job object handle. The Windows path simply constructs the Command with interpreter, args, envs, and piped stdio.

   g. **Return the Command.** The function returns the constructed `Command` by value.

3. **Update `lib.rs`.** Add `pub mod spawn;` after the existing `pub mod env;` line, and add `pub use spawn::build_command;` after the existing `pub use env::build_worker_env;` line. Keep the file under 80 lines.

4. **Create `crates/anvilml-worker/tests/spawn_tests.rs`.** Write tests following the established pattern from `env_tests.rs`:

   a. **`test_python_path_unix`**: Verify the interpreter path ends with `bin/python3` on Unix builds (use `#[cfg(unix)]`). Construct a default config with `venv_path = PathBuf::from("/test/venv")`, call `build_command()`, assert `.get_program()` returns `python3` and the first arg contains the full path.

   b. **`test_python_path_windows`**: Verify the interpreter path ends with `Scripts/python.exe` on Windows builds (use `#[cfg(windows)]`). Same fixture pattern.

   c. **`test_script_arg`**: Verify the second argument is `worker/worker_main.py`. Call `build_command()` and check `.get_args()` contains the expected script path.

   d. **`test_env_injection`**: Verify that `ANVILML_IPC_PORT` and `ANVILML_DEVICE_INDEX` appear in the command's environment. Call `build_command()`, access `.get_envs()` via a helper or assert on the env map directly by calling `build_worker_env` with the same inputs and comparing keys.

   e. **`test_stdin_not_piped`**: Verify stdin is not piped (default `inherit`). Check `.stdio` configuration.

   f. **`test_stdout_piped`**: Verify stdout is piped.

   g. **`test_stderr_piped`**: Verify stderr is piped.

5. **Bump patch version.** Change `version = "0.1.1"` to `version = "0.1.2"` in `crates/anvilml-worker/Cargo.toml`.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `build_command` | `anvilml_worker::spawn` | `pub fn build_command(cfg: &ServerConfig, device: &GpuDevice, port: u16) -> tokio::process::Command` |

Full function signature with doc comment:

```rust
/// Build a `tokio::process::Command` to launch the Python worker subprocess.
///
/// The command uses the venv Python interpreter (platform-specific path),
/// passes `worker/worker_main.py` as the script argument, injects all
/// `ANVILML_*` environment variables via `build_worker_env()`, and pipes
/// stdout/stderr for log capture.
///
/// On Unix, sets `PR_SET_PDEATHSIG` so the worker is killed if the parent
/// supervisor dies.
///
/// # Arguments
///
/// * `cfg` — The server configuration (provides venv path and IPC payload cap).
/// * `device` — The GPU device this worker will operate on.
/// * `port` — The TCP port the worker should connect to for IPC.
///
/// # Returns
///
/// A `tokio::process::Command` ready to be spawned.
pub fn build_command(
    cfg: &ServerConfig,
    device: &GpuDevice,
    port: u16,
) -> tokio::process::Command;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/spawn.rs` | Subprocess Command construction + env injection |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `pub mod spawn;` and `pub use spawn::build_command;` |
| CREATE | `crates/anvilml-worker/tests/spawn_tests.rs` | Unit tests for build_command |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Add `libc` dependency; bump version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_python_path_unix` | Interpreter path is `{venv}/bin/python3` on Unix | `#[cfg(unix)]` | `venv_path = /test/venv` | `.get_program()` == `python3`, path contains `/test/venv/bin/python3` | `cargo test -p anvilml-worker --features mock-hardware -- spawn` exits 0 |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_python_path_windows` | Interpreter path is `{venv}\Scripts\python.exe` on Windows | `#[cfg(windows)]` | `venv_path = \test\venv` | `.get_program()` == `python.exe`, path contains `\test\venv\Scripts\python.exe` | Same command exits 0 |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_script_arg` | Script argument is `worker/worker_main.py` | None | Any config, any device, any port | `.get_args()` contains `worker/worker_main.py` | Same command exits 0 |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_env_injection` | Environment variables from `build_worker_env` are present in the command | None | `port = 9000`, `device.index = 0` | `ANVILML_IPC_PORT` and `ANVILML_DEVICE_INDEX` are set in command env | Same command exits 0 |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_stdin_not_piped` | Stdin inherits from parent (not piped) | None | Any config | `.stdin` is not `Stdio::piped()` | Same command exits 0 |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_stdout_piped` | Stdout is piped for log capture | None | Any config | `.stdout` is `Stdio::piped()` | Same command exits 0 |
| `crates/anvilml-worker/tests/spawn_tests.rs` | `test_stderr_piped` | Stderr is piped for log capture | None | Any config | `.stderr` is `Stdio::piped()` | Same command exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-worker/tests/spawn_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` which runs on every CI job (rust-linux, rust-windows). The `libc` dependency is a direct system library present on all target platforms (Linux, Windows via mingw, macOS). No new CI jobs or gates are needed.

## Platform Considerations

- **`#[cfg(unix)]` guard** on the `pre_exec` block that calls `libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM)`. This is a Linux-only syscall (not available on macOS or FreeBSD). The `pre_exec` API itself is available on Unix but `prctl` with `PR_SET_PDEATHSIG` is Linux-specific. For this task, we guard the entire `prctl` call with `#[cfg(target_os = "linux")]` to be precise — though `#[cfg(unix)]` is also acceptable since the project's primary Unix target is Linux and the `pre_exec` closure on non-Linux Unix would simply not compile if `prctl` were called.

  Actually, `pre_exec` is `#[cfg(unix)]` in tokio. On Linux, `libc::prctl` and `libc::PR_SET_PDEATHSIG` are available. On macOS, `prctl` exists but `PR_SET_PDEATHSIG` is not defined. So the guard should be `#[cfg(target_os = "linux")]` for the `prctl` call specifically.

- **`#[cfg(windows)]`** is implicit — the Windows path is the else branch of the interpreter path selection. No explicit `#[cfg(windows)]` guard is needed because the Unix branch is already `#[cfg(target_os = "linux")]`.

- **Path construction**: `PathBuf::join()` handles platform-specific separators automatically. The venv path for Unix is `{venv_path}/bin/python3` and for Windows is `{venv_path}/Scripts/python.exe`. Both are constructed via `PathBuf::join()` strings which produce correct OS-native paths.

- **`worker/worker_main.py`**: The script path uses forward slashes (`/`) which works on both Unix and Windows in the Rust `Command` API — Windows accepts forward slashes in paths. No platform-specific script path construction is needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM)` may not compile on non-Linux Unix targets (macOS has `prctl` but not `PR_SET_PDEATHSIG`). | Medium | High — compilation failure on macOS cross-check. | Use `#[cfg(target_os = "linux")]` guard around the `prctl` call, not `#[cfg(unix)]`. The `pre_exec` closure itself is `#[cfg(unix)]` in tokio, so the closure body only runs on Unix, and the `prctl` call inside is Linux-only. |
| `Command::get_program()` / `Command::get_args()` / `Command::get_envs()` may not expose internal state in the version of tokio used (1.52.3), making it impossible to test the constructed Command's internals without actually spawning it. | Low | Medium — tests would need to spawn a real subprocess to verify, which is slow and fragile. | If these getters are unavailable, test by actually spawning the command with `Command::spawn()` and checking the process starts (or fails with expected error), then kill it. Alternatively, test `build_worker_env()` keys separately (already done in env_tests.rs) and test the Command construction indirectly by asserting the venv path exists and the script arg is correct string literals. |
| `pre_exec` closure must be `unsafe` and the compiler may require explicit `unsafe` block. | Low | Low — straightforward to handle. | Wrap the `libc::prctl` call in `unsafe { libc::prctl(...) }` inside the `pre_exec` closure. Document the safety justification inline. |
| The `worker/worker_main.py` file does not exist yet (created by P9-B1). Tests that spawn the actual subprocess will fail. | High | Low — tests will only verify Command construction, not actual execution. The acceptance criterion only requires `cargo test ... -- spawn` to exit 0, which can be satisfied by testing Command construction without spawning. | Do not spawn the subprocess in tests. Only verify the Command's program path, arguments, env vars, and stdio configuration using the Command's inspection methods or by constructing a Command and verifying its fields. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- spawn` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (WSL2 Windows cross-check)
- [ ] `cargo fmt --all -- --check` exits 0 (format gate pass 2)
