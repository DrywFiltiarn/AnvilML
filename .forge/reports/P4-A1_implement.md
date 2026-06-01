# Implementation Report: P4-A1

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P4-A1                                       |
| Phase          | 004 — Hardware Detection                    |
| Description    | anvilml-hardware: DeviceDetector trait and CPU detector |
| Project        | anvilml                                     |
| Implemented at | 2026-06-01T16:07:46Z                        |
| Attempt        | 1                                           |

## Summary

Implemented the `anvilml-hardware` crate's foundational abstractions per the approved plan. Added the `sysinfo = "0.32"` dependency to `Cargo.toml`. Defined the `DeviceDetector` trait with `detect()` and `refresh_vram()` methods in `lib.rs`. Created `cpu.rs` implementing `CpuDetector` that returns one synthetic CPU device (`index: 0`, `name: "CPU"`, `device_type: DeviceType::Cpu`, zero VRAM, driver version "n/a"). Added three unit tests for CpuDetector plus a compile-check trait implementation test. Re-exported `AnvilError`, `DeviceType`, and `GpuDevice` from `anvilml_core` for ergonomic downstream use.

## Files Changed

| Action   | Path                              | Description                                            |
|----------|-----------------------------------|--------------------------------------------------------|
| MODIFY   | crates/anvilml-hardware/Cargo.toml | Added `sysinfo = "0.32"` dependency                    |
| MODIFY   | crates/anvilml-hardware/src/lib.rs  | Replaced stub with `DeviceDetector` trait, module decl, re-exports |
| CREATE   | crates/anvilml-hardware/src/cpu.rs  | `CpuDetector` struct implementing `DeviceDetector`     |

## Test Results

### anvilml-hardware unit tests (Linux)

```
running 4 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test tests::cpu_detector_implements_trait ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Full workspace test suite (Linux)

```
running 68 tests  (anvilml-core)
test result: ok. 68 passed; 0 failed
test result: ok. 4 passed; 0 failed  (anvilml-hardware)
test result: ok. 0 passed; 0 failed  (anvilml-ipc)
test result: ok. 0 passed; 0 failed  (anvilml-openapi)
test result: ok. 0 passed; 0 failed  (anvilml-registry)
test result: ok. 0 passed; 0 failed  (anvilml-scheduler)
test result: ok. 2 passed; 0 failed  (anvilml-server)
test result: ok. 0 passed; 0 failed  (anvilml-worker)
test result: ok. 8 passed; 0 failed  (backend bin)
test result: ok. 1 passed; 0 failed  (backend config_reference)
```

### Windows cross-check (x86_64-pc-windows-gnu)

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.17s
```

Zero errors — the hardware crate and full workspace compile cleanly for the windows-gnu target.

### Config drift gate

```
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed
```

No ServerConfig fields were modified in this task, so the config reference test passes without changes to `anvilml.toml`.

## CI Changes

No CI changes made.

## Commit Log

```
 M .forge/state/CURRENT_TASK.md
 M .forge/state/state.json
 M Cargo.lock
 M crates/anvilml-hardware/Cargo.toml
 M crates/anvilml-hardware/src/lib.rs
?? .forge/reports/P4-A1_plan.md
?? crates/anvilml-hardware/src/cpu.rs
```

## Acceptance Criteria — Verification

| Criterion                                      | Status | Evidence                                         |
|------------------------------------------------|--------|--------------------------------------------------|
| `sysinfo = "0.32"` added to Cargo.toml         | PASS   | File inspect: `sysinfo = "0.32"` present          |
| `DeviceDetector` trait defined with 2 methods  | PASS   | lib.rs: `detect()` and `refresh_vram()` defined   |
| `CpuDetector` returns one device               | PASS   | Test `cpu_detect_returns_one_device` passes       |
| Device fields correct (index, name, type, etc.)| PASS   | Test `cpu_device_fields` passes                   |
| `refresh_vram` returns `(0, 0)` for CPU        | PASS   | Test `cpu_refresh_vram` passes                    |
| Tests filterable via `-- cpu`                  | PASS   | `cargo test -p anvilml-hardware -- cpu` runs 3 tests |
| Trait re-exports `GpuDevice`, `DeviceType`, `AnvilError` | PASS | `pub use anvilml_core::{...}` in lib.rs         |
| `cargo fmt --all` passes                       | PASS   | No formatting changes after fmt                  |
| `cargo clippy --workspace --features mock-hardware -- -D warnings` passes | PASS | Zero warnings           |
| Windows cross-check passes                     | PASS   | `cargo check --target x86_64-pc-windows-gnu ...` succeeds |
| Full workspace tests pass (0 failures)         | PASS   | 83 tests across all crates, 0 failed             |
| Config drift gate passes                       | PASS   | `config_reference` test passes                   |
