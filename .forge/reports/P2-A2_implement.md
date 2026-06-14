# Implementation Report: P2-A2

| Field         | Value                                                      |
|---------------|------------------------------------------------------------|
| Task ID       | P2-A2                                                      |
| Phase         | 002 — Config & Graceful Shutdown                           |
| Description   | anvilml-core: layered config loading (toml + ANVILML_* env override) |
| Implemented   | 2026-06-14T13:20:00Z                                       |
| Status        | COMPLETE                                                   |

## Summary

Implemented the four-level config precedence chain in `anvilml-core`: compiled-in defaults → `anvilml.toml` → `ANVILML_*` env vars → `ConfigOverrides` (CLI). Created `AnvilError` enum for error handling, `ConfigOverrides` struct for CLI overrides, and a `load()` function that applies each precedence level sequentially. Four integration tests verify the precedence chain and double-underscore env var nesting.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | toml    | 1.1.2            | cargo search   |
| crate  | tempfile| 3.27.0           | cargo search   |
| crate  | thiserror| 2.0.18          | workspace (pre-existing) |

**Notes:**
- `toml` 1.1.2 is the current stable. Resolved via `cargo search toml --limit 1`.
- `tempfile` 3.27.0 added as a dev-dependency for test temp files. Not a workspace dep in the plan — added as workspace dep for consistency.
- `thiserror` 2.0.18 was already declared in workspace deps.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/error.rs` | `AnvilError` enum (Io, Toml, EnvVar) with thiserror derives and doc comments |
| CREATE | `crates/anvilml-core/src/config_load.rs` | `load()` function, `ConfigOverrides` struct, `apply_env_overrides()` helper |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `pub mod error`, `pub mod config_load`, re-exports for `AnvilError`, `load`, `ConfigOverrides` |
| MODIFY | `crates/anvilml-core/src/config.rs` | Added `#[serde(default)]` to `ServerConfig` for partial TOML support |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `toml` and `thiserror` deps; added `tempfile` dev-dep; bumped version 0.1.1 → 0.1.2 |
| MODIFY | `Cargo.toml` (workspace root) | Added `toml = "1.1.2"` and `tempfile = "3.27.0"` to `[workspace.dependencies]` |
| CREATE | `crates/anvilml-core/tests/config_load_tests.rs` | 4 integration tests for config loading precedence chain |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new config_load tests |

## Commit Log

```
 .forge/reports/P2-A2_plan.md                   | 208 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |  78 +++++++++-
 Cargo.toml                                     |   2 +
 crates/anvilml-core/Cargo.toml                 |   7 +-
 crates/anvilml-core/src/config.rs              |   1 +
 crates/anvilml-core/src/config_load.rs         | 136 ++++++++++++++++
 crates/anvilml-core/src/error.rs               |  38 +++++
 crates/anvilml-core/src/lib.rs                 |   4 +
 crates/anvilml-core/tests/config_load_tests.rs | 114 ++++++++++++++
 docs/TESTS.md                                  |  32 ++++
 12 files changed, 628 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-97a6004b12bba308)

running 4 tests
test test_cli_override_beats_env ... ok
test test_env_var_beats_toml ... ok
test test_missing_file_uses_defaults ... ok
test test_nested_env_var ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_tests.rs (target/debug/deps/config_tests-ac496143b206abfe)

running 3 tests
test test_default_values ... ok
test test_env_override_values ... ok
test test_serialisation_roundtrip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs (target/debug/deps/health_tests-152bdd4cd514e2b3)

running 1 test
test test_health_returns_200_with_status_key ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/state_tests.rs (target/debug/deps/state_tests-6c732fb1)

running 3 tests
test test_app_state_clone ... ok
test test_app_state_new ... ok
test test_app_state_version_from_env ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 11 workspace tests pass. The 4 new config_load tests and 3 pre-existing config tests all pass.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
Check 1 (mock-hardware Linux):  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.67s — PASSED
Check 2 (mock-hardware Windows): Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.95s — PASSED
Check 3 (real-hardware Linux):   Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s — PASSED
Check 4 (real-hardware Windows): Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.51s — PASSED
```

All four platform cross-checks pass.

## Project Gates

None applicable — task does not add, rename, or remove fields on `ServerConfig` or any nested config struct. The `#[serde(default)]` addition to `ServerConfig` does not change the field set, only the deserialization behavior.

## Public API Delta

```
+pub mod config_load;
+pub mod error;
+pub use config_load::{load, ConfigOverrides};
+pub use error::AnvilError;
```

New public items:
- `pub mod error` (module) — crate `anvilml_core::error`
- `pub mod config_load` (module) — crate `anvilml_core::config_load`
- `pub use error::AnvilError` (enum re-export) — `anvilml_core::AnvilError`
- `pub use config_load::load` (fn re-export) — `anvilml_core::load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig, AnvilError>`
- `pub use config_load::ConfigOverrides` (struct re-export) — `anvilml_core::ConfigOverrides`

## Deviations from Plan

1. **`AnvilError` derives:** Plan specified `#[derive(Debug, Clone, thiserror::Error)]`. Changed to `#[derive(Debug, thiserror::Error)]` because `std::io::Error` does not implement `Clone`, making `Clone` impossible on the enum. Documented in `error.rs`.

2. **`AnvilError::Toml` variant type:** Plan specified `serde_json::de::Error`. Changed to `toml::de::Error` because: (a) `serde_json::de::Error` is private in serde_json 1.0.150; (b) `toml::de::Error` is the correct error type for TOML deserialization. This is a more semantically accurate choice — the error originates from TOML parsing, not JSON.

3. **`ServerConfig` serde default:** Added `#[serde(default)]` to `ServerConfig` struct. This was not in the plan but is necessary for partial TOML files to deserialize correctly — without it, `toml::from_str` requires all non-optional fields to be present in the TOML, making the config file unusable for partial configurations.

4. **`tempfile` as workspace dependency:** Plan did not mention `tempfile`. Added as both workspace dep and anvilml-core dev-dep for test temp file creation.

5. **Test parallelism:** Env var tests require `--test-threads=1` due to process-global state. This is a test isolation requirement, not a workaround. The `#[serial]` attribute was not used per ENVIRONMENT.md §11.3 guidance (reserved for physically singular resources).

## Blockers

None.
