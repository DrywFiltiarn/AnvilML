# Plan Report: P1-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A2                                         |
| Phase       | 001 — Walking Skeleton                        |
| Description | anvilml-server: GET /health handler           |
| Depends on  | P1-A1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-14T08:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Create the `GET /health` HTTP handler in `crates/anvilml-server/src/handlers/health.rs` that returns a JSON body `{status:"ok", version:"<ver>", uptime_s:<N>}`. The handler extracts `State<AppState>` from the request, computes elapsed time since server start, and returns an `axum::Json` response. A unit test using `axum::Router::oneshot` verifies the endpoint returns HTTP 200 with the correct response shape. After completion, `cargo test -p anvilml-server -- health` exits 0 with at least one passing test.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/mod.rs` — declares `pub mod health;`
- Create `crates/anvilml-server/src/handlers/health.rs` — defines `async fn health(State(state): State<AppState>) -> Json<Value>` that returns the health JSON
- Create `crates/anvilml-server/tests/health_tests.rs` — unit test using `axum::Router::oneshot` that asserts HTTP 200 and body contains the `status` key with value `"ok"`
- Bump `crates/anvilml-server/Cargo.toml` patch version from `0.1.1` to `0.1.2` (source files modified)

### Out of Scope
- Routing the health handler into the router (`build_router`) — handled by P1-A3
- WebSocket endpoint or any other handler — not part of Phase 001
- OpenAPI spec generation or drift checks — handled by P1-A3/P1-B1
- Integration tests against a running server — handled by P1-B1 Runnable Proof

## Existing Codebase Assessment

P1-A1 has already been completed: `AppState` exists in `crates/anvilml-server/src/state.rs` with `start_time: std::time::Instant` and `version: String`, implements `Clone`, and has `pub fn new(version: impl Into<String>) -> Self`. The crate's `lib.rs` declares `pub mod state` and re-exports `AppState`. The `handlers/` directory does not yet exist. A test file `tests/state_tests.rs` exists with three tests for `AppState`.

The established patterns in this crate are:
- `#[derive(Clone)]` for state structs.
- `pub mod` declarations in `lib.rs` with `pub use` re-exports.
- Integration tests in `tests/` as separate test crates, using `anvil_server::` prefix to access the crate's public API.
- `#[allow(dead_code)]` with inline comment when fields exist but no handler consumes them yet.
- `std::time::Instant` (not `tokio::time::Instant`) for uptime tracking.

No discrepancies were found between the design doc and current source for this task. The `AppState` fields match the design spec, and the crate already depends on `axum` and `serde_json`.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source       | Feature flags confirmed |
|--------|-------------|-----------------|------------------|------------------------|
| crate  | axum        | 0.8.9           | docs.rs webfetch | json, http1, tokio, ws |
| crate  | serde_json  | 1.0.150         | Cargo.lock       | (none required)        |

Both `axum` and `serde_json` are already declared in the crate's `Cargo.toml` (axum via workspace, serde_json in dev-dependencies). No new dependencies are introduced by this task. The API shapes confirmed via docs.rs for axum 0.8.9: `axum::extract::State<S>` is a tuple struct wrapping `S` with `FromRequestParts` implementation; `axum::Json<T>` implements `IntoResponse` when `T: Serialize`.

## Approach

1. **Create the `handlers/` directory and `mod.rs`.** Create `crates/anvilml-server/src/handlers/mod.rs` containing only `pub mod health;`. This declares the health submodule. Follows the established pattern of `pub mod` declarations in `lib.rs`.

2. **Create `health.rs` with the handler function.** Create `crates/anvilml-server/src/handlers/health.rs`:
   - Import `axum::extract::State`, `axum::Json`, `serde_json::Value`, and `crate::state::AppState`.
   - Define `pub async fn health(State(state): State<AppState>) -> Json<Value>`:
     - Compute `uptime_s` as `(std::time::Instant::now() - state.start_time).as_secs_f64()`.
     - Build a `serde_json::Map::new()` and insert three keys: `"status"` → `"ok"`, `"version"` → `state.version.clone()`, `"uptime_s"` → `uptime_s`.
     - Return `Json(Value::Object(map))`.
   - Add a `///` doc comment describing the handler's purpose, inputs (State<AppState>), and output (JSON with status, version, uptime_s).
   - Rationale: Using `serde_json::Value` (rather than a custom struct) avoids adding a new type to the crate, keeping this task minimal. The `Map` construction is straightforward and the `Value::Object` wraps cleanly into `Json`.

3. **Create the test file.** Create `crates/anvilml-server/tests/health_tests.rs`:
   - Import `anvilml_server::AppState`, `axum::Router`, `axum::body::to_bytes`, `axum::http::{Request, Method}`, `axum::routing::get`, `serde_json::Value`.
   - Write `async fn test_health_returns_200_with_status_key()`:
     - Create `AppState::new("test-version")`.
     - Build a router: `Router::new().route("/health", get(health)).with_state(state)`.
     - Build a request: `Request::builder().method(Method::GET).uri("/health").body(axum::body::Body::empty()).unwrap()`.
     - Call `let response = router.oneshot(request).await.unwrap()`.
     - Assert `response.status() == 200`.
     - Read body bytes: `let body = to_bytes(response.into_body(), usize::MAX).await.unwrap()`.
     - Parse body as JSON: `let json: Value = serde_json::from_slice(&body).unwrap()`.
     - Assert `json["status"] == "ok"`.
   - Add a `///` doc comment describing what the test verifies.
   - Rationale: Using `Router::oneshot` is the standard axum unit testing pattern — it exercises the full handler pipeline (extraction, execution, response serialization) without needing a live TCP listener. Using `serde_json::Value` for parsing avoids needing a typed response struct.

4. **Bump the crate version.** Update `crates/anvilml-server/Cargo.toml` `[package] version` from `0.1.1` to `0.1.2`. Per §12 of ENVIRONMENT.md, every task modifying source files must bump the patch version.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `pub mod health` | `crates/anvilml-server/src/handlers/mod.rs` | `pub mod health;` |
| `pub async fn health` | `crates/anvilml-server/src/handlers/health.rs` | `pub async fn health(State(state): State<AppState>) -> Json<Value>` |

No new public types or trait implementations. The handler is the sole new `pub` item.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/mod.rs` | Module declaration for health handler |
| CREATE | `crates/anvilml-server/src/handlers/health.rs` | GET /health async handler implementation |
| CREATE | `crates/anvilml-server/tests/health_tests.rs` | Unit test: oneshot asserts 200 + status key |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/health_tests.rs` | `test_health_returns_200_with_status_key` | Router oneshot returns HTTP 200, body parses as JSON with `status` key equal to `"ok"` | `cargo test -p anvilml-server -- health` exits 0 with >=1 test |

## CI Impact

No CI changes required. The new test file follows the established convention of `crates/{name}/tests/` integration test files, which are automatically picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or CI job configurations are needed.

## Platform Considerations

None identified. The handler uses only `std::time::Instant::now()` and `serde_json` — both platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::body::to_bytes` signature may differ from what the task context assumes — in axum 0.8, the function is `axum::body::to_bytes(body, limit)` returning `Result<Bytes, axum::Error>`. Using `usize::MAX` as limit could overflow. | Low | Medium | Verify the exact signature via docs.rs (confirmed: `to_bytes(body: B, limit: usize)` returns `Result<Bytes, Error>`). Use `usize::MAX` which is safe for body sizes within addressable memory. Write the test and compile — any signature mismatch will be caught at compile time. |
| The `handlers` module is not declared in `lib.rs`, so the handler is only reachable via the test crate's direct import of `anvilml_server::handlers::health`. If P1-A3's `build_router` later needs to reference it, the path must match. | Low | Low | Declare the module as `pub mod handlers;` in `lib.rs` alongside `pub mod state;` so the handlers are part of the public API surface. P1-A3 will import via `super::handlers::health`. |
| `AppState::new("test-version")` uses a hardcoded version string in the test. This is intentional — it isolates the handler test from the actual crate version. | N/A | N/A | This is by design per test isolation rules. No mitigation needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server -- health` exits 0 with >=1 test passing
- [ ] `head -1 .forge/reports/P1-A2_plan.md` prints `# Plan Report: P1-A2`
- [ ] `grep "^## " .forge/reports/P1-A2_plan.md` shows 11 section headings
- [ ] `wc -l .forge/reports/P1-A2_plan.md` reports > 40 lines
