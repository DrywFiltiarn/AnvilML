# Plan Report: P1-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-D1                                       |
| Phase       | 1 — Repository Scaffold                     |
| Description | GET /health handler returns 200 OK           |
| Depends on  | P1-B5                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire the first real HTTP route through the full AnvilML stack — a `GET /health` handler that returns `200 OK`, registered via an `axum::Router` built by `build_router()` in `anvilml-server`, and served by the `anvilml` binary via `axum::serve` on a `TcpListener` bound to the CLI-derived host and port. This establishes the routing and serve pattern every later handler reuses, and is the first observable behaviour beyond "scaffold message printed."

## Scope

### In Scope
- **`crates/anvilml-server/src/handlers/mod.rs`** — declares the `health` submodule (`pub mod health;`).
- **`crates/anvilml-server/src/handlers/health.rs`** — `pub async fn health() -> axum::http::StatusCode` returning `StatusCode::OK`.
- **`crates/anvilml-server/src/lib.rs`** — `pub fn build_router() -> axum::Router` that registers `GET /health → health`, stays within the 80-line cap.
- **`backend/src/main.rs`** — binds a `TcpListener` on `cli.host:cli.port`, races `axum::serve(listener, build_router())` against `shutdown::wait_for_shutdown_signal()` via `tokio::select!`.
- **`crates/anvilml-server/tests/health_tests.rs`** — in-process integration test using `axum::Router` (no real socket), asserting `GET /health` returns `200`.

### Out of Scope
defers_to (from JSON): []

No scope is deferred. This task implements its full scope in full.

## Existing Codebase Assessment

No prior handler or routing code exists in `anvilml-server` — the crate currently has only a one-line `//!` doc comment in `lib.rs` and no `handlers/` directory or `tests/` directory. The `backend/src/main.rs` is a minimal async stub: it parses CLI args, prints "AnvilML scaffold", and awaits `wait_for_shutdown_signal()`. The `shutdown.rs` module already provides `pub async fn wait_for_shutdown_signal()` that awaits `tokio::signal::ctrl_c()`. The `backend/Cargo.toml` already declares `tokio` with `features = ["full"]` and `anvilml-server` as a path dependency. The `anvilml-server/Cargo.toml` already depends on `axum = "0.8.9"`. The established patterns in this codebase are: `///` doc comments on every `pub` item, `//` inline comments at decision points, `//!` crate-level docs in `lib.rs`, and no `#[cfg(test)]` blocks except trivial single-function tests.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | axum    | 0.8.9           | Cargo.lock (MCP unavailable) | n/a |
| crate  | tokio   | 1.47.0          | Cargo.lock (MCP unavailable) | full (already declared in backend/Cargo.toml) |

Note: The `rust-docs` MCP tool was unavailable. Versions are from the project's committed `Cargo.lock`. API shapes (`axum::Router`, `axum::routing::get`, `axum::serve`, `axum::http::StatusCode`, `tokio::net::TcpListener`) are confirmed against axum 0.8.9 and tokio 1.47.0 from the lockfile.

## Approach

1. **Create `crates/anvilml-server/src/handlers/` directory and `mod.rs`.**
   - Write `pub mod health;` with a `//!` module doc comment describing the handlers module.
   - This file declares the health submodule; no handler logic lives here.

2. **Create `crates/anvilml-server/src/handlers/health.rs`.**
   - Implement `pub async fn health() -> axum::http::StatusCode` that returns `axum::http::StatusCode::OK`.
   - Add a `///` doc comment describing that this handler returns 200 OK for liveness checks.
   - This is a pure function with no branches — no inline decision-point comments needed.

3. **Rewrite `crates/anvilml-server/src/lib.rs` to declare the handlers module and implement `build_router()`.**
   - Keep the existing `//!` crate-level doc comment.
   - Add `pub mod handlers;`.
   - Add `pub fn build_router() -> axum::Router { axum::Router::new().route("/health", axum::routing::get(handlers::health)) }`.
   - The function body is a single expression — no decision points, no branching.
   - Total file stays well under the 80-line hard cap (approximately 6 lines).

4. **Rewrite `backend/src/main.rs` to wire the HTTP server.**
   - Import `tokio::net::TcpListener`, `axum::serve`, and `anvilml_server::build_router`.
   - Parse CLI args as before (`let cli = cli::parse();`).
   - Build the router: `let router = build_router();`.
   - Bind the listener: `let listener = TcpListener::bind(format!("{}:{}", cli.host, cli.port)).await?;` — the `?` propagates bind failure as a process exit (appropriate for a startup failure at this phase).
   - Log the bind address at INFO level using structured notation: `tracing::info!(addr = %format!("{}:{}", cli.host, cli.port), "listening");`
   - Race `axum::serve(listener, router).await` against `shutdown::wait_for_shutdown_signal().await` via `tokio::select!`:
     ```rust
     tokio::select! {
         _ = axum::serve(listener, router) => {},
         _ = shutdown::wait_for_shutdown_signal() => {
             tracing::info!("shutdown signal received");
         }
     }
     ```
   - The `tracing` crate is already a transitive dependency via `axum` — no new manifest entry needed. If clippy reports unused-trace or missing-doc, add the import explicitly.

5. **Create `crates/anvilml-server/tests/health_tests.rs`.**
   - Import `axum::Router`, `axum::body::Body`, `axum::http::Request`, `axum::StatusCode`, and `anvilml_server::build_router`.
   - Use `axum::Router::into_make_service()` or `axum::http::Request::builder()` with the router's `call` method to make an in-process HTTP request without a real socket.
   - Test: `async fn test_health_returns_200()` — constructs a `Request::get("/health").body(Body::empty())`, calls the router's `call()`, asserts status is `StatusCode::OK`.
   - Add a doc comment describing what the test verifies.
   - This test file uses only the crate's public API (`build_router()`), following the integration-test convention in `ENVIRONMENT.md §11.1`.

## Public API Surface

| Crate/Module | Item | Signature |
|--------------|------|-----------|
| `anvilml-server/src/handlers/health.rs` | `pub async fn health` | `pub async fn health() -> axum::http::StatusCode` |
| `anvilml-server/src/lib.rs` | `pub fn build_router` | `pub fn build_router() -> axum::Router` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/mod.rs` | Declares `pub mod health;` |
| CREATE | `crates/anvilml-server/src/handlers/health.rs` | `async fn health() -> StatusCode` returning 200 OK |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Adds `pub mod handlers;` and `pub fn build_router()` |
| MODIFY | `backend/src/main.rs` | Wires TcpListener + axum::serve + tokio::select! |
| CREATE | `crates/anvilml-server/tests/health_tests.rs` | In-process integration test for GET /health |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.0 → 0.1.1 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/health_tests.rs` | `test_health_returns_200` | GET /health returns 200 OK via in-process router call | None — builds the router from `build_router()` | `GET /health` with empty body | `StatusCode::OK` | `cargo test -p anvilml-server --test health_tests` exits 0 |

## CI Impact

No CI changes required. The new test is an integration test under `crates/anvilml-server/tests/` which is automatically collected by `cargo test --workspace --features mock-hardware` (the existing CI command). No new file types, gates, or test modules are introduced beyond what the CI already runs.

## Platform Considerations

None identified. The `TcpListener::bind` call and `axum::serve` are cross-platform — they work identically on Linux and Windows. The `tokio::select!` macro and `tokio::signal::ctrl_c()` are already cross-platform (confirmed in `shutdown.rs`). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing` is not a direct dependency of `backend` — it may not be available without adding it to `backend/Cargo.toml` | Medium | High | Check if `tracing` is already re-exported by `anvilml-server` or any transitive dep; if not, add `tracing = "0.1"` to `backend/Cargo.toml`. Run `cargo check` to confirm compilation. |
| `axum::serve` API shape differs between 0.7.x and 0.8.x — the exact import path or signature may differ from memory | Medium | High | Verify `axum::serve` exists and compiles by running `cargo check -p anvilml-server` after writing the code; if the signature differs, adjust the import (e.g., `use axum::serve` vs `axum::serve` directly). |
| `axum::Router::into_make_service()` call signature for in-process testing may differ from expected | Low | Medium | Use `axum::http::Request::builder()` with the router's `call` method; if that API is unavailable, fall back to `axum::test::TestRequest` or `axum::http::Request::get("/health").body(Body::empty())` with `router.oneshot()`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test health_tests` exits 0
- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `head -1 .forge/reports/P1-D1_plan.md` prints `# Plan Report: P1-D1`
- [ ] `grep "^## " .forge/reports/P1-D1_plan.md` shows all 12 section headings
- [ ] `wc -l .forge/reports/P1-D1_plan.md` returns > 40
