# Implementation Report: P6-A4

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P6-A4                                             |
| Phase         | 006 — Model Registry & Artifacts                  |
| Description   | anvilml-registry: ModelScanner hashing + ModelKind/Dtype inference |
| Implemented   | 2026-06-29T19:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created `crates/anvilml-registry/src/scanner.rs` implementing `ModelScanner`, the directory-walking scanner that derives `ModelMeta` from real files on disk. The scanner computes a stable SHA256-based model ID from the first 1 MiB of each file, infers architecture family from the parent directory name, and deduces data type and file format from filename substrings. A file already in the store with unchanged size and mtime is skipped — never re-hashed. Added `sha2` and `digest` crate dependencies, updated `ModelMeta` to include `mtime_unix`, and created 20 integration tests covering hashing stability, kind/dtype/format inference, depth limits, deduplication, and mixed scenarios.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | sha2    | 0.11.0           | rust-docs MCP  |
| crate  | digest  | 0.11.3           | rust-docs MCP  |
| crate  | uuid    | 1.23.0 (dev)     | rust-docs MCP  |

sha2 0.11.0 API confirmed: `sha2::Sha256` with `digest::Digest` trait methods `update(&[u8])` and `finalize()` returning `GenericArray<u8, U32>`. MSRV 1.85 is compatible with project's Rust 1.96.0.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/scanner.rs` | ModelScanner implementation (358 lines) |
| CREATE | `crates/anvilml-registry/tests/scanner_tests.rs` | Integration tests (712 lines, 20 tests) |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod scanner;` and `pub use scanner::ModelScanner;` |
| MODIFY | `crates/anvilml-registry/src/store.rs` | Added `get_path_info()` method and `PathInfoRow` helper; updated `upsert()` to use `meta.mtime_unix`; updated `row_to_meta()` to include `mtime_unix` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Added `sha2 = "0.11"`, `digest = "0.11"`, `uuid` dev-dep; bumped version 0.1.2 → 0.1.3 |
| MODIFY | `crates/anvilml-core/src/types/model.rs` | Added `mtime_unix: i64` field to `ModelMeta` |
| MODIFY | `crates/anvilml-core/tests/model_tests.rs` | Added `mtime_unix` to `ModelMeta` test construction |
| MODIFY | `crates/anvilml-registry/tests/store_tests.rs` | Fixed in-memory SQLite isolation with unique UUID-based names |

## Commit Log

```
 .forge/reports/P6-A4_plan.md                   | 189 +++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |  12 +-
 crates/anvilml-core/src/types/model.rs         |   5 +-
 crates/anvilml-core/tests/model_tests.rs       |   1 +
 crates/anvilml-registry/Cargo.toml             |   5 +-
 crates/anvilml-registry/src/lib.rs             |   2 +
 crates/anvilml-registry/src/scanner.rs         | 358 +++++++++++++
 crates/anvilml-registry/src/store.rs           |  56 +-
 crates/anvilml-registry/tests/scanner_tests.rs | 712 +++++++++++++++++++++++++
 crates/anvilml-registry/tests/store_tests.rs   |  20 +-
 12 files changed, 1355 insertions(+), 24 deletions(-)
```

## Test Results

```
     Running tests/scanner_tests.rs (target/debug/deps/scanner_tests-4b7727ef3d51cd20)

running 20 tests
test test_dtype_inference_bf16 ... ok
test test_kind_inference_diffusion ... ok
test test_format_inference_ckpt ... ok
test test_dtype_inference_fp8_e4m3fn ... ok
test test_depth_limit_respected ... ok
test test_format_inference_safetensors ... ok
test test_hash_small_file ... ok
test test_dtype_inference_fp16 ... ok
test test_format_inference_pt ... ok
test test_dtype_inference_fp32 ... ok
test test_format_inference_bin ... ok
test test_depth_zero_scans_only_root ... ok
test test_kind_inference_text_encoders ... ok
test test_kind_inference_unknown_dir ... ok
test test_kind_inference_vae ... ok
test test_root_level_kind_unknown ... ok
test test_unchanged_file_skips_rehash ... ok
test test_multiple_files_scanned ... ok
test test_mixed_formats_and_dtypes ... ok
test test_hash_stability_across_rename ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 204 tests passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, all files formatted)
```

## Platform Cross-Check

All four checks passed:
1. `cargo check --workspace --features mock-hardware` — Finished in 0.84s
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished in 27.08s
3. `cargo check --bin anvilml` — Finished in 22.14s
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished in 19.77s

## Project Gates

Gate 1 — Config Surface Sync: `cargo test -p anvilml --features mock-hardware -- config_reference` — OK (1 passed)

## Public API Delta

```
+    pub mtime_unix: i64,
+pub mod scanner;
+pub use scanner::ModelScanner;
+    pub async fn get_path_info(
```

New public items:
- `pub mod scanner` — module declaration in `anvilml_registry`
- `pub use scanner::ModelScanner` — re-export of `ModelScanner`
- `pub struct ModelScanner` — the scanner (in `scanner.rs`)
- `pub fn ModelScanner::new(pool: SqlitePool) -> Self` — constructor
- `pub async fn ModelScanner::scan_dir(&self, root: &Path, depth: u32) -> Result<Vec<ModelMeta>, AnvilError>` — core scan method
- `pub async fn ModelStore::get_path_info(&self, path: &Path) -> Result<Option<(u64, i64)>, AnvilError>` — dedup helper
- `pub mtime_unix: i64` — new field on `ModelMeta`

## Deviations from Plan

- **Added `mtime_unix` to `ModelMeta`**: The plan assumed `ModelMeta` didn't include `mtime_unix` and that dedup would use a separate query. In practice, `ModelMeta` needed the field so the scanner could populate it during `upsert()`, enabling the dedup check to compare actual stored values. This required updating `store.rs`'s `upsert()`, `row_to_meta()`, and the `ModelMetaRow` struct.
- **Added `uuid` as a dev-dependency**: Required for unique in-memory SQLite database names to prevent cross-test interference when tests run in parallel.
- **In-memory SQLite isolation fix**: Both `store_tests.rs` and `scanner_tests.rs` were changed to use UUID-based unique in-memory database names with `cache=shared` and `max_connections(1)` to ensure each test gets its own isolated database. This was a pre-existing issue that surfaced during implementation.
- **`hash_file` is private**: The plan listed `hash_file` as a private helper, so tests access it indirectly through `scan_dir()` results rather than calling it directly.

## Blockers

None.
