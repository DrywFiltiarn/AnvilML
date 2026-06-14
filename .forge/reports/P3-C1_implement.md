# Implementation Report: P3-C1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-C1                              |
| Phase         | 003 — Core Domain Types            |
| Description   | anvilml-server: stub GET /v1/system/env returning default EnvReport |
| Implemented   | 2026-06-14T23:45:00Z              |
| Status        | COMPLETE                           |

## Summary

Implemented a stub `GET /v1/system/env` handler in `anvilml-server` that returns `EnvReport::default()` as JSON. Added `Default` derives to the entire type chain (`SlotType` → `SlotDescriptor` → `NodeTypeDescriptor` → `ProvisioningState` → `EnvReport`) in `anvilml-core`, added an `env_report` field to `AppState`, created the `get_env` handler, mounted the route at `/v1/system/env`, and wrote an integration test verifying the endpoint returns HTTP 200 with `preflight_ok: false` and `provisioning: "not_started"`.

## Resolved Dependencies

None — no new external dependencies introduced. All types (`EnvReport`, `ProvisioningState`, `State`, `Json`, `Router`, `ServiceExt::oneshot`) are from existing workspace dependencies confirmed against `Cargo.lock`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/node.rs` | Added `Default` derive to `SlotType` (with `#[default]` on `Any` variant), `SlotDescriptor`, and `NodeTypeDescriptor` — required transitive chain for `EnvReport::default()` |
| Modify | `crates/anvilml-core/src/types/worker.rs` | Added `Default` derive to `EnvReport`; replaced `Default` derive on `ProvisioningState` with manual `impl Default` returning `NotStarted` (per plan §1) |
| Modify | `crates/anvilml-server/src/state.rs` | Added `pub env_report: EnvReport` field to `AppState`; updated `new()` to initialise it with `EnvReport::default()` |
| CREATE | `crates/anvilml-server/src/handlers/system.rs` | New handler module with `pub async fn get_env(State(state): State<AppState>) -> Json<EnvReport>` |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Added `pub mod system;` and `pub use system::get_env;` |
| Modify | `crates/anvilml-server/src/lib.rs` | Mounted `.route("/v1/system/env", get(get_env))` after `/health`; added `pub use handlers::system::get_env;` |
| CREATE | `crates/anvilml-server/tests/system_tests.rs` | Integration test `test_system_env_returns_200_with_default_report` |
| Modify | `crates/anvilml-server/Cargo.toml` | Bumped version `0.1.4` → `0.1.5` |
| Modify | `docs/TESTS.md` | Added 5 test catalogue entries for new and existing tests |

## Commit Log

```
 .forge/reports/P3-C1_plan.md                 | 125 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +--
 Cargo.lock                                   |   2 +-
 crates/anvilml-core/src/types/node.rs        |   7 +-
 crates/anvilml-core/src/types/worker.rs      |  13 ++-
 crates/anvilml-server/Cargo.toml             |   2 +-
 crates/anvilml-server/src/handlers/mod.rs    |   2 +
 crates/anvilml-server/src/handlers/system.rs |  16 ++++
 crates/anvilml-server/src/lib.rs             |   4 +
 crates/anvilml-server/src/state.rs           |  17 +++-
 crates/anvilml-server/tests/system_tests.rs  |  44 ++++++++++
 docs/TESTS.md                                |  42 +++++++++
 13 files changed, 274 insertions(+), 19 deletions(-)
```

## Test Results

```
     Running tests/system_tests.rs (target/debug/deps/system_tests-b2cbfa5463fbef65)

running 1 test
test test_system_env_returns_200_with_default_report ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished at 0.00s
```

Full workspace test suite: 60 tests passed, 0 failed, 0 ignored across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.74s

# 2. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.03s

# 3. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.72s
```

All four cross-checks (mock-hardware Linux, mock-hardware Windows, real-hardware Linux, real-hardware Windows) exited 0.

## Project Gates

**Gate 2 — OpenAPI Drift:**
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.22s
Running `target/debug/anvilml-openapi`
(no diff — git diff --exit-code returned 0)
```

Gate 1 (config_reference) not applicable — task does not modify `ServerConfig`.
Gate 3 (Node Parity) not applicable — task does not modify node types.

## Public API Delta

```
+pub mod system;
+pub use system::get_env;
+pub async fn get_env(State(state): State<AppState>) -> Json<anvilml_core::types::EnvReport> {
+pub use handlers::system::get_env;
    pub env_report: anvilml_core::types::EnvReport,
```

New pub items:
- `pub mod system` — module in `anvilml_server::handlers`
- `pub use system::get_env` — re-export in `anvilml_server::handlers`
- `pub async fn get_env` — handler fn in `anvilml_server::handlers::system`
- `pub use handlers::system::get_env` — re-export in `anvilml_server` crate root
- `pub env_report: EnvReport` — field in `anvilml_server::state::AppState`

All match the plan's Public API Surface table.

## Deviations from Plan

1. **Transitive `Default` chain**: The plan assumed `EnvReport::default()` would work by adding `Default` to `EnvReport` and `ProvisioningState` only. However, `EnvReport.node_types` is `Vec<NodeTypeDescriptor>`, which requires `NodeTypeDescriptor: Default`. `NodeTypeDescriptor` contains `Vec<SlotDescriptor>`, requiring `SlotDescriptor: Default`. `SlotDescriptor` contains `SlotType`, requiring `SlotType: Default`. I added `Default` derive to `SlotType` (with `#[default]` on `Any` variant), `SlotDescriptor`, and `NodeTypeDescriptor` in `crates/anvilml-core/src/types/node.rs` as a transitive requirement.

2. **Rust 2024 `#[default]` attribute**: Rust 2024 requires an explicit `#[default]` attribute on the chosen variant when deriving `Default` on an enum. Added `#[default]` on `SlotType::Any` (the intended default for slots accepting multiple types).

3. **Version bump in Cargo.lock**: `Cargo.lock` was automatically updated by cargo when the `anvilml-server` version changed from `0.1.4` to `0.1.5`. This is expected behavior — no manual lockfile editing was performed.

## Blockers

None.
