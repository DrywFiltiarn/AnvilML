# Implementation Report: P6-A1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P6-A1                                             |
| Phase         | 006 — Model Registry                              |
| Description   | anvilml-registry: ModelScanner directory walk and metadata derivation |
| Implemented   | 2026-06-15T19:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented `ModelScanner` in a new `scanner.rs` module within the `anvilml-registry` crate. The scanner walks configured model directories, inspects `.safetensors` files, and derives `ModelMeta` entries with kind (from directory name), dtype (from filename), format (from extension), and id (SHA256 of first 1 MiB). All 7 tests pass, clippy is clean, and all platform cross-checks and project gates succeed.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source              |
|--------|---------|-----------------|---------------------|
| crate  | tokio   | 1.52.3          | Workspace dep (v1.52.3) |

Note: `tokio` is already defined in workspace deps as `tokio = { version = "1.52.3", features = ["full"] }`. Added as a regular dependency with minimal features: `fs`, `io-util`, `rt`. The `io` feature name from the plan was corrected to `io-util` (the actual tokio feature name for `AsyncReadExt`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/scanner.rs` | ModelScanner module with scan(), infer_kind(), infer_dtype(), infer_format(), compute_id() |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod scanner;` and `pub use scanner::ModelScanner;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Added `tokio` dependency with `fs`, `io-util`, `rt` features; bumped version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-registry/tests/scanner_tests.rs` | 7 integration tests for kind inference, dtype inference, id derivation, scan behavior |
| MODIFY | `docs/TESTS.md` | Added 7 entries for new scanner tests |

## Commit Log

```
 .forge/reports/P6-A1_plan.md                   | 189 ++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 crates/anvilml-registry/Cargo.toml             |   3 +-
 crates/anvilml-registry/src/lib.rs             |   2 +
 crates/anvilml-registry/src/scanner.rs         | 340 +++++++++++++++++++++
 crates/anvilml-registry/tests/scanner_tests.rs | 404 +++++++++++++++++++++++++
 docs/TESTS.md                                  |  63 ++++
 9 files changed, 1011 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/scanner_tests.rs (target/debug/deps/scanner_tests-68cb18208b6ed2ef)

running 7 tests
test test_scan_nonexistent_dir ... ok
test test_infer_kind_diffusion ... ok
test test_scan_empty_dir ... ok
test test_scan_with_files ... ok
test test_compute_id_deterministic ... ok
test test_infer_dtype_fp8_before_fp16 ... ok
test test_infer_kind_text_encoder ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Full workspace test suite: 97 tests passed, 0 failed across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.45s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.60s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.92s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.94s
```

All four checks passed with zero errors.

## Project Gates

```
# Gate 1 — config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. No config fields were added or modified by this task.

## Public API Delta

```
+pub mod scanner;
+pub use scanner::ModelScanner;
```

New public items:
- `pub mod scanner` — module declaration in `anvilml_registry`
- `pub use scanner::ModelScanner` — re-export in `anvilml_registry`
- `pub struct ModelScanner` — unit struct in `anvilml_registry::scanner`
- `pub async fn scan(&self, dirs: &[ModelDirConfig]) -> Vec<ModelMeta>` — public method in `anvilml_registry::scanner::ModelScanner`

## Deviations from Plan

- **Dependency feature name**: Plan specified `features = ["fs", "rt"]`. The actual tokio feature for `AsyncReadExt` is `io-util` (not `io`). Updated to `features = ["fs", "io-util", "rt"]`.
- **Private method visibility**: Plan specified `infer_kind`, `infer_dtype`, `infer_format`, and `compute_id` as private methods. Since integration tests cannot access private methods (separate crate), these were made `pub(crate)` but tests exercise them indirectly through the public `scan()` API, which is the correct testing approach.
- **`compute_id` test**: Plan specified a direct call to `compute_id` from the test file. Since `compute_id` is private, the deterministic ID test was rewritten to verify the ID through `scan()` results (same verification, different approach).

## Blockers

None.
