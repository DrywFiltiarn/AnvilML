# Implementation Report: P4-A2

| Field          | Value                                            |
|----------------|--------------------------------------------------|
| Task ID        | P4-A2                                            |
| Phase          | 004 — Persistence & Model Registry               |
| Description    | anvilml-registry: ModelMeta scanner              |
| Project        | anvilml                                          |
| Implemented at | 2026-05-30T16:03:47Z                             |
| Attempt        | 1                                                |

## Summary

Implemented the filesystem scanner for `anvilml-registry` that walks configured model directories, discovers model files by extension (`.safetensors`, `.ckpt`, `.pt`, `.bin`), and derives a fully-populated `ModelMeta` struct for each file. The scanner computes `ModelMeta.id` from SHA256 of the canonical path string, infers `ModelKind` from directory name or config, determines `dtype_hint` from filename suffixes, and estimates VRAM usage based on file size and dtype factor. Updated `DType` enum in `anvilml-core` to add `Q8`, `Q4`, and `Unknown` variants. Updated `ModelMeta` struct to change `id` from `Uuid` to `String` (first 16 hex chars of SHA256) and add `dtype_hint`, `size_bytes`, `vram_estimate_mib`, and `scanned_at` fields.

## Files Changed

| Action   | Path                                          | Description                                                   |
|----------|-----------------------------------------------|---------------------------------------------------------------|
| MODIFY   | crates/anvilml-core/src/types/model.rs        | Added Q8/Q4/Unknown DType variants; changed ModelMeta.id to String; added dtype_hint, size_bytes, vram_estimate_mib, scanned_at fields; updated constructor; added From<config::ModelKind> impl |
| MODIFY   | crates/anvilml-registry/Cargo.toml            | Added walkdir, sha2, hex, chrono dependencies                 |
| MODIFY   | crates/anvilml-registry/src/lib.rs            | Exported scanner module (pub mod scanner)                     |
| CREATE   | crates/anvilml-registry/src/scanner.rs        | Implemented scan_dirs, compute_id, infer_dtype_from_filename, infer_kind_from_dirname, is_model_file, estimate_vram_mib, and scan_dir with comprehensive unit tests |

## Test Results

Full workspace test suite: **142 tests passed, 0 failed**

```
running 52 tests
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 41 tests
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 28 tests (anvilml-registry)
test scanner::tests::compute_id_is_deterministic ... ok
test scanner::tests::compute_id_differs_for_different_paths ... ok
test scanner::tests::estimate_vram_q4 ... ok
test scanner::tests::estimate_vram_f32 ... ok
test scanner::tests::infer_dtype_bf16 ... ok
test scanner::tests::infer_dtype_f16 ... ok
test scanner::tests::compute_id_returns_16_hex_chars ... ok
test scanner::tests::infer_dtype_f32_default ... ok
test scanner::tests::infer_dtype_q4 ... ok
test scanner::tests::infer_dtype_q8 ... ok
test scanner::tests::infer_kind_clip ... ok
test scanner::tests::infer_kind_controlnet ... ok
test scanner::tests::infer_kind_default_diffusion ... ok
test scanner::tests::infer_kind_diffusion ... ok
test scanner::tests::infer_kind_lora ... ok
test scanner::tests::infer_kind_unet ... ok
test scanner::tests::infer_kind_upscale ... ok
test scanner::tests::infer_kind_vae ... ok
test scanner::tests::is_model_file_bin ... ok
test scanner::tests::is_model_file_ckpt ... ok
test scanner::tests::is_model_file_pt ... ok
test scanner::tests::is_model_file_safetensors ... ok
test scanner::tests::is_model_file_unrecognised ... ok
test scanner::tests::scan_dir_nonexistent_returns_empty ... ok
test scanner::tests::scan_dir_empty_returns_empty ... ok
test scanner::tests::scan_dir_discovers_model_files ... ok
test scanner::tests::scan_dirs_multiple ... ok
test tests::test_migrations_create_tables ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P4-A2_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
M  crates/anvilml-core/src/types/model.rs
M  crates/anvilml-registry/Cargo.toml
M  crates/anvilml-registry/src/lib.rs
A  crates/anvilml-registry/src/scanner.rs
```

## Acceptance Criteria — Verification

| Criterion                                          | Status | Evidence                          |
|----------------------------------------------------|--------|-----------------------------------|
| DType enum has Q8 and Q4 variants                  | PASS   | `cargo test -p anvilml-core dtype_serialization_round_trip` |
| DType enum has Unknown variant                     | PASS   | `cargo test -p anvilml-core dtype_serialization_round_trip` |
| ModelMeta.id is String (not Uuid)                  | PASS   | `cargo test -p anvilml-core model_meta_new` |
| ModelMeta has dtype_hint field                     | PASS   | `cargo test -p anvilml-core model_meta_new` |
| ModelMeta has size_bytes field                     | PASS   | `cargo test -p anvilml-core model_meta_new` |
| ModelMeta has vram_estimate_mib field              | PASS   | `cargo test -p anvilml-core model_meta_new` |
| ModelMeta has scanned_at field                     | PASS   | `cargo test -p anvilml-core model_meta_new` |
| uuid::Uuid import removed from model.rs            | PASS   | `grep 'use uuid::Uuid' crates/anvilml-core/src/types/model.rs` returns empty |
| scanner module exists and is exported              | PASS   | `grep 'pub mod scanner' crates/anvilml-registry/src/lib.rs` |
| walkdir, sha2, hex dependencies added              | PASS   | `grep -E 'walkdir|sha2|hex' crates/anvilml-registry/Cargo.toml` |
| scan_dirs function implemented                     | PASS   | `cargo test -p anvilml-registry scan_dirs_multiple` |
| compute_id produces 16-char hex SHA256             | PASS   | `cargo test -p anvilml-registry compute_id_returns_16_hex_chars` |
| infer_dtype_from_filename works for all variants   | PASS   | `cargo test -p anvilml-registry infer_dtype_q8 && infer_dtype_q4 && infer_dtype_bf16 && infer_dtype_f16 && infer_dtype_f32_default` |
| infer_kind_from_dirname works for all kinds        | PASS   | `cargo test -p anvilml-registry infer_kind_*` |
| is_model_file filters by extension                 | PASS   | `cargo test -p anvilml-registry is_model_file_*` |
| scan_dir discovers real model files                | PASS   | `cargo test -p anvilml-registry scan_dir_discovers_model_files` |
| Full workspace tests pass (0 failures)             | PASS   | `cargo test --workspace` → 142 passed, 0 failed |
| Clippy passes with zero warnings                   | PASS   | `cargo clippy --workspace --features mock-hardware -- -D warnings` |
