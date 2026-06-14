# Implementation Report: P2-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P2-A1                              |
| Phase         | 002 — Config & Graceful Shutdown   |
| Description   | ServerConfig struct with all fields and Default impl |
| Implemented   | 2026-06-14T12:45:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the `ServerConfig` struct and all 5 nested configuration structs in `crates/anvilml-core/src/config.rs`, with `Default`, `Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`, and `Eq` derives. The `Default` impl for `ServerConfig` uses documented defaults from `ENVIRONMENT.md §4`. All 3 tests pass: default values, serialisation roundtrip, and env-override-compatible values. Updated `lib.rs` to export the new module and re-exports. Created `docs/TESTS.md` with entries for all 3 new tests.

## Resolved Dependencies

No new dependencies introduced. `serde` (1.0.228, workspace) and `serde_json` (1.0.150, workspace) already declared in `crates/anvilml-core/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/config.rs` | ServerConfig + 5 nested structs, manual Default impls, path_as_string helper |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `pub mod config;` and `pub use` re-exports; removed stub function |
| CREATE | `crates/anvilml-core/tests/config_tests.rs` | 3 tests: default values, roundtrip, env-override values |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.0 → 0.1.1 (from workspace inheritance to direct) |
| CREATE | `docs/TESTS.md` | Test catalogue with 3 entries for new tests |

## Commit Log

```
 .forge/reports/P2-A1_plan.md              | 146 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 Cargo.lock                                |   2 +-
 crates/anvilml-core/Cargo.toml            |   2 +-
 crates/anvilml-core/src/config.rs         | 215 ++++++++++++++++++++++++++++++
 crates/anvilml-core/src/lib.rs            |   8 +-
 crates/anvilml-core/tests/config_tests.rs | 165 +++++++++++++++++++++++
 docs/TESTS.md                             |  25 ++++
 9 files changed, 569 insertions(+), 13 deletions(-)
```

## Test Results

```
running 3 tests
test test_default_values ... ok
test test_env_override_values ... ok
test test_serialisation_roundtrip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: all 7 tests pass (3 new config tests + 1 health test + 3 state tests). Zero failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.67s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.48s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s
```

All four cross-checks exit 0.

## Project Gates

Gate 1 (config_reference): Not applicable — `anvilml.toml` does not yet exist and the `config_reference` test is created in Phase 003.
Gate 2 (OpenAPI drift): Not applicable — `backend/openapi.json` does not yet exist.
Gate 3 (Node parity): Not applicable — task does not touch node types or scheduler node registry.

## Public API Delta

```
+pub mod config;
+pub use config::{
```

New pub items in `crates/anvilml-core/src/config.rs`:
- `pub struct ModelDirConfig` — `anvilml_core::config::ModelDirConfig`
- `pub struct GpuSelectionConfig` — `anvilml_core::config::GpuSelectionConfig`
- `pub struct LimitsConfig` — `anvilml_core::config::LimitsConfig`
- `pub struct RocmConfig` — `anvilml_core::config::RocmConfig`
- `pub struct HardwareOverrideConfig` — `anvilml_core::config::HardwareOverrideConfig`
- `pub struct ServerConfig` — `anvilml_core::config::ServerConfig`
- `impl Default for ServerConfig` — `anvilml_core::config::ServerConfig::default()`
- `impl Default for GpuSelectionConfig` — `anvilml_core::config::GpuSelectionConfig::default()`
- `impl Default for LimitsConfig` — `anvilml_core::config::LimitsConfig::default()`
- `impl Default for HardwareOverrideConfig` — `anvilml_core::config::HardwareOverrideConfig::default()`

All match the plan's Public API Surface table.

## Deviations from Plan

- **Manual `Default` impls for nested structs**: The plan specified deriving `Default` on nested structs, but derived `Default` uses `String::default()` = `""` and `u32::default()` = `0`, which conflict with documented defaults (`"auto"`, `100`, `1`, `"cpu"`, `8192`). Implemented `Default` manually for `GpuSelectionConfig`, `LimitsConfig`, and `HardwareOverrideConfig` to produce correct defaults. `ModelDirConfig` and `RocmConfig` retain derived `Default` since their fields use `bool` (default `false`), `Option` (default `None`), and `PathBuf` (default `""`), all of which match the documented defaults.
- **Added `PartialEq, Eq` derives**: The plan did not mention `PartialEq`/`Eq`, but they are required for the `assert_eq!` assertions in the test suite. Added to all 6 structs.
- **`path_as_string` helper module**: The plan assumed `PathBuf` would serialize/deserialize via `#[serde(with = "...")]` but did not specify the module. Created a local `mod path_as_string` in `config.rs` to handle `PathBuf` ↔ `String` conversion for JSON serialisation, avoiding the need for an external dependency.

## Blockers

None.
