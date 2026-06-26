# Plan Report: P1-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P1-A3                                             |
| Phase       | 001 — Repository Scaffold                         |
| Description | backend: shutdown.rs signal handler stub           |
| Depends on  | P1-A2                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-26T14:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `backend/src/shutdown.rs` containing `pub async fn wait_for_shutdown_signal()` that awaits `tokio::signal::ctrl_c()`, providing a cross-platform shutdown-signal future the binary can race against. Convert `backend/src/main.rs` from a synchronous `fn main()` to `#[tokio::main] async fn main()`, wiring the shutdown handler after the scaffold print. Add `tokio` with `features = ["full"]` to `backend/Cargo.toml`. Acceptance: `cargo build -p anvilml` exits 0 and `cargo test -p anvilml --test shutdown_tests` passes with at least one test.

## Scope

### In Scope
- Create `backend/src/shutdown.rs` with `pub async fn wait_for_shutdown_signal()` that awaits `tokio::signal::ctrl_c()`.
- Convert `backend/src/main.rs` to `#[tokio::main] async fn main()`, importing and calling `shutdown::wait_for_shutdown_signal()` after the scaffold print.
- Add `tokio = { version = "...", features = ["full"] }` to `backend/Cargo.toml`.
- Create `backend/tests/shutdown_tests.rs` with ≥1 test asserting the function returns when Ctrl+C fires, using `tokio::select!` with a short timeout to prevent CI hangs.

### Out of Scope
- SIGTERM handling on Unix (full signal sequence is a later phase — `ANVILML_DESIGN.md §19.3`).
- 30-second worker-drain sequence on shutdown (requires `WorkerPool` from a later phase).
- Graceful HTTP server shutdown (wired in P1-D1 via `tokio::select!` between server and shutdown signal).
- Any clap/CLI changes — P1-A2 handles CLI parsing.

defers_to (from JSON): []

## Existing Codebase Assessment

No prior source exists for shutdown handling. `backend/src/main.rs` currently uses a synchronous `fn main()` that parses CLI args via `cli::parse()`, prints `"AnvilML scaffold"`, and returns. The TODO comment on line 12 (`// TODO: Wire HTTP server and shutdown signal handler (P1-A3, P1-D1)`) explicitly marks P1-A3 as the next step.

`backend/src/cli.rs` is a well-documented clap-derive module with `pub fn parse() -> Cli`. It is unaffected by this task but remains a sibling module.

`backend/tests/cli_help_test.rs` demonstrates the project's test style: an integration test crate under `backend/tests/`, using `std::process::Command` to spawn the built binary, with inline comments explaining assertions. This pattern will be followed for the shutdown tests.

The established patterns to follow:
- `///` doc comments on public items (cli.rs and main.rs both use them).
- Integration tests in `backend/tests/` as separate test crates (not `#[cfg(test)]` inline blocks).
- Inline `//` comments at decision points.
- No `#[allow(dead_code)]` or similar without explanation.

## Resolved Dependencies

| Type   | Name  | Version verified | MCP source     | Feature flags confirmed |
|--------|-------|-----------------|----------------|------------------------|
| crate  | tokio | 1.47.0          | crates.io fallback (MCP unavailable) | full |

**Note:** MCP tool (`rust-docs`) was unavailable during planning. Version 1.47.0 was selected based on crates.io registry knowledge as a recent stable release compatible with Rust 1.96.0 / edition 2024. The ACT agent MUST verify this version via `rust-docs` MCP at session start and override if a newer version is available. The `full` feature flag is the standard tokio feature that enables all tokio sub-crates including `signal` (for `ctrl_c()`) and `macros` (for `#[tokio::main]`).

## Approach

1. **Add tokio dependency to `backend/Cargo.toml`.** Append a new `[dependencies]` line for tokio:
   ```toml
   tokio = { version = "1.47.0", features = ["full"] }
   ```
   Keep the existing `clap` dependency as-is. The `mock-hardware` feature and `[features]` section remain unchanged.

2. **Create `backend/src/shutdown.rs`.** Write a single public async function:
   ```rust
   /// Await a cross-platform shutdown signal (Ctrl+C).
   ///
   /// On Unix this receives SIGINT; on Windows this receives Ctrl+C.
   /// tokio normalises both signals into a single awaitable future.
   ///
   /// Full graceful shutdown (SIGTERM handling, worker drain sequence)
   /// is implemented in a later phase (`ANVILML_DESIGN.md §19.3`).
   pub async fn wait_for_shutdown_signal() {
       // Await Ctrl+C — tokio::signal::ctrl_c() returns () on success
       // or Err on signal-handler setup failure (extremely rare).
       // Discard the result at this stage; error handling is a later phase.
       let _ = tokio::signal::ctrl_c().await;
   }
   ```
   This is a minimal stub: it awaits exactly one signal and returns. No cfg branches are needed because `tokio::signal::ctrl_c()` handles cross-platform signal registration internally.

3. **Convert `backend/src/main.rs` to async.** Replace the synchronous `fn main()` with `#[tokio::main] async fn main()`. The changes:
   - Add `mod shutdown;` alongside the existing `mod cli;`.
   - Change `fn main()` to `#[tokio::main] async fn main()`.
   - After the scaffold `println!`, call `shutdown::wait_for_shutdown_signal().await;` and remove the `let _ = cli;` dead-code suppression (cli is now used).
   - Keep the existing doc comment on main but update it to reflect async status.

4. **Create `backend/tests/shutdown_tests.rs`.** Write an integration test that:
   - Uses `tokio::select!` with a short timeout to race `shutdown::wait_for_shutdown_signal()` against a simulated Ctrl+C signal.
   - The test sends a SIGINT to the current process via `tokio::signal::unix::SignalKind::interrupt()` (Unix-only test, guarded by `#[cfg(unix)]`) or uses a cross-platform approach.
   - Actually, since the test must run in CI on both platforms and `tokio::signal::ctrl_c()` itself handles cross-platform signals, the correct test approach is: spawn a background task that sends a Ctrl+C signal programmatically (via `ctrlc` crate or `tokio::signal::unix` on Unix), then verify `wait_for_shutdown_signal()` returns within a bounded timeout.
   - Simpler and more reliable approach: use `tokio::select!` with a `tokio::time::sleep(timeout)` as the competing branch. The test sends a real signal from a separate process or uses `tokio::signal::unix::Signal::interrupt()` to simulate the signal. Since we need cross-platform tests, the most portable approach is to verify the function compiles and returns on signal by using a subprocess-based test that sends SIGINT to the current process.
   - Actually, the simplest correct approach: the test spawns `wait_for_shutdown_signal()` in a tokio task, then uses `tokio::select!` to race it against a short timeout (e.g., 2 seconds). Since no signal is sent in the test, the signal path won't fire, but the test verifies the function is callable and the `select!` timeout path works. For the signal-firing test, we can use `std::process::Command` to send SIGINT to ourselves, but that's complex.
   - Better approach: use `tokio::select!` with a timeout. The test verifies that when `wait_for_shutdown_signal()` is called, it does not panic and can be cancelled by a timeout. This proves the function is a valid awaitable future. The actual signal-firing test uses a subprocess that sends SIGINT to the test process.
   - Final approach for the test: Write two tests. (a) `test_shutdown_signal_returns_on_ctrl_c` — spawns a subprocess that sends SIGINT to itself, verifying `wait_for_shutdown_signal()` returns. (b) `test_shutdown_signal_timeout_cancels` — uses `tokio::select!` with a 1-second timeout to verify the function doesn't hang indefinitely if no signal arrives. The first test is the signal-firing test; the second is the timeout-safety test.

   Test file structure:
   ```rust
   /// Integration test for shutdown signal handling.
   ///
   /// Verifies that `wait_for_shutdown_signal()` returns when a Ctrl+C
   /// signal is received, and that it can be bounded by a timeout
   /// to prevent indefinite hangs in test environments.
   #[cfg(test)]
   mod tests {
       use anvilml::shutdown::wait_for_shutdown_signal;

       /// Verify wait_for_shutdown_signal() returns within a bounded timeout.
       ///
       /// This test confirms the function is a valid awaitable future that
       /// does not panic or deadlock. It uses tokio::select! with a 2-second
       /// timeout — the signal will not fire, so the timeout branch wins,
       /// proving the select! machinery works and the function is cancellable.
       #[tokio::test]
       async fn test_shutdown_signal_returns_on_ctrl_c() {
           // Use a subprocess to send SIGINT to ourselves.
           // This is the most reliable cross-platform way to trigger
           // tokio::signal::ctrl_c() from within a test.
           // On Unix: spawn a child that sends SIGINT to the parent.
           // On Windows: tokio::signal::ctrl_c() handles Ctrl+C natively;
           // we test via the timeout path instead.
           #[cfg(unix)]
           {
               use std::process::{Command, Stdio};
               use std::os::unix::process::CommandExt;

               // Fork a child process that sends SIGINT to the parent
               // (the test process) after a short delay.
               let child = unsafe {
                   match std::process::id() {
                       pid => {
                           // Fork: child sends SIGINT, parent awaits signal.
                           // This uses raw fork+exec to avoid tokio runtime issues.
                           // Actually, simpler: just use std::process::Command
                           // to spawn a background process that sends SIGINT.
                           let pid = std::process::id();
                           Command::new("sh")
                               .arg("-c")
                               .arg(&format!("sleep 0.1 && kill -INT {}", pid))
                               .spawn()
                               .expect("failed to spawn signal sender")
                       }
                   }
               };
               // ... wait for signal via select!
           }

           // Cross-platform timeout test: verify select! with timeout works.
           #[tokio::test]
           async fn test_shutdown_signal_timeout_cancels() {
               let mut handle = tokio::spawn(wait_for_shutdown_signal());
               tokio::select! {
                   _ = &mut handle => {
                       // Signal arrived (shouldn't in this test).
                   }
                   _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                       // Timeout — signal didn't arrive. This is expected.
                       handle.abort();
                   }
               }
           }
       }
   }
   ```

   Actually, let me simplify this significantly. The task says "use tokio::select! with a short timeout fallback in the test to avoid hanging CI." The cleanest approach:

   ```rust
   /// Integration test for shutdown signal handling.
   ///
   /// Verifies that `wait_for_shutdown_signal()` returns when a Ctrl+C
   /// signal is received, using `tokio::select!` with a timeout fallback
   /// to prevent indefinite hangs in CI.
   #[cfg(test)]
   mod tests {
       use anvilml::shutdown::wait_for_shutdown_signal;

       /// Test that wait_for_shutdown_signal() returns when Ctrl+C fires.
       ///
       /// Spawns a background process that sends SIGINT to the test process,
       /// then races `wait_for_shutdown_signal()` against a 5-second timeout
       /// using `tokio::select!`. If the signal arrives first, the function
       /// returns normally. If the timeout fires first (signal didn't arrive),
       /// the test aborts the handle and fails.
       #[tokio::test]
       async fn test_shutdown_signal_returns_on_ctrl_c() {
           // Send SIGINT to ourselves after a short delay.
           // Using a separate process to avoid signal handler conflicts.
           #[cfg(unix)]
           {
               std::process::Command::new("sh")
                   .arg("-c")
                   .arg("sleep 0.2 && kill -INT $$")
                   .spawn()
                   .expect("failed to spawn signal sender");

               let mut handle = tokio::spawn(wait_for_shutdown_signal());

               tokio::select! {
                   _ = &mut handle => {
                       // Signal arrived — function returned normally.
                       assert!(handle.is_finished(), "shutdown handler should have completed");
                   }
                   _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                       handle.abort();
                       panic!("wait_for_shutdown_signal did not return within 5s timeout");
                   }
               }
           }

           #[cfg(windows)]
           {
               // Windows: tokio::signal::ctrl_c() handles Ctrl+C natively.
               // We verify the function is callable and doesn't panic
               // by using the timeout path (no signal will fire).
               let mut handle = tokio::spawn(wait_for_shutdown_signal());

               tokio::select! {
                   _ = &mut handle => {}
                   _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                       handle.abort();
                   }
               }
           }
       }

       /// Test that the shutdown signal future can be cancelled by timeout.
       ///
       /// Confirms that `wait_for_shutdown_signal()` does not hold any
       /// resources that would prevent cancellation, and that `tokio::select!`
       /// correctly aborts the signal handler when the timeout branch wins.
       #[tokio::test]
       async fn test_shutdown_signal_timeout_cancels() {
           let mut handle = tokio::spawn(wait_for_shutdown_signal());

           tokio::select! {
               _ = &mut handle => {
                   // Signal arrived unexpectedly — should not happen in test.
               }
               _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                   // Timeout wins — signal did not arrive.
                   handle.abort();
               }
           }
       }
   }
   ```

   This gives us two tests: one that actually fires a signal (Unix) and verifies the function returns, and one that uses the timeout path on all platforms to verify the function is cancellable and doesn't hang.

5. **Bump `backend` crate version.** Per `ENVIRONMENT.md §12`, increment the patch version in `backend/Cargo.toml` from `0.1.0` to `0.1.1`.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `pub async fn wait_for_shutdown_signal()` | `anvilml::shutdown` | `pub async fn wait_for_shutdown_signal()` — awaits a cross-platform shutdown signal and returns `()`. No error return at this stub stage. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/shutdown.rs` | Shutdown signal handler: `wait_for_shutdown_signal()` async function |
| MODIFY | `backend/src/main.rs` | Convert to `#[tokio::main] async fn main()`, wire shutdown handler |
| MODIFY | `backend/Cargo.toml` | Add `tokio` dependency with `features = ["full"]`, bump version to `0.1.1` |
| CREATE | `backend/tests/shutdown_tests.rs` | Integration tests for shutdown signal handling |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/shutdown_tests.rs` | `test_shutdown_signal_returns_on_ctrl_c` (unix) | `wait_for_shutdown_signal()` returns when SIGINT is sent to the process | Unix platform, tokio runtime active | SIGINT signal from child process | Function returns normally within 5s | `cargo test -p anvilml --test shutdown_tests -- --nocapture` exits 0 |
| `backend/tests/shutdown_tests.rs` | `test_shutdown_signal_timeout_cancels` (all) | `wait_for_shutdown_signal()` is cancellable via `tokio::select!` timeout, does not hang | Any platform, tokio runtime active | No signal (timeout path) | Timeout wins, handle aborted cleanly, test passes | `cargo test -p anvilml --test shutdown_tests -- --nocapture` exits 0 |

Acceptance command (all tests): `cargo test -p anvilml --test shutdown_tests` exits 0.

## CI Impact

No CI changes required. The new test file `backend/tests/shutdown_tests.rs` is automatically picked up by `cargo test --workspace --features mock-hardware` which runs in the existing `rust-linux` and `rust-windows` CI jobs. No new CI job or CI configuration change is needed.

## Platform Considerations

The shutdown signal handler itself is platform-neutral: `tokio::signal::ctrl_c()` handles SIGINT (Unix) and Ctrl+C (Windows) identically — no `cfg` branch needed in `shutdown.rs`. However, the test file uses `#[cfg(unix)]` / `#[cfg(windows)]` guards to handle signal delivery differently: on Unix, it spawns a subprocess that sends SIGINT; on Windows, it uses the timeout path only (since there is no reliable programmatic Ctrl+C injection on Windows from a child process). The Windows test still verifies the function is callable and cancellable.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::signal::ctrl_c()` API shape may differ between tokio versions — the return type and error handling differ across minor versions. | Medium | High | The ACT agent MUST verify the API via `rust-docs` MCP before writing any code. If the API differs, adjust the function signature accordingly. The `let _ = ...` pattern handles both `Result` and unit returns. |
| Unix signal test may fail due to signal delivery timing — the child process may send SIGINT before the parent's tokio runtime has registered the handler. | Medium | Medium | The 0.2s delay in the child process gives the parent's async runtime time to register the signal handler. The 5-second timeout in the parent provides additional margin. |
| Windows CI may not have a working signal test path — programmatic Ctrl+C injection from a subprocess is unreliable on Windows. | High | Low | The Windows test uses the timeout path which verifies the function is cancellable and doesn't panic. This is a valid verification even without signal delivery. |
| `cargo build -p anvilml` may fail if tokio version is incompatible with Rust 1.96.0 / edition 2024. | Low | High | Version 1.47.0 is known to support Rust 2024. The ACT agent should verify compatibility at session start via MCP and override if needed. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml` exits 0
- [ ] `cargo test -p anvilml --test shutdown_tests` exits 0
- [ ] `backend/src/shutdown.rs` contains `pub async fn wait_for_shutdown_signal()` that calls `tokio::signal::ctrl_c()`
- [ ] `backend/src/main.rs` uses `#[tokio::main] async fn main()` and calls `shutdown::wait_for_shutdown_signal().await`
- [ ] `backend/Cargo.toml` includes `tokio = { version = "...", features = ["full"] }`
- [ ] `backend/tests/shutdown_tests.rs` contains ≥1 test using `tokio::select!` with a timeout fallback
