# Implementation Report: P4-A4B

| Field | Value |
|-------|-------|
| Task ID | P4-A4B |
| Phase | 004 — Hardware Detection |
| Description | anvilml-hardware device_db PCI-ID capability table + resolution |
| Implemented | 2026-06-03T18:15:00Z |
| Status | COMPLETE |

## Summary

Implemented the PCI-ID capability database for `anvilml-hardware` as a single new module `device_db.rs`. The module defines `DeviceCapabilityEntry` with fields `vendor_id`, `device_id`, `model_name`, `arch`, `fp16`, `bf16`, and `flash_attention`, seeded with 6 entries covering NVIDIA (RTX 3090, A100, H100, RTX 3080) and AMD (RX 7900 XTX, MI250X). Provides `lookup()` for static-reference PCI ID lookup and `resolve_caps()` stub that sets `dev.name` from the matched entry on hit and logs a warning on miss. Seven unit tests verify seed lookups, miss behavior, duplicate-ID prevention, arch format validation, boolean flag consistency, field count/no-VRAM guarantee, and seed entry integrity. Module registered in `lib.rs` with one line addition. All gates pass: fmt, clippy (zero warnings), Windows cross-check (x86_64-pc-windows-gnu), full workspace tests (123 passed), and config drift gate.

## Resolved Dependencies

No new dependencies added. The task uses only existing workspace dependencies (`anvilml-core`, `log`).

| Type   | Name          | Version resolved | Source        |
|--------|---------------|-----------------|---------------|
| crate  | anvilml-core  | (workspace path) | Cargo.toml    |
| crate  | log           | 0.4             | Cargo.toml    |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | crates/anvilml-hardware/src/device_db.rs | New PCI-ID capability table module with DeviceCapabilityEntry struct, PCI_CAPABILITY_TABLE const slice, lookup(), resolve_caps(), and 7 unit tests |
| Modify | crates/anvilml-hardware/src/lib.rs | Added `pub mod device_db;` module registration line |
| Modify | crates/anvilml-hardware/src/sysfs.rs | Formatting-only change from `cargo fmt --all` (multi-line assertions collapsed) |

## Commit Log

```
 .forge/reports/P4-A4B_plan.md            |  79 +++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +-
 crates/anvilml-hardware/src/device_db.rs | 375 +++++++++++++++++++++++++++++++
 crates/anvilml-hardware/src/lib.rs       |   2 +
 crates/anvilml-hardware/src/sysfs.rs     |  10 +-
 6 files changed, 468 insertions(+), 17 deletions(-)
```

## Test Results

### Device DB unit tests (filtered)

```
running 7 tests
test device_db::tests::arch_format_validation ... ok
test device_db::tests::boolean_flag_consistency ... ok
test device_db::tests::field_count_no_vram ... ok
test device_db::tests::miss_returns_none ... ok
test device_db::tests::no_duplicate_pci_ids ... ok
test device_db::tests::seed_entries_lookup ... ok
test device_db::tests::seed_entry_integrity ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out
```

### Full workspace test suite

```
anvilml_core:     68 passed; 0 failed
anvilml_hardware: 44 passed; 0 failed (includes 7 device_db tests)
anvilml_ipc:      0 passed; 0 failed
anvilml_openapi:  0 passed; 0 failed
anvilml_registry: 0 passed; 0 failed
anvilml_scheduler: 0 passed; 0 failed
anvilml_server:   2 passed; 0 failed
anvilml_worker:   0 passed; 0 failed
anvilml (binary): 8 passed; 0 failed
config_reference: 1 passed; 0 failed
Doc-tests anvilml_hardware: 2 passed; 0 failed

Total: 125 tests passed; 0 failed
```

### Config drift gate

```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored
```

## Windows Cross-Check

```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.48s
```

Zero errors. Clean cross-compilation for x86_64-pc-windows-gnu.

## Config Drift Gate

Test `test_toml_key_set_matches_default` passed. No ServerConfig fields were added/renamed/removed by this task — only a new module in `anvilml-hardware`, so no config surface change.

## Deviations from Plan

- The plan specifies `DeviceCapabilityEntry` with 5 fields (model_name, arch, fp16, bf16, flash_attention). However, the linear scan lookup requires PCI ID fields on the entry to match against. Added `vendor_id: u16` and `device_id: u16` as struct fields so `lookup(vendor_id, device_id)` can iterate and find matching entries via `iter().find()`. This is necessary for the lookup function to work — without PCI IDs on the entry, there would be no way to match a query against table rows.
- The plan's `resolve_caps(dev: &mut GpuDevice)` signature was adjusted to `resolve_caps(dev: &mut GpuDevice, vendor_id: u16, device_id: u16)` because `GpuDevice` currently lacks `vendor_id` and `device_id` fields (those belong to P3-B2). The function accepts the PCI IDs as explicit parameters instead.
- The arch format validation test was adjusted to handle AMD's trailing-letter architecture identifiers (e.g., `gfx90a`) which have a non-digit suffix after the core digits.

## Blockers

None. All gates passed cleanly.
