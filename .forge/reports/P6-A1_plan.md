# Plan Report: P6-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A1                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml-registry: model directory scanner         |
| Depends on  | P5-A4                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-03T20:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement the model directory scanner module for `anvilml-registry` that walks configured model directories, discovers model weight files (`.safetensors`, `.ckpt`, `.pt`, `.bin`), computes a deterministic ID via SHA256 of the canonical path, infers kind and data type from filename/directory heuristics, and returns a `Vec<ModelMeta>` per directory config.

## Scope

### In Scope
- Add `walkdir`, `sha2`, and `hex` dependencies to `crates/anvilml-registry/Cargo.toml`
- Create `crates/anvilml-registry/src/scanner.rs` with:
  - `async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta>`
  - Directory walking via `walkdir` (follow_links = false)
  - File extension matching for `.safetensors`, `.ckpt`, `.pt`, `.bin`
  - ID generation: first 16 hex chars of SHA256 of canonical path string
  - Name extraction from file stem
  - Kind resolution: explicit `ModelDirConfig.kind` or inference from parent directory name (case-insensitive mapping to `ModelKind`)
  - Data type inference from filename suffix (`f32`, `fp16`, `bf16`, `q8`, `q4`), fallback to `DType::Unknown`
  - VRAM estimate: `size_mib * factor` per dtype (f32=2.0, f16/bf16=1.0, q8=0.5, q4=0.25, unknown=1.0), minimum 1 MiB
- Export the scanner module from `crates/anvilml-registry/src/lib.rs`
- Create test file `crates/anvilml-registry/tests/scanner.rs` with tempdir fixture containing 2 model files

### Out of Scope
- SQLite persistence (store.rs — task P6-A2)
- List/get/rescan API handlers (P6-A3, P6-A4, P6-A6, P6-A7)
- Startup scan orchestration in main.rs (P6-A5)
- Any changes to `anvilml-core` types or config
- Any changes to backend/ or crates/anvilml-server/

## Approach

1. **Add dependencies** to `crates/anvilml-registry/Cargo.toml`:
   - `walkdir = "2"` (directory tree traversal)
   - `sha2 = "0.10"` (SHA256 hashing)
   - `hex = "0.4"` (hex encoding for ID generation)

2. **Create `src/scanner.rs`** with the following structure:
   - Import `ModelDirConfig` from `anvilml_core::config`, `ModelMeta`, `DType`, `ModelKind` from `anvilml_core::types`
   - Implement `fn infer_kind(parent_dir: &str) -> ModelKind` — case-insensitive match of parent directory name to known `ModelKind` variants (`diffusion`→Diffusion, `vae`→Vae, `lora`→Lora, `clip`→Clip, `controlnet`→ControlNet, `unet`→Unet, `upscale`→Upscale). Falls back to `ModelKind::default()` (Upscale) if no match.
   - Implement `fn infer_dtype(filename: &str) -> DType` — check filename suffix (case-insensitive): `f32`→F32, `fp16`/`f16`→F16, `bf16`→BF16, `q8`→Q8, `q4`→Q4. Default to `DType::Unknown`.
   - Implement `fn vram_estimate_mib(size_bytes: u64, dtype: DType) -> u32` — convert bytes to MiB (divide by 1024*1024), multiply by dtype factor, apply minimum of 1.
   - Implement `async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta>` — for each dir config, walk the directory tree with `walkdir::WalkDir` (follow_links=false), filter entries that are files and match allowed extensions, compute SHA256 of the canonical path string, extract first 16 hex chars as ID, build `ModelMeta` struct.
   - Implement `fn sha256_hex(input: &str) -> String` — helper using `sha2::Sha256` and `hex::encode`, return full hex digest (caller takes first 16).

3. **Update `src/lib.rs`** to add `pub mod scanner;` and re-export `scan_dirs`.

4. **Create test file `tests/scanner.rs`**:
   - Create a tempdir with 2 model files: e.g. `model-fp16.safetensors` (write some content) and `weights-q8.pt` (write different content)
   - Call `scan_dirs(&[ModelDirConfig { path: tmp_path, kind: Some(ModelKind::Diffusion) }])`
   - Assert: result length is 2
   - Assert: each entry has correct name (file stem), non-empty id (16 hex chars), correct kind (Diffusion), dtype inferred from suffix (F16 and Q8), positive vram_estimate_mib, valid path

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/Cargo.toml` | Add walkdir, sha2, hex dependencies |
| Create | `crates/anvilml-registry/src/scanner.rs` | Scanner module with scan_dirs, inference helpers |
| Modify | `crates/anvilml-registry/src/lib.rs` | Export scanner module |
| Create | `crates/anvilml-registry/tests/scanner.rs` | Integration test with tempdir fixture (2 files) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/scanner.rs` | `test_scan_dirs_two_files` | scan_dirs returns 2 ModelMeta entries for a tempdir with 2 model files; checks name, id (16 hex), kind, dtype inference, vram_estimate_mib > 0, and valid path |

## CI Impact

No CI changes required. The task only adds dependencies and source code within `anvilml-registry`. The existing CI matrix (rust fmt/clippy/test on Linux, clippy/test on Windows) covers this crate. The new dependencies (`walkdir`, `sha2`, `hex`) are well-established crates with no platform-specific concerns.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| SHA256 of path string vs actual file content — task spec says "SHA256(canonical path string)", so we hash the path text, not file contents. This is deterministic for same paths but does not detect content changes. | Follow spec exactly: hash canonical path string. Future tasks (rescan dedup) can add content-based hashing if needed. |
| `hex` crate API mismatch — `hex::encode()` returns a `String`, which is fine for taking `&[..16]`. | Verify at plan time; the `hex 0.4` API is stable and well-documented. |
| Parent dir inference — directory names may not perfectly match ModelKind variants (e.g., "diffusion_models" vs "diffusion"). | Only do exact case-insensitive match against known variant names. If no match, fall back to default (Upscale). This matches the spec's "infer from parent dir name" without over-engineering fuzzy matching. |
| `walkdir` on Windows — may encounter access-denied errors on protected directories. | `walkdir` skips inaccessible entries by default; we only care about files under the configured model dirs, which should be accessible. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- scanner` exits 0 with tempdir fixture containing 2 files
- [ ] Scanner module compiles and exports `scan_dirs` function
- [ ] Dependencies `walkdir`, `sha2`, `hex` present in `Cargo.toml`
- [ ] SHA256 of canonical path string produces first 16 hex chars as ID
- [ ] Kind inferred from parent directory name when `ModelDirConfig.kind` is None
- [ ] Dtype inferred from filename suffix (f32/fp16/bf16/q8/q4), Unknown fallback
- [ ] VRAM estimate uses correct per-dtype factor with minimum of 1 MiB
