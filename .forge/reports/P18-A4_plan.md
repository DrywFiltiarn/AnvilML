# Plan Report: P18-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-A4                                            |
| Phase       | 018 — Worker Restart API & Preflight              |
| Description | anvilml: wire graceful shutdown to WorkerPool.shutdown_all |
| Depends on  | P18-A1, P18-A2, P18-A3                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T00:22:00Z                              |
| Attempt     | 1                                                 |

## Objective

Wire the existing `WorkerPool::shutdown_all` (implemented in P18-A1) into the graceful shutdown path so that on SIGINT/SIGTERM/Ctrl-C the server drains workers cleanly, closes the SQLite connection pool (WAL flush), and exits 0 — replacing the current stub that force-exits without draining.

## Scope

### In Scope
- Extend `backend/src/shutdown.rs` to accept `Arc<AppState>` and `SqlitePool`, and implement the full shutdown sequence:
  1. Set a `shutdown` flag on `AppState` (submissions-closed gate).
  2. Log `INFO: graceful shutdown initiated`.
  3. Call `workers.shutdown_all()` (with 10 s timeout, force-kill stragglers).
  4. Drop/close the sqlx pool (WAL flush).
  5. Log `INFO: shutdown complete, exiting`.
- Add a `shutdown` flag to `AppState` (an `Arc<RwLock<bool>>` or `AtomicBool`) that `submit_job` checks before accepting new jobs.
- Update `backend/src/main.rs` to pass the shutdown resources to `shutdown::shutdown_signal()` and remove the `std::process::exit(0)` stub.
- Add required INFO log points per `FORGE_AGENT_RULES.md §11.3` (server lifecycle, graceful shutdown).

### Out of Scope
- Modifying `WorkerPool::shutdown_all` logic (already implemented in P18-A1).
- Adding new HTTP endpoints.
- Modifying the job scheduler dispatch loop (it will naturally stop receiving jobs once submissions are closed).
- Writing integration tests that require a live Python worker (manual verification per task description).

## Approach

### Step 1: Add `shutdown` flag to `AppState`

**File:** `crates/anvilml-server/src/state.rs`

Add a field to `AppState<A>`:
```rust
/// Set to true when a shutdown signal is received. Prevents new job submissions.
shutdown: Arc<tokio::sync::AtomicBool>,
```

Update both `new()` and `new_with_hardware()` constructors to initialise it to `false`.

Update the `Clone` impl to clone the `Arc`.

Add a setter:
```rust
pub fn set_shutdown(&self) {
    self.shutdown.store(true, Ordering::SeqCst);
}

pub fn is_shutdown(&self) -> bool {
    self.shutdown.load(Ordering::SeqCst)
}
```

### Step 2: Gate `submit_job` on the shutdown flag

**File:** `crates/anvilml-server/src/handlers/jobs.rs`

In `submit_job`, after the existing preflight gate, add a shutdown gate:
```rust
if state.is_shutdown() {
    return (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(error_body(
            "server_shutting_down",
            "server is shutting down — no new submissions accepted",
        )),
    );
}
```

This returns 503, consistent with the task description ("POST /v1/jobs -> 503").

### Step 3: Extend `shutdown.rs` with full shutdown sequence

**File:** `backend/src/shutdown.rs`

Change the function signature from:
```rust
pub async fn shutdown_signal()
```
to:
```rust
pub async fn shutdown_signal(state: Arc<App>, pool: SqlitePool)
```

Inside the function, implement the shutdown sequence:
```rust
pub async fn shutdown_signal(state: Arc<App>, pool: SqlitePool) {
    // Wait for any termination signal (existing platform-specific logic).
    tokio::select! {
        _ = pending_or_terminate() => {
            tracing::info!("Received termination signal, shutting down");
        }
        _ = pending_or_ctrl_shutdown() => {
            tracing::info!("Received Ctrl-SHUTDOWN, shutting down");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT (Ctrl-C), shutting down");
        }
        _ = std::future::pending::<()>() => {}
    }

    // 1. Set the shutdown flag to reject new submissions.
    state.set_shutdown();
    tracing::info!("submissions closed — rejecting new job submissions");

    // 2. Drain workers.
    if let Some(workers) = &state.workers {
        tracing::info!("draining workers");
        workers.shutdown_all().await;
        tracing::info!("all workers drained");
    } else {
        tracing::warn!("no worker pool configured, skipping drain");
    }

    // 3. Close the SQLx pool (flushes WAL).
    drop(pool);
    tracing::info!("database connection pool closed");

    tracing::info!("graceful shutdown complete");
}
```

### Step 4: Wire shutdown in `main.rs`

**File:** `backend/src/main.rs`

Replace the current call:
```rust
let _ = axum::serve(listener, router)
    .with_graceful_shutdown(shutdown::shutdown_signal())
    .await;

// Worker drain (P18-A4) is not yet implemented. Force-exit so the
// tokio runtime does not hang on live background tasks (keepalive,
// scheduler, system-stats). The worker child process is terminated
// by the OS when this process exits because CREATE_NEW_PROCESS_GROUP
// is set and the named pipe is closed.
tracing::info!("HTTP server drained, exiting");
std::process::exit(0);
```

With:
```rust
let _ = axum::serve(listener, router)
    .with_graceful_shutdown(shutdown::shutdown_signal(
        Arc::clone(&state),
        db.clone().expect("db pool exists at shutdown"),
    ))
    .await;

tracing::info!("HTTP server drained, exiting");
std::process::exit(0);
```

The `db` variable is already opened earlier in `main()` and stored in `state`. We clone it for the shutdown handler.

### Step 5: Update imports and types

**File:** `backend/src/main.rs`

Ensure `sqlx::SqlitePool` is imported (already available via `anvilml_registry::SqlitePool` re-export, but the `db` variable is typed as `SqlitePool` from the registry).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `shutdown: Arc<tokio::sync::AtomicBool>` field, setter, and getter to `AppState` |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add shutdown flag check in `submit_job` (503 on shutdown) |
| Modify | `backend/src/shutdown.rs` | Extend `shutdown_signal` to accept `Arc<App>` + `SqlitePool`, implement drain sequence |
| Modify | `backend/src/main.rs` | Wire shutdown handler with resources, remove P18-A4 stub comment |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| (none new) | — | Existing tests in `handlers/jobs.rs` and `pool.rs` continue to pass. The shutdown flag is a simple `AtomicBool` — no unit test needed for it. Integration verification is manual per task description. |

Note: No new test files are written. The task description specifies manual verification (`cargo run --features mock-hardware` + Ctrl-C + log inspection). The existing `shutdown_all_stops_all` test in `pool.rs` covers the worker drain path.

## CI Impact

No CI workflow files are modified. The change is purely application logic. All existing CI gates (format, clippy, tests, platform cross-checks) apply normally. The `submit_job` handler gains one additional early-return branch; existing tests run in mock mode (`ANVILML_WORKER_MOCK=1`) and the shutdown flag defaults to `false`, so no test regression is expected.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Tokio runtime hangs on background tasks (scheduler dispatch loop, system stats tick, worker keepalive) after shutdown | Low | High — process doesn't exit cleanly | The task description says workers are force-killed by `shutdown_all()`. Background tasks that loop indefinitely (stats tick, dispatch loop) will still prevent exit. Mitigation: the `drop(pool)` and worker shutdown should release all held resources; if the runtime still hangs, the existing `std::process::exit(0)` at the end of `main` is retained as a safety net. |
| `Arc<App>` clone in `main.rs` may not compile due to generic `AppState<A>` | Low | Medium — build failure | `App` is a type alias for `AppState<ArtifactStore>`. The `state` variable in `main.rs` is already `App` (via `build_router(state)` which takes `App`). We pass `Arc::clone(&state)` before `build_router` consumes it. If `build_router` takes `state` by value, we must clone before calling `build_router`. |
| `SqlitePool::drop` doesn't flush WAL synchronously | Low | Medium — data loss on crash during shutdown | sqlx's `SqlitePool::disconnect` is the explicit way; `drop` on the pool should close all connections which triggers WAL checkpoint. If needed, we can call `pool.disconnect()` explicitly before `drop`. |
| `state.workers` is `Option<Arc<WorkerPool>>` — could be `None` if workers weren't spawned | Low | Low — harmless warning | Already handled: the shutdown code checks `if let Some(workers) = &state.workers`. |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` passes
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes
- [ ] `cargo test --workspace --features mock-hardware` passes (no regressions)
- [ ] `POST /v1/jobs` returns 503 `server_shutting_down` after shutdown flag is set (verifiable via unit test or manual run)
- [ ] Manual run: `cargo run --features mock-hardware`, send Ctrl-C, logs show: "Received SIGINT" → "submissions closed" → "draining workers" → "all workers drained" → "database connection pool closed" → "graceful shutdown complete" → process exits 0 within ~10s
- [ ] `cargo fmt --all -- --check` passes
- [ ] Windows cross-check: `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` passes
- [ ] No new or removed `pub` items in any crate (refactor-safe — verified via `grep -n "^pub "` on modified files)
