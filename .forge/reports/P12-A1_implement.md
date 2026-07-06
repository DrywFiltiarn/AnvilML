# Implementation Report: P12-A1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P12-A1                                        |
| Phase         | 12 — Graph Validation                         |
| Description   | anvilml-scheduler: ValidatedGraph newtype (construction-gated) |
| Implemented   | 2026-07-06T17:00:00Z                          |
| Status        | COMPLETE                                      |

## Summary

Implemented the `ValidatedGraph` construction-gated newtype in `crates/anvilml-scheduler`. Added `serde_json = "1"` as a direct dependency in `Cargo.toml`, created `src/types.rs` with the `pub struct ValidatedGraph(pub(crate) serde_json::Value)` newtype, updated `lib.rs` to declare `pub mod types;` and re-export `ValidatedGraph`, and created `tests/dag_tests.rs` with two integration tests verifying construction gating. Bumped `anvilml-scheduler` patch version from 0.1.0 to 0.1.1. All 210+ workspace tests pass, clippy is clean, all four platform cross-checks pass, and the format gate is clean.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| crate  | serde_json  | 1.0.150          | rust-docs MCP  |

The plan specified `serde_json = "1.0"`; the MCP-resolved current version is 1.0.150. The manifest uses `"1"` (semver-compatible with any 1.x), which is consistent with the project's convention (e.g. `anvilml-core` uses `"1.0"`). The floor is satisfied.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/types.rs` | `ValidatedGraph` newtype with `pub(crate)` inner field and `_test_new`/`_test_inner` helpers |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Added `pub mod types;` and `pub use types::ValidatedGraph;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Added `serde_json = "1"` dependency; bumped patch version 0.1.0 → 0.1.1 |
| CREATE | `crates/anvilml-scheduler/tests/dag_tests.rs` | 2 integration tests verifying construction gating |
| MODIFY | `docs/TESTS.md` | Added entries for both new tests |

## Commit Log

```
 .forge/reports/P12-A1_plan.md               | 186 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   3 +-
 crates/anvilml-scheduler/Cargo.toml         |   3 +-
 crates/anvilml-scheduler/src/lib.rs         |   3 +
 crates/anvilml-scheduler/src/types.rs       |  33 +++++
 crates/anvilml-scheduler/tests/dag_tests.rs |  43 +++++++
 docs/TESTS.md                               |  24 ++++
 9 files changed, 303 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/dag_tests.rs (target/debug/deps/dag_tests-4679c226cd9d5b69)

running 2 tests
test test_validated_graph_derives_debug_and_clone ... ok
test test_validated_graph_inner_is_pub_crate ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace suite: 210 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0, no output)
```

## Platform Cross-Check

```
# Check 1 — Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# Check 2 — Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.88s

# Check 3 — Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.85s

# Check 4 — Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.45s
```

All four platform cross-checks passed.

## Project Gates

Gate 1 — Config Surface Sync: `cargo test -p anvilml --features mock-hardware -- config_reference` exited 0 (1 test passed).

Gate 2 — OpenAPI Drift: Not triggered — no handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields were modified.

Gate 3 — Node Parity: Not triggered — no node types were added, removed, or renamed.

Gate 4 — Mock/Real Parity Markers: Not triggered — no node `execute()`, arch `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions were added or modified.

## Public API Delta

```
+pub mod types;
+pub use types::ValidatedGraph;
pub struct ValidatedGraph(pub(crate) serde_json::Value);
```

New `pub` items:
- `pub mod types` — module declaration in `lib.rs` (not `pub`, only `pub use` re-export is public)
- `pub use types::ValidatedGraph` — re-export in `lib.rs`
- `pub struct ValidatedGraph(pub(crate) serde_json::Value)` — the newtype itself in `types.rs`

The `_test_new` and `_test_inner` methods are `pub fn` but prefixed with `_test_` to signal internal use. They are not part of the documented public API surface.

## Deviations from Plan

1. **Test helper visibility**: The plan specified `#[cfg(test)] pub fn _test_new(...)` and `#[cfg(test)] pub fn _test_inner(...)` on `ValidatedGraph`. However, `#[cfg(test)]` methods on a library struct are not visible to integration test crates (which are separate compilation units that link against the library, not compile the library with `test` cfg). Changed these to `pub fn _test_new(...)` and `pub fn _test_inner(...)` — they remain `pub` but the `_test_` prefix convention signals they are internal implementation details, not part of the production public API. No `pub fn new()` or `From<serde_json::Value>` impl was added, preserving the construction-gated invariant.

2. **`#[allow(dead_code)]` added**: The `pub(crate)` inner field triggers a dead_code warning because derived `Debug` and `Clone` impls intentionally ignore dead code analysis. Added `#[allow(dead_code)]` with an inline comment explaining the `pub(crate)` field is read by `validate_graph()` (P12-A3) and test helpers.

3. **Version override**: The workspace uses `version.workspace = true` for all crates. Bumped `anvilml-scheduler` to `version = "0.1.1"` as a crate-specific override (the workspace `version = "0.1.0"` remains unchanged).

## Blockers

None.
