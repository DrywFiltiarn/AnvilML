# Plan Report: P6-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A3                                       |
| Phase       | 006 — Model Registry & Artifacts            |
| Description | anvilml-registry: ModelStore CRUD for ModelMeta |
| Depends on  | P6-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T15:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-registry/src/store.rs` implementing `ModelStore` — the SQLite-backed persistence layer for `ModelMeta` records — and wire it into the crate's public API. This gives every later consumer (the scanner in P6-A4, the future `/v1/models` handler) a single entry point to insert, query, list, and delete model metadata rows. Acceptance: `cargo test -p anvilml-registry --test store_tests` exits 0 with ≥5 tests covering all four methods.

## Scope

### In Scope
- `crates/anvilml-registry/src/store.rs`: `ModelStore` struct with `pool: SqlitePool` field, and four methods: `upsert`, `get`, `list`, `delete`.
- `crates/anvilml-registry/src/lib.rs`: add `pub mod store;` and `pub use store::ModelStore;`.
- `crates/anvilml-registry/tests/store_tests.rs`: ≥5 integration tests using in-memory SQLite per test.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope. The scanner (P6-A4) that populates `mtime_unix` is a separate task and is not part of this task's scope.

## Existing Codebase Assessment

The codebase already has `anvilml-registry` as a buildable stub crate. P6-A1 created `database/migrations/001_initial.sql` with the `models` table schema (id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at). P6-A2 implemented `db.rs` with `create_pool()` — a function that creates a `SqlitePool`, enables WAL mode, and runs migrations. The `anvilml-registry/Cargo.toml` already declares `sqlx` (0.9.0) with features `["sqlite", "runtime-tokio", "migrate", "chrono"]` and `tempfile` as a dev-dependency.

The `anvilml-core` crate exports `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` via `pub use types::*;`. `ModelMeta` has fields: `id: String`, `name: String`, `path: PathBuf`, `kind: ModelKind`, `dtype: ModelDtype`, `format: ModelFormat`, `size_bytes: u64`, `scanned_at: DateTime<Utc>`. The `#[serde(rename_all = "snake_case")]` attribute means enum values serialize to lowercase text (e.g., `"diffusion"`, `"fp32"`, `"safetensors"`), which matches the SQL column values expected by the migration.

The `AnvilError` enum includes `Db(#[from] sqlx::Error)` and `Io(#[from] std::io::Error)`, enabling `?` propagation from both sqlx and std I/O. The `lib.rs` currently only declares `pub mod db;` and `pub use db::create_pool;` (5 lines, well within the 80-line cap).

No gap between design doc and source affects this task: the models table schema in the migration matches the ModelMeta field list (plus the extra `mtime_unix` column noted in the task context), and the sqlx features already declared include `chrono` which is needed for DateTime serialization.

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source     | Feature flags confirmed          |
|--------|--------|-----------------|----------------|----------------------------------|
| crate  | sqlx   | 0.9.0           | rust-docs MCP  | sqlite, runtime-tokio, migrate, chrono |

The sqlx features used by the existing `db.rs` (`sqlite`, `runtime-tokio`, `migrate`, `chrono`) are confirmed present in sqlx 0.9.0. No new dependencies are introduced by this task — only new source files and queries against the existing pool.

## Approach

1. **Create `crates/anvilml-registry/src/store.rs`.** Define the `ModelStore` struct:
   ```rust
   pub struct ModelStore {
       pool: SqlitePool,
   }
   ```
   Add a `///` doc comment to the struct describing its responsibility (CRUD for ModelMeta rows in the `models` table). Implement `new(pool: SqlitePool) -> Self` as a simple constructor.

2. **Implement `upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>`.**
   - Use `sqlx::query` with `INSERT OR REPLACE INTO models (id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`.
   - Bind parameters from `meta`: `meta.id`, `meta.name`, `meta.path.to_string_lossy().into_owned()`, `meta.kind` (serialized via serde as snake_case text), `meta.dtype`, `meta.format`, `meta.size_bytes as i64`, `0i64` for `mtime_unix` (placeholder — the scanner populates the real value in P6-A4), and `meta.scanned_at.format(&chrono::format_description::well_known::Rfc3339)` or equivalent ISO 8601 string for `scanned_at`.
   - `INSERT OR REPLACE` handles both insert and update in one statement, keyed by the `id` primary key. If the row already exists, it is replaced entirely.
   - Return `Ok(())` on success. Any sqlx error propagates via `?` and converts to `AnvilError::Db` through `#[from]`.
   - Add `#[tracing::instrument(fields(id = %meta.id))]` for observability.

3. **Implement `get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>`.**
   - Use `sqlx::query_as!(ModelMetaRow, "SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at FROM models WHERE id = ?", id)` to fetch a row.
   - Since `ModelMeta` cannot be directly mapped by sqlx's `query_as!` (it uses `PathBuf` and `DateTime<Utc>` which sqlx does not natively map), implement a helper struct `ModelMetaRow` with string/integer fields and convert it to `ModelMeta`:
     ```rust
     struct ModelMetaRow {
         id: String,
         name: String,
         path: String,
         kind: String,
         dtype: String,
         format: String,
         size_bytes: i64,
         mtime_unix: i64,
         scanned_at: String,
     }
     ```
   - Convert `ModelMetaRow` → `ModelMeta` by parsing the text fields back through serde (using `serde_json::from_str` on a constructed JSON object, or manual field-by-field conversion). Manual conversion is preferred for clarity:
     - `kind`: `serde_json::from_str::<ModelKind>(&format!("\"{}\"", row.kind))?`
     - `dtype`: same pattern for `ModelDtype`
     - `format`: same pattern for `ModelFormat`
     - `path`: `PathBuf::from(row.path)`
     - `size_bytes`: `row.size_bytes as u64`
     - `scanned_at`: parse the ISO 8601 string to `DateTime<Utc>` via `chrono::DateTime::parse_from_rfc3339`
   - If no row found, return `Ok(None)`. Use `fetch_optional` from sqlx.

4. **Implement `list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>`.**
   - Build the SQL query with an optional `WHERE kind = ?` clause:
     - If `kind` is `Some(k)`: `"SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at FROM models WHERE kind = ?"`
     - If `kind` is `None`: `"SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at FROM models"`
   - Use `fetch_all` to get all matching rows, then convert each `ModelMetaRow` → `ModelMeta` using the same conversion logic as `get`.
   - Return `Ok(vec)` — empty vec if no rows match.

5. **Implement `delete(&self, id: &str) -> Result<(), AnvilError>`.**
   - Use `sqlx::query("DELETE FROM models WHERE id = ?").execute(&self.pool).await?` to remove the row.
   - Return `Ok(())` on success (no error if the row didn't exist — SQL DELETE is a no-op for missing rows).

6. **Update `crates/anvilml-registry/src/lib.rs`.** Add `pub mod store;` and `pub use store::ModelStore;` after the existing `db` declarations. The file will have 7 lines total, well within the 80-line cap.

7. **Create `crates/anvilml-registry/tests/store_tests.rs`.** Write ≥5 integration tests, each creating its own in-memory SQLite pool via a helper function:
   - Helper: `fn make_pool() -> SqlitePool` — creates an in-memory pool with `SqliteConnectOptions::from_uri("sqlite::memory:")`, runs the migration from `001_initial.sql` (using `sqlx::migrate!` or executing the SQL file content directly), and returns the pool.
   - Each test constructs a `ModelStore` from its own pool.
   - Tests: (a) `test_upsert_get_roundtrip`, (b) `test_list_no_filter`, (c) `test_list_with_kind_filter`, (d) `test_delete_removes_row`, (e) `test_get_missing_id_returns_none`.

## Public API Surface

| Path | Item | Signature |
|------|------|-----------|
| `anvilml-registry::store::ModelStore` | struct | `pub struct ModelStore { pool: SqlitePool }` |
| `anvilml-registry::store::ModelStore::new` | fn | `pub fn new(pool: SqlitePool) -> Self` |
| `anvilml-registry::store::ModelStore::upsert` | fn | `pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>` |
| `anvilml-registry::store::ModelStore::get` | fn | `pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` |
| `anvilml-registry::store::ModelStore::list` | fn | `pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>` |
| `anvilml-registry::store::ModelStore::delete` | fn | `pub async fn delete(&self, id: &str) -> Result<(), AnvilError>` |

Re-export in `lib.rs`:
| Path | Item |
|------|------|
| `anvilml-registry::store` | `pub mod store;` |
| `anvilml-registry::ModelStore` | `pub use store::ModelStore;` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/store.rs` | `ModelStore` CRUD implementation |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod store;` and `pub use store::ModelStore;` |
| CREATE | `crates/anvilml-registry/tests/store_tests.rs` | ≥5 integration tests for ModelStore |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/store_tests.rs` | `test_upsert_get_roundtrip` | upsert then get returns the same ModelMeta | Fresh in-memory DB with migration applied | ModelMeta with id="test-1", name="test-model", path="/tmp/model.safetensors", kind=Diffusion, dtype=Fp32, format=Safetensors, size_bytes=1024, scanned_at=now() | `get("test-1")` returns `Some(meta)` matching the inserted values | `cargo test -p anvilml-registry --test store_tests -- test_upsert_get_roundtrip` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_list_no_filter` | list without kind filter returns all rows | Fresh in-memory DB; 3 models inserted with different kinds | 3 ModelMeta rows (Diffusion, TextEncoder, Vae) | `list(None)` returns all 3 rows | `cargo test -p anvilml-registry --test store_tests -- test_list_no_filter` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_list_with_kind_filter` | list with kind filter returns only matching rows | Fresh in-memory DB; 3 models inserted with different kinds | kind filter = `Some(ModelKind::Diffusion)` | `list(Some(Diffusion))` returns exactly 1 row | `cargo test -p anvilml-registry --test store_tests -- test_list_with_kind_filter` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_delete_removes_row` | delete removes the row; subsequent get returns None | Fresh in-memory DB; 1 model inserted | id of the inserted model | `delete(id)` succeeds, then `get(id)` returns `None` | `cargo test -p anvilml-registry --test store_tests -- test_delete_removes_row` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_get_missing_id_returns_none` | get on a non-existent id returns None | Fresh in-memory DB; no rows inserted | id="nonexistent" | `get("nonexistent")` returns `None` | `cargo test -p anvilml-registry --test store_tests -- test_get_missing_id_returns_none` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-registry/tests/`, which is already picked up by `cargo test --workspace --features mock-hardware` (the standard CI test command). No new file types, gates, or CI jobs are needed.

## Platform Considerations

None identified. The models table schema and ModelStore CRUD are platform-neutral — all operations use SQLite in-memory or file-backed storage with no platform-specific code paths. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::query_as!` cannot directly map `ModelMeta` because it contains `PathBuf` and `DateTime<Utc>`. A naive `query_as!(ModelMeta, ...)` will fail at compile time. | High | High | Use a separate `ModelMetaRow` struct with string/integer fields, then convert row → `ModelMeta` manually in Rust. This is the standard pattern for sqlx when the target type is not a simple primitive. |
| `chrono::DateTime<Utc>` serialization to ISO 8601 text for `scanned_at` and back. The RFC 3339 format is what sqlx's `chrono` feature expects, but `DateTime<Utc>` does not implement `FromSql` directly in all sqlx versions. | Medium | Medium | Use `chrono::format_description::well_known::Rfc3339` for formatting on insert, and `DateTime::parse_from_rfc3339` + `.with_timezone(&Utc)` for parsing on retrieval. If `query_as!` supports `DateTime<Utc>` via the `chrono` feature, use it directly and fall back to the manual conversion only if compilation fails. |
| `INSERT OR REPLACE` replaces the entire row including `mtime_unix`. When the scanner later calls `upsert` with a real `mtime_unix`, the previous row's `mtime_unix` (which was 0) is overwritten — this is the correct behaviour since the scanner is the authoritative source for that field. | Low | Low | Document in code comment that `mtime_unix` is a scanner-populated field and `upsert` uses 0 as a placeholder. The scanner task (P6-A4) will call `upsert` with the real value. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry --test store_tests` exits 0
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` reports ≤80 (verifies lib.rs cap still holds)
- [ ] `grep "pub use store::ModelStore" crates/anvilml-registry/src/lib.rs` matches exactly once
