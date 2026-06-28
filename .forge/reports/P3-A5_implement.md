# Implementation Report: P3-A5

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P3-A5                           |
| Phase         | 003 — Core Domain Types: Data Model |
| Description   | anvilml-core: InferenceCaps, EnumerationSource, CapabilitySource |
| Implemented   | 2026-06-28T17:35:00Z            |
| Status        | COMPLETE                        |

## Summary

Verified that `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` types in `hardware.rs` already match the design spec (6 bool fields, 7 variants with Cpu as 5th, 3 variants, all with correct derives and serde annotations). Added two new tests to `hardware_tests.rs`: `test_inference_caps_non_default_roundtrip` (test 8) and `test_enumeration_source_copy_trait` (test 9), bringing the total to 9 tests. Updated `docs/TESTS.md` with entries for both new tests. Bumped `anvilml-core` patch version from 0.1.9 to 0.1.10. All gates pass: compile, clippy, format, platform cross-check, config_reference, and 9/9 hardware tests.

## Resolved Dependencies

None. This task adds no new dependencies — only tests for existing types.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/tests/hardware_tests.rs` | Added 2 new tests: `test_inference_caps_non_default_roundtrip` and `test_enumeration_source_copy_trait` |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bumped patch version 0.1.9 → 0.1.10 |
| MODIFY | `docs/TESTS.md` | Added entries for the 2 new tests with Mode, context, inputs, expected outputs |

## Commit Log

```
 .forge/reports/P3-A5_plan.md                | 105 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 ++--
 Cargo.lock                                  |   2 +-
 crates/anvilml-core/Cargo.toml              |   2 +-
 crates/anvilml-core/tests/hardware_tests.rs |  68 ++++++++++++++++++
 docs/TESTS.md                               |  24 +++++++
 7 files changed, 209 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-64934888ffd02911)

running 9 tests
test test_capability_source_serde_snake_case ... ok
test test_device_type_serde_snake_case ... ok
test test_enumeration_source_serde_snake_case ... ok
test test_enumeration_source_copy_trait ... ok
test test_host_info_serde_roundtrip ... ok
test test_gpu_device_construction_and_serde ... ok
test test_hardware_info_serde_roundtrip ... ok
test test_inference_caps_default_roundtrip ... ok
test test_inference_caps_non_default_roundtrip ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.54s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.03s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.69s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.67s
```

All four checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
     Running tests/config_reference.rs
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes. No other gates triggered (task does not modify handler signatures, node types, or config surface).

## Public API Delta

```
(no new pub items — task only adds tests, no source changes)
```

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- Step 1: Verified existing types match spec — confirmed all derives, field names, variant names, doc comments, and serde attributes are correct.
- Step 2: Added `test_inference_caps_non_default_roundtrip` with mixed true/false values and JSON field name verification.
- Step 3: Added `test_enumeration_source_copy_trait` verifying Copy for both enums.
- Step 4: Acceptance command `cargo test -p anvilml-core --test hardware_tests` exits 0 with 9 tests.
- Step 5: Updated `docs/TESTS.md` with entries for both new tests.

## Blockers

None.
