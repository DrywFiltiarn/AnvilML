# Plan Report: P3-A6

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A6                                         |
| Phase       | 003 — Core Domain Types                      |
| Description | anvilml-server: /v1/system/env handler returning stub EnvReport |
| Depends on  | P3-A5                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T13:06:54Z                          |
| Attempt     | 1                                             |

## Objective

Add a `GET /v1/system/env` endpoint to the anvilml-server crate that returns a stubbed `EnvReport` JSON object. This proves that domain types defined in `anvilml-core` (specifically `EnvReport` from `types::worker`) serialize correctly over HTTP and surfaces the first piece of system state through a real REST endpoint. The stub values (`python_path=''`, `preflight_ok=false`, `reason='not_checked'`) are placeholders that will be replaced by real preflight logic in Phase 18.

## Scope

### In Scope
- Add `env_report: Arc<RwLock<EnvReport>>` field to `AppState` (in `state.rs`)
- Initialize `env_report` with stub values in `AppState::new()`
- Create `handlers/system.rs` with an async handler `get_env` that reads the `Arc<RwLock<EnvReport>>` and returns `(StatusCode, Json<EnvReport>)`
- Wire `pub mod system;` into `handlers/mod.rs`
- Wire `GET /v1/system/env` route into `build_router()` in `lib.rs`
- Add an integration test verifying the endpoint returns 200 with the correct stub JSON
- No changes to `anvilml-core` — `EnvReport` is already defined and re-exported

### Out of Scope
- Real Python preflight logic (deferred to Phase 18)
- Any changes to `anvilml-core` types or modules
- Changes to the `backend/` launcher binary or CLI
- WebSocket event emission for env changes
- OpenAPI schema regeneration (handled separately by `anvilml-openapi`)

## Approach

1. **Add `env_report` field to `AppState`** (`crates/anvilml-server/src/state.rs`):
   - Add `use std::sync::{Arc, RwLock};`
   - Add `use anvilml_core::EnvReport;`
   - Add field: `env_report: Arc<RwLock<EnvReport>>`
   - In `AppState::new()`, initialize with stub: `python_path=""`, `python_version=""`, `torch_version=""`, `preflight_ok=false`, `reason="not_checked"`, wrapped in `Arc::new(RwLock::new(...))`

2. **Create `handlers/system.rs`** (`crates/anvilml-server/src/handlers/system.rs`):
   - Import `axum::{extract::State, http::StatusCode, response::Json}`
   - Import `std::sync::Arc`
   - Import `anvilml_core::EnvReport` and `crate::state::AppState`
   - Define async fn `get_env(State(state): State<Arc<AppState>>) -> (StatusCode, Json<EnvReport>)`
   - Read the lock: `let report = state.env_report.read().unwrap();`
   - Return `(StatusCode::OK, Json((*report).clone()))`

3. **Wire system module into handlers** (`crates/anvilml-server/src/handlers/mod.rs`):
   - Add line: `pub mod system;`

4. **Wire route into `build_router`** (`crates/anvilml-server/src/lib.rs`):
   - Add `.route("/v1/system/env", get(handlers::system::get_env))` to the Router chain in `build_router()`

5. **Add integration test** (in `crates/anvilml-server/src/lib.rs` mod tests):
   - Follow the existing `health_returns_200` test pattern
   - Create `AppState::new("0.1.0")`
   - Build router, send GET to `/v1/system/env`
   - Assert status is 200
   - Parse body as JSON and verify keys: `python_path`="", `preflight_ok`=false, `reason`="not_checked"

## Files Affected

| Action   | Path                                          | Description                                                  |
|----------|-----------------------------------------------|--------------------------------------------------------------|
| MODIFY   | crates/anvilml-server/src/state.rs            | Add `env_report: Arc<RwLock<EnvReport>>` field and init      |
| CREATE   | crates/anvilml-server/src/handlers/system.rs  | New handler module with `get_env` async fn                   |
| MODIFY   | crates/anvilml-server/src/handlers/mod.rs     | Add `pub mod system;` to export the new handler module       |
| MODIFY   | crates/anvilml-server/src/lib.rs              | Wire `GET /v1/system/env` route into `build_router()`        |

## Tests

| Test ID / Name          | File                                         | Validates                                    |
|-------------------------|----------------------------------------------|----------------------------------------------|
| health_returns_200      | crates/anvilml-server/src/lib.rs (existing)  | Existing health endpoint still works         |
| env_returns_stub        | crates/anvilml-server/src/lib.rs (new test)  | GET /v1/system/env returns 200 with stub JSON|

## CI Impact

No CI changes required. The existing CI matrix already runs `cargo test -p anvilml-server --features mock-hardware` as part of the workspace test suite. No new jobs, steps, or workflow modifications are needed.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `RwLock` requires importing `std::sync` — already available in std, no crate dep needed | Low | None | Trivial import; no risk |
| Handler signature mismatch (e.g., forgetting `Arc` wrapper) | Low | Medium | Follow existing `health` handler pattern exactly which uses `State<Arc<AppState>>` |
| Route path conflict with existing routes | Low | None | `/v1/system/env` is a new prefix; no overlap with `/health` or other routes |
| `EnvReport` serialization fails due to missing serde derives | Low | Medium | `EnvReport` already derives Serialize/Deserialize in anvilml-core (verified); test will catch any breakage |

## Acceptance Criteria

- [ ] `AppState` has an `env_report: Arc<RwLock<EnvReport>>` field initialized with stub values (`python_path=""`, `preflight_ok=false`, `reason="not_checked"`)
- [ ] `handlers/system.rs` exists and exports `get_env` async handler
- [ ] `handlers/mod.rs` includes `pub mod system;`
- [ ] `build_router()` in `lib.rs` wires `GET /v1/system/env` route
- [ ] `cargo test -p anvilml-server --features mock-hardware` passes (0 failures)
- [ ] Integration test verifies GET `/v1/system/env` returns HTTP 200 with JSON containing all five fields (`python_path`, `python_version`, `torch_version`, `preflight_ok`, `reason`) and correct stub values
