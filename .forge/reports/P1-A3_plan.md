# Plan Report: P1-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A3                                         |
| Phase       | 001 — Walking Skeleton                       |
| Description | anvilml-server: build_router with /health handler and AppState skeleton |
| Depends on  | P1-A1                                          |
| Project     | anvilml                                        |
| Planned at  | 2026-05-31T22:44:16Z                          |
| Attempt     | 1                                              |

## Objective

Implement the axum HTTP server skeleton for the `anvilml-server` crate by adding the required dependencies (axum, tower, tokio), creating a minimal `AppState` struct that tracks start time and version, building a `/health` endpoint handler that returns `{"status":"ok","version":"0.1.0","uptime_s":<seconds>}`, and wiring everything together via a `build_router(AppState) -> Router` function. A unit test using `axum::body` validates the endpoint returns HTTP 200 with the expected JSON shape.

## Scope

### In Scope
- Add `axum`, `tower`, and `tokio` (with `macros` + `rt-multi-thread` features) to `crates/anvilml-server/Cargo.toml`
- Create `crates/anvilml-server/src/state.rs` with `AppState { start_time: Instant, version: String }`
- Create `crates/anvilml-server/src/handlers/mod.rs` and `src/handlers/health.rs` with the health handler
- Modify `crates/anvilml-server/src/lib.rs` to declare modules, implement `build_router`, and include a unit test
- Add `serde_json` as a dev dependency for response-body parsing in tests

### Out of Scope
- Any changes to `backend/src/main.rs` (handled by P1-A4)
- Database, config, worker pool, or scheduler integration (later phases)
- WebSocket support (later phases)
- Artifact storage or model registry (later phases)
- CI workflow file (handled by P1-A5)
- Graceful shutdown (later phases)

## Approach

1. **Update Cargo.toml** — Add `axum = { version = "0.7", features = ["json"] }`, `tower = { version = "0.4", features = ["util"] }`, `tokio = { version = "1", features = ["macros", "rt-multi-thread"] }` to `[dependencies]`. Add `serde_json = { version = "1", optional = true }` to `[dev-dependencies]` (or as a regular dependency since it is needed by both the handler and tests). Use `axum::Json`, `tower::ServiceExt::oneshot`, and `tokio::time::Instant`.

2. **Create `src/state.rs`** — Define `pub struct AppState { start_time: std::time::Instant, version: String }` with a `pub fn new(version: impl Into<String>) -> Self` constructor. Derive or manually implement `Clone` (required by axum State extractor). Use `version = env!("CARGO_PKG_VERSION")` for the workspace default "0.1.0".

3. **Create `src/handlers/mod.rs`** — Declare `pub mod health;`.

4. **Create `src/handlers/health.rs`** — Define:
   ```rust
   use axum::{extract::State, response::Json, http::StatusCode};
   use serde::Serialize;
   use crate::state::AppState;

   #[derive(Serialize)]
   pub struct HealthResponse {
       status: &'static str,
       version: String,
       uptime_s: u64,
   }

   pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
       let uptime_s = state.start_time.elapsed().as_secs();
       (StatusCode::OK, Json(HealthResponse {
           status: "ok",
           version: state.version,
           uptime_s,
       }))
   }
   ```

5. **Modify `src/lib.rs`** — Replace the stub with:
   - `pub mod state;`
   - `pub mod handlers;`
   - `pub use handlers::health;
   - `use axum::Router;`
   - `pub fn build_router(state: AppState) -> Router` that calls `.route("/health", get(health::health)).with_state(state)`
   - A `#[cfg(test)]` module with an async test using `axum::body::to_bytes` and `tower::ServiceExt::oneshot` to verify the `/health` endpoint returns 200 with valid JSON.

6. **Verify** — Run `cargo test -p anvilml-server` (exits 0) and `cargo build -p anvilml-server` (exits 0).

## Files Affected

| Action   | Path                                              | Description                                          |
|----------|---------------------------------------------------|------------------------------------------------------|
| MODIFY   | `crates/anvilml-server/Cargo.toml`                | Add axum, tower, tokio, serde_json dependencies       |
| CREATE   | `crates/anvilml-server/src/state.rs`              | AppState struct with Instant and version fields       |
| CREATE   | `crates/anvilml-server/src/handlers/mod.rs`       | Module declaration for handlers sub-module            |
| CREATE   | `crates/anvilml-server/src/handlers/health.rs`    | Health endpoint handler returning JSON status         |
| MODIFY   | `crates/anvilml-server/src/lib.rs`                | Wire modules, implement build_router, add unit test   |

## Tests

| Test ID / Name            | File                                             | Validates                                          |
|---------------------------|--------------------------------------------------|----------------------------------------------------|
| `health_returns_ok`       | `crates/anvilml-server/src/lib.rs` (mod tests)   | GET /health returns 200 with JSON body containing status="ok", version, and uptime_s >= 0 |

## CI Impact

No CI changes required. The existing CI workflow (P1-A5, handled in a separate task) will automatically pick up `cargo test -p anvilml-server` as part of the workspace test suite.

## Risks and Mitigations

| Risk                              | Likelihood | Impact | Mitigation                                              |
|-----------------------------------|-----------|--------|---------------------------------------------------------|
| axum version incompatibility      | Low       | Medium | Pin to axum 0.7.x which is stable; verify docs.rs before implementation |
| tower::ServiceExt oneshot needs   | Low       | Low    | Import from `tower::service_ext` or `tower::util::ServiceExt` — well-documented pattern for testing |
| serde_json needed in both lib and tests | Low | Low  | Add serde_json as a regular dependency (already used by axum Json internally) |
| tokio features mismatch           | Low       | Low    | Use "macros" + "rt-multi-thread" to satisfy async fn + runtime requirements |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-server` exits 0 (no compilation errors)
- [ ] `cargo test -p anvilml-server` exits 0 with the health handler unit test passing
- [ ] `cargo build -p anvilml-server` produces a compiled library crate
- [ ] `AppState` struct is `pub` in `src/state.rs` with `start_time: Instant` and `version: String` fields
- [ ] `build_router(AppState) -> Router` function is `pub` in `src/lib.rs` and registers GET /health
- [ ] Health handler returns HTTP 200 with JSON `{"status":"ok","version":"0.1.0","uptime_s":<positive integer>}`
- [ ] Unit test uses `axum::body::to_bytes` to inspect the response body
