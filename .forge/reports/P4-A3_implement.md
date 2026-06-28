# Implementation Report: P4-A3

| Field         | Value                                                |
|---------------|------------------------------------------------------|
| Task ID       | P4-A3                                                |
| Phase         | 004 — Hardware Detection: Detectors                  |
| Description   | anvilml-hardware: MockDetector env-var driven stub   |
| Implemented   | 2026-06-28T23:45:00Z                                 |
| Status        | COMPLETE                                             |

## Summary

Created `MockDetector`, a zero-sized struct implementing `DeviceDetector` gated behind
the `mock-hardware` feature flag. It reads three environment variables (`ANVILML_MOCK_DEVICE_TYPE`,
`ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_DEVICE_NAME`) with defaults `"cpu"`, `8192`, and
`"Mock GPU"` respectively, and returns one synthetic `GpuDevice`. Six integration tests
cover default behavior, device type overrides, VRAM override, device name override, and
`refresh_vram`. All 82 workspace tests pass (6 new + 76 existing).

## Resolved Dependencies

None. This task introduces no new external crates. It uses types already in `anvilml-core`
(`GpuDevice`, `DeviceType`, `EnumerationSource`, `CapabilitySource`, `InferenceCaps`,
`AnvilError`) and standard library (`std::env`). The `serial_test` dev-dependency
(version `3.5.0`) matches the version already used in `anvilml-core`.

| Type   | Name       | Version resolved | Source        |
|--------|------------|------------------|---------------|
| crate  | serial_test| 3.5.0            | lockfile      |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-hardware/src/mock.rs` | MockDetector impl, feature-gated, 80 lines |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Add `#[cfg(feature = "mock-hardware")]` gated `pub mod mock;` and `pub use mock::MockDetector;` |
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2; add `serial_test = "3.5.0"` dev-dependency |
| CREATE | `crates/anvilml-hardware/tests/mock_tests.rs` | 6 integration tests with `#[serial]` env-var isolation |
| MODIFY | `docs/TESTS.md` | Append 6 new test catalogue entries |

## Commit Log

```
 .forge/reports/P4-A3_plan.md                | 126 +++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   3 +-
 crates/anvilml-hardware/Cargo.toml          |   5 +-
 crates/anvilml-hardware/src/lib.rs          |   5 +
 crates/anvilml-hardware/src/mock.rs         |  79 +++++++++++
 crates/anvilml-hardware/tests/mock_tests.rs | 208 ++++++++++++++++++++++++++++
 docs/TESTS.md                               |  72 ++++++++++
 9 files changed, 506 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/mock_tests.rs (target/debug/deps/mock_tests-6abc79197520e203)

running 6 tests
test test_mock_detector_defaults ... ok
test test_mock_cuda_device_type ... ok
test test_mock_device_name_override ... ok
test test_mock_refresh_vram ... ok
test test_mock_vram_override ... ok
test test_mock_rocm_device_type ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace result: 82 passed, 0 failed, 0 ignored across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.48s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.78s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.63s
```

All four cross-checks exit 0.

## Project Gates

No gates triggered by this task:
- Gate 1 (Config Surface Sync): no ServerConfig field changes
- Gate 2 (OpenAPI Drift): no handler/ToSchema changes
- Gate 3 (Node Parity): no node type changes
- Gate 4 (Mock/Real Parity Markers): no node execute() or arch load()/sample()/decode() changes

## Public API Delta

```
+pub mod mock;
+pub use mock::MockDetector;
```

Two new public items:
- `pub mod mock` — feature-gated module declaration in `anvilml_hardware`
- `pub use mock::MockDetector` — re-export of `MockDetector` struct gated by `mock-hardware`

The `MockDetector` struct and its `DeviceDetector` trait methods (`detect`, `refresh_vram`)
are not `pub` at the module level — they are reachable via `anvilml_hardware::mock::MockDetector`
and the trait's `impl` block. This matches the plan's Public API Surface table.

## Deviations from Plan

- **Rust 2024 `unsafe` requirement**: `std::env::set_var` and `std::env::remove_var` are
  `unsafe` in Rust 2024 edition. All env-var mutations in the test file are wrapped in
  `unsafe { ... }` blocks. This is a platform/compiler behavior, not a plan deviation.
- **Empty line after doc comment**: The initial mock.rs had a blank line between the
  module-level doc comment and `pub struct MockDetector`, which clippy flagged as
  `empty_line_after_doc_comments`. Removed the blank line to match the `cpu.rs` convention.

## Blockers

None.
