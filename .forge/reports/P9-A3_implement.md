# Implementation Report: P9-A3

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P9-A3                                           |
| Phase       | 009 — Worker Spawn & Handshake                  |
| Description | anvilml-worker: env.rs build_worker_env         |
| Implemented | 2026-06-06T12:00:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Created `crates/anvilml-worker/src/env.rs` with the `build_worker_env(device, cfg)` function that produces a `HashMap<String, String>` of environment variables for Python worker child processes. The function handles all three device types (CUDA, ROCm, CPU), sets platform-appropriate device isolation vars, ROCm-specific performance flags with Unix-only cfg-gating for HSA_OVERRIDE_GFX_VERSION, universal threading variables, worker identity variables, and mock-mode propagation. Five unit tests cover CUDA, ROCm Linux with HSA, ROCm Windows without HSA, CPU, and mock propagation scenarios.

## Resolved Dependencies

| Type   | Name           | Version resolved | Source         |
|--------|---------------|-----------------|----------------|
| path   | anvilml-core  | (workspace)     | Cargo.toml     |

No new external dependencies added — `anvilml-core` was already present in `crates/anvilml-worker/Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-worker/src/env.rs` | New module: `build_worker_env` function + 5 unit tests |
| Modify | `crates/anvilml-worker/src/lib.rs` | Replace stub with `pub mod env; pub use env::build_worker_env;` |

## Commit Log

```
 .forge/reports/P9-A3_plan.md     | 109 +++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 crates/anvilml-worker/src/env.rs | 342 +++++++++++++++++++++++++++++++++++++++
 crates/anvilml-worker/src/lib.rs |   4 +-
 5 files changed, 464 insertions(+), 10 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-5279b6e9927ec3ad)

running 5 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 214 tests, 0 failures across all crates (anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, backend).

## Format Gate

```
(no output — exit 0)
```

Formatter check (`cargo fmt --all -- --check`) passed with zero drift.

## Platform Cross-Check

All four platform cross-checks pass:

1. **Mock-hardware Linux:** `cargo check --workspace --features mock-hardware` → Finished in 0.84s
2. **Mock-hardware Windows:** `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` → Finished in 1.93s
3. **Real-hardware Linux:** `cargo check --bin anvilml` → Finished in 0.76s
4. **Real-hardware Windows:** `cargo check --bin anvilml --target x86_64-pc-windows-gnu` → Finished in 1.09s

## Project Gates

Gate 1 — Config Surface Sync:
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-dc805b141a934a81)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

Config drift gate passes. No fields were added to `ServerConfig` or nested structs, so the config file does not need updating.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
