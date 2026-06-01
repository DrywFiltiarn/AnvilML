# Plan Report: P2-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A5                                       |
| Phase       | 002 — Config & Graceful Shutdown            |
| Description | anvilml: cross-platform graceful shutdown signal handler |
| Depends on  | P2-A4                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T10:55:00Z                        |
| Attempt     | 2                                           |

## Objective

Create a cross-platform async shutdown signal handler (`backend/src/shutdown.rs`) that listens for termination signals (Ctrl-C / SIGINT on Unix, SIGTERM on Unix; Ctrl-C, Ctrl-CLOSE, Ctrl-SHUTDOWN on Windows) and passes it to `axum::serve(...).with_graceful_shutdown(...)`. This enables the server to log "Shutting down" and exit cleanly (exit code 0) when the user presses Ctrl-C or sends SIGTERM, without hanging or panicking.

## Scope

### In Scope
- Create `backend/src/shutdown.rs` with an `async fn shutdown_signal()` function:
  - On Unix (`#[cfg(unix)]`): join `tokio::signal::ctrl_c()` and `tokio::signal::unix::signal(SignalKind::terminate())` via `tokio::select!`.
  - On Windows (`#[cfg(windows)]`): join `tokio::signal::ctrl_c()`, `tokio::signal::windows::ctrl_close()`, and `tokio::signal::windows::ctrl_shutdown()` via `tokio::select!`.
  - Use `std::future::pending::<()>()` for the inactive-platform arm so the function compiles on all targets.
- Modify `backend/src/main.rs`:
  - Add `mod shutdown;` after `mod cli;`.
  - Replace `axum::serve(listener, router).await` with `.with_graceful_shutdown(shutdown::shutdown_signal()).await`.
  - Log `"Shutting down"` via `tracing::info!` before awaiting serve.
- No changes to any crate in `crates/`.
- No new dependencies (all required APIs exist in existing `tokio` v1 `"full"` and `axum` v0.7).

### Out of Scope
- Any signal-handling logic beyond Unix, Windows, and the generic pending fallback.
- Worker process cleanup on shutdown (deferred to later phases).
- Any changes to `crates/anvilml-server/`, `crates/anvilml-core/`, or other workspace crates.
- Signal masking, priority, or race-condition handling beyond the basic `select!`.
- Windows Ctrl-BREAK or Ctrl-LOGOFF signals.

## Approach

1. **Create `backend/src/shutdown.rs`** with the following structure:

   ```rust
   use tokio::signal;
   #[cfg(unix)]
   use tokio::signal::unix::{signal, SignalKind};
   #[cfg(windows)]
   use tokio::signal::windows::{ctrl_close, ctrl_shutdown};

   /// Cross-platform async shutdown signal handler.
   ///
   /// On Unix: waits for SIGINT (Ctrl-C) or SIGTERM.
   /// On Windows: waits for Ctrl-C, Ctrl-CLOSE, or Ctrl-SHUTDOWN.
   /// Uses `std::future::pending` for inactive-platform arms.
   pub async fn shutdown_signal() {
       #[cfg(unix)]
       {
           let mut sigterm = signal(SignalKind::terminate())
               .expect("failed to register SIGTERM handler");

           tokio::select! {
               _ = signal::ctrl_c() => {},
               _ = sigterm.recv() => {},
           }
       }

       #[cfg(windows)]
       {
           let mut close = ctrl_close()
               .expect("failed to register Ctrl-CLOSE handler");
           let mut shutdown_ev = ctrl_shutdown()
               .expect("failed to register Ctrl-SHUTDOWN handler");

           tokio::select! {
               _ = signal::ctrl_c() => {},
               _ = close.recv() => {},
               _ = shutdown_ev.recv() => {},
           }
       }

       #[cfg(not(any(unix, windows)))]
       {
           std::future::pending().await
       }
   }
   ```

   **Design note**: `tokio::signal::ctrl_c()` is available at the root of `tokio::signal` on all platforms (it is not in the `windows` submodule). On Unix, `SignalKind::terminate()` maps to SIGTERM. On Windows, `ctrl_close()` fires when the console window is closed and `ctrl_shutdown()` fires on OS shutdown.

2. **Modify `backend/src/main.rs`**:
   - Add `mod shutdown;` after `mod cli;` (after line 1).
   - Replace line 61 (`let _ = axum::serve(listener, router).await;`) with:
     ```rust
     tracing::info!("Shutting down");
     let _ = axum::serve(listener, router)
         .with_graceful_shutdown(shutdown::shutdown_signal())
         .await;
     ```
   - The `tracing::info!` fires when the signal arrives and serve begins draining active connections.

3. **Verify Linux compilation**:
   - Run `cargo check --features mock-hardware` to confirm zero errors.

4. **Verify cross-platform compilation**:
   - Run `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` per `.clinerules` §7.7 (Windows cross-check).

5. **Runtime verification**:
   - Run `cargo run -- --port 9000`, then press Ctrl-C in the terminal.
   - Confirm "Shutting down" appears in logs and process exits with code 0 within ~1s.

## Files Affected

| Action | Path                              | Description                                                   |
|--------|-----------------------------------|---------------------------------------------------------------|
| CREATE | `backend/src/shutdown.rs`         | Cross-platform graceful shutdown signal handler (`shutdown_signal()`) |
| MODIFY | `backend/src/main.rs`             | Add `mod shutdown;`, wire `with_graceful_shutdown(...)`, log "Shutting down" |

## Tests

| Test ID / Name                  | File                          | Validates                                              |
|---------------------------------|-------------------------------|--------------------------------------------------------|
| Linux compile check             | —                             | `cargo check --features mock-hardware` exits 0         |
| Windows cross-check compile     | —                             | `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0 |
| Runtime Ctrl-C graceful shutdown | —                            | `cargo run -- --port 9000` + Ctrl-C → "Shutting down" log + exit 0 within ~1s |

No unit tests are written for `shutdown.rs` because the function is a signal-waiting combinator with no pure logic to unit-test. Verification is entirely integration-level (manual run + Ctrl-C).

## CI Impact

No CI changes required. The task uses only existing dependencies (`tokio` v1 with `"full"`, `axum` 0.7) and `#[cfg]` conditional compilation resolved at build time. The existing CI matrix already covers both platforms. No new jobs or steps are needed.

## Risks and Mitigations

| Risk                                      | Likelihood | Impact | Mitigation                                                                                         |
|-------------------------------------------|-----------|--------|----------------------------------------------------------------------------------------------------|
| `tokio::signal::ctrl_c()` unavailable on Windows root-level | Low       | Medium | Verified via docs.rs that `ctrl_c()` exists at `tokio::signal::ctrl_c()` on all platforms.         |
| `std::future::pending()` triggers clippy warnings | Low    | Low    | Use `#[allow(unreachable_code)]` on the `#[cfg(not(any(unix, windows)))]` arm if needed.          |
| SIGTERM on Linux conflicts with existing handlers | Low  | Medium | No other signal handlers exist in the codebase (confirmed: no prior usage of `signal-hook`).      |
| Windows `ctrl_close()` / `ctrl_shutdown()` may not fire in all scenarios | Low | Low | These are standard Windows console event handlers; Ctrl-C remains the primary user-facing signal. |

## Acceptance Criteria

- [ ] `backend/src/shutdown.rs` exists and exports `pub async fn shutdown_signal()`
- [ ] `backend/src/main.rs` includes `mod shutdown;` and calls `.with_graceful_shutdown(shutdown::shutdown_signal())`
- [ ] `cargo check --features mock-hardware` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] Running `cargo run -- --port 9000` binds on port 9000
- [ ] Pressing Ctrl-C logs "Shutting down" and the process exits with code 0 within ~1s (no hang, no panic)
- [ ] No new dependencies added to any Cargo.toml
- [ ] No modifications outside `backend/src/shutdown.rs` and `backend/src/main.rs`
