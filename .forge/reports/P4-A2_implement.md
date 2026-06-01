# Implementation Report: P4-A2

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P4-A2                                        |
| Phase          | 004 — Hardware Detection                     |
| Description    | anvilml-hardware: mock detector (feature mock-hardware, env-driven) |
| Project        | anvilml                                      |
| Implemented at | 2026-06-01T20:28:05Z                         |
| Attempt        | 1                                            |

## Summary

Implemented a mock GPU detector in `crates/anvilml-hardware/src/mock.rs`, gated behind the `mock-hardware` feature flag. `MockDetector` reads three environment variables (`ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_GFX_ARCH`) with built-in defaults (cpu, 8192 MiB, gfx1100) and returns a single deterministic `GpuDevice`. Added `serial_test` as a dev-dependency to serialize tests that share global state (environment variables). Modified `lib.rs` to conditionally include the mock module under the feature flag. Three fixture unit tests validate each device type path (cpu, cuda, rocm).

## Files Changed

| Action   | Path                              | Description                                              |
|----------|-----------------------------------|----------------------------------------------------------|
| MODIFY   | crates/anvilml-hardware/Cargo.toml | Added `serial_test = "3.5"` under `[dev-dependencies]`   |
| CREATE   | crates/anvilml-hardware/src/mock.rs | New mock detector with `MockDetector` and 3 fixture tests |
| MODIFY   | crates/anvilml-hardware/src/lib.rs  | Conditionally include `mock` module behind feature flag   |

## Test Results

### anvilml-hardware unit tests (with mock-hardware feature)

```
running 7 tests
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_refresh_vram ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_default_cpu ... ok
test mock::tests::mock_detect_rocm ... ok
test tests::cpu_detector_implements_trait ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Full workspace test suite (with mock-hardware feature)

```
running 68 tests — anvilml-core: all passed
test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 7 tests — anvilml-hardware: all passed
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests — anvilml_ipc: ok
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests — anvilml_openapi: ok
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests — anvilml_registry: ok
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests — anvilml_scheduler: ok
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 2 tests — anvilml_server: all passed
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 0 tests — anvilml_worker: ok
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 8 tests — backend: all passed
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

running 1 test — config_reference: passed
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Windows cross-check (x86_64-pc-windows-gnu)

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.13s
```

Zero errors.

### Config drift gate

```
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Zero failures.

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P4-A2_plan.md
A  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
M  crates/anvilml-hardware/Cargo.toml
M  crates/anvilml-hardware/src/lib.rs
A  crates/anvilml-hardware/src/mock.rs
```

## Acceptance Criteria — Verification

| Criterion                                                            | Status | Evidence                                              |
|----------------------------------------------------------------------|--------|-------------------------------------------------------|
| `serial_test = "3.5"` added under `[dev-dependencies]` in Cargo.toml | PASS   | File content verified                                 |
| `MockDetector` struct created with `Default` impl                     | PASS   | crates/anvilml-hardware/src/mock.rs line 17–18        |
| `DeviceDetector` trait implemented for `MockDetector`                 | PASS   | mock.rs lines 20–54                                   |
| Reads `ANVILML_MOCK_DEVICE_TYPE` (cpu/cuda/rocm, default cpu)         | PASS   | mock.rs lines 22–29; test `mock_detect_default_cpu`   |
| Reads `ANVILML_MOCK_VRAM_MIB` (default 8192)                          | PASS   | mock.rs lines 31–34; test `mock_detect_default_cpu`   |
| Reads `ANVILML_MOCK_GFX_ARCH` (default gfx1100)                       | PASS   | mock.rs line 36; test `mock_detect_default_cpu`       |
| Returns one deterministic `GpuDevice` per detection call              | PASS   | mock.rs lines 38–45; all tests assert len == 1        |
| `lib.rs` conditionally includes mock module behind feature flag        | PASS   | lib.rs line 8: `#[cfg(feature = "mock-hardware")]`    |
| Three fixture unit tests (cpu, cuda, rocm)                            | PASS   | Tests compile and pass with `--features mock-hardware`|
| `cargo fmt --all` passes                                              | PASS   | Formatted without errors                              |
| `cargo clippy --workspace --features mock-hardware -D warnings` passes | PASS   | Zero warnings                                         |
| Windows cross-check passes                                            | PASS   | `cargo check --target x86_64-pc-windows-gnu` clean    |
| Full workspace test suite passes                                      | PASS   | 86 tests, 0 failures                                  |
| Config drift gate passes                                              | PASS   | `test_toml_key_set_matches_default` ok                |
