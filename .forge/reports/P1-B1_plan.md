# Plan Report: P1-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P1-B1                                              |
| Phase       | 001 — Walking Skeleton                             |
| Description | backend: main.rs bind and serve                    |
| Depends on  | P1-A1, P1-A2, P1-A3                                |
| Project     | anvilml                                            |
| Planned at  | 2026-06-14T09:00:00Z                               |
| Attempt     | 1                                                  |

## Objective

Implement `backend/src/main.rs` as the entry point that creates `AppState` with the
workspace version, builds the axum router via `build_router`, binds a `TcpListener` on
`127.0.0.1:8488`, and runs the server with `axum::serve`. After this task, a developer
can run `cargo run --features mock-hardware` in the background and execute

```bash
curl -s http://127.0.0.1:8488/health | python3 -m json.tool
```

to receive a 200 response with a JSON body containing `{"status":"ok","version":"0.1.0",...}`.
This is the walking skeleton's Runnable Proof — the first observable end-to-end slice
proving the build toolchain, runtime, and server framework all work together.

## Scope

### In Scope
- Modify `backend/src/main.rs`: implement `#[tokio::main] async fn main()` that creates
  `AppState::new(env!("CARGO_PKG_VERSION"))`, calls `build_router(state)`, binds
  `TcpListener` on `127.0.0.1:8488`, logs the address at INFO, and awaits the serve
  future.
- Add `tokio` and `tracing` as direct dependencies in `backend/Cargo.toml` (workspace-pinned)
  so that `#[tokio::main]` and `tracing::info!` are available in `main.rs`.

### Out of Scope
- CLI argument parsing (clap integration) — that is a later task.
- Config file loading (`anvilml.toml`) — not needed for the walking skeleton.
- Worker supervision, database, model scanning, or any subsystem beyond `/health`.
- Graceful shutdown signal handling (a later task).
- Any changes to `anvilml-server` crate code — it is already complete from P1-A1–A3.

## Existing Codebase Assessment

Phase 000 established the full workspace skeleton with all 9 crates. Phase 001 Group A
completed the `anvilml-server` crate: `AppState` (with `Clone`, `new()`, `start_time`,
`version`), the `GET /health` handler returning `{"status":"ok","version":"…","uptime_s":…}`,
and `build_router(state) -> Router` wiring the handler at `GET /health`. Integration tests
in `crates/anvilml-server/tests/` confirm the health endpoint returns 200 with correct
JSON shape.

The `backend/src/main.rs` file is a stub (`fn main() {}`). The `backend/Cargo.toml` already
declares `anvilml-server` as a path dependency, along with `clap` and all other workspace
crates, but does **not** yet include `tokio` or `tracing` as direct dependencies — they are
only transitively available through `anvilml-server`. The `anvilml` binary version is
`0.1.0` (inherited from the workspace `[workspace.package] version`).

Established patterns:
- Tests use `tower::util::ServiceExt::oneshot` for unit/integration testing without binding
  a live TCP listener (see `health_tests.rs`).
- `AppState` uses `std::time::Instant` (not tokio's), derives `Clone`.
- The `tracing` crate uses `attributes` feature for `#[tracing::instrument]` (workspace).
- Error handling uses `thiserror` via `AnvilError` in `anvilml-core`.

No discrepancies between the design doc and current source: `build_router`, `AppState`,
and the health handler all exist with the signatures described in the task context.

## Resolved Dependencies

MCP tools (`rust-docs`) are unavailable. Versions are resolved from `Cargo.lock` and
workspace dependency declarations. All three are workspace-pinned dependencies already
present in the project.

| Type   | Name     | Version verified | MCP source       | Feature flags confirmed |
|--------|----------|-----------------|------------------|------------------------|
| crate  | tokio    | 1.52.3          | Cargo.lock (fallback) | full (workspace)     |
| crate  | tracing  | 0.1.44          | Cargo.lock (fallback) | std, attributes (workspace) |
| crate  | axum     | 0.8.9           | Cargo.lock (fallback) | json, http1, tokio, ws (workspace) |

Note: `axum::serve` is available in axum 0.8.x with the `tokio` feature enabled (which
is included in the workspace's `axum` features). The function signature is
`axum::serve(listener, router).await`. `TcpListener::bind` is from the Rust standard
library (`std::net::TcpListener`).

## Approach

1. **Add `tokio` and `tracing` to `backend/Cargo.toml` dependencies.**
   Add two lines under `[dependencies]`:
   ```toml
   tokio = { workspace = true }
   tracing = { workspace = true }
   ```
   Rationale: `#[tokio::main]` requires the `macros` feature from the `tokio` crate.
   The workspace declares `tokio` with `features = ["full"]` which includes macros,
   runtime, and sync — the simplest correct approach. `tracing` is needed for the
   `tracing::info!` macro used at the bind address log point. Using workspace-pinned
   versions ensures version consistency across the workspace.

2. **Implement `backend/src/main.rs`.**
   Replace the stub `fn main() {}` with:
   ```rust
   use anvilml_server::{build_router, AppState};
   use std::net::TcpListener;
   use tokio::net::ToSocketAddrs;

   #[tokio::main]
   async fn main() {
       let state = AppState::new(env!("CARGO_PKG_VERSION"));
       let router = build_router(state);
       let addr = "127.0.0.1:8488";
       let listener = TcpListener::bind(addr).expect("failed to bind listener");
       tracing::info!(addr = %addr, "listening");
       axum::serve(listener, router).await.expect("server error");
   }
   ```
   Rationale notes:
   - `TcpListener::bind` (std) is used rather than `tokio::net::TcpListener::bind`
     because `axum::serve` accepts a standard library `TcpListener` — it handles the
     async conversion internally. This avoids an unnecessary dependency on
     `tokio::net`.
   - `env!("CARGO_PKG_VERSION")` is a compile-time string literal (`&'static str`)
     which implements `Into<String>`, matching `AppState::new`'s signature.
   - `tracing::info!(addr = %addr, "listening")` uses structured field notation per
     the mandatory INFO log points table (ENVIRONMENT.md §9.2: "Bind address on
     successful listen" requires `addr=`).
   - `axum::serve(listener, router).await` is the axum 0.8 serve function. The
     `.expect()` provides a user-visible error message if the server encounters a
     fatal error during serving.

3. **Verify the build compiles.**
   Run `cargo build --features mock-hardware -p anvilml` to confirm the new dependencies
   resolve and the binary compiles. This is a verification step, not a deliverable.

## Public API Surface

None. This task creates or modifies only private items: `main.rs` is a binary entry
point (not a library), and `build_router` / `AppState` are already public from prior
tasks (P1-A1–A3). No new `pub` items are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `tokio = { workspace = true }` and `tracing = { workspace = true }` to `[dependencies]` |
| Modify | `backend/src/main.rs` | Replace stub with `#[tokio::main] async fn main()` that creates AppState, builds router, binds listener, serves |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `backend/tests/health_integration.rs` (new) | `test_health_endpoint_returns_200` | The running server responds to `GET /health` with HTTP 200 and JSON body containing `"status": "ok"` | `cargo run --features mock-hardware -p anvilml &` then `curl -sf http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"` then `kill %1` exits 0 |

The integration test is a manual Runnable Proof rather than an automated unit test
because it requires binding a live TCP port. The acceptance command in the task
description IS the test — it exercises the full stack from binary launch through HTTP
request to JSON response validation.

Additionally, the existing `anvilml-server` integration tests (`crates/anvilml-server/tests/health_tests.rs`)
already verify the `/health` handler returns 200 with correct JSON shape via
`Router::oneshot`, so no regression is possible in the handler logic.

## CI Impact

No CI changes required. The existing CI jobs (`rust-linux`, `rust-windows`) run
`cargo test --workspace --features mock-hardware` which will pick up any new test
files in `backend/tests/`. The `config-drift` and `openapi-drift` jobs are unaffected
because this task does not modify handler signatures, config structs, or OpenAPI
annotations.

## Platform Considerations

None identified. The `TcpListener::bind("127.0.0.1:8488")` call is platform-neutral —
it uses the standard library's TCP listener which works identically on Linux, Windows,
and macOS. The `axum::serve` function is also cross-platform. No `#[cfg(unix)]` or
`#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7
(`cargo check --bin anvilml --target x86_64-pc-windows-gnu`) will exercise the same
code paths.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::serve` API shape differs between axum 0.7 and 0.8 — the function may accept different parameter types or require a different import path. | Medium | High | Verify at ACT time: check that `axum::serve(listener, router).await` compiles. If the API differs (e.g. `serve(listener, router)` vs `serve(listener, router).await`), adjust accordingly. Document the actual API in Deviations from Plan. |
| Port 8488 is already in use when the developer runs the binary during manual testing. | Low | Low | The `TcpListener::bind` call will panic with a clear error message ("failed to bind listener"). The developer can kill the occupying process or set `ANVILML_PORT` env var (future task). |
| MCP tools unavailable — version and API shape verified from Cargo.lock only. | High | Medium | Versions from Cargo.lock are authoritative for the committed dependency state. If the ACT agent needs to resolve a newer version, the workspace dependency declaration pins the version via `[workspace.dependencies]`, so the ACT agent must confirm the MCP result matches the lockfile version. |

## Acceptance Criteria

- [ ] `cargo build --features mock-hardware -p anvilml` exits 0
- [ ] `cargo run --features mock-hardware -p anvilml &` starts the server (background process)
- [ ] `curl -sf http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"` exits 0
- [ ] `kill %1` cleanly stops the backgrounded server process (exit 0)
