# Plan Report: P3-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-C1                                             |
| Phase       | 003 — Core Domain Types                           |
| Description | anvilml-server: stub GET /v1/system/env returning default EnvReport |
| Depends on  | P3-A4 (EnvReport, ProvisioningState types in anvilml-core) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-14T22:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create a stub `GET /v1/system/env` handler in `anvilml-server` that returns `EnvReport::default()` as JSON. This endpoint confirms the server can respond to the new API route defined in `ANVILML_DESIGN.md §5.7` and `TASKS_PHASE003.md`, producing a 200 response body of `{"preflight_ok":false,"provisioning":"not_started",...}` when the server is running on port 8488. The observable state after completion: `curl http://127.0.0.1:8488/v1/system/env` returns HTTP 200 with a JSON object containing `preflight_ok: false` and `provisioning: "not_started"`.

## Scope

### In Scope
- Add `Default` derive to `EnvReport` in `crates/anvilml-core/src/types/worker.rs` (with `ProvisioningState` also deriving `Default` so `EnvReport::default()` produces `provisioning: NotStarted`).
- Create `crates/anvilml-server/src/handlers/system.rs` with `pub async fn get_env(State(state): State<AppState>) -> Json<EnvReport>`.
- Add `env_report: EnvReport` field to `AppState` in `crates/anvilml-server/src/state.rs`.
- Mount `GET /v1/system/env` route in `build_router()` in `crates/anvilml-server/src/lib.rs`.
- Export the `system` module and `get_env` handler from `crates/anvilml-server/src/handlers/mod.rs`.
- Add `anvilml-server/tests/system_tests.rs` with one integration test verifying the endpoint.
- Bump `anvilml-server` patch version in `crates/anvilml-server/Cargo.toml` (0.1.4 → 0.1.5).

### Out of Scope
- Populating `EnvReport` from actual hardware/provisioning state (future tasks).
- Any other `/v1/system/*` endpoints (`/v1/system`, `/v1/system/versions`).
- Modifying `anvilml-core` beyond adding `Default` derives to `EnvReport` and `ProvisioningState`.
- OpenAPI doc generation or `openapi.json` regeneration (handled by a separate task in this phase if needed).

## Existing Codebase Assessment

The `anvilml-server` crate already has a minimal but complete structure: `lib.rs` with `build_router()`, `state.rs` with `AppState`, `handlers/mod.rs` and `handlers/health.rs` for the health endpoint, and integration tests in `tests/`. The `health` handler (GET `/health`) follows the pattern: extract `State<AppState>`, read fields, return `Json<Value>` via `serde_json::Map`.

`anvilml-core` already defines `EnvReport` and `ProvisioningState` in `types/worker.rs` with all required fields (`python_path`, `python_version`, `torch_version`, `provisioning`, `preflight_ok`, `reason`, `node_types`) and derives `Debug, Clone, Serialize, Deserialize, ToSchema`. Neither type currently derives `Default`, so `EnvReport::default()` does not compile.

`anvilml-core` is already a dependency of `anvilml-server` (declared at `crates/anvilml-server/Cargo.toml` line 7 as a path dependency), so no new external dependency is needed. The `utoipa` crate is not yet a dependency of `anvilml-server`, but it is not required for this stub — `EnvReport` already carries `ToSchema` from `anvilml-core`, and the handler just returns `Json<EnvReport>` which uses serde serialization.

The test style in `anvilml-server/tests/` uses `tower::util::ServiceExt::oneshot` with `axum::http::Request` builders to exercise the full router pipeline without a live TCP listener. This pattern will be replicated for the system env test.

## Resolved Dependencies

| Type   | Name           | Version verified | MCP source     | Feature flags confirmed |
|--------|----------------|-----------------|----------------|------------------------|
| crate  | axum           | 0.8.9           | Cargo.lock     | json, http1, tokio, ws |
| crate  | serde_json     | 1.0.150         | Cargo.lock     | (none)                 |
| crate  | utoipa         | 5.5.0           | Cargo.lock     | macros, chrono, uuid   |

No new external dependencies are introduced. The `anvilml-core` dependency already exists in `anvilml-server/Cargo.toml`. All API names (`State`, `Json`, `Router::route`, `ServiceExt::oneshot`, `EnvReport`, `ProvisioningState`) have been confirmed against the existing source code and Cargo.lock versions.

## Approach

1. **Add `Default` to `ProvisioningState`** in `crates/anvilml-core/src/types/worker.rs`: Add `Default` to the derive list on the `ProvisioningState` enum. Implement `Default` manually to return `ProvisioningState::NotStarted` — this is non-obvious because `#[derive(Default)]` on an enum would pick the first variant alphabetically or by definition order, and we explicitly need `NotStarted`.

2. **Add `Default` to `EnvReport`** in `crates/anvilml-core/src/types/worker.rs`: Add `Default` to the derive list on the `EnvReport` struct. This works because all fields implement `Default`: `Option<String>` → `None`, `ProvisioningState` (now has `Default`) → `NotStarted`, `bool` → `false`, `Option<String>` → `None`, `Vec<NodeTypeDescriptor>` → `vec![]`.

3. **Add `env_report` field to `AppState`** in `crates/anvilml-server/src/state.rs`: Add `pub env_report: EnvReport` to the struct. Update `AppState::new()` to accept an optional version parameter and initialize `env_report` with `EnvReport::default()`. The `version` field can be constructed from `CARGO_PKG_VERSION` by callers; the `env_report` is a stub that will be populated by future tasks.

4. **Create `handlers/system.rs`** in `crates/anvilml-server/src/handlers/`: Write `pub async fn get_env(State(state): State<AppState>) -> Json<EnvReport>` that returns `Json(state.env_report)`. Include a doc comment following the established pattern (describing the endpoint, what it extracts, what it returns).

5. **Export the system module** in `crates/anvilml-server/src/handlers/mod.rs`: Add `pub mod system;` and `pub use system::get_env;`.

6. **Mount the route** in `crates/anvilml-server/src/lib.rs`: Add `.route("/v1/system/env", get(get_env))` to the router chain in `build_router()`, placing it after the `/health` route. Import `get_env` from the handlers module.

7. **Create integration test** in `crates/anvilml-server/tests/system_tests.rs`: Write one test `test_system_env_returns_200_with_default_report` that builds the router with a default `AppState`, sends a GET request to `/v1/system/env`, asserts HTTP 200, parses the response as JSON, and verifies `preflight_ok` is `false` and `provisioning` is `"not_started"`.

8. **Bump version** in `crates/anvilml-server/Cargo.toml`: Change `version = "0.1.4"` to `version = "0.1.5"`.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `impl Default for ProvisioningState` | `anvilml-core::types::worker` | `fn default() -> Self { ProvisioningState::NotStarted }` |
| `impl Default for EnvReport` | `anvilml-core::types::worker` | (derived — all fields default) |
| `AppState::env_report` | `anvilml-server::state` | `pub env_report: EnvReport` (new field) |
| `AppState::new` | `anvilml-server::state` | `pub fn new(version: impl Into<String>) -> Self` (updated to init `env_report`) |
| `get_env` | `anvilml-server::handlers::system` | `pub async fn get_env(State(state): State<AppState>) -> Json<EnvReport>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/worker.rs` | Add `Default` derive to `ProvisioningState` and `EnvReport` |
| Modify | `crates/anvilml-server/src/state.rs` | Add `env_report: EnvReport` field to `AppState`, update `new()` |
| CREATE | `crates/anvilml-server/src/handlers/system.rs` | New handler module with `get_env` function |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Export `system` module and `get_env` handler |
| Modify | `crates/anvilml-server/src/lib.rs` | Mount `GET /v1/system/env` route, import `get_env` |
| CREATE | `crates/anvilml-server/tests/system_tests.rs` | Integration test for the stub endpoint |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/system_tests.rs` | `test_system_env_returns_200_with_default_report` | The `GET /v1/system/env` handler returns HTTP 200 with a JSON body containing `preflight_ok: false` and `provisioning: "not_started"` | Router built with default `AppState` | GET `/v1/system/env` | 200, JSON with correct fields | `cargo test -p anvilml-server --test system_tests -- --nocapture` exits 0 |

## CI Impact

No CI changes required. The new test file lives under `crates/anvilml-server/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` (the rust-linux and rust-windows CI jobs). No new file types, gates, or test modules are introduced that would require CI configuration changes.

## Platform Considerations

None identified. The handler returns a statically-defaulted `EnvReport` with no platform-specific behavior, no file I/O, no path handling, and no conditional compilation. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `EnvReport` contains `NodeTypeDescriptor` in its `node_types` field, which itself derives `ToSchema`. Adding `Default` to `EnvReport` requires `NodeTypeDescriptor` to also implement `Default`. If `NodeTypeDescriptor` does not derive `Default`, the `EnvReport::default()` derive will fail to compile. | Medium | High | Verify that all fields of `EnvReport` implement `Default` before writing. If `NodeTypeDescriptor` lacks `Default`, add `Default` derive to it as well (it contains only `String`, `Vec<SlotDescriptor>`, and `Option<String>` fields, all of which implement `Default`). |
| Adding `env_report: EnvReport` to `AppState` changes the struct layout. Existing code that constructs `AppState::new()` will need updating. If any other crate or test constructs `AppState` directly, it will fail to compile. | Low | Medium | `AppState::new()` already accepts `version: impl Into<String>`. Adding `env_report: EnvReport::default()` as a field initializer inside `new()` does not change the function signature, so no callers need updating. |
| The `utoipa::ToSchema` derive on `EnvReport` requires all nested types to also implement `ToSchema`. Adding `Default` does not affect `ToSchema`, but if a future task modifies `EnvReport` fields, the `ToSchema` derive must still hold. | Low | Low | Not applicable in this task — no field changes, only a `Default` derive addition. |
| Route path `/v1/system/env` may conflict with a future task that adds a sibling handler (e.g., `/v1/system/versions`). | Low | Low | This is a stub endpoint; the route is correct per the API spec in ANVILML_DESIGN.md §12. Future tasks will add sibling routes under the same prefix. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- types::worker` exits 0 (confirms `EnvReport` and `ProvisioningState` with `Default` derive compile and pass existing tests)
- [ ] `cargo test -p anvilml-server --test system_tests -- --nocapture` exits 0 (confirms the integration test passes)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (confirms no regressions in the full workspace)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (confirms no lint warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (confirms code is formatted)
- [ ] Starting the server and running `curl -s http://127.0.0.1:8488/v1/system/env | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['preflight_ok'] == False; assert d['provisioning'] == 'not_started'"` exits 0 (confirms the endpoint returns the expected default response)
- [ ] `grep '^version' crates/anvilml-server/Cargo.toml` contains `version = "0.1.5"` (confirms version bump)
