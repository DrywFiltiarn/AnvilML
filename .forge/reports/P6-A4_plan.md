# Plan Report: P6-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A4                                             |
| Phase       | 006 — Model Registry & Artifacts                  |
| Description | anvilml-registry: ModelScanner hashing + ModelKind/Dtype inference |
| Depends on  | P6-A3                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-29T16:55:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-registry/src/scanner.rs` implementing `ModelScanner`, the directory-walking scanner that derives `ModelMeta` from real files on disk. The scanner computes a stable SHA256-based model ID from the first 1 MiB of each file, infers architecture family from the parent directory name, and deduces data type from filename substrings. A file already in the store with unchanged size and mtime is skipped — never re-hashed. This produces the `Vec<ModelMeta>` that populates the model registry at server startup or on `/v1/models/rescan`.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/scanner.rs` with `ModelScanner` struct and `scan_dir()` method.
- SHA256 hashing of the first 1 MiB (or whole file if smaller) using `sha2` crate.
- `ModelKind` inference from directory component: `diffusion/` → `Diffusion`, `text_encoders/` → `TextEncoder`, `vae/` → `Vae`, else `Unknown`.
- `ModelDtype` inference from filename substrings (case-insensitive): `fp8`/`fp8_e4m3fn`/`fp8_e5m2` → `Fp8`, `fp16` → `Fp16`, `bf16` → `Bf16`, `fp32` → `Fp32`, ambiguous → `Unknown`.
- `ModelFormat` inference from file extension: `.safetensors` → `Safetensors`, `.ckpt` → `Ckpt`, `.pt`/`.pth` → `Pt`, `.bin`/`.gguf` → `Bin`, else `Unknown`.
- Deduplication: skip files already in the store with unchanged size + mtime.
- Non-recursive directory walk by default; depth configurable via `depth: u32` parameter.
- Declare `pub mod scanner;` and `pub use scanner::ModelScanner;` in `lib.rs`.
- Add `sha2` crate dependency to `Cargo.toml`.
- Create `crates/anvilml-registry/tests/scanner_tests.rs` with ≥ 6 tests using `tempfile` fixtures.
- Bump `anvilml-registry` patch version from `0.1.2` to `0.1.3`.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope.

## Existing Codebase Assessment

The `anvilml-registry` crate currently has two modules: `db.rs` (pool creation with migration runner) and `store.rs` (`ModelStore` CRUD). The `lib.rs` re-exports only `create_pool` and `ModelStore`. The `ModelStore` struct takes a `SqlitePool` and provides `upsert()`, `get()`, `list()`, and `delete()` methods. The `upsert()` method stores enum fields as JSON-trimmed snake_case text, and `mtime_unix` is always inserted as `0` (placeholder) — the scanner is responsible for populating the real value.

The `anvilml-core` crate exports `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` through `pub use types::*`. All three enums derive `Serialize`/`Deserialize` with `#[serde(rename_all = "snake_case")]`. The existing test pattern in `store_tests.rs` uses per-test in-memory SQLite pools, a `test_meta()` helper, and `#[tokio::test]` async functions.

The `db.rs` module uses `sqlx::migrate!("../../database/migrations")` to run migrations, establishing the canonical relative path pattern for migration references from `crates/anvilml-registry/`.

No prior source exists in `scanner.rs` — this task creates the first implementation. The `sha2` crate is already a transitive dependency (versions 0.10.9 and 0.11.0 in Cargo.lock) but not a direct dependency of `anvilml-registry`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sha2    | 0.11.0          | rust-docs MCP  | none (default features: alloc, oid) |

sha2 0.11.0 API shape confirmed: `sha2::Sha256` with `Digest` trait methods `update(&[u8])` and `finalize()` returning `GenericArray<u8, U32>`. MSRV 1.85 is compatible with project's Rust 1.96.0. The `digest` trait (`sha2::Digest`) provides the `update()` and `finalize()` methods.

## Approach

**Step 1: Add sha2 dependency.** Edit `crates/anvilml-registry/Cargo.toml` to add `sha2 = "0.11"` to the `[dependencies]` section. This is a new direct dependency; `sha2` is already a transitive dependency so there are no version conflicts.

**Step 2: Create `crates/anvilml-registry/src/scanner.rs`.** Implement the following:

### 2a. `ModelScanner` struct

```rust
pub struct ModelScanner {
    store: ModelStore,
}
```

The scanner holds a `ModelStore` reference (owned, not borrowed) because `scan_dir()` is async and needs to call `store.get()` for dedup checks. The `ModelStore` struct itself is `Clone` (it wraps an `Arc`-backed `SqlitePool` via sqlx), so ownership transfer is cheap.

### 2b. `ModelScanner::new(pool: SqlitePool)` constructor

```rust
impl ModelScanner {
    pub fn new(pool: SqlitePool) -> Self {
        Self { store: ModelStore::new(pool) }
    }
```

### 2c. `ModelScanner::scan_dir()` — the core method

```rust
pub async fn scan_dir(&self, root: &Path, depth: u32) -> Result<Vec<ModelMeta>, AnvilError>
```

Algorithm:
1. Walk the directory tree from `root` up to `depth` levels deep. At each level, list all files (not directories) using `std::fs::read_dir()`.
2. For each file, check if it already exists in the store by querying `store.get()` with the file path. Actually, the dedup check is: if a row with the same path exists AND its size_bytes and mtime_unix match the current file's size and mtime, skip it. Since `store.get()` returns by ID (SHA256 hash), we need a different approach — iterate all rows or add a path-based check. The simplest correct approach: call `store.list(None)` once before scanning, build a `HashMap<PathBuf, (u64, i64)>` of path → (size, mtime) from existing rows, then for each file check if the path exists in the map with matching size+mtime.
3. For files to scan: compute SHA256 of first 1 MiB using `sha2::Sha256`.
4. Infer `ModelKind` from the directory component relative to `root`.
5. Infer `ModelDtype` from filename substrings (case-insensitive).
6. Infer `ModelFormat` from file extension.
7. Construct `ModelMeta` and collect into result vector.
8. Upsert each new model via `store.upsert()`.

### 2d. `hash_file()` — private helper

```rust
async fn hash_file(path: &Path) -> Result<String, AnvilError>
```

Opens the file, reads up to 1 MiB (or whole file if smaller), computes SHA256 hex lowercase. Uses `std::fs::File` and `std::io::Read`. The `sha2::Sha256::new()` constructor, `.update(&chunk)` for each read, and `.finalize()` to get the result bytes, then `format!("{:x}", result)` for hex string.

### 2e. `infer_kind()` — private helper

Takes `&Path` (the directory path relative to root), extracts the file name component, and matches against known directory names.

### 2f. `infer_dtype()` — private helper

Takes the filename (as `&str`), lowercases it, and checks for substring matches in priority order: `fp8_e4m3fn`, `fp8_e5m2`, `fp8`, `fp16`, `bf16`, `fp32`. First match wins. No match → `Unknown`.

### 2g. `infer_format()` — private helper

Takes the file extension (as `&str`), lowercases it, and matches against known extensions.

**Step 3: Update `lib.rs`.** Add `pub mod scanner;` and `pub use scanner::ModelScanner;`.

**Step 4: Create `crates/anvilml-registry/tests/scanner_tests.rs`.** Write ≥ 6 tests:

1. `test_hash_stability_across_rename` — create a temp file, hash it, rename the file, hash again; assert identical hash.
2. `test_kind_inference_diffusion` — create a tempdir with `diffusion/` subdirectory containing a file; scan with depth=1; assert kind is `Diffusion`.
3. `test_kind_inference_text_encoders` — create `text_encoders/` subdirectory; assert `TextEncoder`.
4. `test_kind_inference_vae` — create `vae/` subdirectory; assert `Vae`.
5. `test_kind_inference_unknown_dir` — create a file in a non-standard subdirectory; assert `Unknown`.
6. `test_dtype_inference_fp8_e4m3fn` — create a file named `model_fp8_e4m3fn.safetensors`; assert `Fp8`.
7. `test_dtype_inference_bf16` — create `model_bf16.safetensors`; assert `Bf16`.
8. `test_unchanged_file_skips_rehash` — insert a row into the store with matching path/size/mtime; scan the file; assert it is not re-upserted (verify via store count).
9. `test_depth_limit_respected` — create nested dirs at depth 2; scan with depth=1; assert only top-level files are returned.

**Step 5: Bump version.** Edit `crates/anvilml-registry/Cargo.toml`: `version = "0.1.2"` → `version = "0.1.3"`.

**Logging:** Per `ANVILML_DESIGN.md §16.3` (DEBUG log points), the scanner should log at DEBUG level when a file is skipped (unchanged) and when a model is scanned. The `#[tracing::instrument]` attribute is applied to `scan_dir()` for span naming.

**Documentation:** All `pub` items get `///` doc comments per `ENVIRONMENT.md §10`. The `ModelScanner` struct, `new()`, and `scan_dir()` all have full doc comments describing arguments, return values, and error variants.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `ModelScanner` | `anvilml_registry::ModelScanner` | `pub struct ModelScanner { store: ModelStore }` |
| `ModelScanner::new` | `anvilml_registry::ModelScanner::new` | `pub fn new(pool: SqlitePool) -> Self` |
| `ModelScanner::scan_dir` | `anvilml_registry::ModelScanner::scan_dir` | `pub async fn scan_dir(&self, root: &Path, depth: u32) -> Result<Vec<ModelMeta>, AnvilError>` |

Private helpers (`hash_file`, `infer_kind`, `infer_dtype`, `infer_format`, directory walk logic) are module-private and not part of the public API.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/scanner.rs` | ModelScanner implementation |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod scanner;` and `pub use scanner::ModelScanner;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Add `sha2 = "0.11"` dependency; bump version 0.1.2 → 0.1.3 |
| CREATE | `crates/anvilml-registry/tests/scanner_tests.rs` | Integration tests (≥6 tests) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/scanner_tests.rs` | `test_hash_stability_across_rename` | SHA256 hash of a file's first 1 MiB is identical before and after renaming the file | Temp file with known content | File with 2 MiB of `b"A"` bytes, then renamed to different filename | Identical hex SHA256 string | `cargo test -p anvilml-registry --test scanner_tests hash_stability` exits 0 |
| `tests/scanner_tests.rs` | `test_kind_inference_diffusion` | Directory component `diffusion/` maps to `ModelKind::Diffusion` | Tempdir with `diffusion/model.safetensors` | Depth=1 scan | `ModelMeta.kind == Diffusion` | `cargo test -p anvilml-registry --test scanner_tests kind_inference_diffusion` exits 0 |
| `tests/scanner_tests.rs` | `test_kind_inference_text_encoders` | Directory component `text_encoders/` maps to `ModelKind::TextEncoder` | Tempdir with `text_encoders/encoder.safetensors` | Depth=1 scan | `ModelMeta.kind == TextEncoder` | `cargo test -p anvilml-registry --test scanner_tests kind_inference_text_encoders` exits 0 |
| `tests/scanner_tests.rs` | `test_kind_inference_vae` | Directory component `vae/` maps to `ModelKind::Vae` | Tempdir with `vae/autoencoder.safetensors` | Depth=1 scan | `ModelMeta.kind == Vae` | `cargo test -p anvilml-registry --test scanner_tests kind_inference_vae` exits 0 |
| `tests/scanner_tests.rs` | `test_dtype_inference_fp8_e4m3fn` | Filename substring `fp8_e4m3fn` maps to `ModelDtype::Fp8` | Tempdir with `model_fp8_e4m3fn.safetensors` | Depth=1 scan | `ModelMeta.dtype == Fp8` | `cargo test -p anvilml-registry --test scanner_tests dtype_inference_fp8` exits 0 |
| `tests/scanner_tests.rs` | `test_dtype_inference_bf16` | Filename substring `bf16` maps to `ModelDtype::Bf16` | Tempdir with `model_bf16.safetensors` | Depth=1 scan | `ModelMeta.dtype == Bf16` | `cargo test -p anvilml-registry --test scanner_tests dtype_inference_bf16` exits 0 |
| `tests/scanner_tests.rs` | `test_unchanged_file_skips_rehash` | File already in store with matching size+mtime is not re-upserted | Store has one row; temp file with same path/size/mtime | Depth=1 scan | Store count remains 1 (no new upsert) | `cargo test -p anvilml-registry --test scanner_tests unchanged_skips` exits 0 |
| `tests/scanner_tests.rs` | `test_depth_limit_respected` | Files at depth > N are not returned when scanning with depth=N | Tempdir with `a/file1.safetensors` and `a/b/file2.safetensors` | Depth=1 scan | Only `a/file1.safetensors` returned; `a/b/file2.safetensors` excluded | `cargo test -p anvilml-registry --test scanner_tests depth_limit` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/scanner_tests.rs` is automatically picked up by `cargo test --workspace --features mock-hardware` (ENVIRONMENT.md §6 Step 6). The `sha2` dependency is a new direct dependency but has no platform-specific build requirements — it's pure Rust with no FFI.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The scanner uses `std::fs::read_dir()` and `std::path::Path` which are cross-platform. File extension matching and directory component matching use `std::path` methods that work identically on Unix and Windows. The `sha2` crate is pure Rust with no platform-specific code paths.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sha2` 0.11 API differs from expected — `Digest` trait methods `update()` and `finalize()` may have different signatures in 0.11 vs 0.10 | Low | Medium | MCP confirmed sha2 0.11.0 uses `sha2::Digest` trait (re-exported from `digest` 0.11) with `update(&[u8])` and `finalize()` returning `GenericArray<u8, U32>`. Write a compile-check before proceeding. |
| `store.get()` returns by ID (SHA256 hash), not by path — the dedup check needs an alternative lookup strategy | Medium | Medium | The plan uses `store.list(None)` to build a path→(size,mtime) index before scanning. This is O(n) for the list call but correct and simple. No schema changes needed. |
| `tempfile` crate version mismatch — existing `Cargo.toml` declares `tempfile = "3.26"` in dev-dependencies | Low | Low | Already present as dev-dependency; no new dependency needed. Verified version 3.26+ exists and is compatible. |
| `scan_dir()` depth logic produces off-by-one errors in recursive walk | Medium | Low | Write `test_depth_limit_respected` as an explicit acceptance test. The walk uses a `(Path, u32)` queue where the initial call is `(root, 0)` and each recursive step increments depth. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --test scanner_tests` exits 0 with ≥ 6 tests
- [ ] `wc -l crates/anvilml-registry/src/scanner.rs` — file exists and is ≤ 400 lines
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` — file is ≤ 80 lines
- [ ] `cargo clippy -p anvilml-registry --features mock-hardware -- -D warnings` exits 0
- [ ] `grep '^## ' .forge/reports/P6-A4_plan.md | wc -l` — returns 12 (all required headings present)
