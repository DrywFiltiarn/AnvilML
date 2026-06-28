# Implementation Report: P3-A2

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P3-A2                                             |
| Phase         | 003 — Core Domain Types: Data Model               |
| Description   | anvilml-core: ModelMeta, ModelKind, ModelDtype, ModelFormat |
| Implemented   | 2026-06-28T16:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created the model metadata types (`ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat`)
in `crates/anvilml-core/src/types/model.rs`, declared the `model` submodule in
`types/mod.rs` with both `pub mod model;` and `pub use model::*;` (matching the existing
`job` module pattern), added 4 integration tests in `crates/anvilml-core/tests/model_tests.rs`
verifying `snake_case` JSON serialisation and `PathBuf` roundtrip, updated `docs/TESTS.md`
with test catalogue entries, and bumped `anvilml-core` patch version from `0.1.6` to `0.1.7`.

## Resolved Dependencies

None. No new external crates are introduced. `chrono` (with `serde` feature), `serde` (with
`derive` feature), and `serde_json` are already declared in `anvilml-core/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/model.rs` | `ModelMeta` struct, `ModelKind`/`ModelDtype`/`ModelFormat` enums with doc comments |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod model;` and `pub use model::*;` |
| CREATE | `crates/anvilml-core/tests/model_tests.rs` | 4 integration tests for serde roundtrips |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bumped patch version 0.1.6 → 0.1.7 |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries |

## Commit Log

```
 .forge/reports/P3-A2_plan.md             | 246 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +-
 Cargo.lock                               |   2 +-
 crates/anvilml-core/Cargo.toml           |   2 +-
 crates/anvilml-core/src/types/mod.rs     |   2 +
 crates/anvilml-core/src/types/model.rs   |  93 ++++++++++++
 crates/anvilml-core/tests/model_tests.rs | 141 ++++++++++++++++++
 docs/TESTS.md                            |  48 ++++++
 9 files changed, 542 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/model_tests.rs (target/debug/deps/model_tests-f214159192c2d15f)

running 4 tests
test test_model_format_serde_snake_case ... ok
test test_model_kind_serde_snake_case ... ok
test test_model_dtype_serde_snake_case ... ok
test test_model_meta_serde_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 4 new tests pass. The full workspace test suite also passes (0 failures across all
crates).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s
→ CHECK 1 OK

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.03s
→ CHECK 2 OK

# 3. Real-hardware Linux
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.28s
→ CHECK 3 OK

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.99s
→ CHECK 4 OK
```

All four platform cross-checks exit 0.

## Project Gates

Not applicable — this task does not modify `ServerConfig`, handler signatures, node types,
or any of the four gate triggers defined in `ENVIRONMENT.md §8`.

## Public API Delta

```
+pub mod model;
+pub use model::*;
```

New public items in `crates/anvilml-core/src/types/model.rs`:

| Item | Type | Module Path |
|------|------|-------------|
| `ModelMeta` | `pub struct` | `anvilml_core::types::model::ModelMeta` |
| `ModelKind` | `pub enum` | `anvilml_core::types::model::ModelKind` |
| `ModelDtype` | `pub enum` | `anvilml_core::types::model::ModelDtype` |
| `ModelFormat` | `pub enum` | `anvilml_core::types::model::ModelFormat` |

All four items match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

1. **`types/mod.rs` includes `pub use model::*;` in addition to `pub mod model;`.**
   The plan specified only `pub mod model;` but the existing `job` module pattern uses
   both `pub mod job;` and `pub use job::*;`. Without `pub use model::*;`, the test
   file's `use anvilml_core::types::*;` import would not resolve `ModelKind`, `ModelDtype`,
   `ModelFormat`, or `ModelMeta` — they would only be accessible as
   `types::model::ModelMeta`, etc. Added `pub use model::*;` to match the established
   convention and make the tests compile.

2. **`ModelMeta` derives `PartialEq, Eq` in addition to the plan's `Debug, Clone, Serialize, Deserialize`.**
   The test uses `assert_eq!` which requires `PartialEq`. The existing `Job` struct in
   `job.rs` also derives `PartialEq, Eq`, so this matches the crate's established pattern.

3. **Test comparison uses `.as_u64()` instead of a direct integer literal comparison.**
   The literal `6_442_529_280` exceeds `i32` range, causing a compile error with
   `serde_json::json!()`. Used `parsed["size_bytes"].as_u64()` to compare against `Some(6_442_529_280)`
   instead, avoiding the overflow while preserving the same semantic check.

## Blockers

None.
