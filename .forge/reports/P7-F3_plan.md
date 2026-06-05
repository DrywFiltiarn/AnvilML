# Plan Report: P7-F3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-F3                                              |
| Phase       | 007 — WebSocket Event Stream                       |
| Description | anvilml-hardware: SEED_ENTRIES from SUPPORTED_DEVICES_DB.md + resolve_caps_from_row |
| Depends on  | P7-F2                                               |
| Project     | anvilml                                             |
| Planned at  | 2026-06-05T13:45:00Z                                |
| Attempt     | 1                                                   |

## Objective

Rewrite `crates/anvilml-hardware/src/device_db.rs` to replace the small, hand-seeded `PCI_CAPABILITY_TABLE` (6 entries) with a full `SEED_ENTRIES` const containing all 126 device entries from `docs/SUPPORTED_DEVICES_DB.md`. Replace the old `lookup()` and `resolve_caps()` functions with a new `resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceCapabilityRow>)` that reads capability data from an `anvilml-registry::DeviceCapabilityRow` at runtime. Add `anvilml-registry` as a workspace dependency to `anvilml-hardware`. Rewrite all existing tests to use `SEED_ENTRIES.iter().find()` and add the required `rx9070xt_entry_correct` test.

## Scope

### In Scope
- Rewrite `DeviceCapabilityEntry` struct: expand from 7 fields (vendor_id, device_id, model_name, arch, fp16, bf16, flash_attention) to 11 fields (vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn) in canonical order matching `DeviceCapabilityRow`.
- Build `pub const SEED_ENTRIES: &[DeviceCapabilityEntry]` with all 126 rows from both the NVIDIA and AMD tables in `docs/SUPPORTED_DEVICES_DB.md`, copied verbatim (Y→true, N→false).
- Implement `resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceCapabilityRow>)`:
  - **Hit:** set `dev.name = row.model_name.clone()`, `dev.arch = Some(row.arch.clone())`, populate all seven `InferenceCaps` fields from the row, set `dev.capabilities_source = CapabilitySource::DeviceTable`, `dev.enumeration_source = EnumerationSource::DeviceTable`.
  - **Miss:** preserve `dev.name` (do not overwrite driver-reported name), set `caps = InferenceCaps::default()`, `capabilities_source = Fallback`, emit `tracing::warn!` with PCI IDs.
- Remove `lookup()` and `resolve_caps()` entirely.
- Rewrite all existing tests to use `SEED_ENTRIES.iter().find(|e| e.vendor_id == v && e.device_id == d)` in place of `lookup()`.
- Add `rx9070xt_entry_correct` test asserting vendor_id=0x1002, device_id=0x7550 resolves to model_name="AMD Radeon RX 9070 XT", arch="gfx1201", fp8=true, fp32=false.
- Update `crates/anvilml-hardware/Cargo.toml` to add `anvilml-registry = { workspace = true }`.
- Add `anvilml-registry` to `[workspace.dependencies]` in root `Cargo.toml` as `{ path = "crates/anvilml-registry" }`.
- Run `cargo tree -p anvilml-hardware` to verify no dependency cycle.

### Out of Scope
- Modifying `detect_all_devices` or any call sites (that is P7-F4).
- Creating the SQL seed file (that is P7-G1).
- Replacing `SEED_ENTRIES` with `SeedLoader` (that is P7-G3).
- Any changes to `anvilml-core`, `anvilml-server`, `backend`, or other crates.

## Approach

1. **Add `anvilml-registry` to workspace dependencies.** In root `Cargo.toml`, add `anvilml-registry = { path = "crates/anvilml-registry" }` under `[workspace.dependencies]`. This is a minimal, non-breaking change — it does not affect any existing crate versions.

2. **Update `anvilml-hardware/Cargo.toml`.** Add `anvilml-registry = { workspace = true }` under `[dependencies]`. This introduces a new intra-workspace dependency: `hardware → registry → core`. The direction is permitted per the architecture rules (registry sits below hardware in the dependency graph).

3. **Rewrite `DeviceCapabilityEntry` struct.** Replace the 7-field struct with an 11-field struct matching `DeviceCapabilityRow` field order exactly:
   ```rust
   pub struct DeviceCapabilityEntry {
       pub vendor_id: u16,
       pub device_id: u16,
       pub model_name: &'static str,
       pub arch: &'static str,
       pub fp32: bool,
       pub fp16: bool,
       pub bf16: bool,
       pub fp8: bool,
       pub fp4: bool,
       pub nvfp4: bool,
       pub flash_attention: bool,
   }
   ```

4. **Build `SEED_ENTRIES` from SUPPORTED_DEVICES_DB.md.** Copy all 126 data rows verbatim from the two Markdown tables (NVIDIA lines 69–145, AMD lines 183–231). For each row:
   - Parse hex vendor_id and device_id from columns like `0x10DE`, `0x7550`.
   - Copy model_name string verbatim (single-quoted-safe; contains apostrophes in names like "NVIDIA GeForce RTX 4090").
   - Copy arch string verbatim (e.g., `"8.6"`, `"gfx1100"`, `"gfx90a"`).
   - Map `Y` → `true`, `N` → `false` for all seven boolean fields in canonical order: fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn.

5. **Implement `resolve_caps_from_row`.**
   ```rust
   pub fn resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceCapabilityRow>) {
       match row {
           Some(r) => {
               dev.name = r.model_name.clone();
               dev.arch = Some(r.arch.clone());
               dev.caps = InferenceCaps {
                   fp32: r.fp32,
                   fp16: r.fp16,
                   bf16: r.bf16,
                   fp8: r.fp8,
                   fp4: r.fp4,
                   nvfp4: r.nvfp4,
                   flash_attention: r.flash_attn,
               };
               dev.capabilities_source = CapabilitySource::DeviceTable;
               dev.enumeration_source = EnumerationSource::DeviceTable;
           }
           None => {
               tracing::warn!(
                   detector = "DeviceDB",
                   vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
                   device_id = %format_args!("0x{:04X}", dev.pci_device_id),
                   "unknown PCI ID — add to SUPPORTED_DEVICES_DB.md"
               );
               dev.caps = InferenceCaps::default();
               dev.capabilities_source = CapabilitySource::Fallback;
           }
       }
   }
   ```

6. **Remove `lookup()` and `resolve_caps()`.** Both are deleted entirely. All callers (currently none in this task's scope) will be updated in P7-F4.

7. **Rewrite tests.** Replace all test functions that used `lookup()` with equivalent code using `SEED_ENTRIES.iter().find()`. Update struct literal constructions to include the four new fields (fp32, fp8, fp4, nvfp4). Keep the `no_duplicate_pci_ids`, `arch_format_validation`, and `seed_entry_integrity` tests but have them iterate over `SEED_ENTRIES` instead of `PCI_CAPABILITY_TABLE`.

8. **Add `rx9070xt_entry_correct` test.** Assert that vendor_id=0x1002, device_id=0x7550 resolves to model_name="AMD Radeon RX 9070 XT", arch="gfx1201", fp8=true, fp32=false.

9. **Verify no dependency cycle.** Run `cargo tree -p anvilml-hardware` and confirm `anvilml-registry` appears in the subtree without `anvilml-hardware` looping back into it.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Add `anvilml-registry = { path = "crates/anvilml-registry" }` to `[workspace.dependencies]` |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Add `anvilml-registry = { workspace = true }` under `[dependencies]` |
| Rewrite | `crates/anvilml-hardware/src/device_db.rs` | Complete rewrite: new struct, SEED_ENTRIES (126 rows), resolve_caps_from_row, rewritten tests |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-hardware/src/device_db.rs` | `seed_entries_lookup` (rewritten) | SEED_ENTRIES.iter().find() resolves known PCI IDs correctly for all 6 original seeded cards |
| `crates/anvilml-hardware/src/device_db.rs` | `miss_returns_none` (rewritten) | find() returns None for unknown PCI ID pairs |
| `crates/anvilml-hardware/src/device_db.rs` | `no_duplicate_pci_ids` (rewritten) | No two SEED_ENTRIES share the same (vendor_id, device_id) pair |
| `crates/anvilml-hardware/src/device_db.rs` | `arch_format_validation` (rewritten) | All 126 arch strings match CUDA SM ("X.Y") or AMD gfx ("gfx\d{4}") format |
| `crates/anvilml-hardware/src/device_db.rs` | `boolean_flag_consistency` (rewritten) | Capability flags are internally consistent per architecture generation |
| `crates/anvilml-hardware/src/device_db.rs` | `field_count_no_vram` (updated struct literal) | Struct literals compile with all 11 fields; no VRAM fields present |
| `crates/anvilml-hardware/src/device_db.rs` | `seed_entry_integrity` (rewritten) | All 126 model_name and arch strings are non-empty and within length bounds |
| `crates/anvilml-hardware/src/device_db.rs` | `rx9070xt_entry_correct` (new) | AMD Radeon RX 9070 XT entry has correct PCI IDs, name, arch, fp8=true, fp32=false |

## CI Impact

No CI workflow files are modified. The new dependency on `anvilml-registry` is an intra-workspace path dependency, not an external crate — no version resolution or lockfile changes beyond what `cargo build` produces. CI tests with `--features mock-hardware` will exercise the rewritten tests. The `mock-hardware` feature does not gate any code in `device_db.rs`, so the module is always compiled and tested.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| 126 SEED_ENTRIES entries are a large const; any typo in a hex ID or boolean flag will be silent until a WARN log fires at runtime | The `rx9070xt_entry_correct` test and the rewritten `seed_entries_lookup` test cover key entries across both vendors and architectures. The `no_duplicate_pci_ids` test catches accidental duplicates. Copy values verbatim from SUPPORTED_DEVICES_DB.md — no inference or memory-based generation. |
| Adding `anvilml-registry` to `anvilml-hardware` creates a new intra-workspace dependency; if the direction is wrong it causes a cycle | The permitted direction is `hardware → registry → core`. Verify with `cargo tree -p anvilml-hardware` before committing. If a cycle exists, stop and document under Blockers. |
| `DeviceCapabilityRow` from `anvilml-registry` uses owned `String` for model_name/arch; SEED_ENTRIES uses `&'static str` — type mismatch in resolve_caps_from_row | The function signature takes `Option<&DeviceCapabilityRow>` (owned strings), and on hit clones them into `dev.name` and `dev.arch`. On the struct side, SEED_ENTRIES entries use `&'static str` which is fine since they are const data. No conflict. |
| The task requires removing `lookup()` and `resolve_caps()` but P7-F4 still references them in its plan | This is correct — the task explicitly says remove them and P7-F4 (the next task) will replace all call sites. The workspace will not compile until P7-F4 lands, which is expected sequential dependency behavior. |
| `anvilml-registry` is not yet in `[workspace.dependencies]` — adding it is a root-level change outside the crate scope | The plan includes this as step 1. It is a minimal addition that does not affect any other crate and is explicitly permitted by the task instructions (`anvilml-registry={workspace=true}`). |

## Acceptance Criteria

- [ ] `SEED_ENTRIES` contains exactly 126 entries (count verified at build time via `assert!(SEED_ENTRIES.len() == 126)`)
- [ ] `DeviceCapabilityEntry` has 11 fields in canonical order: vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attention
- [ ] `lookup()` and `resolve_caps()` are removed — no references to these symbols remain in the module
- [ ] `resolve_caps_from_row` hit-path sets dev.name, dev.arch, all 7 InferenceCaps fields, capabilities_source=DeviceTable, enumeration_source=DeviceTable
- [ ] `resolve_caps_from_row` miss-path preserves dev.name, sets caps=default(), capabilities_source=Fallback, emits tracing::warn!
- [ ] All existing tests rewritten to use `SEED_ENTRIES.iter().find()` — `cargo test -p anvilml-hardware` exits 0
- [ ] `rx9070xt_entry_correct` test passes asserting vendor_id=0x1002, device_id=0x7550 → model_name="AMD Radeon RX 9070 XT", arch="gfx1201", fp8=true, fp32=false
- [ ] `anvilml-registry = { workspace = true }` added to `crates/anvilml-hardware/Cargo.toml`
- [ ] `anvilml-registry` path dependency added to root `[workspace.dependencies]`
- [ ] `cargo tree -p anvilml-hardware` confirms no cycle (hardware → registry, but not registry → hardware)
