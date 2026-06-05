# Plan Report: P7-G4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-G4                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | Fix name resolution priority and --print-hardware display |
| Depends on  | P7-G3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-05T17:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix the GPU name resolution priority so that a generic or empty driver-supplied name is replaced by the database group label, while a specific Vulkan SKU name is preserved with the group label shown in parentheses. Also fix `print_hardware_table` to display the name correctly using the new `db_group_name` field.

## Scope

### In Scope
- Add `db_group_name: Option<String>` field to `GpuDevice` struct in `crates/anvilml-core/src/types/hardware.rs`
- Update `resolve_caps_from_row` in `crates/anvilml-hardware/src/device_db.rs`:
  - Hit path: if `dev.name` is empty or generic → `dev.name = row.model_name.clone()`, `dev.db_group_name = None`; else → preserve `dev.name`, set `dev.db_group_name = Some(row.model_name.clone())`
  - Miss path: if `dev.name` is empty or generic → `dev.name = format!("Unknown GPU (0x{:04X}:0x{:04X})", ...)`, else keep existing name
- Add helper function `is_generic_driver_name(s: &str) -> bool` to `device_db.rs`
- Update `print_hardware_table` in `backend/src/main.rs` to use display name with `({db_group_name})` suffix when it differs from the primary name
- Add four unit tests: `generic_name_replaced_by_group_label`, `specific_vulkan_name_preserved`, `miss_with_empty_name_shows_unknown`, `miss_with_specific_name_preserved`

### Out of Scope
- Adding new device entries to SUPPORTED_DEVICES_DB.md
- Modifying the seed loading infrastructure (handled by P7-G3)
- Changing the `--print-hardware` CLI argument parsing or output format beyond name display
- Adding `db_group_name` to any API response schemas beyond what's implied by the struct field

## Approach

1. **Add `db_group_name` field to `GpuDevice`** in `crates/anvilml-core/src/types/hardware.rs`:
   - Add `pub db_group_name: Option<String>` after the existing `capabilities_source` field
   - Derive with `#[serde(default)]` so old JSON deserializes cleanly

2. **Implement `is_generic_driver_name` helper** in `crates/anvilml-hardware/src/device_db.rs`:
   - Returns `true` for: empty string, `"AMD Radeon Graphics"`, `"AMD proprietary driver"`, or pattern `"Device {hex}"`
   - Keep the list intentionally non-exhaustive per task notes

3. **Rewrite `resolve_caps_from_row` hit path** in `device_db.rs`:
   - Check if `dev.name.is_empty() || is_generic_driver_name(&dev.name)`
   - If yes: `dev.name = r.model_name.clone()`, `dev.db_group_name = None`
   - If no (specific Vulkan SKU name): keep `dev.name` as-is, set `dev.db_group_name = Some(r.model_name.clone())`
   - Keep existing arch/caps/source/enum_source assignments unchanged

4. **Rewrite `resolve_caps_from_row` miss path** in `device_db.rs`:
   - Check if `dev.name.is_empty() || is_generic_driver_name(&dev.name)`
   - If yes: `dev.name = format!("Unknown GPU (0x{:04X}:0x{:04X})", dev.pci_vendor_id, dev.pci_device_id)`
   - If no (specific name already set): keep existing name, do nothing else
   - Keep existing warning log and caps defaults unchanged

5. **Update `print_hardware_table`** in `backend/src/main.rs`:
   - Before the existing name truncation logic, compute display name:
     ```rust
     let display_name = match &dev.db_group_name {
         Some(group) if group != &dev.name => format!("{} ({})", dev.name, group),
         _ => dev.name.clone(),
     };
     let name_trunc: String = display_name.chars().take(20).collect();
     ```

6. **Add unit tests** in `device_db.rs` test module:
   - `generic_name_replaced_by_group_label`: device with empty or generic name, hit row → name becomes model_name, db_group_name is None
   - `specific_vulkan_name_preserved`: device with specific Vulkan SKU name, hit row → name unchanged, db_group_name = Some(model_name)
   - `miss_with_empty_name_shows_unknown`: device with empty name, miss → name = "Unknown GPU (0x...)"; db_group_name is None
   - `miss_with_specific_name_preserved`: device with specific name, miss → name unchanged

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Add `db_group_name: Option<String>` field to `GpuDevice` struct |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Add `is_generic_driver_name()` helper; rewrite `resolve_caps_from_row` hit/miss paths; add 4 unit tests |
| Modify | `backend/src/main.rs` | Update `print_hardware_table` to compute display name with `(db_group_name)` suffix |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `generic_name_replaced_by_group_label` | Empty/generic driver name is replaced by database model name on hit; db_group_name = None |
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `specific_vulkan_name_preserved` | Specific Vulkan SKU name is preserved on hit; db_group_name = Some(model_name) |
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `miss_with_empty_name_shows_unknown` | Miss with empty name sets fallback "Unknown GPU (0x{v}:0x{d})" |
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `miss_with_specific_name_preserved` | Miss with specific name keeps existing name unchanged |

## CI Impact

No CI workflow changes required. The task only modifies source code and adds unit tests within existing crates. The acceptance criterion is `cargo test --workspace --features mock-hardware` exiting 0, which runs under the existing CI test gate. No new dependencies are introduced.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Adding `db_group_name` to `GpuDevice` changes the struct layout — any code that clones or serializes `GpuDevice` will pick up the new field automatically, but existing serialized data will miss it. | Use `#[serde(default)]` on the field so old JSON deserializes to `None`. The field is only used for display in `--print-hardware`, not in API responses that need backward compat. |
| `is_generic_driver_name` pattern matching could be brittle if new generic driver strings appear. | The function is intentionally non-exhaustive per task notes; new generic names can be added later without a new task. The miss path always has a fallback (`"Unknown GPU (0x...)"`). |
| Display name truncation at 20 chars may cut off mid-parenthesis when showing `name (group)`. | Acceptable trade-off — the column is narrow. If truncation occurs, the truncated display still conveys the primary device name which is the most important information. |

## Acceptance Criteria

- [ ] `db_group_name: Option<String>` field added to `GpuDevice` in `crates/anvilml-core/src/types/hardware.rs`
- [ ] `is_generic_driver_name()` helper function present in `device_db.rs`
- [ ] `resolve_caps_from_row` hit path replaces generic/empty names and preserves specific names with `db_group_name` set
- [ ] `resolve_caps_from_row` miss path sets `"Unknown GPU (0x{v}:0x{d})"` for empty/generic names, preserves specific names
- [ ] `print_hardware_table` in `main.rs` displays name with `(group)` suffix when `db_group_name` differs from name
- [ ] Four unit tests added and passing: `generic_name_replaced_by_group_label`, `specific_vulkan_name_preserved`, `miss_with_empty_name_shows_unknown`, `miss_with_specific_name_preserved`
- [ ] `cargo test --workspace --features mock-hardware` exits 0
