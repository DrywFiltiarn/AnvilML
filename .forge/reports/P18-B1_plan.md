# Plan Report: P18-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-B1                                      |
| Phase       | 18 — HTTP/WebSocket Server Completion       |
| Description | GET /v1/system, /v1/system/env handlers     |
| Depends on  | P18-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-12T00:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `GET /v1/system` and `GET /v1/system/env` HTTP handlers that expose the hardware snapshot and Python environment report over the REST API for the first time. Both handlers are thin, one-line delegations to read-locked `AppState` fields, per `ANVILML_DESIGN.md §3.3`. When complete, `cargo test -p anvilml-server --test system_tests` exits 0 with ≥4 tests.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/system.rs` with `get_system()` and `get_system_env()` async handler functions.
- Register `system` module in `crates/anvilml-server/src/handlers/mod.rs`.
- Register `GET /v1/system` and `GET /v1/system/env` routes in `build_router()` in `crates/anvilml-server/src/lib.rs`.
- Create `crates/anvilml-server/tests/system_tests.rs` with ≥4 integration tests.
- Bump `anvilml-server` crate patch version from `0.1.17` to `0.1.18` in `Cargo.toml`.

### Out of Scope
- `GET /v1/system/versions` handler and `ComponentVersions` type — deferred to P18-B2.
- Any business logic beyond read-lock + clone + return.
- OpenAPI annotations — handled by P18-F1.
- Changes to `backend/src/main.rs` — already completed in P18-A1.

## Existing Codebase Assessment

P18-A1 already added the `hardware: Arc<RwLock<HardwareInfo>>` and `env_report: Arc<RwLock<EnvReport>>` fields to `AppState` (`crates/anvilml-server/src/state.rs`, 86 lines). The `state_tests.rs` file already constructs these fields with test values and verifies their construction and Arc-sharing semantics.

The established handler pattern (from `health.rs`) is: a `pub(crate)` async function that takes `State<AppState>`, performs its logic, and returns `Json<T>`. The `handlers/mod.rs` declares each submodule with `pub mod`. The `build_router()` in `lib.rs` uses `.route(path, axum::routing::get(handler))` to wire handlers.

The `HealthResponse` pattern (a `pub(crate)` struct with `serde::Serialize` for custom response shapes) is not needed here — both handlers return domain types (`HardwareInfo`, `EnvReport`) directly via `Json<HardwareInfo>` and `Json<EnvReport>`, which already implement `ToSchema` via the `utoipa` derives present in `anvilml-core`.

The test pattern (from `health_tests.rs` and `state_tests.rs`) uses `build_router()` with a constructed `AppState`, sends requests via `axum::Request::get(...).body(Body::empty())`, and asserts on response status and JSON body content. The `make_test_state()` helper in `health_tests.rs` provides a complete `AppState` with all required fields including `hardware` and `env_report`.

## Resolved Dependencies

No new external dependencies are introduced. The task reuses existing crate dependencies already declared in `anvilml-server/Cargo.toml`: `axum` (0.8.9), `serde` (1.0), `serde_json` (1.0), and transitively available `utoipa::ToSchema` on the response types from `anvilml-core`.

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|------------|-----------------|------------|------------------------|
| crate  | axum       | 0.8.9           | Cargo.lock | ws                     |
| crate  | serde      | 1.0             | Cargo.lock | derive                 |
| crate  | serde_json | 1.0             | Cargo.lock | n/a                    |

## Approach

### Step 1: Create `crates/anvilml-server/src/handlers/system.rs`

Create the file with two async handler functions:

```rust
//! System information handlers.
//!
//! Expose the hardware snapshot (`GET /v1/system`) and Python environment
//! report (`GET /v1/system/env`) over HTTP — per `ANVILML_DESIGN.md §13.4`.
//! Both handlers are thin delegations: read-lock the shared `AppState` field,
//! clone the value, return as JSON. No business logic.

use axum::Json;
use axum::extract::State;

use crate::AppState;

/// GET /v1/system handler.
///
/// Returns `200 OK` with the current `HardwareInfo` snapshot — per
/// `ANVILML_DESIGN.md §13.4`. Acquires a read lock on the shared
/// `hardware` field, clones the value, and returns it as JSON.
pub(crate) async fn get_system(
    State(state): State<AppState>,
) -> Json<anvilml_core::HardwareInfo> {
    // Read-lock the hardware snapshot and clone the value out.
    // The clone ensures the response is independent of any concurrent
    // write that may occur after this handler returns.
    Json(state.hardware.read().await.clone())
}

/// GET /v1/system/env handler.
///
/// Returns `200 OK` with the current `EnvReport` — per
/// `ANVILML_DESIGN.md §13.4`. Acquires a read lock on the shared
/// `env_report` field, clones the value, and returns it as JSON.
pub(crate) async fn get_system_env(
    State(state): State<AppState>,
) -> Json<anvilml_core::EnvReport> {
    // Read-lock the environment report and clone the value out.
    // Same pattern as get_system — one-line delegation with no
    // business logic, per ANVILML_DESIGN.md §3.3.
    Json(state.env_report.read().await.clone())
}
```

Rationale: Both handlers are exactly one-line delegations per `ANVILML_DESIGN.md §3.3`. The `state.hardware.read().await.clone()` pattern is the established idiom for read-only access to `Arc<RwLock<T>>` fields in this codebase (visible in `state_tests.rs` lines 448-453). No `#[tracing::instrument]` is needed — these are trivial pass-through functions with no decision points or side effects.

### Step 2: Register the module in `crates/anvilml-server/src/handlers/mod.rs`

Add `pub mod system;` to the existing module declarations:

```rust
pub mod artifacts;
pub mod health;
pub mod jobs;
pub mod nodes;
pub mod system;
```

### Step 3: Register routes in `crates/anvilml-server/src/lib.rs`

Add two `.route()` calls to `build_router()`, placed before the `.layer(...)` and `.with_state(...)` lines:

```rust
.route("/v1/system", axum::routing::get(handlers::system::get_system))
.route(
    "/v1/system/env",
    axum::routing::get(handlers::system::get_system_env),
)
```

Rationale: Routes are registered in the same order as the route table in `ANVILML_DESIGN.md §13.4` (system routes after health, before jobs). No path parameter conflicts exist — `/v1/system` is a literal path and `/v1/system/env` is a deeper literal, so axum matches them unambiguously.

### Step 4: Create `crates/anvilml-server/tests/system_tests.rs`

Create the test file with four tests following the `health_tests.rs` pattern. The test helper `make_test_state()` constructs a complete `AppState` with `hardware` and `env_report` set to known sentinel values. A second helper `update_hardware()` and `update_env_report()` writes new values through the `RwLock` between requests to verify that handler responses reflect updates.

Test structure:
- `test_get_system_returns_200()` — GET /v1/system → 200, body contains `host.hostname == "test-host"` and `gpus` is empty.
- `test_get_system_reflects_hardware_update()` — Update hardware via `RwLock` write lock, GET /v1/system → 200, body shows updated hostname.
- `test_get_system_env_returns_200()` — GET /v1/system/env → 200, body contains `python_path == "./worker/.venv/bin/python3"`.
- `test_get_system_env_reflects_env_report_update()` — Update env_report via `RwLock` write lock, GET /v1/system/env → 200, body shows updated `python_version`.

### Step 5: Bump crate version

Increment `anvilml-server` patch version in `Cargo.toml` from `0.1.17` to `0.1.18`.

## Public API Surface

| Module | Item | Signature |
|--------|------|-----------|
| `anvilml-server::handlers::system` | `get_system` | `pub(crate) async fn get_system(State(state): State<AppState>) -> Json<HardwareInfo>` |
| `anvilml-server::handlers::system` | `get_system_env` | `pub(crate) async fn get_system_env(State(state): State<AppState>) -> Json<EnvReport>` |

Both are `pub(crate)` — accessible within the crate but not part of the public library API. No new `pub` types are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/system.rs` | New handler module with `get_system()` and `get_system_env()` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod system;` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register `/v1/system` and `/v1/system/env` routes in `build_router()` |
| CREATE | `crates/anvilml-server/tests/system_tests.rs` | ≥4 integration tests for both endpoints |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.17 → 0.1.18 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `system_tests.rs` | `test_get_system_returns_200` | GET /v1/system returns 200 with `HardwareInfo` body containing the sentinel `host.hostname == "test-host"` and empty `gpus` | `cargo test -p anvilml-server --test system_tests test_get_system_returns_200` |
| `system_tests.rs` | `test_get_system_reflects_hardware_update` | After writing a new `HardwareInfo` through the `RwLock` write lock, GET /v1/system returns the updated `host.hostname` | `cargo test -p anvilml-server --test system_tests test_get_system_reflects_hardware_update` |
| `system_tests.rs` | `test_get_system_env_returns_200` | GET /v1/system/env returns 200 with `EnvReport` body containing the sentinel `python_path` and `preflight_ok == false` | `cargo test -p anvilml-server --test system_tests test_get_system_env_returns_200` |
| `system_tests.rs` | `test_get_system_env_reflects_env_report_update` | After writing a new `EnvReport` through the `RwLock` write lock, GET /v1/system/env returns the updated `python_version` | `cargo test -p anvilml-server --test system_tests test_get_system_env_reflects_env_report_update` |

Acceptance command for all tests: `cargo test -p anvilml-server --test system_tests` (exits 0).

## CI Impact

No new CI jobs are added. The existing `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace --features mock-hardware`, which includes `anvilml-server` tests. The new `system_tests.rs` test file is automatically discovered by `cargo test` as an integration test crate (placed in `crates/anvilml-server/tests/`). No changes to `.github/workflows/ci.yml` are needed.

## Platform Considerations

None identified. The handlers are pure data reads with no platform-specific code paths. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The `HardwareInfo` and `EnvReport` types are serialised via `serde` which is cross-platform. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa::ToSchema` on `HardwareInfo`/`EnvReport` may not resolve in the handler file because `utoipa` is not a direct dependency of `anvilml-server` — only a transitive dependency through `anvilml-core`. | Low | Medium | The `Json<T>` wrapper from axum does not require `utoipa` at the handler level; `ToSchema` is only needed for OpenAPI generation (P18-F1). The handlers compile fine with just `serde::Serialize`, which these types already derive. Verify during compilation. |
| Route ordering conflict — `/v1/system` and `/v1/system/env` could be confused by axum's route matching if registered in the wrong order or if a catch-all route exists. | Low | Low | Both are literal paths with no path parameters, so axum matches them deterministically. No catch-all routes exist in the current router. Register `/v1/system` before `/v1/system/env` (longer path first) for clarity, though axum handles this correctly either way. |
| Test helper `make_test_state()` duplicates logic from `health_tests.rs` — risk of drift between test helpers. | Low | Low | Follow the established pattern from `state_tests.rs` which already has a complete `make_full_state()` helper that includes all `AppState` fields. Reuse that pattern rather than copying from `health_tests.rs`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test system_tests` exits 0
- [ ] `cargo build -p anvilml-server` exits 0
- [ ] `cargo clippy -p anvilml-server -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `head -1 .forge/reports/P18-B1_plan.md` prints `# Plan Report: P18-B1`
- [ ] `grep "^## " .forge/reports/P18-B1_plan.md` shows all 12 required section headings
- [ ] `wc -l .forge/reports/P18-B1_plan.md` reports > 40 lines
