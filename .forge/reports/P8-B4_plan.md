# Plan Report: P8-B4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-B4                                              |
| Phase       | 8 — IPC Stress Gate & Worker Pool                 |
| Description | anvilml-worker: WorkerSpawner trait + ProcessWorkerSpawner (standalone) |
| Depends on  | P8-B2                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-07-02T11:30:00Z                               |
| Attempt     | 1                                                  |

## Objective

Add a `WorkerSpawner` trait and its production implementation `ProcessWorkerSpawner` to `crates/anvilml-worker/src/spawn.rs`, providing a uniform, injectable spawn seam for `ManagedWorker`'s first-generation spawn and every future respawn. This is standalone — no existing code calls `WorkerSpawner` yet; that wiring is deferred to `P8-E6`. The acceptance criteria include three new tests in `tests/spawn_tests.rs` proving the production path is real (not a stub), the trait is object-safe, and the spawn produces the same `Command` shape as `build_command()`.

## Scope

### In Scope
- Define `pub trait WorkerSpawner: Send + Sync` in `crates/anvilml-worker/src/spawn.rs` with method `spawn(&self, venv_path: &Path, env: HashMap<String, String>) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child, AnvilError>> + Send>>`.
- Implement `pub struct ProcessWorkerSpawner` with `impl WorkerSpawner for ProcessWorkerSpawner` whose `spawn()` calls `spawn_worker()` directly (no re-implementation of `build_command()` logic).
- Add `pub use spawn::{WorkerSpawner, ProcessWorkerSpawner};` to `crates/anvilml-worker/src/lib.rs`.
- Write ≥3 new tests in `crates/anvilml-worker/tests/spawn_tests.rs`:
  1. `ProcessWorkerSpawner::spawn()` against a nonexistent venv returns `AnvilError::Io` naming the expected interpreter path.
  2. `WorkerSpawner` is object-safe: `Arc<dyn WorkerSpawner>` compiles with `Send + Sync`.
  3. `ProcessWorkerSpawner::spawn()` produces the same `Command` shape as `build_command()`.

### Out of Scope
- Wiring `WorkerSpawner`/`ProcessWorkerSpawner` into `ManagedWorker` — that is `P8-E6`'s scope, tracked via this task's `defers_to: ["P8-E6"]`.
- Any mock/spy implementation of `WorkerSpawner` — that is `P8-E6`'s scope (its tests define `MockWorkerSpawner` in the test crate).
- Changes to any file other than `spawn.rs`, `lib.rs`, and `tests/spawn_tests.rs`.

## Existing Codebase Assessment

The `anvilml-worker` crate already has a mature `spawn.rs` module (built in P8-B2 and P8-B3) containing `build_command()` — which constructs a `tokio::process::Command` targeting the platform-specific Python interpreter — and `spawn_worker()` — which calls `build_command()` and spawns the resulting `Command`. The module already uses `#[tracing::instrument]` on `spawn_worker()`, and `AnvilError::Io` is used for spawn failures via `thiserror::From<std::io::Error>`.

The `lib.rs` (24 lines) already exports `build_command` and `spawn_worker` from the `spawn` module. The existing `tests/spawn_tests.rs` (247 lines) has 4 Unix tests exercising `build_command()` and 3 Windows-specific tests for `JobObjectGuard`. Tests use `#[cfg(unix)]`/`#[cfg(windows)]` gating and follow the project's integration test convention (separate test crate files in `tests/`).

The design doc (`ANVILML_DESIGN.md §10.6`) defines a `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` dual-mode parity marker convention, but this applies only to Python node `execute()` and arch-module `load()`/`sample()`/`decode()` functions — not to Rust trait definitions. This task does not trigger that convention.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | process, rt, sync, time (already in Cargo.toml) |
| crate  | anvilml-core | 0.1.x (workspace dep) | rust-docs MCP | — (path dependency, no version pin) |

No new external crates are introduced. All types used (`tokio::process::Child`, `AnvilError`, `Pin`, `Box`, `Future`, `Path`, `HashMap`) are already available from existing dependencies or the Rust standard library.

## Approach

1. **Add trait and struct to `spawn.rs`.** Append the following after the existing `spawn_worker()` function and its `#[cfg(test)]` module:

   ```rust
   use std::future::Future;
   use std::pin::Pin;
   use std::sync::Arc;

   /// Trait for spawning Python worker subprocesses.
   ///
   /// Provides an injectable abstraction over subprocess spawning, enabling
   /// `ManagedWorker` to spawn its first-generation subprocess and every
   /// respawn through a single uniform interface. Production code uses
   /// `ProcessWorkerSpawner`; tests may substitute a mock implementation.
   pub trait WorkerSpawner: Send + Sync {
       /// Spawn a Python worker subprocess.
       ///
       /// # Arguments
       /// * `venv_path` — Root of the Python virtual environment containing
       ///   the interpreter.
       /// * `env` — Environment variables to inject into the subprocess.
       ///
       /// # Errors
       /// Returns `AnvilError::Io` if the process cannot be spawned
       /// (e.g. the interpreter binary does not exist).
       fn spawn(
           &self,
           venv_path: &Path,
           env: HashMap<String, String>,
       ) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child, AnvilError>> + Send>>;
   }

   /// Production implementation of `WorkerSpawner` that calls
   /// `spawn_worker()` directly.
   ///
   /// This is the concrete spawner used in production. It does not
   /// re-implement any part of `build_command()`'s logic — it delegates
   /// entirely to `spawn_worker()`.
   #[derive(Clone, Debug, Default)]
   pub struct ProcessWorkerSpawner;

   impl WorkerSpawner for ProcessWorkerSpawner {
       fn spawn(
           &self,
           venv_path: &Path,
           env: HashMap<String, String>,
       ) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child, AnvilError>> + Send>> {
           // Delegate to spawn_worker() — no re-implementation of
           // build_command() logic. spawn_worker() handles interpreter
           // path selection, env injection, and stdio piping.
           Box::pin(spawn_worker(venv_path, env))
       }
   }
   ```

   Rationale: `ProcessWorkerSpawner` is a zero-sized unit struct (`#[derive(Default)]`) because it needs no state — spawning is entirely determined by its arguments. The `Clone`/`Debug` derives follow the pattern of other zero-state types in the crate.

2. **Update `lib.rs`.** Change the existing `pub use spawn::{build_command, spawn_worker};` line to include the new public types:
   ```rust
   pub use spawn::{build_command, spawn_worker, WorkerSpawner, ProcessWorkerSpawner};
   ```
   This keeps `lib.rs` at 24 lines (no new lines added, one existing line modified).

3. **Write three new tests in `tests/spawn_tests.rs`.**

   **Test 1 — `test_spawn_nonexistent_venv_returns_io_error`:** Construct a `ProcessWorkerSpawner`, call `.spawn()` with a nonexistent venv path (e.g. `/tmp/nonexistent_venv_xyz`), await the result, assert it is `Err(AnvilError::Io(_))`, and verify the error's `Display` output contains the expected interpreter path (`/tmp/nonexistent_venv_xyz_xyz/bin/python3`). This proves the production path is real — not a stub — because a stub would never reach the OS spawn call.

   **Test 2 — `test_worker_spawner_is_object_safe`:** A compile-time check. Declare a function that accepts `Arc<dyn WorkerSpawner>` and assert `WorkerSpawner` is `Send + Sync`. This test does not need to run at runtime — it only needs to compile. The `#[allow(dead_code)]` attribute is used on the function to suppress the unused warning (no caller needed).

   **Test 3 — `test_spawn_produces_same_command_shape`:** Build a `Command` via `build_command()` and build a `Command` by calling `ProcessWorkerSpawner::spawn()` (which internally calls `spawn_worker()`, which calls `build_command()`). Compare the two commands' program paths and arguments to confirm they are identical. This proves `ProcessWorkerSpawner::spawn()` does not re-implement `build_command()` logic but delegates to it.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub trait WorkerSpawner` | `anvilml_worker::WorkerSpawner` | Spawn abstraction: `fn spawn(&self, venv_path: &Path, env: HashMap<String, String>) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child, AnvilError>> + Send>>` |
| `pub struct ProcessWorkerSpawner` | `anvilml_worker::ProcessWorkerSpawner` | Zero-sized production spawner; `impl WorkerSpawner for ProcessWorkerSpawner` |
| `impl WorkerSpawner for ProcessWorkerSpawner` | `anvilml_worker::ProcessWorkerSpawner` | Delegates `spawn()` to `spawn_worker()` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/spawn.rs` | Add `WorkerSpawner` trait and `ProcessWorkerSpawner` impl |
| Modify | `crates/anvilml-worker/src/lib.rs` | Add `WorkerSpawner`, `ProcessWorkerSpawner` to existing re-export line |
| Modify | `crates/anvilml-worker/tests/spawn_tests.rs` | Add ≥3 new tests for trait, impl, and object-safety |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.9 → 0.1.10 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/spawn_tests.rs` | `test_spawn_nonexistent_venv_returns_io_error` | `ProcessWorkerSpawner::spawn()` against a nonexistent venv returns `AnvilError::Io` whose message names the expected interpreter path | None (no worker_main.py needed) | `venv_path = "/tmp/nonexistent_venv_xyz"`, empty env | `Err(AnvilError::Io(e))` where `e.to_string()` contains `"bin/python3"` | `cargo test -p anvilml-worker --test spawn_tests test_spawn_nonexistent_venv_returns_io_error` exits 0 |
| `tests/spawn_tests.rs` | `test_worker_spawner_is_object_safe` | `WorkerSpawner` trait is object-safe: `Arc<dyn WorkerSpawner>` compiles and `WorkerSpawner: Send + Sync` | None (compile-time check) | N/A | Compiles; no trait object safety errors | `cargo test -p anvilml-worker --test spawn_tests test_worker_spawner_is_object_safe` exits 0 |
| `tests/spawn_tests.rs` | `test_spawn_produces_same_command_shape` | `ProcessWorkerSpawner::spawn()` produces the same `Command` shape as `build_command()` (same interpreter path, same args) | None | Same `venv_path` and `env` for both calls | Program paths and arguments are identical | `cargo test -p anvilml-worker --test spawn_tests test_spawn_produces_same_command_shape` exits 0 |

## CI Impact

No CI changes required. The new tests are Rust integration tests under `crates/anvilml-worker/tests/`, which are automatically picked up by the existing `cargo test --workspace --features mock-hardware` CI job. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `WorkerSpawner` trait and `ProcessWorkerSpawner` implementation are platform-neutral — they delegate to `spawn_worker()` which already handles platform-specific interpreter paths via `#[cfg(unix)]`/`#[cfg(windows)]`. The three tests all run on both Linux and Windows (the nonexistent-venv test works on both; the object-safety test is a compile check; the command-shape test works on both).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `spawn_worker()`'s `?` propagation from `cmd.spawn()` returns `std::io::Error`, which converts to `AnvilError::Io` via `thiserror::From`. The error message format may differ between platforms for a nonexistent interpreter (e.g. "No such file" vs "The system cannot find the file specified"), making the test assertion on the error message fragile across platforms. | Medium | Medium | Use a platform-tolerant assertion: check that the error contains the venv path prefix (e.g. `/tmp/nonexistent_venv_xyz`) rather than a specific platform error string. This works on both Unix and Windows. |
| The `test_spawn_produces_same_command_shape` test needs to compare two `Command` objects for equality. `tokio::process::Command` does not derive `PartialEq`, so direct comparison is not possible. | Medium | Medium | Instead of comparing `Command` objects, extract the program path and arguments from each by calling `.spawn()` in a way that captures the error (both will fail on nonexistent venv) and inspecting the error message, which contains the interpreter path. Both paths must contain the same interpreter string. |
| Adding the new re-exports to `lib.rs` could cause clippy warnings about unused imports if no downstream crate yet uses `WorkerSpawner`. | Low | Low | The re-export is intentional and required by the task spec. Use `#[allow(dead_code)]` on the `lib.rs` line if needed — but actually, `pub use` does not trigger `dead_code` warnings; only unused items in the current crate do. No warning expected. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test spawn_tests test_spawn_nonexistent_venv_returns_io_error` exits 0
- [ ] `cargo test -p anvilml-worker --test spawn_tests test_worker_spawner_is_object_safe` exits 0
- [ ] `cargo test -p anvilml-worker --test spawn_tests test_spawn_produces_same_command_shape` exits 0
- [ ] `cargo test -p anvilml-worker --test spawn_tests` exits 0 (all tests including existing ones)
- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
