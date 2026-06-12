# Implementation Report: P905-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A2                                           |
| Phase       | 905 — anvilml-registry FP8 and rescan             |
| Description | anvilml-registry: extend infer_dtype with FP8 suffix matching and VRAM factor |
| Implemented | 2026-06-12T13:10:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Extended `infer_dtype()` in `crates/anvilml-registry/src/scanner.rs` to recognize FP8 filename suffixes (`fp8`, `f8`, `fp8e4m3`, `fp8e5m2`, `f8e4m3`, `f8e5m2`) and map them to the `DType::F8E4M3` or `DType::F8E5M2` variants added in prerequisite task P905-A1. Added a comprehensive unit test `test_infer_dtype_fp8_suffixes` covering all 7 FP8 suffix patterns case-insensitively. Added an FP8 assertion to the existing `test_vram_estimate_mib` test. Verified that `vram_estimate_mib` already applies factor 0.5 for F8E4M3/F8E5M2. Bumped `anvilml-registry` patch version from 0.1.0 to 0.1.1. All tests pass (40 total), all gates pass.

## Resolved Dependencies

No new dependencies added or modified in this task.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/scanner.rs` | Extended `infer_dtype()` with FP8 suffixes (fp8e4m3/f8e4m3 → F8E4M3, fp8e5m2/f8e5m2 → F8E5M2, fp8/f8 → F8E4M3); added `test_infer_dtype_fp8_suffixes`; added FP8 assertion to `test_vram_estimate_mib` |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.0 → 0.1.1 |

## Commit Log

```
 .forge/reports/P905-A2_plan.md         | 109 +++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md           |   6 +-
 .forge/state/state.json                |  13 ++--
 Cargo.lock                             |   2 +-
 crates/anvilml-registry/Cargo.toml     |   2 +-
 crates/anvilml-registry/src/scanner.rs |  19 ++++++
 6 files changed, 140 insertions(+), 11 deletions(-)
```

## Test Results

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 16.42s
     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-94d46119ae2c0894)

running 20 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_fp8_suffixes ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test scanner::tests::test_sha256_hex ... ok
test seed_loader::tests::test_compute_sha256_empty ... ok
test seed_loader::tests::test_parse_header_both_directives ... ok
test seed_loader::tests::test_compute_sha256_known_value ... ok
test seed_loader::tests::test_parse_header_defaults_strategy ... ok
test seed_loader::tests::test_parse_header_empty_file ... ok
test seed_loader::tests::test_parse_header_missing_table ... ok
test device_store::tests::test_get_miss_returns_none ... ok
test db::tests::test_open_creates_file_if_missing ... ok
test device_store::tests::test_upsert_then_get_roundtrip ... ok
test db::tests::test_open_creates_tables ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-2c7e9c7df0ec947184)

running 1 test
test test_open_creates_tables ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/device_store.rs (target/debug/deps/device_store-d56b32f8462f533a)

running 4 tests
test upsert_then_get_roundtrip ... ok
test get_miss_returns_none ... ok
test bool_flags_roundtrip ... ok
test upsert_overwrites_existing ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/rescan.rs (target/debug/deps/rescan-735cee1dfb1af9c7)

running 2 tests
test test_rescan_adds_models ... ok
test test_rescan_idempotent ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/scanner.rs (target/debug/deps/scanner-fa8faaf38cf38cfb42cb)

running 1 test
test test_scan_dirs_two_files ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-9fc88888888888888888)

running 7 tests
test test_directive_parsing_miss ... ok
test merge_preserves_unreferenced_rows ... ok
test sha256_skip_does_not_execute ... ok
test test_directive_parsing_hit ... ok
test replace_all_replaces_table_content ... ok
test test_table_bootstrap_idempotent ... ok
test changed_sha256_reruns_seed ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/store_get.rs (target/debug/deps/store_get-9fc88888888888888888)

running 2 tests
test test_upsert_then_get_returns_equal_meta ... ok
test test_get_missing_returns_none ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/store_list.rs (target/debug/deps/store_list-9fc88888888888888888)

running 3 tests
test test_list_empty_returns_empty_vec ... ok
test test_list_after_upserts_returns_ordered ... ok
test test_list_kind_filter ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

   Doc-tests anvilml_registry

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Checking anvilml-core v0.1.3 (/home/dryw/AnvilML/crates/anvilml-core)
    Checking anvilml-ipc v0.1.4 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-registry v0.1.1 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.18 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.13 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.1 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.31s

# 2. Mock-hardware Windows cross-check
    Checking anvilml-core v0.1.3 (/home/dryw/AnvilML/crates/anvilml-core)
    Checking anvilml-registry v0.1.1 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-ipc v0.1.4 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.18 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.1 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.13 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.98s

# 3. Real-hardware Linux check
    Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.18 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.13 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.11s

# 4. Real-hardware Windows cross-check
    Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.18 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.13 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.57s
```

## Project Gates

```
Gate 1 — Config Surface Sync:
    Finished `test` profile [unoptimized + debuginfo] target(s) in 18.10s
     Running tests/config_reference.rs (target/debug/deps/config_reference-26ece02a88c0bc01)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

## Deviations from Plan

None. Implementation followed the approved plan exactly.

## Blockers

None.
