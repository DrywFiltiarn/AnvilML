# Plan Report: P4-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P4-A2                                       |
| Phase       | 004 — Persistence & Model Registry          |
| Description | anvilml-registry: ModelMeta scanner         |
| Depends on  | P3-B1 (anvilml-core types stable)           |
| Project     | anvilml                                     |
| Planned at  | 2026-05-30T15:31:30Z                        |
| Attempt     | 1                                           |

## Objective

Implement the filesystem scanner for `anvilml-registry` that walks configured model directories, discovers model files by extension (`.safetensors`, `.ckpt`, `.pt`, `.bin`), and derives a fully-populated `ModelMeta` struct for each file. The scanner computes `ModelMeta.id` from SHA256 of the canonical path string, infers `ModelKind` from directory name or config, determines `dtype_hint` from filename suffixes, and estimates VRAM usage based on file size and dtype factor. This phase also updates the `ModelMeta` and `DType` types in `anvilml-core` so they match the migration schema created by P4-A1 (adding `dtype_hint`, `size_bytes`, `vram_estimate_mib`, `scanned_at` fields and `Q8`/`Q4` dtype variants).

## Scope

### In Scope
- Update `DType` enum in `anvilml-core/src/types/model.rs`: add `Q8`, `Q4` variants; add `Unknown` variant as a new non-exhaustive sentinel.
- Update `ModelMeta` struct in `anvilml-core/src/types/model.rs`: change `id` from `Uuid` to `String` (first 16 hex chars of SHA256); add `dtype_hint: DType`, `size_bytes: u64`, `vram_estimate_mib: u64`, `scanned_at: String`; remove `use uuid::Uuid` import; update `ModelMeta::new()` constructor to accept the new field set.
- Create `anvilml-registry/src/scanner.rs`: implement `pub async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta>`.
- Add dependencies to `anvilml-registry/Cargo.toml`: `walkdir`, `sha2`, `hex`.
- Update `anvilml-registry/src/lib.rs`: export the `scanner` module (`pub mod scanner`).
- Add unit tests in `scanner.rs` (via `#[cfg(test)] mod tests`) using `tempfile::NamedTempFile` (already a dev-dependency).
- Ensure `ModelMeta` tests in `anvilml-core` remain passing after the struct changes.

### Out of Scope
- Database persistence / upsert logic (handled by P4-A3).
- HTTP API handlers for `/v1/models` and `/v1/models/rescan` (phases 007–008).
- Binary naming corrections (`anvilml` binary, `anvilml.db`) — handled by P4-A2B.
- Integration tests that touch the SQLite store.
- Symlink cycle handling beyond `walkdir::FollowLinks::yes` (the default) — symlinks are followed as walkdir defaults.

## Approach

1. **Update `DType` enum** in `crates/anvilml-core/src/types/model.rs`:
   - Add `Q8` and `Q4` variants after `BF16`.
   - Add `Unknown` variant as the last variant (non-exhaustive sentinel for future dtype values).
   - Verify existing serialization round-trip tests still pass.

2. **Update `ModelMeta` struct** in `crates/anvilml-core/src/types/model.rs`:
   - Change `id: Uuid` to `id: String`. Remove `use uuid::Uuid;` import (check if `uuid` is still needed by other types in the crate; if not, remove it from Cargo.toml — but since other types like job IDs may still use it, leave the dependency and only remove the import).
   - Add fields: `dtype_hint: DType`, `size_bytes: u64`, `vram_estimate_mib: u64`, `scanned_at: String`.
   - Keep existing `dtype: Option<DType>` field (for future use; scanner populates `dtype_hint` instead).
   - Update `ModelMeta::new()` to accept `id: String, name: String, kind: ModelKind, path: String, dtype_hint: DType, size_bytes: u64, vram_estimate_mib: u64, scanned_at: String`.
   - Update existing tests (`model_meta_new`, `model_meta_serialization_round_trip`, `model_meta_skip_none_dtype`) to use the new struct layout.

3. **Add dependencies** to `crates/anvilml-registry/Cargo.toml`:
   - `walkdir = "2"` — recursive directory traversal.
   - `sha2 = "0.10"` — SHA-256 hashing.
   - `hex = "0.4"` — hex string encoding for the SHA256 output.

4. **Create `crates/anvilml-registry/src/scanner.rs`**:
   - Define `pub async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta>`.
   - For each `ModelDirConfig`, call `walkdir::WalkDir::new(path)` with `max_depth` set to `None` (unlimited recursion), skip unreadable dirs with `warn!()` logging (use `tracing` which is already a dependency of the workspace).
   - For each file entry, check extension: `.safetensors`, `.ckpt`, `.pt`, `.bin`. Skip non-matching files.
   - Compute `ModelMeta.id`: canonicalize the path via `std::fs::canonicalize()`, convert to string, compute SHA256 with `sha2::Sha256`, take first 16 hex chars with `hex::encode()`.
   - Set `ModelMeta.name` = filename stem (no extension).
   - Determine `ModelMeta.kind`: if `ModelDirConfig.kind.is_some()`, use it. Otherwise, infer from parent directory name (case-insensitive): `diffusion`/`unet` → `Diffusion`; `vae` → `Vae`; `lora` → `Lora`; `controlnet` → `ControlNet`; `clip` → `Clip`; `upscale` → `Upscale`; default `Diffusion`.
   - Determine `dtype_hint`: scan the filename (stem, case-insensitive) for substrings: `fp16`/`f16` → `F16`; `bf16` → `BF16`; `q8` → `Q8`; `q4` → `Q4`; `fp32`/`f32` → `F32`; else `Unknown`.
   - Compute `vram_estimate_mib`: `(size_bytes as f64 / 1024.0 / 1024.0) * factor` where factor is `F32=2.0`, `F16=1.0`, `BF16=1.0`, `Q8=0.5`, `Q4=0.25`, `Unknown=1.0`. Cast to `u64`, minimum 1.
   - Set `scanned_at` = `chrono::Utc::now().to_rfc3339()`.
   - Collect all `ModelMeta` into a `Vec<ModelMeta>` and return.

5. **Export scanner module** in `crates/anvilml-registry/src/lib.rs`:
   - Add `pub mod scanner;` alongside the existing `pub mod db;`.

6. **Write tests** in `scanner.rs` under `#[cfg(test)] mod tests`:
   - Create a temp directory with two fixture files of known sizes and names using `std::fs::write` on paths inside a `tempfile::tempdir()`.
   - Call `scan_dirs(&[ModelDirConfig { path: tmp_path, kind: None }])`.
   - Assert the returned `Vec<ModelMeta>` has exactly 2 entries.
   - Verify each entry's `name`, `id` (first 16 hex chars of SHA256 of canonical path), `kind` (inferred from parent dir or default), `dtype_hint` (from filename suffix), `size_bytes` (matches written size), `vram_estimate_mib` (matches formula), and `scanned_at` (valid RFC3339).
   - Add a test for directory with `ModelDirConfig.kind = Some(ModelKind::Vae)` to verify config kind takes priority over inference.

## Files Affected

| Action   | Path                                                    | Description                                                    |
|----------|---------------------------------------------------------|----------------------------------------------------------------|
| MODIFY   | crates/anvilml-core/src/types/model.rs                  | Add Q8/Q4/Unknown to DType; update ModelMeta struct fields and constructor |
| MODIFY   | crates/anvilml-registry/Cargo.toml                      | Add walkdir, sha2, hex dependencies                            |
| CREATE   | crates/anvilml-registry/src/scanner.rs                  | Implement scan_dirs() and unit tests                           |
| MODIFY   | crates/anvilml-registry/src/lib.rs                      | Add `pub mod scanner;` export                                  |

## Tests

| Test ID / Name                        | File                                          | Validates                                                      |
|---------------------------------------|-----------------------------------------------|----------------------------------------------------------------|
| `dtype_serialization_round_trip`      | crates/anvilml-core/src/types/model.rs        | Q8, Q4, Unknown variants serialize/deserialize correctly       |
| `model_meta_new`                      | crates/anvilml-core/src/types/model.rs        | Updated constructor accepts new field set                      |
| `model_meta_serialization_round_trip` | crates/anvilml-core/src/types/model.rs        | New ModelMeta fields round-trip through JSON                   |
| `scan_dirs_basic`                     | crates/anvilml-registry/src/scanner.rs        | Two fixture files produce correct ModelMeta entries            |
| `scan_dirs_kind_inference`            | crates/anvilml-registry/src/scanner.rs        | Kind inferred from parent dir name (e.g. "vae" → Vae)          |
| `scan_dirs_config_kind_overrides`     | crates/anvilml-registry/src/scanner.rs        | ModelDirConfig.kind takes priority over directory inference    |
| `scan_dirs_dtype_inference`           | crates/anvilml-registry/src/scanner.rs        | dtype_hint derived from filename suffix (fp16→F16, q8→Q8)      |
| `scan_dirs_vram_estimate`             | crates/anvilml-registry/src/scanner.rs        | vram_estimate_mib matches size_bytes * dtype_factor formula    |
| `scan_dirs_id_hash`                   | crates/anvilml-registry/src/scanner.rs        | ModelMeta.id = first 16 hex chars of SHA256(canonical_path)   |

## CI Impact

No CI workflow changes required. The new dependencies (`walkdir`, `sha2`, `hex`) are lightweight, well-maintained crates that do not introduce platform-specific build requirements. The `mock-hardware` feature flag is not needed for this task since the scanner performs only filesystem I/O and hashing. The existing CI matrix (rust, python-worker, openapi-diff, rust-windows) will automatically pick up the new tests via `cargo test --workspace --features mock-hardware`.

## Risks and Mitigations

| Risk                                      | Likelihood | Impact | Mitigation                                                       |
|-------------------------------------------|-----------|--------|------------------------------------------------------------------|
| ModelMeta struct change breaks downstream crates (hardware, scheduler, server) | Medium    | High   | The change is scoped to fields added (dtype_hint, size_bytes, vram_estimate_mib, scanned_at) and id type change. Downstream crates that construct ModelMeta will get compile errors; these are fixed in P4-A3 (store) which is the immediate next task. Document the required changes in P4-A3 plan. |
| `chrono` already a dependency but needs verification of API shape for `to_rfc3339()` | Low       | Low    | `chrono` with `serde` feature is already in anvilml-core Cargo.toml. `DateTime::<Utc>::to_rfc3339()` is stable since chrono 0.4.10. Verify during implementation. |
| `walkdir` follows symlinks by default, potential infinite loop on cyclic symlinks | Low       | Medium | Use `walkdir::WalkDir::new(path).max_depth(None).follow_links(true)` — the default. If a cyclic symlink is encountered, walkdir returns an error for that entry and continues. Add `follow_links(false)` only if explicitly required by later design. |
| SHA256 of canonical path string differs across OS (Linux vs Windows path separators) | Low       | Low    | Spec says "canonical path string" — `std::fs::canonicalize()` returns OS-native absolute paths. The ID is location-stable by design (changes on file move). Document this behavior; no cross-platform normalization needed since each OS has stable canonicalization. |
| `uuid` crate becomes unused in anvilml-core after removing `id: Uuid` | Low       | Low    | Check if other types in anvilml-core still use `Uuid` (e.g., job IDs). If so, keep the dependency. If not, remove it in a follow-up cleanup task. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- model` exits 0 with all ModelMeta and DType tests passing
- [ ] `cargo build -p anvilml-registry` compiles without errors or warnings
- [ ] `cargo test -p anvilml-registry -- scanner` exits 0 with at least 6 tests covering scan, kind inference, dtype inference, vram estimate, id hash, and config override
- [ ] `ModelMeta` struct in `anvilml-core/src/types/model.rs` contains fields: `id: String`, `name: String`, `kind: ModelKind`, `dtype: Option<DType>`, `path: String`, `dtype_hint: DType`, `size_bytes: u64`, `vram_estimate_mib: u64`, `scanned_at: String`
- [ ] `DType` enum in `anvilml-core/src/types/model.rs` contains variants: `F32`, `F16`, `I8`, `BF16`, `Q8`, `Q4`, `Unknown`
- [ ] `scan_dirs()` returns empty vec when given a directory with no matching model files
- [ ] `scan_dirs()` silently skips unreadable directories (no panic, no error propagation)
- [ ] `cargo fmt --all --check` passes (no formatting issues introduced)
