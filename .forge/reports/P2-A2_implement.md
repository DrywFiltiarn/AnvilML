# Implementation Report: P2-A2

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P2-A2                                           |
| Phase       | 002 — Core Domain Types: Config & Errors        |
| Description | anvilml-core: ServerConfig top-level scalar fields |
| Implemented | 2026-06-26T18:52:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Created `crates/anvilml-core/src/config.rs` defining the `ServerConfig` struct with eight scalar fields (`host`, `port`, `db_path`, `artifact_dir`, `venv_path`, `model_scan_depth`, `max_ipc_payload_mib`, `num_threads`) and a `Default` implementation matching the compiled-in defaults from ENVIRONMENT.md §4. Updated `lib.rs` to export `ServerConfig`. Created 8 unit tests in `config_tests.rs`, one per field, all passing. Updated `docs/TESTS.md` with entries for all 8 new tests. Bumped `anvilml-core` version from 0.1.1 to 0.1.2.

## Resolved Dependencies

No new external dependencies. The `serde` crate with `features = ["derive"]` was already present in `crates/anvilml-core/Cargo.toml`.

| Type   | Name  | Version resolved | Source         | Feature flags confirmed |
|--------|-------|------------------|----------------|------------------------|
| crate  | serde | 1.0 (existing)   | Cargo.lock     | derive                   |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/config.rs` | `ServerConfig` struct with 8 scalar fields, `Default` impl, doc comments |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `mod config;` and `pub use config::ServerConfig;` |
| CREATE | `crates/anvilml-core/tests/config_tests.rs` | 8 tests asserting each scalar field's default value |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Version bump 0.1.1 → 0.1.2 |
| MODIFY | `docs/TESTS.md` | Added 8 test entries for new config tests |

## Commit Log

```
 .forge/reports/P2-A2_plan.md              | 170 ++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +--
 Cargo.lock                                |   2 +-
 crates/anvilml-core/Cargo.toml            |   2 +-
 crates/anvilml-core/src/config.rs         |  43 ++++++++
 crates/anvilml-core/src/lib.rs            |   2 +
 crates/anvilml-core/tests/config_tests.rs |  62 +++++++++++
 docs/TESTS.md                             |  96 +++++++++++++++++
 9 files changed, 385 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/config_tests.rs (target/debug/deps/config_tests-cc689bebf5cd3273)

running 8 tests
test test_artifact_dir_default ... ok
test test_db_path_default ... ok
test test_host_default ... ok
test test_max_ipc_payload_mib_default ... ok
test test_model_scan_depth_default ... ok
test test_port_default ... ok
test test_num_threads_default ... ok
test test_venv_path_default ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 8 new config tests pass. The full workspace test suite (28 tests across all crates) passes with zero failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.35s
--- CHECK 1 OK ---

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.10s
--- CHECK 2 OK ---

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.53s
--- CHECK 3 OK ---

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s
--- CHECK 4 OK ---
```

All four platform cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync (`config_reference`):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.68s
     Running unittests src/lib.rs (target/debug/deps/anvilml-0efb98862752b8fc)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
The `config_reference` test is not yet present in the `anvilml` backend crate (deferred to P2-A7 per the approved plan). Gate 1 passes because no test named `config_reference` exists to fail.

**Gate 2 — OpenAPI Drift:** Not triggered — no handler function signatures, `#[utoipa::path]` annotations, or `ToSchema` derives were modified.

**Gate 3 — Node Parity:** Not triggered — no node types added/removed/renamed.

**Gate 4 — Mock/Real Parity Markers:** Not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions added.

## Public API Delta

```
+pub use config::ServerConfig;
```

New public items:
- `pub struct ServerConfig` — `anvilml_core::ServerConfig` — Top-level config with 8 scalar fields; derives Debug, Clone, Serialize, Deserialize
- `impl Default for ServerConfig` — `anvilml_core::ServerConfig` — Provides compiled-in defaults for all fields
- `ServerConfig::default()` — `anvilml_core::ServerConfig::default()` — Returns `Self` with all eight scalar defaults

Matches the plan's Public API Surface exactly. No unexpected additions or removals.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- All 8 scalar fields implemented with correct types and defaults.
- `Default` impl uses `PathBuf::from()` for path fields as planned.
- `lib.rs` updated with `mod config;` and `pub use config::ServerConfig;` in the correct position.
- 8 tests written (exceeding the ≥4 minimum), each with `///` doc comments.
- `docs/TESTS.md` updated with entries for all 8 new tests.
- No `defers_to` stubs needed — the plan only defines scalar fields; nested tables are not written at all (deferred to P2-A3), so no stub code exists that needs the `// defers_to:` comment marker.
- No dual-mode parity markers needed — this task creates a plain data struct, not a node or arch module function.

## Blockers

None.
