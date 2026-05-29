# Tasks: Phase 003 — Hardware Detection

| Field            | Value                                                                     |
|------------------|---------------------------------------------------------------------------|
| Phase            | 003                                                                       |
| Name             | Hardware Detection                                                        |
| ANVIL Milestone  | M1 (part 2)                                                               |
| Status           | Draft                                                                     |
| Depends on phases| 1, 2                                                                      |
| Task file        | `forge/tasks/tasks_phase003.json`                                         |
| Design reference | `ANVILML_DESIGN.md` §5 (Hardware Detection), §4.3 (Hardware Types)       |

---

## Overview

Phase 003 implements the `anvilml-hardware` crate: the subsystem responsible for detecting available compute devices (CUDA GPUs, ROCm GPUs, or CPU fallback) and providing a refreshable VRAM snapshot. The output of this phase is a stable `detect_all_devices()` function that returns a `HardwareInfo` value — the same struct defined in `anvilml-core` in phase 002 — and a `DeviceDetector` trait that allows the mock implementation used in all CI tests to substitute for real hardware.

This phase completes M1 alongside phase 002. The design document's exit criterion for M1 is "round-trip and detector fixture tests green; `openapi.json` generates." The round-trip tests were delivered in phase 002; the detector fixture tests are delivered here. Once this phase is done, `cargo test --workspace --features mock-hardware` covers all three crates that constitute M1.

Hardware detection is placed before persistence, worker management, scheduling, and the HTTP server because every subsequent crate that needs to know what devices are available calls into `anvilml-hardware`. The worker spawner (phase 005) needs device information to set per-worker environment variables. The scheduler (phase 006) needs it to initialize the `VramLedger`. The server (phase 007) needs it to respond to `GET /v1/system/hardware`. Implementing those later phases before hardware detection would require each to stub the device list, producing dependency inversions that are harder to test correctly.

At the end of this phase, `cargo test -p anvilml-hardware --features mock-hardware` passes with at least 8 tests. The mock detector is driven entirely by environment variables, so CI has full deterministic control. The CUDA and ROCm detectors are tested against fixture strings — captured real `nvidia-smi` and `rocm-smi` output — and do not require any GPU to be present in the test environment.

---

## Group Reference

| Group | Subsystem          | Tasks          | Summary                                                             |
|-------|--------------------|----------------|---------------------------------------------------------------------|
| A     | anvilml-hardware   | P3-A1 … P3-A4  | DeviceDetector trait, CPU, CUDA, ROCm detectors                     |
| B     | anvilml-hardware   | P3-B1          | HardwareOverrideConfig wiring, HostInfo, integration tests          |

---

## Prerequisites

- P2-A4 complete: `HardwareInfo`, `GpuDevice`, `DeviceType`, `HostInfo`, `InferenceCaps` are defined and exported from `anvilml-core`.
- `anvilml-hardware` already has the `mock-hardware` feature declared in its `Cargo.toml` (from P1-A1). This phase populates `src/lib.rs`, `src/cuda.rs`, `src/rocm.rs`, `src/cpu.rs`, and `src/mock.rs`.

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks | What must match                                                                |
|---------------------------|----------------|--------------------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §5    | P3-A1 … P3-B1  | Full detection logic, `nvidia-smi` and `rocm-smi` invocation and parse format  |
| `ANVILML_DESIGN.md` §4.3  | P3-A1 … P3-B1  | `HardwareInfo`, `GpuDevice`, `DeviceType` field names and semantics             |
| `ANVILML_DESIGN.md` §3.1  | P3-B1          | `HardwareOverrideConfig` fields: `force_device_type`, `mock_vram_mib`           |

---

## Task Descriptions

### Group A — anvilml-hardware: Detectors

#### P3-A1: anvilml-hardware — DeviceDetector trait and CPU detector

**Goal:** Define the `DeviceDetector` trait that all concrete detectors implement, and deliver the CPU fallback detector — the simplest concrete implementation and the one that guarantees at least one device is always available.

**Files to create or modify:**
- `crates/anvilml-hardware/src/lib.rs` — `DeviceDetector` trait, `detect_all_devices` stub, `pub use` of all submodules
- `crates/anvilml-hardware/src/cpu.rs` — `CpuDetector` struct implementing `DeviceDetector`
- `crates/anvilml-hardware/Cargo.toml` — add `anvilml-core` (path dep)

**Key implementation notes:**
- `DeviceDetector` trait: `fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError>` and `fn refresh_vram(&self, device_index: u32) -> Result<(u32 /*used*/, u32 /*total*/), AnvilError>`. The trait must be object-safe so it can be used as `Box<dyn DeviceDetector>`.
- `CpuDetector::detect()` returns exactly one `GpuDevice`: `index=0`, `name="CPU"`, `device_type=DeviceType::Cpu`, `vram_total_mib=0`, `vram_free_mib=0`, `driver_version="n/a"`.
- `detect_all_devices` at this stage: call `CpuDetector` and return a `HardwareInfo` with host fields zeroed (host info is filled in P3-B1).
- `InferenceCaps` for CPU: `fp16=false`, `bf16=false`, `flash_attention=false`.

**Acceptance criterion:** `cargo test -p anvilml-hardware -- cpu` exits 0.

---

#### P3-A2: anvilml-hardware — mock detector driven by env vars

**Goal:** Implement the deterministic mock detector used in all CI runs, gated behind the `mock-hardware` feature flag.

**Files to create or modify:**
- `crates/anvilml-hardware/src/mock.rs` — `MockDetector` struct, feature-gated
- `crates/anvilml-hardware/src/lib.rs` — conditionally use `MockDetector` when `mock-hardware` feature active
- `crates/anvilml-hardware/Cargo.toml` — feature `mock-hardware` already declared; verify it needs no new deps

**Key implementation notes:**
- `MockDetector::detect()` reads: `ANVILML_MOCK_DEVICE_TYPE` (values: `cpu`, `cuda`, `rocm`; default `cpu`), `ANVILML_MOCK_VRAM_MIB` (u32; default `8192`), `ANVILML_MOCK_GFX_ARCH` (string; default `gfx1100`). Returns one `GpuDevice` with these values set.
- When `mock-hardware` feature is active, `detect_all_devices()` must use `MockDetector` and ignore real detectors entirely. This is the invariant that makes CI hermetic.
- Write three fixture tests in `mock.rs` (cfg-gated): (1) default env → DeviceType::Cpu; (2) `ANVILML_MOCK_DEVICE_TYPE=cuda` → DeviceType::Cuda with correct vram; (3) `ANVILML_MOCK_DEVICE_TYPE=rocm` + custom VRAM → DeviceType::Rocm.
- Tests must set and unset env vars carefully. Use `std::env::set_var` inside the test and restore or scope with a serial test attribute if needed to avoid cross-test pollution.

**Acceptance criterion:** `cargo test -p anvilml-hardware --features mock-hardware -- mock` exits 0 with ≥3 fixture tests passing.

---

#### P3-A3: anvilml-hardware — CUDA detector via nvidia-smi

**Goal:** Implement detection of NVIDIA GPUs by parsing `nvidia-smi` CSV output. No GPU hardware is required — tests operate against captured fixture strings.

**Files to create or modify:**
- `crates/anvilml-hardware/src/cuda.rs` — `CudaDetector` struct
- `crates/anvilml-hardware/src/lib.rs` — integrate `CudaDetector` into `detect_all_devices` (non-mock path)

**Key implementation notes:**
- Invocation: `nvidia-smi --query-gpu=index,name,memory.total,memory.free,driver_version --format=csv,noheader,nounits`. Output is one line per GPU, comma-separated, with numeric memory values in MiB and driver_version as a dotted string.
- If `nvidia-smi` is not on PATH, or if the command exits non-zero, return `Ok(vec![])` — absence of NVIDIA hardware is not an error.
- `InferenceCaps` heuristic for CUDA: `fp16=true` always; `bf16=true` if driver_version major ≥ 525 (Ampere+ support); `flash_attention=true` if bf16=true (same gate for MVP).
- Write tests using a helper `parse_nvidia_smi_output(raw: &str) -> Vec<GpuDevice>` extracted from the detector so tests do not need to spawn a process. Provide at least two fixture strings: a single-GPU case and a dual-GPU case.

**Acceptance criterion:** `cargo test -p anvilml-hardware -- cuda` exits 0.

---

#### P3-A4: anvilml-hardware — ROCm detector via rocm-smi

**Goal:** Implement detection of AMD GPUs by parsing `rocm-smi` JSON output.

**Files to create or modify:**
- `crates/anvilml-hardware/src/rocm.rs` — `RocmDetector` struct
- `crates/anvilml-hardware/src/lib.rs` — integrate `RocmDetector` into `detect_all_devices` (non-mock path)

**Key implementation notes:**
- Invocation: `rocm-smi --showmeminfo vram --json`. The JSON structure varies across ROCm versions; parse defensively. Keys of interest: per-card VRAM total and VRAM used in bytes. Convert to MiB (`/ 1024 / 1024`).
- If `rocm-smi` is not on PATH or exits non-zero, return `Ok(vec![])`.
- `InferenceCaps` for ROCm: `fp16=true` always; `bf16=true` if `gfx_arch` from `HardwareOverrideConfig.hsa_override_gfx_version` starts with `gfx11` or higher (gfx1100, gfx1101 = RDNA3); if no override is provided, default `bf16=false` for safety.
- `refresh_vram` re-invokes `rocm-smi` and parses the single-card result for the requested `device_index`. Return `(used_mib, total_mib)`.
- Write fixture-based tests using a `parse_rocm_smi_output(raw: &str)` helper. Provide at least one fixture representing a single ROCm GPU.

**Acceptance criterion:** `cargo test -p anvilml-hardware -- rocm` exits 0 with fixture-based tests.

---

### Group B — anvilml-hardware: Integration

#### P3-B1: anvilml-hardware — HardwareOverrideConfig wiring, HostInfo, and integration tests

**Goal:** Complete `detect_all_devices()` with the full priority logic, fill in real `HostInfo`, and verify the combined system against all override scenarios.

**Files to create or modify:**
- `crates/anvilml-hardware/src/lib.rs` — full `detect_all_devices(override)` implementation
- `crates/anvilml-hardware/Cargo.toml` — add `sysinfo` crate for host metrics

**Key implementation notes:**
- `detect_all_devices(override: Option<&HardwareOverrideConfig>)` priority:
  1. If `override.force_device_type == Some(DeviceType::Cuda)`: run only `CudaDetector`; if result is empty, return `AnvilError::InvalidGraph("forced CUDA but no CUDA devices found")` — or a more specific error variant. Similarly for `Rocm`.
  2. If `override.force_device_type == Some(DeviceType::Cpu)`: use `CpuDetector` directly.
  3. If no override: run `CudaDetector`, then `RocmDetector`. If both return empty, fall back to `CpuDetector`. The first non-empty result wins (do not merge CUDA + ROCm in MVP).
- `HostInfo`: use the `sysinfo` crate to populate `os` (OS name + version string), `cpu_model` (first logical CPU name), `ram_total_mib`, `ram_free_mib`.
- Write integration tests (features: mock-hardware) covering: no-override → MockDetector result; force-cpu override → CpuDetector result; verify `HardwareInfo.host.ram_total_mib > 0`.

**Acceptance criterion:** `cargo test -p anvilml-hardware --features mock-hardware` exits 0 with ≥8 total tests.

---

## Phase Acceptance Criteria

```
cargo test -p anvilml-hardware --features mock-hardware
cargo test -p anvilml-core
cargo test -p anvilml-ipc
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo fmt --all --check
```

---

## Known Constraints and Gotchas

- `std::env::set_var` is not thread-safe in Rust. Mock detector tests that manipulate env vars must either run with `--test-threads=1` or use the `serial_test` crate to serialize them. Add `serial_test` as a dev-dependency if needed.
- The `mock-hardware` feature must short-circuit ALL real detector invocations, not just add the mock alongside them. If CI accidentally runs `nvidia-smi` or `rocm-smi` (both absent on the CI runner), those invocations will return non-zero exits, which must be handled as "device absent" not as errors. Verify this path is tested.
- `sysinfo` crate API changes frequently between minor versions. Pin the version in `Cargo.toml`. The relevant call pattern at the time of writing is `sysinfo::System::new_all()` followed by `.total_memory()` and `.available_memory()`. Check the crate docs for the pinned version.
- `rocm-smi --json` output is not stable across ROCm versions. The parser must tolerate missing keys gracefully and return partial data rather than an error. A missing VRAM key should default to `0` with a log warning, not abort detection.
- CUDA and ROCm detectors are compiled into the binary even when `mock-hardware` is active — they are simply not called. This is intentional: it ensures the real detectors always compile and are covered by `cargo clippy`, even in CI.
