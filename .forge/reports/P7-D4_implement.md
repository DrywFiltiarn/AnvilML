# Implementation Report: P7-D4

| Field       | Value                                              |
|-------------|----------------------------------------------------|
| Task ID     | P7-D4                                              |
| Phase       | 007 — WebSocket Event Stream                       |
| Description | fix OS field blank and stray colon in --print-hardware output |
| Implemented | 2026-06-05T00:00:00Z                               |
| Status      | COMPLETE                                           |

## Summary

Fixed two bugs causing the `--print-hardware` CLI flag to display a blank OS field with a stray trailing colon. In `backend/src/main.rs`, the format string used `" ".repeat(50 - 8)` instead of the actual `hw.host.os` value, and had an extra `:` in the format specifier. In `crates/anvilml-hardware/src/lib.rs`, `populate_host_info()` called `sysinfo::System::name()` which returns `Some("")` on many systems (including Linux) instead of `None`, producing an empty OS string. The fix uses `long_os_version()` as the primary source with a non-empty filter and `name()` as fallback.

## Resolved Dependencies

No new dependencies added or modified. No MCP lookups required for this task.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Fix OS `println!` in `print_hardware_table()` — replace stray colon and blank field with `hw.host.os` interpolation (line 17) |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Fix `populate_host_info()` OS resolution — use `long_os_version()` with `.filter(|s| !s.is_empty())` guard, clippy-reduced redundant closure (lines 84-87) |

## Commit Log

```
.forge/reports/P7-D4_plan.md       | 92 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |  6 +--
 .forge/state/state.json            | 13 +++---
 backend/src/main.rs                |  2 +-
 crates/anvilml-hardware/src/lib.rs |  5 ++-
 5 files changed, 107 insertions(+), 11 deletions(-)
```

## Test Results

```
running 74 tests
test config::tests::test_device_type_default ... ok
... (all 74 tests passed)
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 59 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
... (all 59 tests passed, including host_info_populated)
test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests (anvilml_ipc)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests (anvilml_openapi)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 11 tests (anvilml_registry)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test (registry db integration)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests (registry rescan integration)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test (registry scanner integration)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests (registry store_get integration)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 3 tests (registry store_list integration)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests (anvilml_scheduler)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 8 tests (anvilml_server)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 3 tests (server api_models integration)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test (server api_ws_events integration)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 0 tests (anvilml_worker)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 8 tests (backend cli tests)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test (config_reference integration)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Doc-tests anvilml_core — 0 tests, all ok
Doc-tests anvilml_hardware — 2 tests, all ok (sysfs::parse_pci_id, sysfs::read_vram_from_amdgpu_sysfs)
Doc-tests anvilml_ipc — 0 tests, all ok
Doc-tests anvilml_registry — 0 tests, all ok
Doc-tests anvilml_scheduler — 0 tests, all ok
Doc-tests anvilml_server — 0 tests, all ok
Doc-tests anvilml_worker — 0 tests, all ok

Total: 173 passed; 0 failed; 0 ignored
```

## Platform Cross-Check

### a) `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.71s
```

### b) `cargo check --bin anvilml`
```
Blocking waiting for file lock on build directory
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.53s
```

### c) `cargo check --bin anvilml --target x86_64-pc-windows-gnu`
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.55s
```

All three cross-checks passed with zero errors.

## Project Gates

| Gate | Command | Result |
|------|---------|--------|
| Config drift gate | `cargo test -p backend --features mock-hardware --test config_reference` | ok. 1 passed; 0 failed |

## Deviations from Plan

- **Clippy fix (redundant_closure):** The plan specified `.or_else(|| sysinfo::System::name())` but clippy (`-D warnings`) flagged this as `redundant_closure` — the closure is unnecessary since `sysinfo::System::name` already matches the required function signature. Changed to `.or_else(sysinfo::System::name)`. This is a minimal, correct fix that does not change behavior.

## Blockers

None. All gates passed, all tests passed, all cross-checks passed.
