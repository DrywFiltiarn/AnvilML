# Plan Report: P6-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-C1                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-core: add db_name field to GpuDevice  |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T23:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a `pub db_name: Option<String>` field to the `GpuDevice` struct in `anvilml-core`, then update every `GpuDevice` struct literal across the workspace to initialise the new field as `None`. This is a prerequisite for P6-C2, which will populate `db_name` from the SQLite `device_capabilities` table. After this task completes, `cargo test --workspace --features mock-hardware` exits 0 with no struct initialisation errors, and `GET /v1/system` response includes `"db_name": null` on all GPU devices in the JSON output.

## Scope

### In Scope

- **`crates/anvilml-core/src/types/hardware.rs`** — Add `pub db_name: Option<String>` field to `GpuDevice` struct, positioned immediately after the `name: String` field. Add `///` doc comment.
- **`crates/anvilml-core/Cargo.toml`** — Bump patch version `0.1.12 → 0.1.13`.
- **`crates/anvilml-hardware/Cargo.toml`** — Bump patch version `0.1.6 → 0.1.7`.
- **`crates/anvilml-core/tests/hardware_tests.rs`** — Add `db_name: None` to both `GpuDevice` literals in `test_hardware_info_json_roundtrip`; add `assert_eq!` for `db_name` in the roundtrip comparison loop.
- **`crates/anvilml-hardware/src/detect.rs`** — Add `db_name: None` to the override-path `GpuDevice` literal (line 84).
- **`crates/anvilml-hardware/src/mock.rs`** — Add `db_name: None` to the `GpuDevice` literal (line 97).
- **`crates/anvilml-hardware/src/vulkan.rs`** — Add `db_name: None` to the `GpuDevice` literal (line 157).
- **`crates/anvilml-hardware/src/cpu.rs`** — Add `db_name: None` to the `GpuDevice` literal (line 93).
- **`crates/anvilml-hardware/src/dxgi.rs`** — Add `db_name: None` to the `GpuDevice` literal (line 154).
- **`crates/anvilml-hardware/src/sysfs.rs`** — Add `db_name: None` to the `GpuDevice` literal (line 139).
- **`crates/anvilml-hardware/tests/device_db_tests.rs`** — Add `db_name: None` to all six `GpuDevice` literals (lines 20, 59, 94, 132, 165, 203).
- **`crates/anvilml-server/tests/system_tests.rs`** — Add `db_name: None` to the `GpuDevice` literal (line 67).

### Out of Scope

- Populating `db_name` from SQLite (deferred to P6-C2).
- Any changes to `resolve_caps_from_row` in `device_db.rs` (deferred to P6-C2).
- Changes to the `anvilml-registry` crate (deferred to P6-C2).
- OpenAPI spec regeneration (the `db_name` field is `Option<String>` which serialises as `null` — no schema shape change that would cause a drift).

## Existing Codebase Assessment

The `GpuDevice` struct in `crates/anvilml-core/src/types/hardware.rs` currently has 12 fields: `index`, `name`, `device_type`, `vram_total_mib`, `vram_free_mib`, `driver_version`, `pci_vendor_id`, `pci_device_id`, `arch`, `caps`, `enumeration_source`, and `capabilities_source`. It derives `Debug`, `Clone`, `Serialize`, `Deserialize`, and `ToSchema`. There is no `Default` derive on `GpuDevice` — every construction site uses a full struct literal.

All 10 construction sites across the workspace follow an identical pattern: a named-argument struct literal with every field explicitly set. The fields are ordered consistently in the struct definition, and each site sets all 12 fields. No site uses `..Default::default()` or any shorthand.

The established patterns in this codebase include:
- **Doc comments:** Every `pub` struct field has a `///` doc comment describing what it represents.
- **Error handling:** No `.unwrap()` or `.expect()` in production code; `?` propagation used throughout.
- **Test style:** Tests in `crates/{name}/tests/` as separate files; each test has a doc comment describing what it verifies; `#[serial_test::serial]` used for tests that need isolation.
- **Logging:** `tracing::info!` at device detection; `tracing::debug!` at internal detail level; structured field notation (`field = %value`).

No discrepancy exists between the design doc and current source — the design doc's `GpuDevice` definition matches the actual struct exactly (minus the new `db_name` field to be added).

## Resolved Dependencies

None. This task adds no new external crates or packages. It only modifies an existing struct field and its construction sites. The `serde`, `utoipa`, and `sqlx` crates that `GpuDevice` already depends on are unaffected.

## Approach

1. **Add `db_name` field to `GpuDevice` in `hardware.rs`.** Insert a new field `pub db_name: Option<String>` immediately after `pub name: String` (line 125). Add a `///` doc comment: "Database-resolved device group name from the device_capabilities table. `None` until enriched by the SQLite capability lookup." This field is `Option<String>` because it starts as `None` and is populated by P6-C2's SQLite enrichment step. Positioning after `name` keeps the logical grouping: enumerator name → database name → rest.

2. **Bump `anvilml-core` version in `Cargo.toml`.** Change `version = "0.1.12"` to `version = "0.1.13"`. This is a patch bump per the crate version convention (§14 of FORGE_AGENT_RULES).

3. **Bump `anvilml-hardware` version in `Cargo.toml`.** Change `version = "0.1.6"` to `version = "0.1.7"`. This reflects the struct literal updates in this crate.

4. **Update all `GpuDevice` struct literals across the workspace.** Add `db_name: None,` after the `name:` line in every struct literal. There are exactly 10 construction sites:
   - `crates/anvilml-core/tests/hardware_tests.rs` (2 sites, lines 29 and 50)
   - `crates/anvilml-hardware/src/detect.rs` (1 site, line 84)
   - `crates/anvilml-hardware/src/mock.rs` (1 site, line 97)
   - `crates/anvilml-hardware/src/vulkan.rs` (1 site, line 157)
   - `crates/anvilml-hardware/src/cpu.rs` (1 site, line 93)
   - `crates/anvilml-hardware/src/dxgi.rs` (1 site, line 154)
   - `crates/anvilml-hardware/src/sysfs.rs` (1 site, line 139)
   - `crates/anvilml-hardware/tests/device_db_tests.rs` (6 sites, lines 20, 59, 94, 132, 165, 203)
   - `crates/anvilml-server/tests/system_tests.rs` (1 site, line 67)
   
   Each update is a single-line insertion: `db_name: None,` after the `name:` field. No other fields change.

5. **Update `test_hardware_info_json_roundtrip` in `hardware_tests.rs`.** Add `assert_eq!(rest.db_name, orig.db_name, "gpu[{}].db_name", i);` inside the per-GPU comparison loop (after the `capabilities_source` assertion at line 136). This verifies the new field roundtrips through JSON serialisation/deserialisation.

6. **Verify compilation and tests.** Run `cargo test --workspace --features mock-hardware` to confirm all construction sites compile and all tests pass.

## Public API Surface

| Item | Type | Crate/Module | Description |
|------|------|-------------|-------------|
| `GpuDevice::db_name` | `pub db_name: Option<String>` | `anvilml-core::types::hardware` | New field on existing struct. Not a new `pub` item — it extends an existing struct. The struct's `pub` visibility is unchanged. |

No new `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub const`, or `pub type` items are introduced. The only change to the public API surface is the addition of a field to an existing public struct, which is backwards-compatible (existing code that constructs `GpuDevice` will fail to compile until updated, but no existing code that only reads `GpuDevice` fields is affected).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add `pub db_name: Option<String>` field to `GpuDevice` after `name` |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version `0.1.12 → 0.1.13` |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Bump patch version `0.1.6 → 0.1.7` |
| Modify | `crates/anvilml-core/tests/hardware_tests.rs` | Add `db_name: None` to 2 GpuDevice literals; add roundtrip assertion |
| Modify | `crates/anvilml-hardware/src/detect.rs` | Add `db_name: None` to override-path GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/mock.rs` | Add `db_name: None` to mock GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/vulkan.rs` | Add `db_name: None` to Vulkan-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Add `db_name: None` to CPU-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/dxgi.rs` | Add `db_name: None` to DXGI-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/src/sysfs.rs` | Add `db_name: None` to sysfs-detected GpuDevice literal |
| Modify | `crates/anvilml-hardware/tests/device_db_tests.rs` | Add `db_name: None` to 6 GpuDevice literals |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Add `db_name: None` to test GpuDevice literal |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/hardware_tests.rs` | `test_hardware_info_json_roundtrip` | `db_name: None` roundtrips through JSON serialisation/deserialisation; all fields including new `db_name` are equal after roundtrip | None | `HardwareInfo` with 2 `GpuDevice`s, each with `db_name: None` | `serde_json` roundtrip succeeds; `rest.gpus[0].db_name == orig.gpus[0].db_name` and `rest.gpus[1].db_name == orig.gpus[1].db_name` | `cargo test -p anvilml-core -- test_hardware_info_json_roundtrip` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_resolve_nvidia_ampere` | Known NVIDIA A100 resolves correctly; `db_name: None` does not interfere with capability resolution | None | `GpuDevice` with `pci_vendor_id: 0x10de, pci_device_id: 0x2204` | `arch = Some("Ampere")`, `fp8 = true`, `flash_attention = true` | `cargo test -p anvilml-hardware -- test_resolve_nvidia_ampere` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_resolve_amd_rdna3` | Known AMD RX 7900 XTX resolves correctly | None | `GpuDevice` with `pci_vendor_id: 0x1002, pci_device_id: 0x74AF` | `arch = Some("RDNA3")`, `fp8 = false` | `cargo test -p anvilml-hardware -- test_resolve_amd_rdna3` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_resolve_unknown_device` | Unknown PCI IDs leave device unchanged | None | `GpuDevice` with `pci_vendor_id: 0x9999, pci_device_id: 0x9999` | `arch = None`, caps unchanged | `cargo test -p anvilml-hardware -- test_resolve_unknown_device` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_resolve_cpu_fallback` | CPU device (zero PCI IDs) resolves to no row | None | `GpuDevice` with `pci_vendor_id: 0, pci_device_id: 0` | `arch = None`, caps unchanged | `cargo test -p anvilml-hardware -- test_resolve_cpu_fallback` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_resolve_vram_untouched` | VRAM fields preserved after resolve | None | `GpuDevice` with `pci_vendor_id: 0x10de, pci_device_id: 0x2488` | `vram_total_mib` and `vram_free_mib` unchanged | `cargo test -p anvilml-hardware -- test_resolve_vram_untouched` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_resolve_name_overwrite` | Canonical name from DEVICE_DB overwrites `name` | None | `GpuDevice` with `pci_vendor_id: 0x10de, pci_device_id: 0x2488` | `name = "NVIDIA RTX 4090"` | `cargo test -p anvilml-hardware -- test_resolve_name_overwrite` exits 0 |
| `crates/anvilml-hardware/tests/device_db_tests.rs` | `test_device_db_non_empty` | DEVICE_DB contains ≥ 12 entries | None | None | `DEVICE_DB.len() >= 12` | `cargo test -p anvilml-hardware -- test_device_db_non_empty` exits 0 |
| `crates/anvilml-server/tests/system_tests.rs` | `test_system_returns_200_with_hardware_info` | GET /v1/system returns 200 with valid hardware info including `db_name: null` in JSON | None | Synthetic `HardwareInfo` with 1 GPU | HTTP 200, `gpus[0].db_name` is `null` in JSON | `cargo test -p anvilml-server -- test_system_returns_200_with_hardware_info` exits 0 |

## CI Impact

No CI changes required. The `cargo test --workspace --features mock-hardware` command (used by `rust-linux` and `rust-windows` CI jobs) will pick up the new `db_name` field in all construction sites automatically. The `config-drift` job runs `cargo test -p anvilml --features mock-hardware -- config_reference`, which tests `ServerConfig` — this task does not modify `ServerConfig`, so no config drift. The `openapi-drift` job regenerates `openapi.json` and checks for diff; the new `db_name` field is `Option<String>` which adds a field to the `GpuDevice` schema — if `anvilml-openapi` has not been regenerated to include it, the gate will fail. However, since this is a pure data addition (no handler signature changes, no `ToSchema` derive changes), the existing `openapi.json` already defines `GpuDevice` with its fields, and adding a new field to the struct means the regenerated OpenAPI will include it — so the gate may flag a drift. If that occurs, the ACT agent should regenerate `openapi.json` via `cargo run -p anvilml-openapi` and stage it.

## Platform Considerations

None identified. The `db_name: Option<String>` field is platform-neutral:
- `Option<String>` serialises identically on all platforms (JSON `null`).
- No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.
- The field is placed after `name` in the struct, maintaining consistent field ordering across all construction sites.
- The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A `GpuDevice` construction site was missed, causing a compilation error | Low | Medium | Use `cargo build --workspace` as the definitive check — the compiler will list every site that fails. The grep search found exactly 10 construction sites across the workspace (9 source files + 1 test file). After editing all 10, run `cargo build --workspace --features mock-hardware` to confirm zero errors before running tests. |
| The `db_name` field position after `name` causes a field ordering mismatch in one of the 10 struct literals | Low | Low | Each edit is a single-line insertion: `db_name: None,` after the `name:` line. The field ordering in every literal matches the struct definition. No reordering of existing fields occurs. |
| OpenAPI drift gate fails because the regenerated `openapi.json` now includes `db_name` in the `GpuDevice` schema | Medium | Low | If the `openapi-drift` CI job fails after this task lands, the ACT agent for P6-C2 or a follow-up task should regenerate `openapi.json` via `cargo run -p anvilml-openapi` and stage it. This is a benign drift — the new field is `null` in all current responses. |
| The new field changes the JSON output of `GET /v1/system`, potentially breaking clients that do not expect `db_name` | Low | Low | `db_name` is `Option<String>` with value `None` everywhere, so it serialises as `"db_name": null`. This is a backwards-compatible addition — clients that ignore unknown fields are unaffected. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0 (no struct initialisation errors)
- [ ] `cargo test -p anvilml-core -- test_hardware_info_json_roundtrip` exits 0
- [ ] `cargo test -p anvilml-hardware -- device_db_tests` exits 0 (all 7 device_db tests pass)
- [ ] `cargo test -p anvilml-server -- test_system_returns_200_with_hardware_info` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace test suite)
- [ ] `grep -c "db_name: None" crates/anvilml-core/tests/hardware_tests.rs crates/anvilml-hardware/src/detect.rs crates/anvilml-hardware/src/mock.rs crates/anvilml-hardware/src/vulkan.rs crates/anvilml-hardware/src/cpu.rs crates/anvilml-hardware/src/dxgi.rs crates/anvilml-hardware/src/sysfs.rs crates/anvilml-hardware/tests/device_db_tests.rs crates/anvilml-server/tests/system_tests.rs` sums to 10 (all construction sites updated)
