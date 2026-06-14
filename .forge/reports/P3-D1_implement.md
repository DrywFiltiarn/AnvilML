# Implementation Report: P3-D1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-D1                              |
| Phase         | 003 — Core Domain Types            |
| Description   | backend: config_reference drift guard integration test |
| Implemented   | 2026-06-15T00:00:00Z               |
| Status        | COMPLETE                           |

## Summary

Created the `config_reference` integration test (`backend/tests/config_reference.rs`) and the checked-in reference config (`anvilml.toml` at repo root) that together form the config drift guard (Gate 1). The test serialises `ServerConfig::default()` to a TOML string via `toml::to_string_pretty`, parses both that string and the `anvilml.toml` file content into `toml::Value`, recursively compares their key sets, and fails if any key is present in one but absent in the other. Added `toml` as a dev-dependency of `backend` and bumped the `backend` crate version from `0.1.5` to `0.1.6`. All 56 workspace tests pass, all 4 platform cross-checks pass, both clippy passes pass, and both format passes pass.

## Resolved Dependencies

| Type   | Name   | Version resolved | Source          |
|--------|--------|-----------------|-----------------|
| crate  | toml   | 1.1.2           | Cargo.lock fallback (MCP rust-docs unavailable) |

Note: `toml = "1.1.2"` was already declared in `[workspace.dependencies]` of the root `Cargo.toml`. The Cargo.lock confirms version `1.1.2+spec-1.1.0`. The `toml` crate v1.1.2 implements TOML spec 1.1.0. During implementation it was discovered that `toml::to_string_pretty` omits `Option<T>` fields with `None` values entirely (rather than serialising them as the TOML `null` literal), and the `toml` parser v1.1.2 rejects `null` literals in TOML files with "invalid float, expected `nan`". The `anvilml.toml` file was therefore written to match the actual serialized output of `ServerConfig::default()`, which omits `num_threads`, `rocm`, and `hardware_override` keys.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `anvilml.toml` | Reference config with all keys at documented defaults |
| CREATE | `backend/tests/config_reference.rs` | Integration test: config drift guard comparing key sets |
| Modify | `backend/Cargo.toml` | Add `[dev-dependencies]` with `toml = { workspace = true }`; bump version 0.1.5 → 0.1.6 |
| Modify | `docs/TESTS.md` | Add entry for `test_config_reference (anvilml)` |

## Commit Log

```
 .forge/reports/P3-D1_plan.md      | 124 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 ++--
 Cargo.lock                        |   3 +-
 anvilml.toml                      |  15 +++++
 backend/Cargo.toml                |   5 +-
 backend/tests/config_reference.rs |  79 ++++++++++++++++++++++++
 docs/TESTS.md                     |   9 +++
 8 files changed, 243 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/config_reference.rs (target/debug/deps/config_reference-fa0279e836915a9c)

running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Full workspace test suite (cargo test --workspace --features mock-hardware):

     Running tests/cli_tests.rs ... 1 passed; 0 failed
     Running tests/config_reference.rs ... 1 passed; 0 failed
     Running tests/artifact_tests.rs ... 3 passed; 0 failed
     Running tests/config_load_tests.rs ... 4 passed; 0 failed
     Running tests/config_tests.rs ... 3 passed; 0 failed
     Running tests/error_tests.rs ... 17 passed; 0 failed
     Running tests/events_tests.rs ... 4 passed; 0 failed
     Running tests/hardware_tests.rs ... 4 passed; 0 failed
     Running tests/job_tests.rs ... 5 passed; 0 failed
     Running tests/model_tests.rs ... 3 passed; 0 failed
     Running tests/node_tests.rs ... 3 passed; 0 failed
     Running tests/worker_tests.rs ... 3 passed; 0 failed
     Running tests/health_tests.rs ... 1 passed; 0 failed
     Running tests/state_tests.rs ... 3 passed; 0 failed
     Running tests/system_tests.rs ... 1 passed; 0 failed
     Doc-tests (8 crates) ... 0 passed; 0 failed

Total: 56 tests passed; 0 failed
```

## Format Gate

```
cargo fmt --all -- --check
(exit 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.39s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Gate 2 (OpenAPI Drift) — not applicable: task does not modify handler signatures, ToSchema derives, or AppState fields.
Gate 3 (Node Parity) — not applicable: task does not add/remove/renames node types.
```

## Public API Delta

```
(no output — no new pub items introduced)
```

No new `pub` items introduced. The test file and `anvilml.toml` use only private functions and the public API of `anvilml-core`'s `config` module.

## Deviations from Plan

1. **`anvilml.toml` does not include `null` values for `Option<T>` fields.** The approved plan specified that `rocm = null` and `hardware_override = null` should be present in `anvilml.toml` because "serde serialises `Option<T>` with `None` as `null` in TOML." This was incorrect — `toml::to_string_pretty` on `ServerConfig::default()` omits `Option<T>` fields with `None` values entirely (no key is produced). The `num_threads`, `rocm`, and `hardware_override` keys are absent from the serialized output. The `anvilml.toml` file was therefore written without these keys to match the actual serialized output.

2. **`toml` crate v1.1.2 rejects `null` literals in TOML files.** During implementation, attempts to write `rocm = null` and `hardware_override = null` in the `anvilml.toml` file caused parse errors: "invalid float, expected `nan`". The `toml` crate v1.1.2 (implementing TOML spec 1.1.0) appears to have a parsing issue where `null` literals are misinterpreted as float literals. This confirmed that the `null` approach in the plan would not work.

3. **`num_threads` is also absent from the serialized output.** The plan only mentioned `rocm` and `hardware_override` as `Option` fields needing `null` values, but `num_threads: Option<usize>` is also `None` in `ServerConfig::default()` and is similarly omitted.

## Blockers

None.
