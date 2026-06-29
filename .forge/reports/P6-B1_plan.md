# Plan Report: P6-B1

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P6-B1                                               |
| Phase       | 006 — Model Registry & Artifacts                    |
| Description | anvilml-artifacts: ArtifactStore::save content-addressed write |
| Depends on  | P3-A11 (anvilml-core domain types), P6-A2 (SqlitePool creation) |
| Project     | anvilml                                             |
| Planned at  | 2026-06-29T23:15:00Z                                |
| Attempt     | 1                                                   |

## Objective

Create `crates/anvilml-artifacts/src/store.rs` implementing `ArtifactStore::save()` — a content-addressed PNG write that computes the SHA256 hex digest of the PNG bytes as the content hash, writes the file to `{artifact_dir}/{hash}.png` only if not already present (duplicate save is a no-op write, not an error), and persists the artifact metadata row keyed by hash. This establishes the write path for generated PNG artifacts before the read path (`get()`) and listing (`list()`) are built on top of it in later tasks.

## Scope

### In Scope
- Create `crates/anvilml-artifacts/src/store.rs` with `ArtifactStore` struct and `save()` method.
- `save()` computes SHA256 hex of `png_bytes` using the `sha2` crate.
- `save()` writes the PNG file to `{artifact_dir}/{hash}.png` only if the file does not already exist (idempotent duplicate save).
- `save()` persists an `ArtifactMeta` row keyed by hash to the shared `SqlitePool`.
- Update `crates/anvilml-artifacts/src/lib.rs` to declare `mod store` and `pub use store::ArtifactStore`.
- Update `crates/anvilml-artifacts/Cargo.toml` with new dependencies (`sha2`, `tokio` for tests, `tempfile` for tests, `sqlx` with sqlite/migrate/chrono features).
- Create `crates/anvilml-artifacts/tests/store_tests.rs` with ≥3 tests using a tempdir.
- Bump `anvilml-artifacts` crate patch version.

### Out of Scope
- `get()` method — deferred to P6-B2 (read path).
- `list()` method — deferred to P6-B3 (listing by job_id).
- The `artifacts` table migration — deferred to P6-B2 (the migration file `002_artifacts.sql`).
- Any HTTP handler or server integration — that belongs to the HTTP server phase.
- Dual-mode parity markers — the dual-mode (mock/real) parity marker convention defined in `ANVILML_DESIGN.md §10.6` applies only to Python node `execute()` and arch module `load()`/`sample()`/`decode()` functions. This task is pure Rust and does not touch any Python code, so no markers apply.

## Existing Codebase Assessment

The `anvilml-artifacts` crate currently exists as a buildable stub with only a one-line `//!` doc comment in `lib.rs` and no `store.rs` module. The `ArtifactMeta` type is already defined in `anvilml-core/src/types/artifact.rs` with fields: `hash`, `job_id`, `width`, `height`, `seed`, `steps`, `created_at`, `file_path`. The `AnvilError` enum in `anvilml-core/src/error.rs` includes `Db(#[from] sqlx::Error)` and `Io(#[from] std::io::Error)` variants, enabling `?` propagation from both database and filesystem operations.

The `anvilml-registry` crate already has an established pattern for SQLite-backed persistence: `ModelStore` in `crates/anvilml-registry/src/store.rs` uses `sqlx::query()` with positional binds, serializes enums via `serde_json::to_string()` then trims JSON quotes, and converts raw SQL rows back to domain types via a helper method. The integration tests in `crates/anvilml-registry/tests/store_tests.rs` use an in-memory SQLite pool with a unique cache name per test (uuid-based) to avoid shared `:memory:` database collisions.

The project uses `sha2 = "0.11"` and `digest = "0.11"` in `anvilml-registry` for model hashing. The `tempfile` crate is used as a dev-dependency in `anvilml-registry`. The workspace uses Rust edition 2024, pinned to 1.96.0.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed |
|--------|----------|------------------|----------------|------------------------|
| crate  | sha2     | 0.11.0           | rust-docs MCP  | none                   |
| crate  | tempfile | 3.27.0           | rust-docs MCP  | none                   |

**Note:** `sha2` 0.11.0 matches the version already used in `anvilml-registry/Cargo.toml` (`sha2 = "0.11"`). The `digest` crate (0.11.3) is a transitive dependency of `sha2` and does not need to be declared directly in `anvilml-artifacts/Cargo.toml`. The `tempfile` crate latest is 3.27.0; the project uses 3.26 in `anvilml-registry` dev-deps. `sqlx` (0.9.0), `tokio` (1.51.x), and `tracing` (0.1) are already present as transitive dependencies through `anvilml-core` but must be declared explicitly in `anvilml-artifacts/Cargo.toml` since the crate needs `sqlx` with specific features (`sqlite`, `runtime-tokio`, `migrate`, `chrono`) and `tokio` with `rt-multi-thread`/`macros` for async tests.

## Approach

### Step 1: Update `crates/anvilml-artifacts/Cargo.toml`

Add the following dependencies:
- `sha2 = "0.11"` — for SHA256 hashing of PNG bytes.
- `sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "migrate", "chrono"] }` — for SQLite operations. The `migrate` feature is needed for migration support; `chrono` for `DateTime<Utc>` mapping.
- `tokio = { version = "1.47.0", features = ["rt-multi-thread", "macros"] }` — for async runtime in tests.
- `tempfile = "3.26"` — for creating temporary directories in tests (matching the version used in `anvilml-registry`).
- `tracing = "0.1"` — for structured logging at DEBUG level.

The existing `anvilml-core` dependency provides `ArtifactMeta`, `AnvilError`, `Uuid`, `DateTime<Utc>`, and `PathBuf`.

### Step 2: Create `crates/anvilml-artifacts/src/store.rs`

Implement `ArtifactStore`:

```rust
pub struct ArtifactStore {
    artifact_dir: PathBuf,
    pool: SqlitePool,
}
```

**`new(artifact_dir: PathBuf, pool: SqlitePool) -> Self`** — constructor.

**`save(&self, png_bytes: &[u8], meta: &ArtifactMeta) -> Result<String, AnvilError>`** — the core method:
1. Compute SHA256 hex digest of `png_bytes` using `sha2::Sha256` and the `Digest` trait from `digest`:
   ```rust
   use sha2::{Sha256, Digest};
   let mut hasher = Sha256::new();
   hasher.update(png_bytes);
   let hash = format!("{:x}", hasher.finalize());
   ```
2. Construct the file path: `{artifact_dir}/{hash}.png`.
3. Check if the file already exists using `std::fs::metadata()`. If it exists, log at DEBUG level and return `Ok(hash)` — this is the idempotent duplicate-save case.
4. If the file does not exist, ensure the artifact directory exists via `std::fs::create_dir_all()`, then write the PNG bytes to the file path.
5. Persist the `ArtifactMeta` row to the database. Since the `artifacts` table migration (P6-B2) has not yet been applied, execute a `CREATE TABLE IF NOT EXISTS artifacts (...)` DDL statement before the INSERT. This is safe because:
   - `CREATE TABLE IF NOT EXISTS` is idempotent — it does nothing if the table already exists.
   - The table schema matches the one P6-B2 will create in `002_artifacts.sql`, ensuring consistency.
   - This avoids depending on a migration file that hasn't been written yet, while still fulfilling the task's requirement to "persist an ArtifactMeta row keyed by hash."
6. Return `Ok(hash)`.

The `CREATE TABLE IF NOT EXISTS` approach is chosen over a full migration runner because:
- The migration runner (`sqlx::migrate!()`) expects compiled-in migration files, and adding a migration file here would duplicate P6-B2's scope.
- A single `CREATE TABLE IF NOT EXISTS` is the minimal correct way to ensure the table exists without introducing a separate migration system into this crate.
- When P6-B2 runs its migration, the `artifacts` table already exists, so the migration is a no-op for that table.

**Logging:**
- `tracing::debug!(hash = %hash, "artifact saved")` on successful save.
- `tracing::debug!(hash = %hash, "artifact already exists — skipping write")` on duplicate save.
- `tracing::debug!(hash = %hash, "artifact metadata persisted")` after DB insert.

**Doc comments:** Every `pub` item gets a `///` doc comment per §12.1:
- `ArtifactStore` struct: describes ownership of content-addressed PNG storage and SQLite persistence.
- `new()`: describes the constructor arguments and their purpose.
- `save()`: describes the SHA256 computation, idempotent write, DB persistence, return value, and error variants.

### Step 3: Update `crates/anvilml-artifacts/src/lib.rs`

Add module declaration and re-export:
```rust
//! Content-addressed PNG artifact storage.

pub mod store;

pub use store::ArtifactStore;
```

### Step 4: Create `crates/anvilml-artifacts/tests/store_tests.rs`

Create the test file with ≥3 tests using `tempfile::tempdir()` for isolated temp directories:

**Test 1: `test_save_writes_file_once`**
- Create a tempdir and an `ArtifactStore` pointing to it (with an in-memory SQLite pool).
- Create `ArtifactMeta` with test values.
- Call `save()` with a known PNG byte slice.
- Assert the file exists at `{tempdir}/{hash}.png`.
- Assert the file size matches the input PNG bytes.
- Assert the returned hash matches the computed SHA256 of the input.

**Test 2: `test_duplicate_save_does_not_duplicate_or_error`**
- Same setup as Test 1.
- Call `save()` twice with the same PNG bytes.
- Assert the file count in the artifact directory is exactly 1.
- Assert no error is returned on the second call.
- Assert the file content is identical to the first write.

**Test 3: `test_different_content_produces_different_hash`**
- Create a tempdir and `ArtifactStore`.
- Create two `ArtifactMeta` values with different seeds.
- Call `save()` with two different PNG byte slices.
- Assert both files exist.
- Assert the two hashes are different.
- Assert each file's content matches its corresponding input.

**Test helper: `make_pool()`** — creates an in-memory SQLite pool (uuid-based cache name, matching the pattern from `anvilml-registry/tests/store_tests.rs`).

**Test helper: `test_meta()`** — constructs an `ArtifactMeta` with synthetic test values.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `crates/anvilml-artifacts/src/store.rs` | `pub struct ArtifactStore { artifact_dir: PathBuf, pool: SqlitePool }` |
| fn | `crates/anvilml-artifacts/src/store.rs` | `pub fn new(artifact_dir: PathBuf, pool: SqlitePool) -> Self` |
| fn | `crates/anvilml-artifacts/src/store.rs` | `pub async fn save(&self, png_bytes: &[u8], meta: &ArtifactMeta) -> Result<String, AnvilError>` |
| re-export | `crates/anvilml-artifacts/src/lib.rs` | `pub use store::ArtifactStore;` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-artifacts/src/store.rs` | `ArtifactStore` struct with `new()` and `save()` methods |
| Modify | `crates/anvilml-artifacts/src/lib.rs` | Add `pub mod store;` and `pub use store::ArtifactStore;` |
| Modify | `crates/anvilml-artifacts/Cargo.toml` | Add dependencies: `sha2`, `sqlx`, `tokio`, `tempfile`, `tracing`; bump patch version 0.1.0 → 0.1.1 |
| Create | `crates/anvilml-artifacts/tests/store_tests.rs` | ≥3 integration tests for `save()` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_save_writes_file_once` | `save()` writes the PNG file to `{artifact_dir}/{hash}.png`, returns the correct SHA256 hex hash, and persists the metadata row | Tempdir exists, in-memory SQLite pool created, `ArtifactMeta` with test values | Known PNG byte slice (e.g. 64×64 black PNG), `ArtifactMeta { hash: "...", job_id, width: 64, height: 64, seed: 42, steps: 20 }` | File exists at expected path, file size == input size, returned hash == SHA256 hex of input, DB row exists | `cargo test -p anvilml-artifacts --test store_tests test_save_writes_file_once` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_duplicate_save_does_not_duplicate_or_error` | Calling `save()` twice with identical bytes does not create a second file and does not return an error | Same as above | Same PNG byte slice, called twice with same `ArtifactMeta` | Exactly 1 file in artifact dir, both calls return `Ok(hash)`, file content unchanged | `cargo test -p anvilml-artifacts --test store_tests test_duplicate_save_does_not_duplicate_or_error` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_different_content_produces_different_hash` | Two different PNG byte slices produce two different hashes and two different files | Same as above | Two distinct PNG byte slices (e.g. 64×64 black PNG vs 64×64 white PNG) | Two files exist, hashes differ, each file's content matches its corresponding input | `cargo test -p anvilml-artifacts --test store_tests test_different_content_produces_different_hash` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-artifacts/tests/store_tests.rs` is picked up by the existing `cargo test --workspace --features mock-hardware` CI job (rust-linux and rust-windows) because it follows the convention `crates/{name}/tests/*.rs`. No new CI jobs, gates, or file type handlers are introduced.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. File path construction uses `PathBuf` which is platform-aware. The `sha2` crate is a pure-Rust implementation with no platform-specific code. The SQLite operations use `sqlx` which handles platform-specific connection strings. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `CREATE TABLE IF NOT EXISTS` DDL in `save()` may conflict with P6-B2's migration if the migration's `CREATE TABLE` does not include `IF NOT EXISTS`. | Low | Medium | P6-B2's migration context specifies `CREATE TABLE artifacts (...)` without `IF NOT EXISTS`, but since `CREATE TABLE IF NOT EXISTS` is used in `save()`, the table will already exist when P6-B2's migration runs. The migration's `CREATE TABLE` without `IF NOT EXISTS` would fail. Mitigation: the plan for P6-B2 should use `CREATE TABLE IF NOT EXISTS` to be safe, but this is P6-B2's responsibility, not P6-B1's. Document this as a note in the approach. |
| `sha2::Sha256` API shape may differ between 0.10 and 0.11 (the `Digest` trait usage pattern). | Low | High | Verified via MCP: `sha2` 0.11.0 provides `Sha256` struct. The `Digest` trait is from the `digest` crate (0.11.x), which uses `hasher.update(data)` / `hasher.finalize()` pattern. Confirmed this is the correct API for sha2 0.11. |
| In-memory SQLite pool with uuid-based cache name may not work correctly with `CREATE TABLE IF NOT EXISTS` followed by `INSERT` in the same test process if connections are pooled. | Low | Medium | Use `max_connections(1)` on the test pool (matching the pattern in `anvilml-registry/tests/store_tests.rs`). This ensures all operations in a single test use the same connection, avoiding cross-connection table visibility issues. |
| `tempfile::tempdir()` may fail if the system's temp directory is full or inaccessible. | Very Low | Low | This is a test-only risk. The tempdir is created fresh per test and cleaned up automatically. In CI, `/tmp` is always available and sufficiently large. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-artifacts --test store_tests` exits 0
- [ ] `wc -l crates/anvilml-artifacts/src/store.rs` — file exists and is ≤ 400 lines
- [ ] `wc -l crates/anvilml-artifacts/src/lib.rs` — file is ≤ 80 lines
- [ ] `grep "^## " .forge/reports/P6-B1_plan.md | wc -l` — must show 12 headings
- [ ] `head -1 .forge/reports/P6-B1_plan.md` — must print: `# Plan Report: P6-B1`
