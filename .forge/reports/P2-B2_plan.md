# Plan Report: P2-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-B2                                       |
| Phase       | 002 — Config & Graceful Shutdown            |
| Description | backend: cross-platform graceful shutdown signal handler |
| Depends on  | P2-A1, P2-A2, P2-B1                         |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T15:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `backend/src/shutdown.rs` providing `pub async fn shutdown_signal()` — a cross-platform async function that waits for SIGINT/SIGTERM (Unix) or Ctrl-C (Windows), logs `tracing::info!("shutdown signal received")`, and returns. Wire this function into `backend/src/main.rs` as the argument to `axum::serve(listener, router).with_graceful_shutdown(shutdown::shutdown_signal()).await`. When the server receives SIGTERM on Linux, it stops accepting connections and exits 0 within 3 seconds. This enables safe server restarts without orphaned connections.

## Scope

### In Scope
- **CREATE** `backend/src/shutdown.rs` — one module with:
  - `pub async fn shutdown_signal()` — cross-platform signal handler.
  - `#[cfg(unix)]` path: `tokio::signal::unix::signal(SignalKind::interrupt())` and `signal(SignalKind::terminate())`, merged via `tokio::select!`.
  - `#[cfg(windows)]` path: `tokio::signal::ctrl_c()`.
  - `tracing::info!("shutdown signal received")` log call on signal receipt.
  - Doc comment on the `pub fn` per §12.1 of FORGE_AGENT_RULES.
- **MODIFY** `backend/src/main.rs` — add `mod shutdown;` and change the `axum::serve` line to use `.with_graceful_shutdown(shutdown::shutdown_signal())`.
- **MODIFY** `backend/Cargo.toml` — bump patch version from `0.1.2` to `0.1.3` (per §14 of FORGE_AGENT_RULES).

### Out of Scope
- Worker shutdown coordination (SIGTERM → Shutdown IPC → worker drain). This is described in `ANVILML_DESIGN.md §18.3` but deferred to a later phase — at this phase there are no workers to drain.
- Database WAL checkpoint on shutdown. Deferred.
- `#[tracing::instrument]` on `shutdown_signal` — the function is a thin signal-wait wrapper; §11.5 of FORGE_AGENT_RULES says to instrument "meaningful unit of work" (migration runner, seed loader, worker spawn, job dispatch, model scan). Signal waiting is not in that list.
- Tests for the actual signal delivery (requires spawning a process and sending signals — an integration test that would duplicate `cli_tests.rs`'s existing subprocess pattern). The shutdown module is trivially verified by the acceptance criterion of starting the server and sending SIGTERM.

## Existing Codebase Assessment

The backend is a binary crate (`backend/src/main.rs`) with two modules: `cli` (clap argument parsing) and `config` (re-export of `anvilml_core::load` and `ConfigOverrides`). There is no `backend/src/lib.rs` — the crate is binary-only. The existing `cli.rs` follows established patterns: `#[derive(Parser)]` structs with `///` doc comments, `pub fn parse() -> Args`, and structured field logging via `tracing::info!(field = %value, "message")`.

The workspace `Cargo.toml` declares `tokio = { version = "1.52.3", features = ["full"] }` in `[workspace.dependencies]`, which includes the `signal` feature. The `backend/Cargo.toml` references `tokio = { workspace = true }`, so the signal module is already available — no new dependency is needed.

The `axum::serve` call in `main.rs` (line 104) currently has no graceful shutdown: `.await.expect("server error")`. The `with_graceful_shutdown` method takes a `Future<Output = ()>` — `shutdown_signal()` returns `impl Future<Output = ()>` which is compatible.

No `shutdown.rs` file exists yet. The design doc's architecture diagram (ARCHITECTURE.md §2) lists it as the expected path: `backend/src/shutdown.rs`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| crate  | tokio   | 1.52.3          | docs.rs MCP (webfetch) | `signal` is included in `features = ["full"]` — no new feature flag needed. `SignalKind::interrupt()`, `SignalKind::terminate()`, `tokio::signal::unix::signal()`, `tokio::signal::ctrl_c()` all confirmed on tokio 1.52.3 docs.rs. |
| crate  | tracing | 0.1.44          | workspace dep | Already in `backend/Cargo.toml` as `{ workspace = true }`. No changes needed. |

No new external dependencies are introduced. The `tokio` crate is already a workspace dependency with `features = ["full"]`, which includes the `signal` feature required for `tokio::signal::unix::signal` and `tokio::signal::ctrl_c`.

## Approach

1. **Create `backend/src/shutdown.rs`** with the following content:
   - Module-level `//!` doc comment describing the cross-platform shutdown signal handler.
   - `pub async fn shutdown_signal()` — the public API.
   - `#[cfg(unix)]` arm: use `tokio::signal::unix::signal(SignalKind::interrupt())` and `signal(SignalKind::terminate())`, each returning `Result<Signal>`. Use `tokio::select!` to wait on both streams concurrently. On receiving either signal, log `tracing::info!("shutdown signal received")` and return.
   - `#[cfg(windows)]` arm: use `tokio::signal::ctrl_c().await`, then log and return.
   - The function signature is `pub async fn shutdown_signal()` with no arguments and no return value (`()`).
   - Include `///` doc comment on the function per §12.1 of FORGE_AGENT_RULES.
   - Include `#[cfg(windows)]` inline comment explaining the Windows constraint (no POSIX signals; Ctrl-C is the closest equivalent).

   Rationale for `tokio::select!` on Unix: SIGINT and SIGTERM must both trigger shutdown. Receiving SIGINT first should not block waiting for SIGTERM. `tokio::select!` races the two signal streams, returning on whichever arrives first. This is the standard pattern for multi-signal handling in tokio.

   Rationale for not merging with `tokio::signal::unix::signal(SignalKind::interrupt()) | signal(SignalKind::terminate())` into a single stream: tokio's `Signal` does not implement `Stream` in a way that supports `|` merging at the type level. Two separate `select!` arms is the idiomatic approach.

2. **Modify `backend/src/main.rs`**:
   - Add `mod shutdown;` at the top of the file (after `mod cli;`).
   - Change line 104 from:
     ```rust
     axum::serve(listener, router).await.expect("server error");
     ```
     to:
     ```rust
     axum::serve(listener, router)
         .with_graceful_shutdown(shutdown::shutdown_signal())
         .await
         .expect("server error");
     ```
   - This passes the future from `shutdown_signal()` to `with_graceful_shutdown`, which calls `.await` on it when the server is ready to shut down (e.g., after `SIGTERM`).

3. **Bump `backend/Cargo.toml`** patch version: `0.1.2` → `0.1.3` per FORGE_AGENT_RULES §14.

4. **Verify the build** compiles with `cargo check --bin anvilml --features mock-hardware`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `shutdown_signal` | `pub async fn` | `backend::shutdown::shutdown_signal` | `pub async fn shutdown_signal()` |

This is the only new `pub` item introduced by this task.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/shutdown.rs` | Cross-platform graceful shutdown signal handler module |
| MODIFY | `backend/src/main.rs` | Add `mod shutdown;` and wire `shutdown_signal()` into `axum::serve().with_graceful_shutdown()` |
| MODIFY | `backend/Cargo.toml` | Bump patch version `0.1.2` → `0.1.3` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (integration via acceptance criterion) | — | Server starts, responds to health, exits 0 on SIGTERM within 3s with "shutdown signal received" log | Workspace builds with `mock-hardware`; binary exists at `target/debug/anvilml` | `cargo run --features mock-hardware -- --port 9001 &` then `kill -SIGTERM <pid>` | Process exits 0; stderr contains "shutdown signal received" | Phase acceptance: `cargo build --features mock-hardware && cargo run --features mock-hardware -- --port 9001 & sleep 2 && curl -s http://127.0.0.1:9001/health && kill -SIGTERM %1 && wait %1 && echo $?` exits 0 |

No new unit test file is created. The shutdown module is a thin wrapper around tokio's signal API — testing it would require spawning a process and sending signals, which duplicates the existing `cli_tests.rs` integration pattern. The acceptance criterion (start server → SIGTERM → exit 0) exercises the full path including the `shutdown_signal()` function.

## CI Impact

No CI changes required. The new `backend/src/shutdown.rs` file is a `.rs` source file picked up by the existing `rust-linux` and `rust-windows` CI jobs (which run `cargo clippy` and `cargo test --workspace --features mock-hardware`). No new test modules, gates, or file types are introduced. The Windows cross-check (`cargo check --bin anvilml --target x86_64-pc-windows-gnu`) will exercise the `#[cfg(windows)]` path.

## Platform Considerations

This task introduces platform-specific code via `#[cfg(unix)]` and `#[cfg(windows)]` guards:

- **`#[cfg(unix)]`**: Uses `tokio::signal::unix::signal(SignalKind::interrupt())` and `signal(SignalKind::terminate())`. Both return `Result<Signal>` — the `Result` must be handled (the code uses `unwrap()` on signal creation; if signal registration fails, the process has a fundamentally broken signal setup and panicking is appropriate). The `Signal` stream is an infinite stream of signal notifications.

- **`#[cfg(windows)]`**: Uses `tokio::signal::ctrl_c()`. This handles Ctrl-C (which on Windows maps to SIGINT for console processes). Windows does not support POSIX signals natively; `ctrl_c()` is the tokio-provided cross-platform equivalent.

- **`#[cfg(not(any(unix, windows)))]`**: Not needed — the project only targets Linux and Windows per `docs/ENVIRONMENT.md §7` (the four cross-check commands). If a non-unix/non-windows target were added in the future, the code would fail to compile at `mod shutdown;` in main.rs. This is acceptable given the project's target scope.

The `SignalKind` type requires the `signal` feature of tokio, which is already included in `features = ["full"]` in the workspace dependency. No additional feature flag is needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::signal::unix::signal()` returns `Result<Signal>` — if signal registration fails (e.g., forbidden signal), the `unwrap()` will panic at startup. SIGINT and SIGTERM are never forbidden, so this should not happen, but the unwrap is a silent assumption. | Low | Medium | Use `expect("signal registration failed")` with a descriptive message instead of blind `unwrap()`. The message makes the failure obvious if it ever occurs. |
| `tokio::select!` races two signal streams — if both signals arrive in rapid succession, only the first is logged and the second is silently discarded (the other stream is dropped). This is acceptable because the server is already shutting down. | Low | Low | Document this behavior in an inline comment. No code change needed. |
| The `axum::serve().with_graceful_shutdown()` API may behave differently on Windows — axum's graceful shutdown on Windows may not propagate the Ctrl-C signal correctly through `tokio::signal::ctrl_c()`. | Low | High | Verify during the Windows cross-check (`cargo check --bin anvilml --target x86_64-pc-windows-gnu`). If the check compiles, the API is available. Functional testing on Windows is a runtime concern handled by the ACT agent. |
| The `shutdown` module is created inside the binary crate (`backend`) rather than as a shared crate. This means the shutdown logic cannot be unit-tested independently of the binary. | N/A (by design) | Low | This is intentional — shutdown is tightly coupled to `main.rs` and the server lifecycle. A separate crate would add unnecessary indirection for a two-function module. |

## Acceptance Criteria

- [ ] `cargo check --bin anvilml --features mock-hardware` exits 0
- [ ] `cargo check --bin anvilml --target x86_64-pc-windows-gnu --features mock-hardware` exits 0 (Windows cross-check)
- [ ] `cargo build --bin anvilml --features mock-hardware` exits 0
- [ ] Start server: `cargo run --bin anvilml --features mock-hardware -- --port 9001 &` — server binds and responds
- [ ] Health check: `curl -s http://127.0.0.1:9001/health` returns `{"status":"ok"}` (HTTP 200)
- [ ] Send SIGTERM: `kill -SIGTERM <pid>` — process exits 0 within 3 seconds
- [ ] Log verification: stderr of the server process contains `shutdown signal received`
- [ ] `cargo clippy --bin anvilml --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (no formatting drift)
