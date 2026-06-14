# Plan Report: P3-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-B1                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: AnvilError enum with IntoResponse for axum |
| Depends on  | P3-A1, P3-A2, P3-A3, P3-A4, P3-A5          |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T22:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete the `AnvilError` enum in `crates/anvilml-core/src/error.rs` by expanding it from its current three-variant skeleton (`Io`, `Toml`, `EnvVar`) to the full 12-variant specification defined in `ANVILML_DESIGN.md §5.2` plus the two pre-existing variants that `config_load.rs` depends on. Implement `axum::IntoResponse` for `AnvilError` so that every variant maps to an HTTP status code and a structured JSON body `{ "error": "<kind>", "message": "<human text>", "request_id": "<uuid>" }`. After implementation, `cargo test -p anvilml-core -- error` must exit 0, proving each variant maps to its expected HTTP status code.

## Scope

### In Scope
- **`crates/anvilml-core/src/error.rs`**: Rewrite `AnvilError` enum to include all 14 variants (12 from ANVILML_DESIGN.md §5.2 plus 2 pre-existing: `Toml`, `EnvVar`). Derive `thiserror::Error`. Implement `axum::IntoResponse` for `AnvilError`.
- **`crates/anvilml-core/Cargo.toml`**: Add `axum` as a workspace dependency (with `json` feature).
- **`crates/anvilml-core/tests/error_tests.rs`**: New test file with tests verifying HTTP status mapping for each variant and JSON response body structure.
- **`crates/anvilml-core/Cargo.toml`**: Bump patch version from `0.1.8` to `0.1.9`.

### Out of Scope
- Modifying `config_load.rs` to use the new variants (that is a separate refactoring task; the pre-existing `Toml` and `EnvVar` variants remain on the enum).
- Implementing `IntoResponse` in `anvilml-server` (the design doc lists `anvilml-server/src/error.rs` as a future file, but the task explicitly puts IntoResponse in anvilml-core).
- Any changes to `lib.rs` beyond what is needed to re-export `AnvilError` (already done).
- Database error handling integration (the `Db` variant accepts `sqlx::Error` but the `anvilml-registry` crate that uses sqlx is not modified in this task).

## Existing Codebase Assessment

The existing `crates/anvilml-core/src/error.rs` contains a minimal `AnvilError` enum with three variants: `Io(#[from] std::io::Error)`, `Toml(#[from] toml::de::Error)`, and `EnvVar { name, value }`. The module doc comment explicitly states that additional variants will be added in Phase 003 (P3-B1). This is the placeholder that P3-B1 fills.

The `config_load.rs` module uses `AnvilError::Toml` and `AnvilError::EnvVar` for config loading errors, so these two variants must be preserved. The `lib.rs` already re-exports `AnvilError` via `pub use error::AnvilError`, so no changes are needed there.

The workspace `Cargo.toml` already declares `thiserror = "2.0.18"`, `uuid = { version = "1.23.3", features = ["serde", "v4"] }`, and `axum = { version = "0.8.9", features = ["json", ...] }`. The `anvilml-core` crate already depends on `thiserror` and `uuid` from the workspace. `axum` is not yet a dependency of `anvilml-core` — it must be added.

The crate dependency graph confirms that adding `axum` to `anvilml-core` is safe: no crate above `anvilml-core` in the dependency hierarchy depends on `axum` in a way that would create a cycle. `anvilml-server` already depends on both `anvilml-core` and `axum`, so `anvilml-core → axum` followed by `anvilml-server → axum` is a valid diamond dependency.

The project's test convention (per ENVIRONMENT.md §11) places non-trivial tests in `crates/{name}/tests/` as separate test crate files. The existing test files in `crates/anvilml-core/tests/` follow this pattern (e.g., `job_tests.rs`, `config_tests.rs`).

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source           | Feature flags confirmed |
|--------|---------|-----------------|----------------------|------------------------|
| crate  | axum    | 0.8.9           | docs.rs MCP (webfetch)| json (from workspace)  |
| crate  | thiserror| 2.0.18         | Cargo.lock           | n/a                    |
| crate  | uuid    | 1.23.3          | Cargo.lock           | serde, v4              |

All three dependencies are already declared in the workspace `Cargo.toml`. `axum` is new to `anvilml-core`'s direct dependencies. `thiserror` and `uuid` are already present.

The `IntoResponse` trait was verified via docs.rs for axum 0.8.9: it requires `fn into_response(self) -> Response<Body>`. The built-in impl for `(StatusCode, R)` where `R: IntoResponse` confirms that returning `(StatusCode, Json<T>)` from `into_response` is the correct pattern.

## Approach

1. **Add `axum` to `anvilml-core`'s workspace dependency.** In `crates/anvilml-core/Cargo.toml`, add `axum = { workspace = true }` under `[dependencies]`. This gives `AnvilError` access to `axum::response::IntoResponse`, `axum::http::StatusCode`, and `axum::Json`. Rationale: the task explicitly requires `IntoResponse` in anvilml-core, and axum 0.8.9 does not transitively depend on any of our crates, so no cycle is introduced.

2. **Rewrite `AnvilError` enum in `crates/anvilml-core/src/error.rs`.** Replace the current three-variant enum with the full 14-variant enum:
   - **12 variants from ANVILML_DESIGN.md §5.2**: `Db(#[from] sqlx::Error)`, `Io(#[from] std::io::Error)`, `Serde(String)`, `Ipc(String)`, `PayloadTooLarge(String)`, `WorkerNotFound(String)`, `JobNotFound(String)`, `InvalidGraph(Vec<String>)`, `CycleDetected(Vec<String>)`, `ModelNotFound(String)`, `WorkersUnavailable(String)`, `Internal(String)`.
   - **2 pre-existing variants preserved**: `Toml(#[from] toml::de::Error)`, `EnvVar { name: String, value: String }`.
   - All variants keep `#[derive(Debug, thiserror::Error)]` as specified in the design doc.
   - The `#[from]` attribute on `Db` and `Io` enables automatic `From<sqlx::Error>` and `From<std::io::Error>` conversions (required by downstream crates).
   - The `#[from]` on `Toml` is preserved for `config_load.rs` compatibility.
   - Update the module-level doc comment to reflect that the enum is now complete (remove the "expanded from P3-B1" note).

3. **Implement `IntoResponse` for `AnvilError`.** Add an `impl axum::response::IntoResponse for AnvilError` block. In `into_response`:
   - Generate a new `uuid::Uuid::v4()` for the `request_id` field on every call.
   - Match on the variant to determine the error kind string, HTTP status code, and human-readable message.
   - Build a `serde_json::Value` object with keys `"error"`, `"message"`, and `"request_id"`.
   - Return `(status, axum::Json(value)).into_response()`, leveraging the built-in `(StatusCode, R)` impl.
   - Rationale: generating a fresh UUID on each call ensures every HTTP error response has a unique request_id for log correlation, even if the same error is returned multiple times.

4. **Bump `anvilml-core` patch version.** In `crates/anvilml-core/Cargo.toml`, change `version = "0.1.8"` to `version = "0.1.9"`. Per ENVIRONMENT.md §12, this is required whenever source files in a crate are modified.

5. **Create `crates/anvilml-core/tests/error_tests.rs`.** Write a test module that:
   - Imports `AnvilError` from `anvilml_core`.
   - Tests that each of the 14 variants maps to its expected `StatusCode` via the `status_code()` helper method (see step 5a).
   - Tests that the JSON response body contains the correct `"error"`, `"message"`, and `"request_id"` fields.
   - Tests that `request_id` is a valid v4 UUID.
   - Tests that `Db` variant correctly converts `sqlx::Error` via `From`.

5a. **Add a `status_code()` helper method to `AnvilError`.** Because `IntoResponse` returns an `axum::Response<Body>` which is difficult to assert on in unit tests, add a public `pub fn status_code(&self) -> axum::http::StatusCode` method that returns the same status code that `into_response` would use. This method is tested directly by the test file, while `into_response` delegates to it. Rationale: unit-testable status code logic without needing an axum test client.

6. **Ensure `lib.rs` re-export is unchanged.** The existing `pub use error::AnvilError` already exports the type. No changes needed.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `AnvilError` | enum | `anvilml_core::error::AnvilError` | `pub enum AnvilError { Db(sqlx::Error), Io(std::io::Error), Serde(String), Ipc(String), PayloadTooLarge(String), WorkerNotFound(String), JobNotFound(String), InvalidGraph(Vec<String>), CycleDetected(Vec<String>), ModelNotFound(String), WorkersUnavailable(String), Internal(String), Toml(toml::de::Error), EnvVar { name: String, value: String } }` |
| `status_code` | method | `anvilml_core::error::AnvilError` | `pub fn status_code(&self) -> axum::http::StatusCode` |
| `IntoResponse impl` | trait impl | `anvilml_core::error::AnvilError` | `impl axum::response::IntoResponse for AnvilError { fn into_response(self) -> Response<Body> }` |

Note: `AnvilError` is already re-exported at the crate root via `pub use error::AnvilError` in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/src/error.rs` | Expand AnvilError to 14 variants, implement IntoResponse, add status_code() helper |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add axum workspace dep; bump version 0.1.8 → 0.1.9 |
| CREATE | `crates/anvilml-core/tests/error_tests.rs` | Unit tests for status code mapping and response body structure |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/error_tests.rs` | `test_db_status_code` | `AnvilError::Db` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500) | None | `AnvilError::Db(sqlx::Error::Database(...))` | `StatusCode::INTERNAL_SERVER_ERROR` | `cargo test -p anvilml-core -- error::test_db_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_io_status_code` | `AnvilError::Io` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500) | None | `AnvilError::Io(std::io::Error::other("test"))` | `StatusCode::INTERNAL_SERVER_ERROR` | `cargo test -p anvilml-core -- error::test_io_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_serde_status_code` | `AnvilError::Serde` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500) | None | `AnvilError::Serde("bad json".to_string())` | `StatusCode::INTERNAL_SERVER_ERROR` | `cargo test -p anvilml-core -- error::test_serde_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_ipc_status_code` | `AnvilError::Ipc` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500) | None | `AnvilError::Ipc("timeout".to_string())` | `StatusCode::INTERNAL_SERVER_ERROR` | `cargo test -p anvilml-core -- error::test_ipc_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_payload_too_large_status_code` | `AnvilError::PayloadTooLarge` maps to `StatusCode::PAYLOAD_TOO_LARGE` (413) | None | `AnvilError::PayloadTooLarge("256MiB".to_string())` | `StatusCode::PAYLOAD_TOO_LARGE` | `cargo test -p anvilml-core -- error::test_payload_too_large_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_worker_not_found_status_code` | `AnvilError::WorkerNotFound` maps to `StatusCode::NOT_FOUND` (404) | None | `AnvilError::WorkerNotFound("w-1".to_string())` | `StatusCode::NOT_FOUND` | `cargo test -p anvilml-core -- error::test_worker_not_found_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_job_not_found_status_code` | `AnvilError::JobNotFound` maps to `StatusCode::NOT_FOUND` (404) | None | `AnvilError::JobNotFound("job-abc".to_string())` | `StatusCode::NOT_FOUND` | `cargo test -p anvilml-core -- error::test_job_not_found_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_invalid_graph_status_code` | `AnvilError::InvalidGraph` maps to `StatusCode::BAD_REQUEST` (400) | None | `AnvilError::InvalidGraph(vec!["missing node".to_string()])` | `StatusCode::BAD_REQUEST` | `cargo test -p anvilml-core -- error::test_invalid_graph_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_cycle_detected_status_code` | `AnvilError::CycleDetected` maps to `StatusCode::BAD_REQUEST` (400) | None | `AnvilError::CycleDetected(vec!["A→B→A".to_string()])` | `StatusCode::BAD_REQUEST` | `cargo test -p anvilml-core -- error::test_cycle_detected_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_model_not_found_status_code` | `AnvilError::ModelNotFound` maps to `StatusCode::NOT_FOUND` (404) | None | `AnvilError::ModelNotFound("model-x".to_string())` | `StatusCode::NOT_FOUND` | `cargo test -p anvilml-core -- error::test_model_not_found_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_workers_unavailable_status_code` | `AnvilError::WorkersUnavailable` maps to `StatusCode::SERVICE_UNAVAILABLE` (503) | None | `AnvilError::WorkersUnavailable("no idle".to_string())` | `StatusCode::SERVICE_UNAVAILABLE` | `cargo test -p anvilml-core -- error::test_workers_unavailable_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_internal_status_code` | `AnvilError::Internal` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500) | None | `AnvilError::Internal("panic".to_string())` | `StatusCode::INTERNAL_SERVER_ERROR` | `cargo test -p anvilml-core -- error::test_internal_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_toml_status_code` | `AnvilError::Toml` maps to `StatusCode::BAD_REQUEST` (400) | None | `AnvilError::Toml(toml::de::Error::custom("bad"))` | `StatusCode::BAD_REQUEST` | `cargo test -p anvilml-core -- error::test_toml_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_env_var_status_code` | `AnvilError::EnvVar` maps to `StatusCode::BAD_REQUEST` (400) | None | `AnvilError::EnvVar { name: "PORT".to_string(), value: "abc".to_string() }` | `StatusCode::BAD_REQUEST` | `cargo test -p anvilml-core -- error::test_env_var_status_code` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_response_body_structure` | JSON body contains `"error"`, `"message"`, `"request_id"` keys with correct types | None | `AnvilError::JobNotFound("x".to_string())` | Body has all 3 keys; `request_id` is valid v4 UUID | `cargo test -p anvilml-core -- error::test_response_body_structure` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_from_sqlx_error` | `From<sqlx::Error>` converts to `AnvilError::Db` | None | `sqlx::Error::PoolTimedOut` | `AnvilError::Db(sqlx::Error::PoolTimedOut)` | `cargo test -p anvilml-core -- error::test_from_sqlx_error` exits 0 |

CI Impact: No CI changes required. The new test file follows the established `crates/{name}/tests/` convention and will be picked up by `cargo test --workspace --features mock-hardware` automatically.

## Platform Considerations

None identified. The `AnvilError` enum, `IntoResponse` implementation, and tests are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The UUID generation uses `uuid::Uuid::v4()` which is cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Adding `axum` as a dependency of `anvilml-core` creates a compilation issue because `axum` pulls in features that conflict with `anvilml-core`'s "zero I/O, zero async" constraint (e.g., `axum` depends on `tokio`). | Low | High | `axum` is a trait-only dependency at compile time — the `json` feature adds `axum::Json` which is a newtype wrapper. No runtime async code is needed. Verify at build time with `cargo check -p anvilml-core`. If tokio pulls in unwanted features, use `axum = { workspace = true, optional = false }` and ensure `anvilml-core` does not activate tokio. |
| `sqlx::Error` is not yet a dependency of `anvilml-core`. Adding it would pull in the full sqlx crate (including SQLite driver), significantly increasing compile time. | Medium | High | ANVILML_DESIGN.md §5.2 specifies `Db(#[from] sqlx::Error)`. The `anvilml-registry` crate already depends on sqlx, so adding it to `anvilml-core` creates a shared dependency. Add `sqlx = { workspace = true }` to `anvilml-core`'s Cargo.toml. The compile-time cost is acceptable since this is a core crate. |
| The `#[from]` derive on `toml::de::Error` may conflict with `thiserror 2.0` if the error type changed its `Display` implementation. | Low | Medium | `thiserror 2.0.18` is already used in the workspace and works with `toml::de::Error` in the existing code. Verify at build time. If it fails, switch to a manual `#[error("...")]` impl. |
| Tests reference `sqlx::Error` variants but `sqlx` may not be in `anvilml-core`'s dev-dependencies. | Medium | Medium | Add `sqlx` to `[dev-dependencies]` in `crates/anvilml-core/Cargo.toml` (or to `[dependencies]` if the enum itself needs it). The `#[from] sqlx::Error` on the enum requires sqlx in regular dependencies, not just dev-dependencies. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core` exits 0
- [ ] `cargo test -p anvilml-core -- error` exits 0
- [ ] `grep -c "pub enum AnvilError" crates/anvilml-core/src/error.rs` returns 1 (exactly one enum definition)
- [ ] `grep "impl.*IntoResponse.*for AnvilError" crates/anvilml-core/src/error.rs` matches (IntoResponse is implemented)
- [ ] `grep 'version = "0.1.9"' crates/anvilml-core/Cargo.toml` matches (version bumped)
