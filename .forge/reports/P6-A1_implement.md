# Implementation Report: P6-A1

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P6-A1                                         |
| Phase       | 006 — Model Registry                          |
| Description | anvilml-registry: model directory scanner     |
| Implemented | 2026-06-03T23:15:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Implemented the model directory scanner module for `anvilml-registry`. Added three dependencies (`walkdir`, `sha2`, `hex`) and created `src/scanner.rs` with functions for directory tree traversal, file discovery (`.safetensors`, `.ckpt`, `.pt`, `.bin`), deterministic ID generation via SHA-256 of canonical paths, kind inference from parent directory names, dtype inference from file stem suffixes, and VRAM estimation. Added unit tests for all helper functions and an integration test with a tempdir fixture containing 2 model files. Updated `lib.rs` to export the scanner module and `scan_dirs` function.

## Resolved Dependencies

| Type   | Name    | Version Resolved | Source          |
|--------|---------|-----------------|-----------------|
| crate  | walkdir | 2.5.0           | rust-docs MCP   |
| crate  | sha2    | 0.10.9          | rust-docs MCP   |
| crate  | hex     | 0.4.3           | rust-docs MCP   |

## Files Changed

| Action | Path                                      | Description                                    |
|--------|-------------------------------------------|------------------------------------------------|
| Modify | `crates/anvilml-registry/Cargo.toml`      | Added walkdir, sha2, hex dependencies          |
| Modify | `crates/anvilml-registry/src/lib.rs`      | Added `pub mod scanner;` and re-exported `scan_dirs` |
| Create | `crates/anvilml-registry/src/scanner.rs`  | Scanner module with scan_dirs and helper functions |
| Create | `crates/anvilml-registry/tests/scanner.rs`| Integration test with tempdir fixture (2 files) |

## Commit Log

```
 .forge/reports/P6-A1_plan.md             |  99 +++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +-
 Cargo.lock                               |  31 +++++
 crates/anvilml-registry/Cargo.toml       |   3 +
 crates/anvilml-registry/src/lib.rs       |   2 +
 crates/anvilml-registry/src/scanner.rs   | 231 +++++++++++++++++++++++++++++++
 crates/anvilml-registry/tests/scanner.rs |  77 +++++++++++
 8 files changed, 453 insertions(+), 9 deletions(-)
```

## Test Results

```
cargo test -p anvilml-registry
     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-6d5e8384d0494b34)
running 10 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_sha256_hex ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test db::tests::test_open_creates_tables ... ok
test db::tests::test_reset_ghost_jobs ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-658cac46f3de19aa)
running 1 test
test test_open_creates_tables ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-c3503b16712e8c76)
running 1 test
test test_scan_dirs_two_files ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_registry
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

---

cargo test --workspace --features mock-hardware
anvilml_core:   74 passed; 0 failed
anvilml_hardware: 59 passed; 0 failed
anvilml_ipc:    0 passed; 0 failed
anvilml_openapi: 0 passed; 0 failed
anvilml_registry: 10 passed; 0 failed (lib) + 1 passed (db integration) + 1 passed (scanner integration)
anvilml_scheduler: 0 passed; 0 failed
anvilml_server: 3 passed; 0 failed
anvilml_worker: 0 passed; 0 failed
anvilml binary: 8 passed; 0 failed
backend config_reference: 1 passed; 0 failed
Doc-tests anvilml_hardware: 2 passed; 0 failed
Total: 148 passed; 0 failed
```

## Platform Cross-Check

```
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
    Checking walkdir v2.5.0
    Checking sha2 v0.10.9
    Checking hex v0.4.3
    ...
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.87s
```

Zero errors. Windows cross-compilation check passes cleanly.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p backend --features mock-hardware -- config_reference
     Running tests/config_reference.rs (target/debug/deps/config_reference-c0332568e95e7e41)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not required — no handler signatures or `utoipa` annotations were modified in this task.

## Deviations from Plan

1. **Import paths**: The plan specifies importing `ModelMeta`, `DType`, `ModelKind` from `anvilml_core::types`, but these are re-exported at the crate root level (`anvilml_core::{DType, ModelKind, ModelMeta}`) rather than through the `types` submodule. Used the correct paths that compile.
2. **bf16 inference order**: The `infer_dtype` function checks `bf16` before `f16`/`fp16` because `bf16` ends with the substring `f16`. This is a necessary correction to ensure `model-bf16` correctly resolves to `DType::BF16` rather than `DType::F16`.
3. **Unit tests**: Added additional unit tests for `infer_kind`, `infer_dtype`, `vram_estimate_mib`, and `sha256_hex` beyond the single integration test specified in the plan, to ensure comprehensive coverage of helper functions.

## Blockers

None. All gates pass, all tests pass (148/148), cross-check passes, clippy clean.
