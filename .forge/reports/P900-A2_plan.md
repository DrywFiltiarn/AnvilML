# Plan Report: P900-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A2                                       |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | anvilml-server: /health returns ANVILML_DESIGN.md §13.4 JSON body |
| Depends on  | P900-A1, P6-A9, P6-B3, P1-D1                |
| Project     | anvilml                                       |
| Planned at  | 2026-06-30T14:30:00Z                         |
| Attempt     | 1                                             |

## Objective

Close the spec-implementation mismatch where `GET /health` returns a bare `200 OK` with
no body, while `ANVILML_DESIGN.md §13.4` specifies the response must be a JSON object
`{ status, version, uptime_s }`. This task adds a `HealthResponse` struct, wires a
process-start `Instant` through `build_router()` via `axum::extract::State`, changes the
handler to return `Json<HealthResponse>`, and extends the existing integration test to
assert on all three JSON fields.

## Scope

### In Scope
- Add `serde = { version = "1.0", features = ["derive"] }` to `anvilml-server/Cargo.toml`
  (with version confirmed via rust-docs MCP: latest is 1.0.228).
- Define `HealthResponse { status: String, version: String, uptime_s: u64 }` in
  `crates/anvilml-server/src/handlers/health.rs`, deriving `Debug, Clone, Serialize`.
  `status` is always `"ok"`, `version` is `env!("CARGO_PKG_VERSION")`, `uptime_s` is
  computed from a captured `Instant`.
- Create a minimal `HealthState { start_time: std::time::Instant }` struct (derive `Clone`)
  to carry the start instant through axum's state mechanism.
- Change `build_router()` signature from `pub fn build_router() -> axum::Router` to
  `pub fn build_router(start_time: std::time::Instant) -> axum::Router`, wrapping the
  instant in `HealthState` and calling `.with_state()`.
- Change the health handler from `async fn health() -> StatusCode` to
  `async fn health(State(state): State<HealthState>) -> Json<HealthResponse>`, computing
  `uptime_s` as `(Instant::now() - state.start_time).as_secs()`.
- Update `backend/src/main.rs` line 89 to capture `let start = Instant::now()` before
  calling `build_router(start)`.
- Extend `crates/anvilml-server/tests/health_tests.rs` to parse the response body as
  JSON and assert on `status`, `version`, and `uptime_s` fields.
- Add `serde_json` to `anvilml-server/Cargo.toml` dev-dependencies for the test.

### Out of Scope
defers_to (from JSON): []
— Empty defers_to forbids deferral. No scope is deferred.

## Existing Codebase Assessment

The codebase inspection revealed three key findings:

**(a) What exists:** `health.rs` currently returns a bare `StatusCode::OK` with no body
(8 lines). `build_router()` in `lib.rs` takes no parameters and wires the route with
`Router::new().route("/health", get(health))`. The test file `health_tests.rs` asserts
only the 200 status code using `tower::util::ServiceExt::oneshot`. `backend/src/main.rs`
calls `build_router()` with no arguments at line 89. `anvilml-server/Cargo.toml` has no
`serde` dependency — serde is only transitively available through `anvilml-core` (where it
has the `derive` feature), but that does not make the `Serialize` derive macro available
in the `anvilml-server` crate itself.

**(b) Established patterns:** The handler module uses `pub mod health;` re-exported from
`handlers/mod.rs`. Integration tests use `tower::util::ServiceExt::oneshot` for in-process
HTTP testing (confirmed in `health_tests.rs`). The project follows `#[tokio::test]` for
async tests. No `utoipa`/`ToSchema` annotations exist on the health handler, and none are
required by this task (the design doc §13.4 specifies the JSON shape but not an OpenAPI
schema).

**(c) Gap between design doc and source:** The design doc (§13.4) specifies a JSON body
with three fields (`status`, `version`, `uptime_s`), but the implementation returns only
`StatusCode::OK`. This is the spec-drift defect the phase was created to fix. There is no
uptime tracking mechanism anywhere in the codebase — no `Instant` capture, no state
passing through `build_router()`.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source  | Feature flags confirmed |
|--------|-------------|-----------------|-------------|------------------------|
| crate  | axum        | 0.8.9           | rust-docs   | json (default), tokio (default) |
| crate  | serde       | 1.0.228         | rust-docs   | derive                  |
| crate  | tower       | 0.5.3           | (lockfile)  | n/a                      |

All versions confirmed via rust-docs MCP. No new external crates introduced — `serde`
is being added as a direct dependency to enable `#[derive(Serialize)]` in this crate.

## Approach

1. **Add dependencies to `anvilml-server/Cargo.toml`.**
   - Add `serde = { version = "1.0", features = ["derive"] }` under `[dependencies]`.
   - Add `serde_json = "1.0"` under `[dev-dependencies]` (for the test's JSON parsing).
   - Rationale: `serde` is needed for `#[derive(Serialize)]` on `HealthResponse`. The
     derive feature must be explicit — transitive availability through `anvilml-core` does
     not make the proc-macro available in this crate's compilation context.

2. **Define `HealthState` and `HealthResponse` in `health.rs`.**
   - Add `HealthState` struct (private, derive `Clone`):
     ```rust
     /// Application state carrying the process-start instant for uptime calculation.
     #[derive(Clone)]
     struct HealthState {
         start_time: std::time::Instant,
     }
     ```
   - Add `HealthResponse` struct (private, derive `Debug, Clone, Serialize`):
     ```rust
     /// JSON response body for the /health liveness probe.
     ///
     /// Per ANVILML_DESIGN.md §13.4: 200 { status, version, uptime_s }.
     #[derive(Debug, Clone, Serialize)]
     struct HealthResponse {
         status: String,
         version: String,
         uptime_s: u64,
     }
     ```
   - Rationale: Both structs are private because they are internal implementation
     details — the test parses the response as generic JSON, and no other handler
     or module needs to reference these types.

3. **Change `build_router()` signature in `lib.rs`.**
   - Change from:
     ```rust
     pub fn build_router() -> axum::Router {
         axum::Router::new().route("/health", axum::routing::get(handlers::health::health))
     }
     ```
   - To:
     ```rust
     pub fn build_router(start_time: std::time::Instant) -> axum::Router {
         let state = handlers::health::HealthState { start_time };
         axum::Router::new()
             .route("/health", axum::routing::get(handlers::health::health))
             .with_state(state)
     }
     ```
   - Rationale: `axum::Router::with_state()` is the idiomatic way to pass state to
     handlers in axum 0.8. The `HealthState` struct is created inline and passed to
     `.with_state()`, which sets the router's state type parameter to `HealthState`.

4. **Change the health handler in `health.rs`.**
   - Add imports: `use axum::extract::State; use axum::Json; use std::time::Instant;`
   - Change from:
     ```rust
     pub async fn health() -> axum::http::StatusCode {
         axum::http::StatusCode::OK
     }
     ```
   - To:
     ```rust
     pub async fn health(
         State(state): State<HealthState>,
     ) -> Json<HealthResponse> {
         let uptime_s = (Instant::now() - state.start_time).as_secs();
         Json(HealthResponse {
             status: "ok".to_string(),
             version: env!("CARGO_PKG_VERSION").to_string(),
             uptime_s,
         })
     }
     ```
   - Rationale: `axum::Json<T>` implements `IntoResponse` and automatically produces
     a `200 OK` with `application/json` content type — no explicit status code is needed.
     `env!("CARGO_PKG_VERSION")` returns a `&'static str`, so `.to_string()` converts it
     to an owned `String`.

5. **Update `backend/src/main.rs`.**
   - Add `use std::time::Instant;` at the top of the file.
   - Change line 89 from `let router = build_router();` to:
     ```rust
     let start_time = Instant::now();
     let router = build_router(start_time);
     ```
   - Rationale: `Instant::now()` is captured once at the start of the default path
     (after config load, before binding), giving a real elapsed-time measurement.
     The `start_time` variable is moved into `build_router()`.

6. **Update `crates/anvilml-server/tests/health_tests.rs`.**
   - Add imports: `use axum::body::to_bytes; use serde_json::Value;`
   - Keep the existing `test_health_returns_200` test but extend it to parse the body:
     ```rust
     #[tokio::test]
     async fn test_health_returns_200() {
         let start = std::time::Instant::now();
         let router = build_router(start);
         let req = Request::get("/health").body(Body::empty()).unwrap();
         let res = router.oneshot(req).await.unwrap();
         assert_eq!(res.status(), axum::http::StatusCode::OK);

         // Parse response body and assert on all three JSON fields.
         let body_bytes = to_bytes(res.into_body(), usize::MAX)
             .await
             .expect("body collection must succeed");
         let body: Value = serde_json::from_slice(&body_bytes)
             .expect("response body must be valid JSON");

         assert_eq!(body["status"], "ok");
         assert!(body["version"].is_string());
         let uptime = body["uptime_s"].as_u64()
             .expect("uptime_s must be a non-negative integer");
         assert!(uptime >= 0);
     }
     ```
   - Rationale: Using `serde_json::Value` avoids needing `HealthResponse` to be pub.
     The `uptime_s >= 0` assertion is the minimal correctness check — in a real test
     run the value will be small (milliseconds to seconds) but never negative.

## Public API Surface

| Item | Crate/Module | Before | After |
|------|-------------|--------|-------|
| `build_router()` | `anvilml-server/src/lib.rs` | `pub fn build_router() -> axum::Router` | `pub fn build_router(start_time: Instant) -> axum::Router` |
| `health()` | `anvilml-server/src/handlers/health.rs` | `pub async fn health() -> StatusCode` | `pub async fn health(State<HealthState>) -> Json<HealthResponse>` |
| `HealthState` | `anvilml-server/src/handlers/health.rs` | (none) | `struct HealthState { start_time: Instant }` (private) |
| `HealthResponse` | `anvilml-server/src/handlers/health.rs` | (none) | `struct HealthResponse { status, version, uptime_s }` (private) |

The `build_router()` signature change is the only breaking change to a `pub` item.
Callers must now provide an `Instant` argument. In this codebase, the only caller is
`backend/src/main.rs`, which is updated in this task.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add `serde` dependency (derive feature) and `serde_json` dev-dependency |
| Modify | `crates/anvilml-server/src/handlers/health.rs` | Add `HealthState`, `HealthResponse` structs; change handler to return `Json<HealthResponse>` |
| Modify | `crates/anvilml-server/src/lib.rs` | Change `build_router()` to accept `Instant` and wire state via `.with_state()` |
| Modify | `backend/src/main.rs` | Capture `Instant::now()` before calling `build_router()` |
| Modify | `crates/anvilml-server/tests/health_tests.rs` | Extend test to assert on JSON body fields |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/health_tests.rs` | `test_health_returns_200` | GET /health returns 200 with JSON body containing `status="ok"`, a string `version`, and a non-negative integer `uptime_s` | `cargo test -p anvilml-server --test health_tests` exits 0 |

## CI Impact

No CI changes required. The test file already lives under `crates/anvilml-server/tests/`,
which is picked up by `cargo test --workspace --features mock-hardware` (the CI test
command). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `std::time::Instant` API is platform-neutral (monotonic clock on
all supported platforms). The `serde_json` body parsing is also platform-neutral. The
Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `serde` derive feature not available in test crate's compilation context | Low | Medium | `serde` is added under `[dependencies]` (not `[dev-dependencies]`), so it is available to both the main crate and integration tests in the same package. The `derive` feature enables `#[derive(Serialize)]` for all code in the crate. |
| `build_router()` signature change breaks other callers | Low | High | The only caller of `build_router()` is `backend/src/main.rs`, which is updated in this task. No other crate imports it — `anvilml-server` is a leaf crate in the dependency graph (nothing depends on it except `backend`). |
| `uptime_s` assertion is too loose (any non-negative passes) | Low | Low | The test runs in-process with an `Instant` captured milliseconds before the request — `uptime_s` will realistically be 0 or 1. A stricter assertion (e.g. `uptime_s <= 5`) would be more precise but adds fragility if test timing varies. The current `>= 0` check is sufficient to catch a regression where uptime is hardcoded to 0. |
| `env!("CARGO_PKG_VERSION")` returns the `anvilml-server` crate version, not the workspace version | Low | Low | This is intentional — `CARGO_PKG_VERSION` is resolved at compile time for each crate. The task context explicitly says to use `env!("CARGO_PKG_VERSION")`, and the design doc §13.4 does not specify which version to use. The anvilml-server version (0.1.1) is a reasonable identity for the server component. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test health_tests` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
