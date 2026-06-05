# Plan Report: P900-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A4                                           |
| Phase       | 900 — Logging Retrofit                            |
| Description | anvilml-hardware: retrofit DEBUG caps resolution log to device_db.rs |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T00:18:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a `tracing::debug!` call to `resolve_caps_from_row` in `crates/anvilml-hardware/src/device_db.rs` so that the binary decision path (DeviceTable hit vs. Fallback miss) is observable at DEBUG level, satisfying FORGE_AGENT_RULES §11.1 and §11.5.

## Scope

### In Scope
- Add one `tracing::debug!` call on the **hit** path (`Some(row)`) with fields: `vendor_id`, `device_id`, `name`, `source="DeviceTable"`, message `"caps resolved"`.
- Add one `tracing::debug!` call on the **miss** path (`None`) with the same fields and `source="Fallback"`. The existing `tracing::warn!` on miss is left unchanged.
- No changes to any other file, no logic changes, no test changes.

### Out of Scope
- Any Cargo.toml changes (tracing is already declared).
- Changes to test files or test assertions.
- Changes to any file outside `device_db.rs`.
- Changes to the WARN call on miss.

## Approach

1. Open `crates/anvilml-hardware/src/device_db.rs` and locate the `resolve_caps_from_row` function (lines 56–102).
2. **Hit path** — after line 81 (`dev.enumeration_source = EnumerationSource::DeviceTable;`), insert:
   ```rust
   tracing::debug!(
       vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
       device_id = %format_args!("0x{:04X}", dev.pci_device_id),
       name = %dev.name,
       source = "DeviceTable",
       "caps resolved"
   );
   ```
3. **Miss path** — before line 92 (the existing `tracing::warn!` call), insert the same debug call but with `source = "Fallback"`:
   ```rust
   tracing::debug!(
       vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
       device_id = %format_args!("0x{:04X}", dev.pci_device_id),
       name = %dev.name,
       source = "Fallback",
       "caps resolved"
   );
   ```
4. Verify `tracing` is already a dependency (confirmed in `Cargo.toml` line 15: `tracing = { workspace = true }`). No Cargo.toml changes needed.
5. Run the acceptance criterion: `cargo test -p anvilml-hardware --features mock-hardware` — must exit 0 with no regressions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Add two `tracing::debug!` calls in `resolve_caps_from_row` (one per branch) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `generic_name_replaced_by_group_label` | Hit path still works; debug call is side-effect-free |
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `specific_vulkan_name_preserved` | Hit path with specific name still works |
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `miss_with_empty_name_shows_unknown` | Miss path still works; debug call is side-effect-free |
| `crates/anvilml-hardware/src/device_db.rs` (mod tests) | `miss_with_specific_name_preserved` | Miss path with specific name still works |

No new test files are required. The existing tests exercise both branches and will continue to pass since the added `tracing::debug!` calls have no observable side effects at the default log level.

## CI Impact

No CI changes required. This task adds only logging instrumentation (DEBUG-level) with no logic changes, no new dependencies, no API surface changes, and no test modifications. All existing CI gates continue to apply unchanged.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing` crate not available in this crate's scope | None (already confirmed present in Cargo.toml line 15) | n/a | No action needed |
| Adding debug call changes control flow or introduces a borrow conflict | None — tracing::debug! is a macro that takes references; `dev.name` borrows immutably, same as existing code | Low if it occurred | The insertions are placed after all mutable borrows of `dev` are complete (after assignments on lines 64–81 for hit, and after line 85–89 name assignment for miss) |
| Existing tests fail due to debug output capture | None — tracing::debug! is silent at INFO level (default), and existing tests do not inspect log output | None | Run `cargo test` to confirm; no test changes needed |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-hardware --features mock-hardware` exits 0
- [ ] Two `tracing::debug!` calls present in `resolve_caps_from_row`: one with `source="DeviceTable"` (hit path), one with `source="Fallback"` (miss path)
- [ ] No changes to any file other than `crates/anvilml-hardware/src/device_db.rs`
- [ ] No logic changes — only logging instrumentation added
