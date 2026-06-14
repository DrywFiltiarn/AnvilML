# Implementation Report: P3-B1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P3-B1                                       |
| Phase         | 003 — Core Domain Types                     |
| Description   | anvilml-core: AnvilError enum with IntoResponse for axum |
| Implemented   | 2026-06-14T23:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Expanded the `AnvilError` enum in `crates/anvilml-core/src/error.rs` from its 3-variant placeholder to the full 14-variant specification from ANVILML_DESIGN.md §5.2. Added `axum::IntoResponse` implementation that maps each variant to an HTTP status code and structured JSON body with `"error"`, `"message"`, and `"request_id"` keys. Added a public `status_code()` helper method for unit-testable status code logic. Created 17 integration tests in `crates/anvilml-core/tests/error_tests.rs` covering all variants, response body structure, UUID uniqueness, and `From<sqlx::Error>` conversion. Bumped `anvilml-core` patch version from 0.1.8 to 0.1.9.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | axum    | 0.8.9            | workspace      |
| crate  | sqlx    | 0.9.0            | workspace      |

Both `axum` and `sqlx` were already declared in the workspace `Cargo.toml` `[workspace.dependencies]` section. `axum` is new to `anvilml-core`'s direct dependencies. `sqlx` is added to enable the `Db(#[from] sqlx::Error)` variant.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/src/error.rs` | Expanded AnvilError to 14 variants, implemented IntoResponse, added status_code() and error_kind() helper methods |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added axum and sqlx workspace deps; added dev-dependencies (axum, serde_json, sqlx, uuid); bumped version 0.1.8 → 0.1.9 |
| CREATE | `crates/anvilml-core/tests/error_tests.rs` | 17 integration tests for status code mapping, response body structure, UUID uniqueness, and From conversion |
| MODIFY | `docs/TESTS.md` | Added 17 test entries following ANVILML_DESIGN.md §16.1 format |

## Commit Log

```
 .forge/reports/P3-B1_plan.md             | 147 +++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +-
 Cargo.lock                               |   6 +-
 crates/anvilml-core/Cargo.toml           |  12 +-
 crates/anvilml-core/src/error.rs         | 231 +++++++++++++++++++++++++--
 crates/anvilml-core/tests/error_tests.rs | 260 +++++++++++++++++++++++++++++++
 docs/TESTS.md                            | 136 ++++++++++++++++
 8 files changed, 788 insertions(+), 23 deletions(-)
```

## Test Results

```
     Running tests/error_tests.rs (target/debug/deps/error_tests-4154c7c2876e1ab0)

running 17 tests
test test_cycle_detected_status_code ... ok
test test_db_status_code ... ok
test test_env_var_status_code ... ok
test test_from_sqlx_error ... ok
test test_internal_status_code ... ok
test test_invalid_graph_status_code ... ok
test test_io_status_code ... ok
test test_ipc_status_code ... ok
test test_job_not_found_status_code ... ok
test test_model_not_found_status_code ... ok
test test_payload_too_large_status_code ... ok
test test_response_body_structure ... ok
test test_serde_status_code ... ok
test test_toml_status_code ... ok
test test_unique_request_ids ... ok
test test_worker_not_found_status_code ... ok
test test_workers_unavailable_status_code ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 65 tests, 0 failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, meaning no formatting drift)
```

## Platform Cross-Check

```
Check 1 (mock-hardware Linux):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.26s
Check 2 (mock-hardware Windows):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.33s
Check 3 (real-hardware Linux):    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s
Check 4 (real-hardware Windows):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.70s
```

All four cross-checks passed with zero errors.

## Project Gates

Gate 1 (config_reference): Not applicable — task does not modify ServerConfig or nested config structs.
Gate 2 (openapi_drift): Not applicable — task does not modify handler signatures, utoipa annotations, or AppState.
Gate 3 (node_parity): Not applicable — task does not modify worker nodes or node_registry.

## Public API Delta

```
+    pub fn status_code(&self) -> StatusCode {
+    pub fn error_kind(&self) -> &'static str {
```

New public items introduced by this task:
- `pub fn status_code(&self) -> StatusCode` — method on `AnvilError`, returns the HTTP status code for the variant
- `pub fn error_kind(&self) -> &'static str` — method on `AnvilError`, returns the machine-readable error kind string

Both are helper methods that support the `IntoResponse` implementation and are tested directly by `error_tests.rs`.

## Deviations from Plan

- The plan specified `axum = { workspace = true }` as a dependency but did not explicitly mention `sqlx`. However, the `Db(#[from] sqlx::Error)` variant requires `sqlx` as a direct dependency (not just dev-dependency) since the enum itself references `sqlx::Error` in its public API. Added `sqlx = { workspace = true }` to both `[dependencies]` and `[dev-dependencies]`.
- The plan specified `test_toml_status_code` using `toml::de::Error::custom("bad toml")`, but `toml::de::Error::custom` is private in toml 1.1.2. Used `toml::from_str::<toml::Value>("[invalid toml content {{{").unwrap_err()` instead to create a `toml::de::Error` from invalid TOML input.
- Added `serde_json`, `uuid`, and `axum` to `[dev-dependencies]` in `Cargo.toml` — required because the test crate is a separate compilation unit that needs direct access to these crates (they are not re-exported by `anvilml_core`).
- The `error_kind()` method was made `pub` (instead of private as initially planned) to support test access to the error kind string for response body validation.

## Blockers

None.
