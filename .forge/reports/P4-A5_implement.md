# Implementation Report: P4-A5

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P4-A5                                             |
| Phase         | 004 — Hardware Detection                          |
| Description   | anvilml-hardware: detect_all_devices orchestration function |
| Implemented   | 2026-06-15T12:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented `detect_all_devices` — the full hardware detection pipeline orchestration function for `anvilml-hardware`. The function follows a priority chain: hardware override → mock detection (when `mock-hardware` feature is active) → Vulkan → platform fallbacks (DXGI on Windows, sysfs on Unix) → CPU fallback. Also created `MockDetector` (behind `mock-hardware` feature) that synthesises devices from environment variables, and wrote 9 integration tests covering the mock detector and the full detection pipeline.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | sqlx      | 0.9.0            | Cargo.lock     |
| crate  | tokio     | 1.52.3           | Workspace dep  |

`sqlx 0.9.0` was already declared in `[workspace.dependencies]` with features `runtime-tokio`, `sqlite`, `json`. The `SqlitePool` type is `sqlx::SqlitePool`, stable since 0.7.x. `tokio` was added as a dev-dependency (workspace version) to support `#[tokio::test]` in async test functions.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-hardware/Cargo.toml` | Added `sqlx = { workspace = true }` dependency; added `tokio` dev-dependency; bumped version 0.1.4 → 0.1.5 |
| CREATE | `crates/anvilml-hardware/src/mock.rs` | `MockDetector` struct and `DeviceDetector` impl (behind `mock-hardware` feature) |
| CREATE | `crates/anvilml-hardware/src/detect.rs` | `detect_all_devices()` async function with full priority chain |
| MODIFY | `crates/anvilml-hardware/src/lib.rs` | Added `detect` and `mock` module declarations; added `detect_all_devices` and `MockDetector` re-exports; updated crate doc comment |
| CREATE | `crates/anvilml-hardware/tests/mock_tests.rs` | 9 integration tests for mock detection and `detect_all_devices` pipeline |
| MODIFY | `docs/TESTS.md` | Added 9 test entries for new mock_tests |

## Commit Log

```
 .forge/reports/P4-A5_plan.md                | 177 ++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   4 +-
 crates/anvilml-hardware/Cargo.toml          |   4 +-
 crates/anvilml-hardware/src/detect.rs       | 295 ++++++++++++++++++++++++++
 crates/anvilml-hardware/src/lib.rs          |  11 +
 crates/anvilml-hardware/src/mock.rs         | 138 ++++++++++++
 crates/anvilml-hardware/tests/mock_tests.rs | 318 ++++++++++++++++++++++++++++
 docs/TESTS.md                               |  81 +++++++
 10 files changed, 1036 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/mock_tests.rs (target/debug/deps/mock_tests-cbe21c4a871fe65d)

running 9 tests
test test_detect_all_devices_cpu_fallback ... ok
test test_detect_all_devices_inference_caps_union ... ok
test test_detect_all_devices_hardware_override ... ok
test test_detect_all_devices_mock_cuda ... ok
test_detect_all_devices_returns_ok ... ok
test test_mock_detect_cpu ... ok
test test_mock_detect_cuda ... ok
test test_mock_detect_invalid_type ... ok
test test_mock_detect_rocm ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: 87 tests passed, 0 failed across all crates.
```

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.93s

# Check 2: Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.43s

# Check 3: Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.19s

# Check 4: Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.32s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1: config_reference
cargo test -p anvilml --features mock-hardware -- config_reference
# test config_reference ... ok
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. Gate 2 (openapi drift) not triggered — no handler signatures or ToSchema derives modified. Gate 3 (node parity) not triggered — no node types modified.

## Public API Delta

```
+pub mod detect;
+pub mod mock;
+pub use detect::detect_all_devices;
+pub use mock::MockDetector;
```

New pub items:
- `pub mod detect` — module containing `detect_all_devices` (lib.rs)
- `pub mod mock` — module containing `MockDetector` (lib.rs, behind `mock-hardware`)
- `pub fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool) -> Result<HardwareInfo, AnvilError>` (re-exported from `detect` module at crate root)
- `pub struct MockDetector` (re-exported from `mock` module at crate root, behind `mock-hardware`)

All items match the plan's Public API Surface table.

## Deviations from Plan

1. **`detect_all_devices` placed in `detect.rs` module, not `lib.rs`** — The plan specified adding the function directly to `lib.rs`. However, FORGE_AGENT_RULES §12.3 mandates that `lib.rs` must contain only `pub mod`, `pub use`, and crate-level `//!` doc comments (no implementation code). The function is instead in `crates/anvilml-hardware/src/detect.rs` and re-exported at the crate root via `pub use detect::detect_all_devices;` in `lib.rs`. This preserves the public API surface while respecting the lib.rs discipline rule.

2. **Async test functions use `#[tokio::test]` instead of bare `async fn`** — The `serial_test::serial` macro does not support bare async test functions. Each async test uses `#[tokio::test]` in addition to `#[serial_test::serial]` to enable both serialisation and async test support.

3. **`tokio` added as dev-dependency** — Required for `#[tokio::test]` in the test crate. Added with `features = ["rt-multi-thread", "macros"]` from the workspace declaration.

4. **DB seeding step (h) deferred** — The plan's step h (seed device DB via SQL) was deferred as stated in the plan's "Out of Scope" section. The pool parameter is accepted but not used for actual SQL seeding. A DEBUG log entry records that the pool was passed and devices were detected.

## Blockers

None.
