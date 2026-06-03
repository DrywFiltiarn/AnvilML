# Plan Report: P4-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A5                                       |
| Phase       | 004 — Hardware Detection                    |
| Description | anvilml-hardware: detect_all_devices with override + host info |
| Depends on  | P4-A1, P4-A2, P4-A3, P4-A4, P4-A4B          |
| Project     | anvilml                                     |
| Planned at  | 2026-06-03T15:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the `detect_all_devices(cfg) -> HardwareInfo` orchestration function in `anvilml-hardware`, which selects the appropriate detector backend, enumerates GPU devices (or returns a synthetic override device), populates host information via `sysinfo`, applies PCI-ID capability resolution from `device_db`, and maps vendor IDs to `DeviceType`. This is the central detection entry point that wires together all prior P4-A1–A4B modules.

## Scope

### In Scope
- Add `EnumerationSource` and `CapabilitySource` enums to `anvilml-core/src/types/hardware.rs` (currently missing from `GpuDevice`)
- Extend `GpuDevice` struct with new fields: `pci_vendor_id: u16`, `pci_device_id: u16`, `arch: Option<String>`, `caps: InferenceCaps`, `enumeration_source: EnumerationSource`, `capabilities_source: CapabilitySource`
- Update all existing `GpuDevice` constructions (CPU detector, mock detector) to include the new fields with sensible defaults
- Implement `detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>` in `crates/anvilml-hardware/src/lib.rs` with the following priority logic:
  1. If `mock-hardware` feature is enabled → use `MockDetector::detect()` directly (single device)
  2. If `config.hardware_override.is_some()` → return one synthetic device with `enumeration_source = Override`, using the override's `device_type` and `vram_total_mib`
  3. Else enumerate via `VulkanDetector`; if empty list returned, fall back to platform-specific detectors (DXGI on Windows, sysfs+NVML on Unix)
  4. For each enumerated device, call `device_db::resolve_caps(dev, vendor_id, device_id)` to populate name/arch/caps
  5. Map PCI vendor ID → `DeviceType`: `0x10DE` = Cuda, `0x1002` = Rocm (both OSes), `0x8086` or unknown = Cpu
  6. If zero GPUs detected → add one CPU device via `CpuDetector`
  7. Populate `HostInfo` via `sysinfo::System` (os, cpu_model, ram_total_mib, ram_free_mib)
  8. Set `inference_caps` on `HardwareInfo` to the OR of all device caps (or defaults if no GPUs)
- Update `device_db::resolve_caps` to populate all new `GpuDevice` fields (arch, caps, enumeration_source, capabilities_source) when a table hit occurs; emit `warn!` on miss with PCI IDs and set fallback values
- Add unit tests in `lib.rs` (`mod tests`) covering all detection paths

### Out of Scope
- HTTP endpoint wiring (`GET /v1/system`) — handled by P4-A6
- Worker-side capability refinement at `Ready` — deferred to Phase 9
- `--print-hardware` CLI subcommand — handled by P4-A6
- Real Vulkan/DXGI/sysfs/NVML runtime enumeration (already implemented in prior tasks)
- VRAM refresh loop during operation — deferred to worker integration

## Approach

1. **Add enums and fields to `anvilml-core/src/types/hardware.rs`**:
   - Define `EnumerationSource` enum: `Vulkan`, `Dxgi`, `Sysfs`, `Nvml`, `Override`, `Mock`
   - Define `CapabilitySource` enum: `Worker`, `DeviceTable`, `Fallback`
   - Add to `GpuDevice`: `pci_vendor_id: u16`, `pci_device_id: u16`, `arch: Option<String>`, `caps: InferenceCaps`, `enumeration_source: EnumerationSource`, `capabilities_source: CapabilitySource`
   - All new fields derive standard traits + `ToSchema`; use `#[serde(default)]` for backward compat
   - Update all existing test fixtures in hardware.rs to include the new fields

2. **Update `CpuDetector` in `cpu.rs`**:
   - Set new fields: `pci_vendor_id: 0`, `pci_device_id: 0`, `arch: None`, `caps: InferenceCaps::default()`, `enumeration_source: EnumerationSource::Mock`, `capabilities_source: CapabilitySource::Fallback`

3. **Update `MockDetector` in `mock.rs`**:
   - Set new fields: `pci_vendor_id: 0`, `pci_device_id: 0`, `arch: Some(gfx_arch from env)`, `caps: InferenceCaps::default()`, `enumeration_source: EnumerationSource::Mock`, `capabilities_source: CapabilitySource::Fallback`

4. **Update `device_db::resolve_caps` in `device_db.rs`**:
   - On lookup hit: set `dev.arch = Some(entry.arch.to_string())`, `dev.caps = InferenceCaps { fp16, bf16, flash_attention }`, `dev.enumeration_source = EnumerationSource::DeviceTable`, `dev.capabilities_source = CapabilitySource::DeviceTable`
   - On miss: emit `warn!` with PCI IDs, set conservative defaults (`caps = InferenceCaps::default()`, `capabilities_source = CapabilitySource::Fallback`)

5. **Implement `detect_all_devices` in `lib.rs`**:
   - Function signature: `pub fn detect_all_devices(cfg: &ServerConfig) -> Result<HardwareInfo, AnvilError>`
   - Branch 1 (mock-hardware): compile-gated via `#[cfg(feature = "mock-hardware")]`, use `MockDetector`
   - Branch 2 (override): check `cfg.hardware_override`, build single synthetic device with `EnumerationSource::Override`
   - Branch 3 (enumerate): call `VulkanDetector::detect()`, if empty try DXGI (Windows) or sysfs+NVML (Unix); merge results deduplicating by PCI ID
   - For each device: read vendor_id from `VulkanDetector::Device` (or equivalent), call `device_db::resolve_caps(dev, vendor_id, device_id)`
   - Vendor mapping helper `fn map_vendor_to_device_type(vendor_id: u16) -> DeviceType` mirroring vulkan.rs logic
   - No GPU → append `CpuDetector::detect()` result
   - Build `HostInfo`: use `sysinfo::System`, call `system.refresh_cpu()`, `system.refresh_memory()`, get OS name, CPU model, total/free RAM converted to MiB
   - Compute `inference_caps` as OR of all device caps (or default if no GPUs)
   - Return `HardwareInfo { host, gpus, inference_caps }`

6. **Add tests in `lib.rs`** (minimum 8 tests under `#[cfg(test)] mod tests`):
   - T1: `detect_all_devices_override` — config with hardware_override returns one Override device
   - T2: `detect_all_devices_mock_cuda` — mock-hardware feature, env CUDA → Cuda device
   - T3: `detect_all_devices_mock_rocm` — mock-hardware feature, env ROCm → Rocm device
   - T4: `detect_all_devices_vulkan_empty_fallback` — Vulkan returns empty, fallback detector used (compile-time conditional)
   - T5: `detect_all_devices_vendor_map_cuda` — vendor ID 0x10DE maps to Cuda
   - T6: `detect_all_devices_vendor_map_rocm` — vendor ID 0x1002 maps to Rocm
   - T7: `detect_all_devices_vendor_map_cpu_intel` — vendor ID 0x8086 maps to Cpu
   - T8: `detect_all_devices_cpu_only_when_no_gpu` — zero GPUs → one CPU device appended

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add `EnumerationSource`, `CapabilitySource` enums; extend `GpuDevice` with 6 new fields; update tests |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Add `detect_all_devices()` function; add `mod tests` with ≥8 tests |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Update `GpuDevice` construction with new fields |
| Modify | `crates/anvilml-hardware/src/mock.rs` | Update `GpuDevice` construction with new fields; update tests |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Extend `resolve_caps()` to populate all new fields |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-core/src/types/hardware.rs` | (existing roundtrip tests) | New GpuDevice fields serialize/deserialize correctly |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_override` | hardware_override config produces Override device |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_mock_cuda` | Mock detector returns Cuda device with correct VRAM |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_mock_rocm` | Mock detector returns Rocm device |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_vulkan_empty_fallback` | Empty Vulkan triggers fallback to platform detector |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_vendor_map_cuda` | Vendor 0x10DE → DeviceType::Cuda |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_vendor_map_rocm` | Vendor 0x1002 → DeviceType::Rocm |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_vendor_map_cpu_intel` | Vendor 0x8086 → DeviceType::Cpu |
| `crates/anvilml-hardware/src/lib.rs` | `detect_all_devices_cpu_only_when_no_gpu` | Zero GPUs produces one CPU device |
| `crates/anvilml-hardware/src/device_db.rs` | (existing tests) | resolve_caps populates new fields on hit/miss |
| `crates/anvilml-hardware/src/cpu.rs` | (existing tests) | CpuDetector fields include new defaults |

## CI Impact

No changes to CI workflow files. The existing CI matrix already runs `cargo test -p anvilml-hardware --features mock-hardware` on the `rust` job and `cargo clippy … --features mock-hardware -D warnings`. Adding new fields with `#[serde(default)]` ensures backward-compatible serialization. The `mock-hardware` feature flag is already forwarded by dependent crates per ARCHITECTURE.md §5. No new dependencies are introduced.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `GpuDevice` field additions break existing code in other crates (worker, server, scheduler) that construct `GpuDevice` with struct literal syntax | Use `#[serde(default)]` on all new fields; update all known construction sites (cpu.rs, mock.rs) as part of the same task; compile check across workspace catches any missed sites |
| `device_db::resolve_caps` currently has a TODO stub — updating it may expose missing fields | The function already takes `&mut GpuDevice`; extending it to set new fields is additive and follows the existing pattern; tests verify hit/miss paths |
| `sysinfo` API changes between versions affect `HostInfo` population | Pin `sysinfo = "0.32"` (already in Cargo.toml); use stable methods (`refresh_cpu()`, `refresh_memory()`, `name()`, `physical_processor_id()`); test with current version |
| Platform-specific fallback branches (DXGI/sysfs/NVML) may not compile on the opposite platform during cross-check | Use existing `#[cfg(windows)]` / `#[cfg(unix)]` guards already in place; run `cargo check --target x86_64-pc-windows-gnu` as per FORGE_AGENT_RULES §5.7 |
| Mock detector env-var tests may interfere with each other | Use `serial_test::serial` (already imported in mock.rs); apply same pattern to lib.rs tests that read env vars |

## Acceptance Criteria

- [ ] `EnumerationSource` and `CapabilitySource` enums exist in `anvilml-core/src/types/hardware.rs` with all required variants
- [ ] `GpuDevice` struct has all 6 new fields (`pci_vendor_id`, `pci_device_id`, `arch`, `caps`, `enumeration_source`, `capabilities_source`) with `#[serde(default)]`
- [ ] All existing `GpuDevice` constructions (CPU, mock) compile and include the new fields with correct defaults
- [ ] `device_db::resolve_caps()` populates arch, caps, enumeration_source, capabilities_source on table hit; warns on miss
- [ ] `detect_all_devices(cfg)` function exists in `lib.rs` with correct priority: mock → override → Vulkan → fallback → CPU-only
- [ ] Vendor ID mapping: 0x10DE→Cuda, 0x1002→Rocm, 0x8086/other→Cpu
- [ ] No GPU detected → exactly one CPU device appended
- [ ] `HostInfo` populated via sysinfo (os, cpu_model, ram_total_mib, ram_free_mib)
- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0 with ≥8 tests in lib.rs
- [ ] `cargo clippy -p anvilml-hardware --features mock-hardware -D warnings` passes
- [ ] `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware` passes (cross-platform check)
