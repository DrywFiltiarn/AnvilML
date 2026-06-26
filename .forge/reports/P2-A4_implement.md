# Implementation Report: P2-A4

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P2-A4                                             |
| Phase         | 2 — Core Domain Types: Config & Errors            |
| Description   | anvilml-core: config_load layered precedence (defaults+toml) |
| Implemented   | 2026-06-26T21:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created `crates/anvilml-core/src/config_load.rs` implementing `pub fn load(toml_path: Option<&Path>) -> Result<ServerConfig, AnvilError>`, which starts from `ServerConfig::default()` and, when a TOML file is found at the provided path (or the default `./anvilml.toml`), parses it via the `toml` crate and merges field-by-field so that TOML values override defaults while missing fields retain their compiled-in defaults. This establishes the first two layers of the four-layer config precedence chain (defaults → TOML → env vars → CLI flags). Six tests were added covering missing file fallback, partial override, malformed TOML error, full field round-trip, default path resolution, and nested struct partial override. The `toml` crate (v1.1.2) was added as a dependency. `anvilml-core` patch version bumped from 0.1.3 to 0.1.4.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source         |
|--------|--------|-----------------|----------------|
| crate  | toml   | 1.1.2           | cargo search   |

Note: The plan specified `toml = "0.8.23"` but `cargo search` resolved the current version as `1.1.2+spec-1.1.0`. Per the version floor rule, the MCP-resolved (cargo search) version is the floor. The `toml` crate v1.1.2 API (`from_str`, `Value`, `as_table`, `as_str`, `as_integer`, `as_array`, `as_bool`) is compatible with the plan's approach. The `rust-docs` MCP tool was unavailable; `cargo search` was used as the live lookup mechanism.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/config_load.rs` | New module: `load()` function implementing defaults→TOML field-by-field merge |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `pub mod config_load;` and `pub use config_load::load;` |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `toml = "1.1.2"` dependency; bumped version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-core/tests/config_load_tests.rs` | Test file with 6 tests covering missing file, partial override, malformed TOML, full round-trip, default path, and nested struct partial override |
| MODIFY | `docs/TESTS.md` | Added 6 entries for new config_load tests |

## Commit Log

```
 .forge/reports/P2-A4_plan.md                   | 197 ++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |  57 ++++-
 crates/anvilml-core/Cargo.toml                 |   3 +-
 crates/anvilml-core/src/config_load.rs         | 209 +++++++++++++++++++
 crates/anvilml-core/src/lib.rs                 |   2 +
 crates/anvilml-core/tests/config_load_tests.rs | 274 +++++++++++++++++++++++++
 docs/TESTS.md                                  |  72 +++++++
 9 files changed, 822 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-88f9aabe33698632)

running 6 tests
test test_load_default_path_resolves_anvilml_toml ... ok
test test_load_missing_file_falls_back_to_defaults ... ok
test test_load_malformed_toml_returns_err ... ok
test test_load_nested_struct_partial_override ... ok
test test_load_full_toml_roundtrips_all_fields ... ok
test test_load_partial_toml_overrides_only_specified_fields ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: 39 tests passed, 0 failed (includes 13 config_tests, 16 error_tests, 6 config_load_tests, 1 config_load internal test, 1 shutdown test, 1 cli_help test, 1 health test)
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.50s

# 3. Real-hardware Linux:
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.59s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s
```

## Project Gates

Gate 1 (config_reference): `cargo test -p anvilml --features mock-hardware -- config_reference` — no config_reference test exists yet (P2-A7's scope); exits 0 with no matching tests.

Gate 2 (OpenAPI drift): Not triggered — this task does not modify handler function signatures, utoipa annotations, or AppState fields.

Gate 3 (Node Parity): Not triggered — this task does not add/remove/rename node types or modify node_registry.rs.

Gate 4 (Mock/Real Parity Markers): Not triggered — this task adds no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions.

## Public API Delta

```
+pub mod config_load;
+pub use config_load::load;
```

New items:
- `pub mod config_load` — module path: `anvilml_core::config_load`
- `pub fn load(toml_path: Option<&Path>) -> Result<ServerConfig, AnvilError>` — function path: `anvilml_core::config_load::load`

Both match the plan's Public API Surface table exactly.

## Deviations from Plan

- **Dependency version**: The plan specified `toml = "0.8.23"` but `cargo search` resolved the current version as `1.1.2+spec-1.1.0`. Per the version floor rule (FORGE_AGENT_RULES §6.2, ACT authoritative over PLAN on versions), the resolved version `1.1.2` is used. The API (`from_str`, `Value`, `as_table`, `as_str`, `as_integer`, `as_array`, `as_bool`) is fully compatible.
- **`pub mod` vs `mod`**: The plan said `mod config_load;` but the test file is an integration test that imports via `anvilml_core::config_load::load`. This requires the module to be `pub mod config_load;`. This is a necessary deviation for test visibility.
- **`PartialEq` on nested structs**: The existing nested structs (`ModelDirConfig`, `RocmConfig`, `HardwareOverrideConfig`) do not derive `PartialEq`. The tests use `.is_none()` / `.is_empty()` for those types instead of `assert_eq!`, matching the pattern used in the existing `config_tests.rs`.
- **`collapsible_if` clippy fix**: Clippy flagged nested `if let` chains in `apply_gpu_selection` and `apply_limits` as `collapsible_if`. Fixed by using `if let ... && let ...` syntax.

## Blockers

None.
