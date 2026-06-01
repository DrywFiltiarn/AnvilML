# Implementation Report: P3-A4

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P3-A4                                        |
| Phase          | 003 — Core Domain Types                      |
| Description    | anvilml-core: Hardware and Worker domain types|
| Project        | anvilml                                      |
| Implemented at | 2026-06-01T12:35:23Z                        |
| Attempt        | 1                                            |

## Summary

Implemented the hardware and worker domain type modules in `anvilml-core`, defining all structs and enums specified in ANVILML_DESIGN §4.3 (HardwareInfo, GpuDevice, DeviceType, HostInfo, InferenceCaps) and §4.4 (WorkerInfo, WorkerStatus), plus the EnvReport struct used by the preflight system (§6.1). All types are pure, serializable data types with no I/O or async logic, following the same derive conventions (Serialize, Deserialize, Clone, Debug, ToSchema) established in P3-A2 and P3-A3. DeviceType is re-exported from config.rs to avoid duplication.

## Files Changed

| Action   | Path                              | Description                                     |
|----------|-----------------------------------|-------------------------------------------------|
| CREATE   | crates/anvilml-core/src/types/hardware.rs | Hardware domain types (§4.3): HardwareInfo, GpuDevice, HostInfo, InferenceCaps, DeviceType re-export |
| CREATE   | crates/anvilml-core/src/types/worker.rs   | Worker domain types (§4.4, §6.1): WorkerInfo, WorkerStatus, EnvReport |
| MODIFY   | crates/anvilml-core/src/types/mod.rs      | Register hardware and worker modules            |
| MODIFY   | crates/anvilml-core/src/lib.rs            | Re-export new types for downstream crates       |

## Test Results

### Hardware tests (8 passed)

```
running 8 tests
test types::hardware::tests::device_type_json_strings ... ok
test types::hardware::tests::device_type_variants ... ok
test types::hardware::tests::gpu_device_roundtrip ... ok
test types::hardware::tests::hardware_info_empty_gpus ... ok
test types::hardware::tests::hardware_info_roundtrip ... ok
test types::hardware::tests::host_info_roundtrip ... ok
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::inference_caps_roundtrip ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 47 filtered out
```

### Worker tests (9 passed)

```
running 9 tests
test types::worker::tests::env_report_defaults ... ok
test types::worker::tests::env_report_failure ... ok
test types::worker::tests::env_report_minimal_parse ... ok
test types::worker::tests::env_report_roundtrip ... ok
test types::worker::tests::worker_info_idle ... ok
test types::worker::tests::worker_info_optional_defaults ... ok
test types::worker::tests::worker_info_roundtrip ... ok
test types::worker::tests::worker_status_json_strings ... ok
test types::worker::tests::worker_status_variants ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out
```

### Full workspace test suite (64 tests, all passed)

```
cargo test --workspace --features mock-hardware
running 55 tests in anvilml-core — ok. 55 passed
test result: ok. 55 passed; 0 failed
test result: ok. 0 passed (anvilml-hardware)
test result: ok. 0 passed (anvilml-ipc)
test result: ok. 0 passed (anvilml-openapi)
test result: ok. 0 passed (anvilml-registry)
test result: ok. 0 passed (anvilml-scheduler)
test result: ok. 1 passed (anvilml-server)
test result: ok. 0 passed (anvilml-worker)
test result: ok. 8 passed (backend)
doc-tests: all ok
```

### Clippy (zero warnings)

```
cargo clippy --workspace --features mock-hardware -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.27s
```

### Windows cross-check (x86_64-pc-windows-gnu)

```
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.71s
```

### Config drift gate

The `config_reference` test does not yet exist (added in P3-B2); gate skipped per instructions.

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P3-A4_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  crates/anvilml-core/src/lib.rs
A  crates/anvilml-core/src/types/hardware.rs
M  crates/anvilml-core/src/types/mod.rs
A  crates/anvilml-core/src/types/worker.rs
```

## Acceptance Criteria — Verification

| Criterion                                      | Status | Evidence                          |
|------------------------------------------------|--------|-----------------------------------|
| `hardware.rs` created with all §4.3 types       | PASS   | File exists, compiles             |
| `worker.rs` created with §4.4 + §6.1 types      | PASS   | File exists, compiles             |
| `types/mod.rs` registers both modules           | PASS   | File modified, exports present    |
| `lib.rs` re-exports new types                   | PASS   | File modified, re-exports present |
| Unit tests: JSON round-trip                    | PASS   | hardware + worker roundtrip tests |
| Unit tests: default impl                       | PASS   | inference_caps_defaults, env_report_defaults |
| Unit tests: variant completeness               | PASS   | device_type_variants (3), worker_status_variants (5) |
| `cargo fmt --all` passes                        | PASS   | Formatted in-place                |
| `cargo clippy --workspace --features mock-hardware -- -D warnings` | PASS | Zero warnings |
| `cargo check --target x86_64-pc-windows-gnu`    | PASS | Zero errors                       |
| `cargo test --workspace --features mock-hardware` | PASS  | 64 tests passed, 0 failed         |
| `cargo test -p anvilml-core -- hardware` exits 0 | PASS   | 8 tests passed                    |
