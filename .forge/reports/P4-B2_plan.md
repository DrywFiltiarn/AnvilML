# Plan Report: P4-B2

| Field       | Value                                                                                      |
|-------------|--------------------------------------------------------------------------------------------|
| Task ID     | P4-B2                                                                                      |
| Phase       | 004 — Hardware Detection                                                                   |
| Description | anvilml-core: extend GpuDevice + InferenceCaps for SDK-free detection (retrofit)           |
| Depends on  | P3-A6                                                                                      |
| Project     | anvilml                                                                                    |
| Planned at  | 2026-06-03T17:30:00Z                                                                       |
| Attempt     | 1                                                                                          |

## Objective

Retrofit the already-committed `GpuDevice` type in `anvilml-core/src/types/hardware.rs` to include SDK-free detection fields (`pci_vendor_id`, `pci_device_id`, `vram_free_mib`, `arch`, `enumeration_source`, `capabilities_source`) and add `flash_attention: bool` to `InferenceCaps`. Introduce two new enums — `EnumerationSource` and `CapabilitySource` — with all variants specified in ANVILML_DESIGN §4.3. Ensure backward-compatible JSON deserialization via `#[serde(default)]`. Update all `GpuDevice` constructions in P4-A1 (`cpu.rs`) and P4-A2 (`mock.rs`) to populate the new fields with sensible defaults (CPU: zeros/None/Mock; Mock: env-driven). Run `cargo test -p anvilml-core -- hardware` successfully.

## Scope

### In Scope
- Extend `GpuDevice` struct in `crates/anvilml-core/src/types/hardware.rs` with six new fields:
  - `pci_vendor_id: u16` (serde default = 0)
  - `pci_device_id: u16` (serde default = 0)
  - `vram_free_mib: u32` (already present in OLD 4.3; retain with doc comment about worker refresh)
  - `arch: Option<String>` (serde default = None)
  - `enumeration_source: EnumerationSource` (serde default = Fallback)
  - `capabilities_source: CapabilitySource` (serde default = Fallback)
- Keep existing fields unchanged: `index`, `name`, `device_type`, `vram_total_mib`, `driver_version`.
- Add `EnumerationSource` enum with variants: `Vulkan`, `Dxgi`, `Sysfs`, `Nvml`, `Override`, `Mock` (derive: Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema; Default = Fallback).
- Add `CapabilitySource` enum with variants: `Fallback`, `DeviceTable`, `Worker` (derive: Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema; Default = Fallback).
- Add `flash_attention: bool` to `InferenceCaps` struct (serde default = false).
- Derive standard set on all new/modified types: `Serialize`, `Deserialize`, `Clone`, `Debug`, `ToSchema`. For enums additionally: `Copy`, `PartialEq`, `Eq`, `Default`.
- Update `GpuDevice` construction in `crates/anvilml-hardware/src/cpu.rs` (P4-A1): new fields → zeros/None/Mock/Fallback.
- Update `GpuDevice` construction in `crates/anvilml-hardware/src/mock.rs` (P4-A2): new fields → env-driven or zero/None/Mock/Fallback as appropriate.
- Add/update unit tests in `hardware.rs`: backward-compat deserialization, round-trip for all new fields, variant counts and distinctness for both enums.
- Verify: `cargo test -p anvilml-core -- hardware` exits 0.

### Out of Scope
- Any changes to `anvilml-hardware/lib.rs` detect_all_devices (already handled by P4-A5).
- Changes to `anvilml-server`, `backend`, or any other crate.
- Vulkan/DXGI/sysfs/NVML implementation (handled by P4-A3/A4).
- device_db.rs changes (handled by P4-A4B).
- CI workflow file modifications.
- OpenAPI regeneration (separate task).

## Approach

1. **Read current `hardware.rs`** — identify the OLD `GpuDevice` struct (6 fields) and `InferenceCaps` struct (fp16, bf16 only).
2. **Add `EnumerationSource` enum** above existing types in `hardware.rs` with all 7 variants (Vulkan, Dxgi, Sysfs, Nvml, Override, Mock, DeviceTable — note: ANVILML_DESIGN §4.3 lists Vulkan/Dxgi/Sysfs/Nvml/Override/Mock; DeviceTable is used by P4-A4B's device_db.rs and Fallback for CPU fallback). Apply correct derives and `#[default]` on `Fallback`.
3. **Add `CapabilitySource` enum** with 3 variants (Fallback, DeviceTable, Worker). Apply correct derives and `#[default]` on `Fallback`.
4. **Extend `InferenceCaps`** — add `flash_attention: bool` field with `#[serde(default)]`.
5. **Extend `GpuDevice`** — add the 6 new fields in the order specified by UPDATED 4.3, each with `#[serde(default)]`:
   - `pci_vendor_id: u16` (default 0)
   - `pci_device_id: u16` (default 0)
   - `arch: Option<String>` (default None)
   - `caps: InferenceCaps` (default via InferenceCaps::default())
   - `enumeration_source: EnumerationSource` (default Fallback)
   - `capabilities_source: CapabilitySource` (default Fallback)
6. **Update `cpu.rs`** — add the 6 new fields to the single GpuDevice construction with defaults: `pci_vendor_id: 0`, `pci_device_id: 0`, `arch: None`, `caps: InferenceCaps::default()`, `enumeration_source: EnumerationSource::Mock`, `capabilities_source: CapabilitySource::Fallback`.
7. **Update `mock.rs`** — add the 6 new fields to the GpuDevice construction with appropriate values: `pci_vendor_id: 0`, `pci_device_id: 0`, `arch: Some(gfx_arch)`, `caps: InferenceCaps::default()`, `enumeration_source: EnumerationSource::Mock`, `capabilities_source: CapabilitySource::Fallback`.
8. **Add unit tests** in `hardware.rs` for backward compatibility (deserialize old JSON with only 6 fields), round-trip serialization of new fields, enum variant counts and distinctness.
9. **Run `cargo test -p anvilml-core -- hardware`** to verify all tests pass.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add EnumerationSource, CapabilitySource enums; extend InferenceCaps with flash_attention; extend GpuDevice with 6 new fields; add backward-compat and roundtrip tests |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Update GpuDevice construction to populate new fields (zeros/None/Mock/Fallback) |
| Modify | `crates/anvilml-hardware/src/mock.rs` | Update GpuDevice construction to populate new fields (env-driven arch, zeros for PCI IDs, Mock/Fallback sources) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `gpu_device_backward_compat` | Old JSON with 6 fields deserializes to defaults for new fields |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `gpu_device_roundtrip` | Full GpuDevice round-trips through JSON with all new fields |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `inference_caps_roundtrip` | InferenceCaps including flash_attention round-trips through JSON |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `enumeration_source_variants` | EnumerationSource has exactly 8 variants, all pairwise distinct |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `capability_source_variants` | CapabilitySource has exactly 3 variants, all pairwise distinct |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `enumeration_source_default_is_fallback` | Default(EnumerationSource) == Fallback |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `capability_source_default_is_fallback` | Default(CapabilitySource) == Fallback |
| `crates/anvilml-core/src/types/hardware.rs` (mod tests) | `enumeration_capability_sources_roundtrip` | All enum variants round-trip through JSON serialization |
| `crates/anvilml-hardware/src/cpu.rs` (mod tests) | `cpu_device_new_fields` | CPU device has correct default values for new fields |
| `crates/anvilml-hardware/src/mock.rs` (mod tests) | `mock_device_new_fields` | Mock device has correct values for new fields |

## CI Impact

No CI workflow files are modified. The existing CI matrix (`rust`, `python-worker`, `openapi-diff`, `rust-windows`) already builds with `--features mock-hardware`. Adding new fields to domain types does not change CI job definitions. However, `cargo clippy -D warnings` must pass for the workspace after changes (new derive additions and field insertions must not introduce unused-code or similar warnings).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Adding fields to GpuDevice breaks existing code in crates that construct it (e.g. `lib.rs` detect_all_devices, device_db.rs) | Audit all GpuDevice constructions across the workspace before writing; update each site with appropriate defaults. The retrofit nature means P4-A1/A2 are the primary targets but P4-A5's lib.rs and P4-A4B's device_db.rs must also be checked. |
| `#[serde(default)]` on Option<String> and new enum fields could silently mask bugs during debugging | Document field semantics in doc comments; add backward-compat test that verifies old JSON deserializes correctly. |
| Derive set mismatch (e.g. missing Clone on a struct used with Arc) | Ensure all types derive `Clone`; use `Copy` only for unit-type enums. Verify by running `cargo check -p anvilml-hardware`. |
| `flash_attention` field name conflicts with existing or future fields | Use exact name from ANVILML_DESIGN §4.3: `flash_attention`. The field was already added in P3-A4's InferenceCaps, so verify it is present and not duplicated. |

## Acceptance Criteria

- [ ] `GpuDevice` struct has all 12 fields (6 original + 6 new) with correct types
- [ ] `EnumerationSource` enum has 8 variants: Vulkan, Dxgi, Sysfs, Nvml, Override, Mock, DeviceTable, Fallback (default)
- [ ] `CapabilitySource` enum has 3 variants: Fallback (default), DeviceTable, Worker
- [ ] `InferenceCaps` struct has 3 fields: fp16, bf16, flash_attention
- [ ] All new/modified types derive Debug, Clone, Serialize, Deserialize, ToSchema; enums additionally derive Copy, PartialEq, Eq, Default
- [ ] All GpuDevice constructions in cpu.rs and mock.rs populate new fields with correct defaults
- [ ] `cargo test -p anvilml-core -- hardware` exits 0 (all unit tests pass)
- [ ] Backward-compat test verifies old JSON deserializes with sensible defaults for new fields
