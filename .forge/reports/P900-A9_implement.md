# Implementation Report: P900-A9

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P900-A9                                           |
| Phase         | 900 — Spec-Drift & Logging Retrofit               |
| Description   | anvilml-core: fix EnvReport's field shape to match ANVILML_DESIGN.md |
| Implemented   | 2026-06-30T19:15:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Rewrote `EnvReport` in `crates/anvilml-core/src/types/worker.rs` from its 3-field shape (`python_version: String`, `torch_version: Option<String>`, `torch_importable: bool`) to the design-doc-correct 7-field shape (`python_path: Option<String>`, `python_version: Option<String>`, `torch_version: Option<String>`, `provisioning: ProvisioningState`, `preflight_ok: bool`, `reason: Option<String>`, `node_types: Vec<NodeTypeDescriptor>`). Updated the serde roundtrip test in `worker_tests.rs` to exercise all 7 fields with non-default values and verify all 7 JSON field names. Updated the test catalogue in `docs/TESTS.md`. Bumped `anvilml-core` patch version from 0.1.19 to 0.1.20. All workspace tests pass, format/lint/cross-check gates are clean.

## Resolved Dependencies

None. This task rewrites an existing struct's fields using types already present in the same crate (`ProvisioningState` from `worker.rs`, `NodeTypeDescriptor` from `node.rs`). No new external crate or feature flag is introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/worker.rs` | Rewrote `EnvReport` struct: 3 fields → 7 fields; added `use super::node::NodeTypeDescriptor` import; updated doc comments on all 7 fields |
| Modify | `crates/anvilml-core/tests/worker_tests.rs` | Updated `test_env_report_serde_roundtrip` to construct all 7 fields with non-default values; added JSON field assertions for all 7 field names; updated module-level doc comment |
| Modify | `crates/anvilml-core/Cargo.toml` | Bumped patch version 0.1.19 → 0.1.20 |
| Modify | `docs/TESTS.md` | Updated `test_env_report_serde_roundtrip` entry to reflect 7-field shape |

## Commit Log

```
 Cargo.lock                                |  2 +-
 crates/anvilml-core/Cargo.toml            |  2 +-
 crates/anvilml-core/src/types/worker.rs   | 25 ++++++++++++-------
 crates/anvilml-core/tests/worker_tests.rs | 40 ++++++++++++++++++++++++-------
 docs/TESTS.md                             |  8 +++----
 5 files changed, 54 insertions(+), 23 deletions(-)
```

## Test Results

```
     Running tests/worker_tests.rs (target/debug/deps/worker_tests-6e8cdea89cdf0ed0)

running 4 tests
test test_provisioning_state_serde_snake_case ... ok
test test_env_report_serde_roundtrip ... ok
test test_worker_info_construction_and_serde_roundtrip ... ok
test test_worker_status_serde_snake_case ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

all workspace tests (160 total): ok. 160 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.27s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.66s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.10s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
+    pub python_path: Option<String>,
+    pub python_version: Option<String>,
+    pub provisioning: ProvisioningState,
+    pub preflight_ok: bool,
+    pub reason: Option<String>,
+    pub node_types: Vec<NodeTypeDescriptor>,
```

These are the 6 new/changed `pub` fields on `EnvReport` (the struct itself is already `pub`). The old `python_version: String` and `torch_importable: bool` fields are removed and replaced. No new `pub` items (functions, enums, traits) are introduced.

## Deviations from Plan

None. Implementation follows the approved plan exactly.

## Blockers

None.
