# Plan Report: P1-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A4                                       |
| Phase       | 001 — Walking Skeleton                      |
| Description | anvilml: wire main.rs to bind axum server on 127.0.0.1:8488 |
| Depends on  | P1-A2, P1-A3                                |
| Project     | anvilml                                     |
| Planned at  | 2026-05-31T23:10:10Z                        |
| Attempt     | 1                                           |

## Objective

Replace the stub `backend/src/main.rs` body (which only prints a version string and exits) with a fully wired HTTP server startup: construct `AppState`, build the axum router via `anvilml_server::build_router`, bind a `TcpListener` to `127.0.0.1:8488`, and call `axum::serve` to start accepting requests. This completes Phase 001's runnable skeleton so that `GET /health` returns a valid JSON response.

## Scope

### In Scope
- Rewrite `backend/src/main.rs` body to wire the axum server on `127.0.0.1:8488`
- Add necessary `use` imports (`std::net::TcpListener`, `anvilml_server::{build_router, AppState}`)
- Construct `AppState` with `start_time: Instant::now()` and version from `env!("CARGO_PKG_VERSION")`
- Print a "Listening on http://127.0.0.1:8488" log line before serving
- No graceful shutdown logic (deferred to Phase 2)
- No config file parsing or CLI argument wiring (deferred to later phases)

### Out of Scope
- Any changes to `backend/Cargo.toml` — dependencies (`tokio`, `anvilml-server`) are already declared
- Any changes to `crates/anvilml-server` — `build_router` and `AppState` are already implemented (P1-A3)
- Graceful shutdown / signal handling
- Configuration file loading or CLI argument parsing
- Health endpoint changes (already implemented in P1-A3)
- Tests for main.rs (the health endpoint is tested in anvilml-server's existing unit test)
- CI workflow changes (handled by P1-A5)

## Approach

1. **Read the current `backend/src/main.rs`** to confirm its stub content (three lines: `#[tokio::main]`, `async fn main() { println!(...); }`).

2. **Replace the file body** with the following code:
   - Keep the `#[tokio::main]` attribute on `main()`.
   - Add `use std::net::TcpListener;` at the top.
   - Inside `main()`, create `AppState` using `anvilml_server::AppState::new(env!("CARGO_PKG_VERSION"))`.
   - Build the router: `let router = anvilml_server::build_router(state);`
   - Bind the listener: `let listener = TcpListener::bind("127.0.0.1:8488").expect("Failed to bind port 8488");`
   - Print the listening message: `println!("Listening on http://127.0.0.1:8488");`
   - Serve: `axum::serve(listener, router).await;`

3. **Verify compilation** by running `cargo build --release -p backend` (or `cargo build --release` from workspace root) — this must exit 0.

4. **Verify runtime** by running `cargo run -p backend` in the background, then `curl -s http://127.0.0.1:8488/health` and confirm the response contains `{"status":"ok","version":"0.1.0",...}` with HTTP 200.

## Files Affected

| Action   | Path                          | Description                                              |
|----------|-------------------------------|----------------------------------------------------------|
| MODIFY   | backend/src/main.rs           | Replace stub body with full axum server wiring logic     |

## Tests

No new test files are written or modified. The health endpoint is already tested by the existing unit test in `crates/anvilml-server/src/lib.rs` (`health_returns_200`) which exercises `build_router` and the `/health` handler in isolation.

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| health_returns_200 (existing) | crates/anvilml-server/src/lib.rs | /health returns 200 with correct JSON body |

## CI Impact

No CI changes required. This task only modifies `backend/src/main.rs` which is already covered by the existing CI workflow (P1-A5). No new dependencies are added, no new crate boundaries are introduced.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `axum::serve` requires a feature flag on the axum dependency | Low | High | Verify axum 0.7 default features include `tokio`; if not, add `features = ["tokio"]` to `anvilml-server/Cargo.toml`'s axum dep (out of scope adjustment) |
| Port 8488 already in use during verification | Low | Low | Use a different port or kill the conflicting process; this is a verification concern only, not an implementation risk |
| `TcpListener::bind` panics on failure | Low | Low | The `.expect()` call provides a clear error message; no graceful fallback needed per task spec |

## Acceptance Criteria

- [ ] `backend/src/main.rs` contains a `#[tokio::main] async fn main()` that constructs `AppState`, builds the router via `anvilml_server::build_router`, binds `TcpListener` on `127.0.0.1:8488`, and calls `axum::serve(listener, router).await`
- [ ] The file prints `Listening on http://127.0.0.1:8488` before serving
- [ ] `cargo build --release` exits with code 0
- [ ] Running `cargo run -p backend` and then `curl -s http://127.0.0.1:8488/health` returns HTTP 200 with JSON body containing `status: "ok"`, `version: "0.1.0"`, and `uptime_s` as a number
