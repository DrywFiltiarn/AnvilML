# Plan Report: P6-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A1                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml-registry: ModelScanner directory walk and metadata derivation |
| Depends on  | P5 (SqlitePool open/open_in_memory via db.rs)     |
| Project     | anvilml                                           |
| Planned at  | 2026-06-15T17:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement `ModelScanner` in a new `scanner.rs` module within the `anvilml-registry` crate. The scanner walks configured model directories, reads each `.safetensors` file, derives metadata (kind from directory, dtype from filename, format from extension, id from SHA256 of first 1 MiB), and returns a `Vec<ModelMeta>`. After this task, a developer can call `ModelScanner::scan(&[ModelDirConfig])` from any async context and receive complete model metadata for all discovered files. The acceptance criterion is `cargo test -p anvilml-registry -- scanner` exiting 0 with ≥ 6 tests.

## Scope

### In Scope
- **CREATE** `crates/anvilml-registry/src/scanner.rs` — new module with:
  - `pub struct ModelScanner` (zero-size unit struct, no fields)
  - `pub async fn scan(&self, dirs: &[ModelDirConfig]) -> Vec<ModelMeta>` — directory walk and metadata derivation
  - Private helper functions: `infer_kind(&self, dir_name: &str) -> ModelKind`, `infer_dtype(&self, filename: &str) -> ModelDtype`, `infer_format(&self, filename: &str) -> ModelFormat`, `compute_id(path: &std::path::Path) -> Result<String, std::io::Error>`
- **MODIFY** `crates/anvilml-registry/src/lib.rs` — add `pub mod scanner;` and `pub use scanner::ModelScanner;`
- **MODIFY** `crates/anvilml-registry/Cargo.toml` — add `tokio` dependency (for async filesystem I/O)
- **CREATE** `crates/anvilml-registry/tests/scanner_tests.rs` — ≥ 6 unit/integration tests

### Out of Scope
- SQLite persistence of scanned results (handled by P6-A2, ModelStore)
- Recursive directory walking (the `recursive` and `max_depth` fields on `ModelDirConfig` are acknowledged but not implemented; the scan function reads them from config but only walks the top-level directory — future tasks will add recursive traversal)
- REST endpoint wiring (handled by P6-B1, P6-B2)
- Real-hardware model loading or safetensors parsing (only filename inspection and SHA256 hashing)

## Existing Codebase Assessment

The `anvilml-registry` crate already has `db.rs` (SqlitePool creation with migrations), `seed_loader.rs` (SHA256-gated SQL seed runner), and their test files. The `lib.rs` exports only these two modules plus `open`/`open_in_memory` from `db.rs`. No `scanner.rs` exists yet.

The `anvilml-core` crate provides all domain types the scanner consumes: `ModelMeta` (id, name, path, kind, dtype, format, size_bytes, scanned_at), `ModelKind` (Diffusion, TextEncoder, Vae, Lora, ControlNet, Upscale, Unknown), `ModelDtype` (Fp32, Fp16, Bf16, Fp8, Fp4, Unknown), and `ModelFormat` (Safetensors, Ckpt, Pt, Bin, Unknown). `ModelDirConfig` (path, recursive, max_depth) is defined in `config.rs` and re-exported from `anvilml-core`.

The `sha2` crate version `0.10.9` is already used in `seed_loader_tests.rs` with the pattern `sha2::Sha256::digest(data)` — this is the same API the scanner will use. The `tokio` crate is in dev-dependencies but not in the main dependencies; adding it with minimal features (`fs`, `rt`) is required for the async file I/O.

Test style follows the pattern seen in `db_tests.rs` and `seed_loader_tests.rs`: doc comments on every test describing what it verifies and its preconditions, `#[tokio::test]` for async tests, `tempfile::tempdir()` for unique temp directories, and complete database/file isolation per test.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source    | Feature flags confirmed |
|--------|----------|-----------------|---------------|------------------------|
| crate  | sha2     | 0.10.9          | Cargo.lock    | n/a                    |
| crate  | tokio    | 1.52.3          | Workspace     | fs, rt (new minimal)   |

Note: `sha2` version confirmed from Cargo.lock — the project already uses `sha2 = "0.10"` in workspace deps. The `sha2::Sha256::digest()` convenience method used in `seed_loader_tests.rs:75` is confirmed to exist in 0.10.9. `tokio` version from workspace deps (1.52.3); only `fs` and `rt` features needed (not `full`).

## Approach

1. **Add `tokio` dependency to `anvilml-registry/Cargo.toml`.** Add `tokio = { workspace = true, features = ["fs", "rt"] }` to the `[dependencies]` section. The workspace already defines `tokio = { version = "1.52.3", features = ["full"] }` — we only need `fs` (for `tokio::fs::read` and `tokio::fs::read_dir`) and `rt` (for the async runtime). Use the workspace reference to stay consistent with other crates.

2. **Create `crates/anvilml-registry/src/scanner.rs`.** Implement the module with:
   - `pub struct ModelScanner;` — zero-size unit struct (no state needed; all data is derived from inputs).
   - `impl ModelScanner` block with the following methods:

   **`pub async fn scan(&self, dirs: &[ModelDirConfig]) -> Vec<ModelMeta>`** — the main entry point. Annotated with `#[tracing::instrument(skip(self, dirs))]` per §11.5 logging standards. For each `ModelDirConfig` in `dirs`:
   - If the directory does not exist (check with `std::fs::metadata`), log at DEBUG with `path=` and `reason="directory_not_found"`, then skip to next dir.
   - Open the directory with `tokio::fs::read_dir`. For each entry:
     - Skip non-file entries (directories, symlinks to dirs, etc.) — log at DEBUG with `path=` and `reason="not_a_file"`.
     - Skip non-`.safetensors` files — log at DEBUG with `path=` and `reason="unsupported_format"`. (The scanner currently only processes `.safetensors` files as they are the recommended format per `ModelFormat` docs.)
     - For each valid `.safetensors` file: compute id via `compute_id(&entry_path)`, derive kind via `infer_kind(dir_name)`, derive dtype via `infer_dtype(filename)`, derive format via `infer_format(filename)`, get file size via `std::fs::metadata(&entry_path).map(|m| m.len())`. Construct `ModelMeta` with `scanned_at = chrono::Utc::now()`.
   - After processing all dirs, log at INFO with `count=` (total files scanned) and `dir=` (comma-joined directory paths).
   - Return the collected `Vec<ModelMeta>`.

   **`fn infer_kind(&self, dir_name: &str) -> ModelKind`** — case-insensitive match on directory component:
   - `"diffusion"` → `ModelKind::Diffusion`
   - `"text_encoders"` or `"clip"` → `ModelKind::TextEncoder`
   - `"vae"` → `ModelKind::Vae`
   - `"loras"` or `"lora"` → `ModelKind::Lora`
   - `"controlnet"` → `ModelKind::ControlNet`
   - `"upscale"` → `ModelKind::Upscale`
   - else → `ModelKind::Unknown`
   Rationale: Use `to_lowercase()` then match — simpler than multiple `starts_with` checks and handles edge cases like `"Diffusion/"` correctly.

   **`fn infer_dtype(&self, filename: &str) -> ModelDtype`** — case-insensitive substring check on filename (not full path). Check order matters: `fp8` before `fp16` to avoid false matches on filenames like `fp16_fp8_quantized`.
   - filename contains `"fp8"` → `ModelDtype::Fp8`
   - filename contains `"fp16"` → `ModelDtype::Fp16`
   - filename contains `"bf16"` → `ModelDtype::Bf16`
   - filename contains `"fp32"` → `ModelDtype::Fp32`
   - else → `ModelDtype::Unknown`
   Rationale: Check order is critical — `fp8` must be checked before `fp16` because the substring `fp1` in `fp16` does not overlap with `fp8`, but checking `fp8` first ensures we catch quantized files that mention both precisions.

   **`fn infer_format(&self, filename: &str) -> ModelFormat`** — extension-based:
   - `.safetensors` → `ModelFormat::Safetensors`
   - `.ckpt` → `ModelFormat::Ckpt`
   - `.pt` → `ModelFormat::Pt`
   - `.bin` → `ModelFormat::Bin`
   - else → `ModelFormat::Unknown`

   **`async fn compute_id(path: &std::path::Path) -> Result<String, std::io::Error>`** — reads first 1 MiB (1048576 bytes) of the file, computes SHA256, returns lowercase hex.
   - Use `tokio::fs::read` to read the entire file (files are at most a few GB, but we only hash first 1 MiB).
   - Actually, use `tokio::fs::OpenOptions::new().read(true).open(path)` then `tokio::io::AsyncReadExt::take(1048576)` to read only the first 1 MiB without loading the full file into memory. This is important for large model files (potentially 10+ GB).
   - Compute: `format!("{:x}", sha2::Sha256::digest(buf))` where `buf` is the bytes read.
   - If the file is smaller than 1 MiB, hash the entire file (`.take()` handles this naturally).

3. **Modify `crates/anvilml-registry/src/lib.rs`.** Add:
   ```rust
   pub mod scanner;
   pub use scanner::ModelScanner;
   ```
   Keep existing `pub mod db;`, `pub mod seed_loader;`, and their re-exports.

4. **Create `crates/anvilml-registry/tests/scanner_tests.rs`.** Write ≥ 6 tests following the project's test conventions (doc comments, temp directories, `#[tokio::test]`):
   - `test_infer_kind_diffusion` — verify `"diffusion"` → `Diffusion`
   - `test_infer_kind_text_encoder` — verify `"text_encoders"` → `TextEncoder`, `"clip"` → `TextEncoder`
   - `test_infer_dtype_fp8_before_fp16` — verify check order: `"model_fp16_fp8.safetensors"` → `Fp8` (not `Fp16`)
   - `test_compute_id_deterministic` — write a temp file, compute id twice, assert same result
   - `test_scan_nonexistent_dir` — pass a non-existent directory path, verify returns empty vec (no panic)
   - `test_scan_with_files` — create temp dirs with model files, verify `ModelMeta` fields (kind, dtype, format, id) are correct
   - `test_scan_empty_dir` — create an empty temp dir, verify returns empty vec

5. **Add `///` doc comments** to all public items in `scanner.rs` per §12.1: `ModelScanner` struct doc, `scan` method doc with argument and return descriptions, `infer_kind`/`infer_dtype`/`infer_format`/`compute_id` docs.

6. **Add inline `//` comments** at decision points per §12.2: dtype check order rationale, 1 MiB truncation rationale, non-existent directory handling rationale, format matching rationale.

## Public API Surface

```rust
// crates/anvilml-registry/src/scanner.rs
pub struct ModelScanner;

impl ModelScanner {
    /// Scan configured model directories and return metadata for all discovered model files.
    ///
    /// Walks each directory in `dirs`, inspects `.safetensors` files, and derives
    /// `ModelKind` from the parent directory name, `ModelDtype` from the filename,
    /// and `ModelFormat` from the file extension. The model ID is the SHA256 hex
    /// of the first 1 MiB of file content.
    ///
    /// # Arguments
    /// * `dirs` — slice of `ModelDirConfig` specifying directories to scan.
    ///
    /// # Returns
    /// A `Vec<ModelMeta>` containing metadata for every discovered model file.
    pub async fn scan(&self, dirs: &[ModelDirConfig]) -> Vec<ModelMeta>;
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/scanner.rs` | ModelScanner module with scan() and helper functions |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod scanner;` and `pub use scanner::ModelScanner;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Add `tokio` dependency with `fs` + `rt` features |
| CREATE | `crates/anvilml-registry/tests/scanner_tests.rs` | ≥ 6 tests for kind inference, dtype inference, id derivation, scan behavior |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/scanner_tests.rs` | `test_infer_kind_diffusion` | `infer_kind("diffusion")` returns `ModelKind::Diffusion` | None | `"diffusion"` | `ModelKind::Diffusion` | `cargo test -p anvilml-registry -- scanner test_infer_kind_diffusion` exits 0 |
| `tests/scanner_tests.rs` | `test_infer_kind_text_encoder` | `infer_kind("text_encoders")` → `TextEncoder`, `infer_kind("clip")` → `TextEncoder` | None | `"text_encoders"`, `"clip"` | Both return `ModelKind::TextEncoder` | `cargo test -p anvilml-registry -- scanner test_infer_kind_text_encoder` exits 0 |
| `tests/scanner_tests.rs` | `test_infer_dtype_fp8_before_fp16` | Dtype check order: `"model_fp16_fp8.safetensors"` → `Fp8` (not `Fp16`) | None | `"model_fp16_fp8.safetensors"` | `ModelDtype::Fp8` | `cargo test -p anvilml-registry -- scanner test_infer_dtype_fp8_before_fp16` exits 0 |
| `tests/scanner_tests.rs` | `test_compute_id_deterministic` | SHA256 of first 1 MiB is deterministic and lowercase hex | Temp file with known content | Temp file with 2 MiB of known bytes | Same hex string from two calls | `cargo test -p anvilml-registry -- scanner test_compute_id_deterministic` exits 0 |
| `tests/scanner_tests.rs` | `test_scan_nonexistent_dir` | Passing a non-existent directory returns empty vec, no panic | None | Non-existent path | `Vec::new()` | `cargo test -p anvilml-registry -- scanner test_scan_nonexistent_dir` exits 0 |
| `tests/scanner_tests.rs` | `test_scan_with_files` | Full scan: files in temp dirs produce correct `ModelMeta` with right kind, dtype, format, id | Temp dirs with model files | Temp dir `models/diffusion/model_fp8.safetensors` | `ModelMeta` with `kind=Diffusion`, `dtype=Fp8`, `format=Safetensors`, valid id | `cargo test -p anvilml-registry -- scanner test_scan_with_files` exits 0 |
| `tests/scanner_tests.rs` | `test_scan_empty_dir` | Empty directory returns empty vec | Empty temp directory | Empty temp dir path | `Vec::new()` | `cargo test -p anvilml-registry -- scanner test_scan_empty_dir` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-registry/tests/` which is automatically picked up by `cargo test --workspace --features mock-hardware` (the CI command). No new CI jobs, gates, or matrix entries are needed.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. All file operations use `tokio::fs` which is cross-platform. Path handling uses `std::path::PathBuf` and `std::fs::metadata` — both work identically on Linux and Windows. The directory name matching for kind inference uses `to_lowercase()` which is correct on both platforms (Windows paths use backslashes but `file_name()` extracts the component correctly).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::fs::read_dir` yields entries in arbitrary order, causing non-deterministic test output (sorted vs unsorted `Vec<ModelMeta>`). | Medium | Low | Sort the returned `Vec<ModelMeta>` by path before returning, or sort in tests by comparing sorted results. |
| `ModelMeta.path` field type mismatch: design doc says `PathBuf`, but existing source in `crates/anvilml-core/src/types/model.rs:23` uses `String`. | High | High | The actual source uses `pub path: String` — the plan must use `String` (not `PathBuf`) when constructing `ModelMeta`. Verified by reading `model.rs:23`. |
| `sha2::Sha256::digest()` allocates a `GenericArray` — calling it inside a tight file walk loop could be slow for thousands of files. | Low | Medium | The 1 MiB buffer read via `.take()` limits the allocation size. For large directories this is acceptable; if profiling shows a bottleneck, switch to incremental `update()` calls. |
| `ModelDirConfig.recursive` and `max_depth` are ignored — scanner only walks the top-level directory. Future tasks may expect recursive behavior. | High | Medium | Document clearly in the `scan()` doc comment that recursive walking is not yet implemented. The `dirs` parameter accepts the config but the scan only processes direct children of each path. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- scanner` exits 0 (all ≥ 6 tests pass)
- [ ] `cargo clippy --package anvilml-registry --features mock-hardware -- -D warnings` exits 0
- [ ] `head -1 .forge/reports/P6-A1_plan.md` prints `# Plan Report: P6-A1`
- [ ] `grep "^## " .forge/reports/P6-A1_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P6-A1_plan.md` shows > 40 lines
