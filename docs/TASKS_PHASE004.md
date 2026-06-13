# Tasks: Phase 004 — Hardware Detection

| Field | Value |
|-------|-------|
| Phase | 004 |
| Name | Hardware Detection |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 3 |

## Overview

Phase 004 implements SDK-free GPU detection and surfaces it through `GET /v1/system`. The detection strategy is defined in `ANVILML_DESIGN.md §6`: Vulkan is the primary enumerator on both Linux and Windows; DXGI provides the Windows fallback; PCI sysfs + NVML provide the Linux fallback. A CPU device is always synthesised when no GPU is found.

Every detection path must never panic — if the Vulkan loader is absent or returns no devices, the function returns `Ok(vec![])` and the fallback chain continues. The final `Ok(HardwareInfo)` always contains at least one device.

The `mock-hardware` feature replaces all real detection with `MockDetector`, which reads `ANVILML_MOCK_*` env vars. All CI builds use this feature. Detection tests on real hardware pass only on developer machines with physical GPUs.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-hardware | P4-A1 … P4-A5 | DeviceDetector trait, Vulkan, platform fallbacks, orchestration |
| B | anvilml-hardware | P4-B1 | MockDetector + device_db.rs PCI capability table |
| C | anvilml-server | P4-C1 | `GET /v1/system` wired to real HardwareInfo |

## Prerequisites

Phase 003 complete: `HardwareInfo`, `GpuDevice`, `InferenceCaps`, `AnvilError` types exist.

## Task Descriptions

### Group A — anvilml-hardware real detection

#### P4-A1: anvilml-hardware: DeviceDetector trait + CpuDetector

**Goal:** Define the `DeviceDetector` trait and implement `CpuDetector` returning a synthetic CPU device using `sysinfo`.

**Acceptance criterion:** `cargo test -p anvilml-hardware -- cpu` exits 0.

#### P4-A2: anvilml-hardware: VulkanDetector (primary SDK-free path)

**Goal:** Implement `VulkanDetector` using `ash` crate with runtime loader (`Entry::load()`). Enumerate physical devices, read `KHR_driver_properties` + `EXT_memory_budget`. Return `Ok(vec![])` gracefully when loader is absent.

**Acceptance criterion:** `cargo test -p anvilml-hardware -- vulkan` exits 0 regardless of GPU presence (never panics).

#### P4-A3: anvilml-hardware: DXGI fallback (Windows) + sysfs/NVML fallback (Linux)

**Goal:** Implement `DxgiDetector` (`#[cfg(windows)]`) using `windows` crate DXGI COM APIs, and `SysfsPciDetector` + `NvmlDetector` (`#[cfg(unix)]`). Both implement `DeviceDetector`.

**Acceptance criterion:** `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (DXGI path compiles on Linux cross-check).

#### P4-A4: anvilml-hardware: device_db.rs PCI-ID capability table

**Goal:** Implement `device_db.rs` with `resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceRow>)` that populates `arch`, `caps`, and canonical `name` from a curated PCI-ID table. VRAM is never read from this table.

**Acceptance criterion:** `cargo test -p anvilml-hardware -- device_db` exits 0 with ≥ 6 tests (known NVIDIA, AMD, unknown, CPU fallback).

#### P4-A5: anvilml-hardware: detect_all_devices orchestration

**Goal:** Implement `pub async fn detect_all_devices(cfg: &ServerConfig, pool: &SqlitePool) -> Result<HardwareInfo>` in `lib.rs` orchestrating the priority chain: hardware_override → mock → Vulkan → DXGI/sysfs → CPU. Seed the device DB before detection.

**Acceptance criterion:** With mock-hardware feature and `ANVILML_MOCK_DEVICE_TYPE=cuda`, `detect_all_devices` returns one CUDA device.

### Group B — Mock + tests

#### P4-B1: anvilml-hardware: MockDetector driven by ANVILML_MOCK_* env vars

**Goal:** Implement `MockDetector` in `mock.rs` (behind `#[cfg(feature="mock-hardware")]`) reading `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_MOCK_DEVICE_NAME`. All mock tests must restore env vars unconditionally per `ENVIRONMENT.md §11.3`.

**Acceptance criterion:** `cargo test -p anvilml-hardware --features mock-hardware` exits 0 with ≥ 8 tests including cuda/rocm/cpu mock variants.

### Group C — anvilml-server

#### P4-C1: anvilml-server: GET /v1/system wired to HardwareInfo

**Goal:** Add `get_system` handler in `handlers/system.rs` returning `hardware` from `AppState`. Wire `detect_all_devices` in `main.rs` to populate `AppState.hardware` at startup. Log each detected device at INFO with required fields.

**Acceptance criterion:** `curl /v1/system` → 200 JSON with `gpus` array containing at least one entry; with mock-hardware, entry has `device_type: "Cpu"` or `ANVILML_MOCK_DEVICE_TYPE` value.

## Phase Acceptance Criteria

```bash
cargo test -p anvilml-hardware --features mock-hardware
cargo test --workspace --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
kill %1
```

## Known Constraints and Gotchas

- Use `ash = { version = "0.38", default-features = false, features = ["loaded"] }` — the `loaded` feature defers loader discovery to runtime via `dlopen`, avoiding a compile-time link requirement against `libvulkan.so`.
- `SqlitePool` is passed to `detect_all_devices` because the device DB seed requires it. Phase 005 will implement this pool; for Phase 004 use `open_in_memory()` in tests.
- All mock tests that set `ANVILML_MOCK_*` must use `#[serial]` from the `serial_test` crate to prevent race conditions between tests that share process-global env vars.
