# Implementation Report: P7-F3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-F3                                              |
| Phase       | 007 — WebSocket Event Stream                       |
| Description | anvilml-hardware: SEED_ENTRIES from SUPPORTED_DEVICES_DB.md + resolve_caps_from_row |
| Implemented | 2026-06-05T16:45:00Z                               |
| Status      | COMPLETE                                           |

## Summary

Rewrote `crates/anvilml-hardware/src/device_db.rs` to replace the small 6-entry `PCI_CAPABILITY_TABLE` with a full `SEED_ENTRIES` const containing all 126 device entries from `docs/SUPPORTED_DEVICES_DB.md` (77 NVIDIA + 49 AMD). Expanded `DeviceCapabilityEntry` from 7 to 11 fields matching `DeviceCapabilityRow` canonical order. Removed `lookup()` and `resolve_caps()`, replacing them with `resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceCapabilityRow>)`. Added `anvilml-registry` as a workspace dependency consumed by `anvilml-hardware`. Updated callers in `lib.rs` to use the new API. All 63 hardware crate tests pass, all 180+ workspace tests pass.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source        |
|--------|-------------------|-----------------|---------------|
| crate  | anvilml-registry  | path dep        | intra-workspace |

No external dependencies added — `anvilml-registry` is an existing workspace member at `crates/anvilml-registry`, using only workspace dependencies (`anvilml-core`, `sqlx`). No MCP lookup required.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Added `anvilml-registry = { path = "crates/anvilml-registry" }` to `[workspace.dependencies]` |
| Modify | `crates/anvilml-hardware/Cargo.toml` | Added `anvilml-registry = { workspace = true }` under `[dependencies]` |
| Rewrite | `crates/anvilml-hardware/src/device_db.rs` | New 11-field struct, 126-entry SEED_ENTRIES, resolve_caps_from_row, 10 rewritten tests |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Updated both mock-hardware and real-hardware branches to use resolve_caps_from_row with SEED_ENTRIES lookup |

## Commit Log

```
 .forge/reports/P7-F3_plan.md             |  154 +++
 .forge/state/CURRENT_TASK.md             |    6 +-
 .forge/state/state.json                  |   13 +-
 Cargo.lock                               |    1 +
 Cargo.toml                               |    1 +
 crates/anvilml-hardware/Cargo.toml       |    1 +
 crates/anvilml-hardware/src/device_db.rs | 1928 ++++++++++++++++++++++++++++--
 crates/anvilml-hardware/src/lib.rs       |   38 +-
 8 files changed, 2037 insertions(+), 105 deletions(-)
```

## Test Results

Full workspace test suite (`cargo test --workspace --features mock-hardware`):

```
anvilml-core:    74 passed; 0 failed
anvilml_hardware: 63 passed; 0 failed (includes 10 device_db tests)
anvilml_ipc:      0 passed; 0 failed
anvilml_openapi:  0 passed; 0 failed
anvilml_registry: 13 passed; 0 failed (unit) + 1 integration + 6 device_store + 2 rescan + 1 scanner + 2 store_get + 3 store_list = 28 total
anvilml_scheduler: 0 passed; 0 failed
anvilml_server:   8 passed; 0 failed (unit) + 3 api_models + 1 api_ws_events = 12 total
anvilml_worker:   0 passed; 0 failed
anvilml (bin):    8 passed; 0 failed
backend config_reference: 1 passed; 0 failed
Doc-tests anvilml_hardware: 2 passed; 0 failed
```

All 180+ tests pass with zero failures.

## Platform Cross-Check

All three checks exited 0:

**1. Mock-hardware Windows-gnu cross-check:**
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.82s
```

**2. Real-hardware Linux native:**
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.88s
```

**3. Real-hardware Windows-gnu cross-check:**
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.24s
```

## Project Gates

**Config drift gate** (`cargo test -p backend --features mock-hardware`):
```
Running tests/config_reference.rs
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

None — all 9 plan steps implemented exactly as specified. The `boolean_flag_consistency` test expectations were updated to match the full 126-entry database (the old 6-entry table had different capability values than the authoritative SUPPORTED_DEVICES_DB.md). Specifically:
- SM 8.6 consumer Ampere cards have `flash_attention=true` per the DB (plan's test expected false based on old table)
- AMD RDNA1 (gfx101x) cards have `fp16=false, bf16=false` — added explicit match arm
- AMD cut-down cards (RX 6500 XT, RX 6400, RX 7400) have `bf16=false, flash_attn=false` — added explicit match arms
- AMD CDNA1 (MI100, gfx908) has `bf16=false` — added explicit match arm

## Blockers

None.
