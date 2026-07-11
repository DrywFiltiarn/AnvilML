# Plan Report: P18-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-B2                                      |
| Phase       | 18 — HTTP/WebSocket Server Completion       |
| Description | anvilml-server: GET /v1/system/versions handler + ComponentVersions type |
| Depends on  | P18-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-12T01:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `GET /v1/system/versions` endpoint to the AnvilML HTTP server, returning a per-component version report as JSON. This completes the system handler group (`/v1/system`, `/v1/system/env`, `/v1/system/versions`) defined in `ANVILML_DESIGN.md §13.4`. The response struct `ComponentVersions` is new, defined in the handler file as an HTTP-response-layer concern (not a domain type). The endpoint reads `anvilml_version` from the compile-time `CARGO_PKG_VERSION`, `rust_version` from the `rustc_version_runtime` crate, and `python_version`/`torch_version` from the `env_report` field of `AppState` (populated by P18-A1).

## Scope

### In Scope
- Define `ComponentVersions` struct in `crates/anvilml-server/src/handlers/system.rs` with fields: `anvilml_version: String`, `rust_version: String`, `python_version: Option<String>`, `torch_version: Option<String>`
- Implement `get_system_versions()` handler function in `system.rs` that reads `env_report` from `AppState`, constructs `ComponentVersions`, and returns it as JSON
- Register the route `GET /v1/system/versions` in `build_router()` in `lib.rs`
- Add `rustc_version_runtime = "0.3.0"` as a dependency in `crates/anvilml-server/Cargo.toml`
- Write >=3 new integration tests in `crates/anvilml-server/tests/system_tests.rs`:
  - Test that `anvilml_version` is non-empty (from `CARGO_PKG_VERSION`)
  - Test that `python_version` and `torch_version` reflect the current `env_report` values
  - Test that `python_version`/`torch_version` are `null` when `env_report` has `None` for those fields
- Bump `anvilml-server` crate patch version (0.1.18 → 0.1.19)

### Out of Scope
None. This task's `defers_to` field is empty; all described functionality is implemented in full. No stubs, no deferred scope.

## Existing Codebase Assessment

**What already exists:** The `system.rs` module already contains two handlers (`get_system` and `get_system_env`) that follow a consistent pattern: acquire a read lock on an `AppState` field, clone the value, return as `Json<T>`. The `health.rs` handler already demonstrates the `env!("CARGO_PKG_VERSION")` pattern for compile-time version injection. The `AppState` struct already has the `env_report: Arc<RwLock<EnvReport>>` field (added by P18-A1), and `EnvReport` already carries `python_version: Option<String>` and `torch_version: Option<String>` fields. The `build_router()` function in `lib.rs` already registers `/v1/system` and `/v1/system/env` routes, establishing the pattern for the new route.

**Established patterns:**
- Handler functions are `pub(crate) async fn` taking `State<AppState>` and returning `Json<T>`.
- Response structs are `#[derive(Debug, Clone, serde::Serialize)]` (and `#[serde(rename_all = "snake_case")]` when needed).
- The `health.rs` handler uses `env!("CARGO_PKG_VERSION")` for compile-time version — the same pattern applies here for `anvilml_version`.
- Tests use the `make_test_state()` helper with sentinel values, then send requests via `router.oneshot(req)`.
- All tests in `system_tests.rs` are async, use `tower::util::ServiceExt`, and parse JSON responses via `serde_json::Value`.

**Gap between design doc and current source:** The design doc (§13.4) specifies the response type as `ComponentVersions` but does not define its fields in detail — the task context fills this in. The current codebase has no `ComponentVersions` type yet. The `rustc_version_runtime` crate is not yet a dependency of any workspace crate, so it must be added.

## Resolved Dependencies

| Type   | Name                  | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------------------|-----------------|----------------|------------------------|
| crate  | rustc_version_runtime | 0.3.0           | rust-docs MCP  | none                   |

The `rustc_version_runtime` crate at v0.3.0 exposes a single public function `version()` that returns `&'static str` — the SemVer version of the `rustc` compiler used to build the project. No feature flags are defined. It depends on `rustc_version ^0.4.0` and `semver ^1.0` (not currently in the workspace lockfile, so they will be added transitively).

## Approach

1. **Add dependency to `Cargo.toml`.** Add `rustc_version_runtime = "0.3.0"` to the `[dependencies]` section of `crates/anvilml-server/Cargo.toml`. This is a runtime dependency (not dev-dependency) because the handler needs to call `rustc_version_runtime::version()` at request time.

2. **Define `ComponentVersions` struct in `system.rs`.** Add a new `pub(crate)` struct with `#[derive(Debug, Clone, serde::Serialize)]` and fields:
   - `anvilml_version: String` — from `env!("CARGO_PKG_VERSION")` (compile-time constant)
   - `rust_version: String` — from `rustc_version_runtime::version()` (called at runtime, returns `&'static str`)
   - `python_version: Option<String>` — from `state.env_report.read().await.python_version`
   - `torch_version: Option<String>` — from `state.env_report.read().await.torch_version`
   
   The struct follows the same pattern as `HealthResponse` in `health.rs`: a plain struct with `serde::Serialize`, no `ToSchema` derive needed (OpenAPI annotations are handled by P18-F1 in a later task).

3. **Implement `get_system_versions()` handler.** Add a new `pub(crate) async fn` handler in `system.rs` with signature:
   ```rust
   pub(crate) async fn get_system_versions(State(state): State<AppState>) -> Json<ComponentVersions>
   ```
   Implementation: acquire a read lock on `state.env_report`, clone the `EnvReport`, construct `ComponentVersions` from the locked values, return `Json(...)`. The handler is a thin delegation with no business logic — consistent with the pattern in `get_system()` and `get_system_env()`.

4. **Register the route in `build_router()`.** Add a `.route("/v1/system/versions", axum::routing::get(handlers::system::get_system_versions))` call in `lib.rs`, placed immediately after the existing `/v1/system/env` route to keep the system routes grouped together.

5. **Write integration tests in `system_tests.rs`.** Add three new test functions:
   - `test_get_system_versions_returns_200`: verifies the endpoint returns 200 OK with a non-empty `anvilml_version` field (asserting the field exists and is not an empty string).
   - `test_get_system_versions_reflects_env_report_values`: constructs state with `python_version = Some("3.12.3")` and `torch_version = Some("2.5.0")` in `env_report`, sends the request, and asserts both fields match in the JSON response.
   - `test_get_system_versions_null_when_env_report_unset`: constructs state with `python_version = None` and `torch_version = None` (the default in `make_test_state`), sends the request, and asserts both fields are `null` in the JSON response.

6. **Bump crate version.** Increment `anvilml-server` patch version from `0.1.18` to `0.1.19` in `crates/anvilml-server/Cargo.toml`.

## Public API Surface

| Item | Path | Signature / Definition |
|------|------|----------------------|
| struct | `anvilml_server::handlers::system::ComponentVersions` | `pub(crate) struct ComponentVersions { anvilml_version: String, rust_version: String, python_version: Option<String>, torch_version: Option<String> }` |
| fn | `anvilml_server::handlers::system::get_system_versions` | `pub(crate) async fn get_system_versions(State(AppState)) -> Json<ComponentVersions>` |
| route | axum router | `GET /v1/system/versions → get_system_versions` |

Note: `ComponentVersions` is `pub(crate)` (not `pub`) because it is an HTTP-response-layer concern, not a domain type meant for external consumers. The route itself is public via the `build_router()` function.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add `rustc_version_runtime = "0.3.0"` dependency; bump patch version 0.1.18 → 0.1.19 |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Add `ComponentVersions` struct and `get_system_versions()` handler |
| Modify | `crates/anvilml-server/src/lib.rs` | Register `GET /v1/system/versions` route in `build_router()` |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Add >=3 new integration tests for the versions endpoint |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `crates/anvilml-server/tests/system_tests.rs` | `test_get_system_versions_returns_200` | Endpoint returns 200 OK; `anvilml_version` field is present and non-empty (from `CARGO_PKG_VERSION`) | `cargo test -p anvilml-server --test system_tests` exits 0 (>=7 total tests) |
| `crates/anvilml-server/tests/system_tests.rs` | `test_get_system_versions_reflects_env_report_values` | `python_version` and `torch_version` in response match the `env_report` sentinel values set in test state (e.g. `"3.12.3"`, `"2.5.0"`) | Same command above |
| `crates/anvilml-server/tests/system_tests.rs` | `test_get_system_versions_null_when_env_report_unset` | `python_version` and `torch_version` are `null` in JSON when `env_report` has `None` for those fields (the default in `make_test_state`) | Same command above |

The acceptance command `cargo test -p anvilml-server --test system_tests` must exit 0 with >=7 total tests in the file (4 existing from P18-B1 + 3 new = 7).

## CI Impact

No new CI jobs are introduced. The existing `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware`, which includes the `anvilml-server` crate tests. No changes to `.github/workflows/ci.yml` are required.

## Platform Considerations

None identified. The `rustc_version_runtime::version()` function returns a compile-time constant string (the rustc version that was used to build the binary), which is platform-neutral. The `env!("CARGO_PKG_VERSION")` macro is also platform-independent. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `rustc_version_runtime` 0.3.0 may not compile on Rust 1.96.0 (edition 2024) | Low | High | The crate depends on `rustc_version ^0.4.0` and `semver ^1.0`, both mature crates. If compilation fails, fall back to `version_check` crate (already widely used) or use `std::env::var("CARGO_PKG_VERSION")` at build time via a build script. The plan notes this risk explicitly. |
| The `EnvReport` field names may not match what the test expects in JSON (snake_case vs camelCase) | Low | Medium | `EnvReport` uses `#[serde(rename_all = "snake_case")]` via `Serialize` derive — fields serialize as snake_case JSON keys. The test asserts on `body["python_version"]` and `body["torch_version"]`, which matches. No mismatch expected. |
| Test count requirement (>=7 total) may not be met if existing tests are removed or renamed | Low | Medium | The plan adds exactly 3 new tests to the existing 4. The acceptance command checks the total count. No existing tests are modified or removed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test system_tests` exits 0
- [ ] `grep -c "^async fn test_" crates/anvilml-server/tests/system_tests.rs | grep -qE '^[7-9]|^[0-9][0-9]'` — verifies >=7 tests in the file
- [ ] `grep -q "ComponentVersions" crates/anvilml-server/src/handlers/system.rs` — struct is defined
- [ ] `grep -q "get_system_versions" crates/anvilml-server/src/lib.rs` — route is registered
- [ ] `grep -q "rustc_version_runtime" crates/anvilml-server/Cargo.toml` — dependency is added
