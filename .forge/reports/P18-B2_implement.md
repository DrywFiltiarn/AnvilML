# Implementation Report: P18-B2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-B2                          |
| Phase         | 18 — HTTP/WebSocket Server Completion |
| Description   | anvilml-server: GET /v1/system/versions handler + ComponentVersions type |
| Implemented   | 2026-07-12T02:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented the `GET /v1/system/versions` endpoint for the AnvilML HTTP server. Added the `ComponentVersions` struct and `get_system_versions()` handler in `system.rs`, registered the route in `lib.rs`, added the `rustc_version_runtime` dependency to `Cargo.toml`, wrote 3 integration tests in `system_tests.rs`, and bumped the crate patch version from 0.1.18 to 0.1.19. All 267 workspace tests pass (including the 3 new system versions tests), all 4 platform cross-checks pass, clippy exits clean, and format gates pass.

## Resolved Dependencies

| Type   | Name                  | Version resolved | Source         |
|--------|-----------------------|------------------|----------------|
| crate  | rustc_version_runtime | 0.3.0            | rust-docs MCP  |

The `rustc_version_runtime` crate at v0.3.0 exposes a `version()` function returning `&'static str` — the SemVer version of the `rustc` compiler used to build the binary. No feature flags are defined. Transitive dependencies `rustc_version ^0.4.1` and `semver ^1.0.28` were added to the lockfile automatically.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add `rustc_version_runtime = "0.3.0"` dependency; bump patch version 0.1.18 → 0.1.19 |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Add `ComponentVersions` struct (pub(crate)) and `get_system_versions()` handler (pub(crate) async fn); update module doc comment |
| Modify | `crates/anvilml-server/src/lib.rs` | Register `GET /v1/system/versions` route in `build_router()` after the `/v1/system/env` route |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Add 3 new integration tests: `test_get_system_versions_returns_200`, `test_get_system_versions_reflects_env_report_values`, `test_get_system_versions_null_when_env_report_unset`; update module doc comment |
| Modify | `docs/TESTS.md` | Add 3 new test entries for the versions endpoint tests |
| Modify | `Cargo.lock` | Updated by cargo with 3 new dependency entries (rustc_version_runtime, rustc_version, semver) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 ++--
 Cargo.lock                                   |  28 ++++++-
 crates/anvilml-server/Cargo.toml             |   3 +-
 crates/anvilml-server/src/handlers/system.rs |  57 +++++++++++++-
 crates/anvilml-server/src/lib.rs             |   5 ++
 crates/anvilml-server/tests/system_tests.rs  | 112 ++++++++++++++++++++++++++-
 docs/TESTS.md                                |  36 +++++++++
 8 files changed, 242 insertions(+), 18 deletions(-)
```

## Test Results

```
     Running tests/system_tests.rs (target/debug/deps/system_tests-c3dc63c58bf55ee9)

running 7 tests
test test_get_system_env_returns_200 ... ok
test test_get_system_versions_reflects_env_report_values ... ok
test test_get_system_versions_null_when_env_report_unset ... ok
test test_get_system_versions_returns_200 ... ok
test test_get_system_returns_200 ... ok
test test_get_system_reflects_hardware_update ... ok
test test_get_system_env_reflects_env_report_update ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

Full workspace test suite: 267 tests passed, 0 failed across all crates (anvilml, anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-artifacts, anvilml-worker, anvilml-scheduler, anvilml-server, anvilml-openapi).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.96s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.92s

# 3. Real-hardware Linux
cargo check --bin anvilml
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.82s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  → Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.43s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  → test tests::config_reference_matches_defaults ... ok
  → test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out
```

Gate 2 (OpenAPI Drift) was not triggered — no handler function signatures, `#[utoipa::path]` annotations, `ToSchema` derives, or `AppState` fields were modified. This task only adds a new route and response struct.

## Public API Delta

```
+pub(crate) struct ComponentVersions {
+    pub(crate) anvilml_version: String,
+    pub(crate) rust_version: String,
+    pub(crate) python_version: Option<String>,
+    pub(crate) torch_version: Option<String>,
+pub(crate) async fn get_system_versions(State(state): State<AppState>) -> Json<ComponentVersions> {
```

All new items are `pub(crate)` (not `pub`), matching the plan's declared API surface. No public items outside the crate were introduced.

## Deviations from Plan

None. All implementation steps were executed exactly as specified in the approved plan. The `rustc_version_runtime` crate version 0.3.0 was confirmed via rust-docs MCP and matches the plan's specification. The `version()` function exists and returns `&'static str` as expected.

## Blockers

None.
