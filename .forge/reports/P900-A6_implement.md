# Implementation Report: P900-A6

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A6                                           |
| Phase       | 900 — Logging Retrofit                            |
| Description | anvilml-registry: retrofit WARN discipline and DEBUG per-file log to scanner.rs |
| Implemented | 2026-06-06T01:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Retrofitted `crates/anvilml-registry/src/scanner.rs` with two logging improvements required by FORGE_AGENT_RULES §11: (1) applied WARN field discipline at all three existing `tracing::warn!` call sites — `NotFound` errors now omit the redundant `error=` field while other errors retain it; (2) added mandatory DEBUG per-file logging so every examined file is logged at DEBUG level, whether accepted or skipped. No logic changes were made; control flow remains identical to the pre-task state.

## Resolved Dependencies

No new dependencies added. The `tracing` crate was already present in `Cargo.toml`. The `std::io` module is a Rust standard library import.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/scanner.rs` | Split WARN calls at 3 sites by error kind; add DEBUG per-file logs for accepted and skipped paths |

## Commit Log

```
 crates/anvilml-registry/src/scanner.rs | 33 ++++++++++++++++++++++++++++-----
 1 file changed, 28 insertions(+), 5 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-082335e7febadcdc)

running 8 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_sha256_hex ... ok
test scanner::tests::test_vram_estimate_mib ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-a8d8cf26e2973b)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

     Running tests/device_store.rs (target/debug/deps/device_store-92c654a3b7e3f8ab)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s

     Running tests/rescan.rs (target/debug/deps/rescan-6bc089f80be2701c)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

     Running tests/scanner.rs (target/debug/deps/scanner-ff4857f361177efa)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-f7d1c1c83c7a3559)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

     Running tests/store_get.rs (target/debug/deps/store_get-5cb98cd23f67b4c3)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s

     Running tests/store_list.rs (target/debug/deps/store_list-6bc089f80be2701c)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

   Doc-tests anvilml_registry

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Format Gate

```
(No output — exit 0, no formatting drift detected)
```

## Platform Cross-Check

**1. Mock-hardware Linux:**
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.79s
```

**2. Mock-hardware Windows cross-check:**
```
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.41s
```

**3. Real-hardware Linux:**
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.25s
```

**4. Real-hardware Windows cross-check:**
```
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.54s
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.89s
     Running unittests src/main.rs (target/debug/deps/anvilml-6620d99f4fa33a4c)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.00s

     Running tests/config_reference.rs (target/debug/deps/config_reference-4d2ccda098c0c609)
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Deviations from Plan

- **WARN discipline — metadata error (step 2):** The plan specified `e.kind() == io::ErrorKind::NotFound` for the `entry.metadata()` error branch. However, `walkdir::DirEntry::metadata()` returns `walkdir::Result<fs::Metadata>` (i.e., `Result<T, walkdir::Error>`), not `std::io::Result`. The `walkdir::Error` type does not expose a `.kind()` method directly; it must be unwrapped via `.io_error().map(|inner| inner.kind())`. This was corrected to match the actual API.
- **WARN discipline — canonicalize error (step 3):** The plan's code used `e.kind() == io::ErrorKind::NotFound` for `entry.path().canonicalize()` errors. Since `Path::canonicalize()` returns `std::io::Result<PathBuf>`, the error type is `std::io::Error` and `.kind()` works directly — this matched the plan exactly.
- **Accepted file DEBUG log:** The plan specified `path = %canonical_path.display()`, but `canonical_path` was moved into `ModelMeta`. A clone (`id_clone`) was introduced for `id`, and `canonical_path.clone()` was used for the `path` field in `ModelMeta` so the original remains available for the debug log.

## Blockers

None.
