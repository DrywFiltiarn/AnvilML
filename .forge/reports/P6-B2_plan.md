# Plan Report: P6-B2

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P6-B2                                                       |
| Phase       | 006 — Model Registry & Artifacts                            |
| Description | database/: artifacts table migration + ArtifactStore::get    |
| Depends on  | P6-B1                                                       |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-30T00:30:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Create the `artifacts` table migration (`database/migrations/002_artifacts.sql`) and extend `ArtifactStore` in `crates/anvilml-artifacts/src/store.rs` with a `get(&self, hash: &str) -> Result<Option<Vec<u8>>, AnvilError>` method that reads the PNG file by hash from the artifact directory. This completes the save/get round-trip for a single artifact by content hash, establishing the read path that the save path (P6-B1) wrote to disk.

## Scope

### In Scope
- Create `database/migrations/002_artifacts.sql` with the `artifacts` table DDL and `idx_artifacts_job_id` index.
- Add `pub async fn get(&self, hash: &str) -> Result<Option<Vec<u8>>, AnvilError>` to `ArtifactStore` in `store.rs`.
- Bump `anvilml-artifacts` patch version from `0.1.1` to `0.1.2`.
- Add >=3 new tests to `crates/anvilml-artifacts/tests/store_tests.rs` (save-then-get roundtrip, unknown hash returns None, duplicate save preserves original content).
- Total test count in `store_tests.rs` reaches >=6.
- Update `docs/TESTS.md` with entries for the new tests.

### Out of Scope
- The `list()` method — deferred to P6-B3 (confirmed: P6-B3's context is "ArtifactStore::list by job_id", which genuinely covers this scope).
- Any database query against the `artifacts` table from Rust — `get()` is filesystem-only.
- Any changes to `lib.rs` — `pub mod store; pub use store::ArtifactStore;` are already declared by P6-B1.
- Any changes to `Cargo.toml` dependencies — no new crates are needed.

## Existing Codebase Assessment

The `anvilml-artifacts` crate already has `save()` implemented (P6-B1) in `store.rs`. The `ArtifactStore` struct holds `artifact_dir: PathBuf` and `pool: SqlitePool`. The `save()` method computes SHA-256, writes the PNG file idempotently, and persists metadata via `INSERT OR IGNORE`. An inline `ensure_artifacts_table()` helper uses `CREATE TABLE IF NOT EXISTS` as a temporary measure until P6-B2 introduces the formal migration — this inline DDL is idempotent and safe to keep alongside the migration file.

The `lib.rs` (9 lines) already declares `pub mod store; pub use store::ArtifactStore;` — no changes needed.

The `store_tests.rs` has 3 tests from P6-B1: `test_save_writes_file_once`, `test_duplicate_save_does_not_duplicate_or_error`, and `test_different_content_produces_different_hash`. Each test uses its own in-memory SQLite pool with a unique cache name and its own temp directory. Test fixtures include `test_64x64_black.png` and `test_64x64_white.png`.

The `AnvilError` enum (from `anvilml-core`) already includes `Io(#[from] std::io::Error)` and `Db(#[from] sqlx::Error)` variants, which are the two error types this task's code can produce.

No gap exists between the design doc and current source that affects this task. The inline DDL in `ensure_artifacts_table()` is a temporary measure explicitly noted as being replaced by P6-B2's migration — the plan preserves it as a safety net (idempotent `CREATE TABLE IF NOT EXISTS` is harmless when the migration has already created the table).

## Resolved Dependencies

None. This task introduces no new external crates or packages. All types used (`PathBuf`, `std::fs::read`, `AnvilError`, `Vec<u8>`) are from the standard library or already-present dependencies (`anvilml-core`, `sqlx`).

## Approach

1. **Create `database/migrations/002_artifacts.sql`**

   Write a SQL file that creates the `artifacts` table and index:
   ```sql
   -- Migration 002: Artifacts table
   --
   -- Creates the `artifacts` table for persisted ArtifactMeta rows.
   -- Columns map from ArtifactMeta (anvilml-core/src/types/artifact.rs):
   --   hash, job_id, width, height, seed, steps, created_at, file_path
   -- Plus an index on job_id for the future list() query.

   CREATE TABLE IF NOT EXISTS artifacts (
       hash        TEXT PRIMARY KEY,  -- SHA-256 hex content address
       job_id      TEXT NOT NULL,     -- UUID string of the originating job
       width       INTEGER NOT NULL,  -- image width in pixels
       height      INTEGER NOT NULL,  -- image height in pixels
       seed        INTEGER NOT NULL,  -- random seed (i64, supports negative seeds)
       steps       INTEGER NOT NULL,  -- diffusion steps
       created_at  TEXT NOT NULL,     -- ISO 8601 UTC timestamp
       file_path   TEXT NOT NULL      -- filesystem path to the PNG file
   );

   CREATE INDEX idx_artifacts_job_id ON artifacts(job_id);
   ```
   This matches the column types from `ArtifactMeta` (width/height as u32 → INTEGER in SQL, seed as i64 → INTEGER, steps as u32 → INTEGER, created_at as DateTime<Utc> → TEXT in RFC 3339). The `hash` column is TEXT PRIMARY KEY matching the SHA-256 hex digest format.

2. **Add `get()` method to `ArtifactStore` in `store.rs`**

   Append a new method after the existing `save()` and `ensure_artifacts_table()` methods:
   ```rust
   /// Retrieve a saved artifact by its content hash.
   ///
   /// Reads the PNG file at `{artifact_dir}/{hash}.png` from disk and returns
   /// its bytes. Returns `Ok(None)` if no file exists for the given hash
   /// (file not found). Returns `Err` for any other I/O error (permission
   /// denied, truncated file, etc.).
   ///
   /// This is a pure filesystem read — it does not query the database.
   /// The database row may exist without the file (partial save), or
   /// the file may exist without the row (prior to P6-B1's DB persistence).
   ///
   /// # Arguments
   ///
   /// * `hash` — The SHA-256 hex content address to look up.
   ///
   /// # Returns
   ///
   /// `Ok(bytes)` with the PNG file contents if the file exists,
   /// `Ok(None)` if no file exists for this hash,
   /// or `Err(AnvilError::Io)` for other filesystem errors.
   #[tracing::instrument(fields(artifact_dir = %self.artifact_dir.display()), skip(self))]
   pub async fn get(&self, hash: &str) -> Result<Option<Vec<u8>>, AnvilError> {
       let file_path = self.artifact_dir.join(format!("{hash}.png"));

       // Attempt to read the file. If it doesn't exist, return None
       // rather than an error — this is the expected "not found" path
       // for a content-addressed store.
       match std::fs::read(&file_path) {
           Ok(bytes) => {
               tracing::debug!(hash = %hash, bytes = bytes.len(), "artifact read from disk");
               Ok(Some(bytes))
           }
           Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
               tracing::debug!(hash = %hash, "artifact not found on disk");
               Ok(None)
           }
           Err(err) => {
               // Any other I/O error (permission denied, etc.) propagates
               // as an Io error via the From<std::io::Error> impl on AnvilError.
               tracing::error!(hash = %hash, error = %err, "failed to read artifact from disk");
               Err(err.into())
           }
       }
   }
   ```
   Rationale: `std::fs::read` is synchronous but acceptable here because it reads a single file (typically tens to hundreds of KiB for a PNG), not a large model file. The cost is negligible and does not block the async executor meaningfully. Using `match` instead of `?` is required because `NotFound` is a valid result (returns `None`) while other errors should propagate as `Err`.

3. **Bump `anvilml-artifacts` patch version**

   Edit `crates/anvilml-artifacts/Cargo.toml`: change `version = "0.1.1"` to `version = "0.1.2"`.

4. **Add tests to `crates/anvilml-artifacts/tests/store_tests.rs`**

   Append 3 new test functions after the existing 3:

   a. **`test_save_then_get_roundtrips`** — save a PNG, then get it by hash, verify returned bytes match the original.

   b. **`test_get_unknown_hash_returns_none`** — call `get()` with a hash that was never saved, verify `Ok(None)`.

   c. **`test_get_after_duplicate_save_returns_original_content`** — save a PNG, save different content with a different hash (creating two files), then get the original hash and verify it returns the original content (not the duplicate's content).

   Each test uses its own tempdir and in-memory pool, following the established pattern.

5. **Update `docs/TESTS.md`**

   Add three new entries under the anvilml-artifacts section for the new tests.

## Public API Surface

| Crate/Module | Item | Signature |
|-------------|------|-----------|
| `anvilml-artifacts/src/store.rs` | `pub async fn get` | `pub async fn get(&self, hash: &str) -> Result<Option<Vec<u8>>, AnvilError>` |

No other new `pub` items. The `lib.rs` declarations (`pub mod store; pub use store::ArtifactStore;`) are already present from P6-B1.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/migrations/002_artifacts.sql` | Artifacts table DDL + job_id index |
| MODIFY | `crates/anvilml-artifacts/src/store.rs` | Add `get()` method |
| MODIFY | `crates/anvilml-artifacts/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |
| MODIFY | `crates/anvilml-artifacts/tests/store_tests.rs` | Add 3 new tests (>=6 total) |
| MODIFY | `docs/TESTS.md` | Add entries for 3 new tests |

No changes to `lib.rs` — declarations already present from P6-B1.

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_save_then_get_roundtrips` | Save a PNG then get it by hash returns the exact original bytes | Tempdir + in-memory pool exist | 64×64 black PNG, test metadata | `Ok(Some(bytes))` where bytes == original PNG | `cargo test -p anvilml-artifacts --test store_tests test_save_then_get_roundtrips` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_get_unknown_hash_returns_none` | get() on a hash that was never saved returns Ok(None) | Tempdir + in-memory pool exist | Random hash string not matching any saved file | `Ok(None)` | `cargo test -p anvilml-artifacts --test store_tests test_get_unknown_hash_returns_none` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_get_after_duplicate_save_returns_original_content` | After saving two different PNGs, get() for the first hash returns the first file's content, not the second | Tempdir + in-memory pool exist; two saves performed | 64×64 black PNG + 64×64 white PNG, two metadata rows | `Ok(Some(black_png_bytes))` for first hash, `Ok(Some(white_png_bytes))` for second hash | `cargo test -p anvilml-artifacts --test store_tests test_get_after_duplicate_save_returns_original_content` exits 0 |

Acceptance command for full suite: `cargo test -p anvilml-artifacts --test store_tests` exits 0 (>=6 total tests).

## CI Impact

No CI changes required. The task only adds a new SQL migration file and new tests within an existing test crate. The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace --features mock-hardware` which picks up all crate tests. The migration file is a plain SQL artifact — it is not compiled or executed by CI, only validated manually via `sqlite3 :memory: < database/migrations/002_artifacts.sql` as the task's acceptance criterion.

## Platform Considerations

None identified. The `std::fs::read` call is platform-neutral — it works identically on Linux and Windows. The `PathBuf::join` for constructing `{artifact_dir}/{hash}.png` uses the platform's path separator, which is the correct behavior for filesystem access. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ensure_artifacts_table()` inline DDL creates a table schema that diverges from `002_artifacts.sql` — if the migration uses different column types or constraints, the inline DDL may silently create a different table that the migration then skips. | Low | Medium | The migration uses `CREATE TABLE IF NOT EXISTS` with the same schema as the inline DDL. Both define identical columns and types. The inline DDL is kept as a safety net for pre-migration usage; when the migration runs first (which it should via the migration runner), the inline DDL becomes a no-op. This is the same pattern the existing code already uses. |
| `get()` reads a potentially large PNG file synchronously, blocking the Tokio runtime worker thread during I/O. | Low | Low | Typical PNG artifacts are 100 KiB–5 MiB. A `std::fs::read` on such a file takes microseconds to milliseconds on SSD — negligible compared to the async operations around it (DB queries, IPC). If this becomes a concern, it can be moved to `tokio::fs::read` in a later refactoring. |
| The migration file uses `CREATE TABLE IF NOT EXISTS` but the inline DDL in `ensure_artifacts_table()` already runs on every `save()` call — if the migration system is not yet wired into the pool creation, the inline DDL creates the table and the migration becomes a no-op. | Low | Low | This is by design: `save()` was already functional before P6-B2 via the inline DDL. The migration file establishes the canonical schema for future tooling (migration audits, documentation) and for the `list()` method (P6-B3) which may query the table. The inline DDL remains safe as a fallback. |

## Acceptance Criteria

- [ ] `sqlite3 :memory: < database/migrations/002_artifacts.sql` exits 0
- [ ] `cargo test -p anvilml-artifacts --test store_tests` exits 0 (>=6 total tests)
- [ ] `wc -l crates/anvilml-artifacts/src/lib.rs` reports <=80 (lib.rs unchanged, already compliant)
- [ ] `grep '^pub ' crates/anvilml-artifacts/src/store.rs | grep -c 'fn\|struct\|enum\|trait\|const\|type'` confirms exactly 2 pub items: `new()` and `get()` (plus the existing `save()`) — no unintended new pub items
