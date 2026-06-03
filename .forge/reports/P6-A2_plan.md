# Plan Report: P6-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P6-A2                                             |
| Phase       | 006 — Model Registry                              |
| Description | anvilml-registry: ModelRegistry store (upsert, get) |
| Depends on  | P6-A1                                              |
| Project     | anvilml                                            |
| Planned at  | 2026-06-04T00:45:00Z                              |
| Attempt     | 1                                                  |

## Objective

Create `src/store.rs` in the `anvilml-registry` crate implementing a `ModelRegistry` struct backed by a SQLite `SqlitePool`, providing `upsert` (INSERT OR REPLACE) and `get` (single model lookup by ID) methods, and re-export it from `lib.rs`.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/store.rs` with `ModelRegistry{pool: SqlitePool}` struct
- Implement `ModelRegistry::new(pool)` constructor
- Implement `async fn upsert(&self, meta: &ModelMeta) -> Result<()>` using `INSERT OR REPLACE INTO models VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
- Implement `async fn get(&self, id: &str) -> Result<Option<ModelMeta>>` using `SELECT * FROM models WHERE id = ?`, mapping all 8 columns via sqlx `query_as!` or manual column extraction
- Re-export `ModelRegistry` from `crates/anvilml-registry/src/lib.rs`
- Add integration test module `store_get` in `tests/store_get.rs`: upsert then get returns equal meta; get missing returns None
- Use `tempfile` for a temporary SQLite database path in tests

### Out of Scope
- List method (P6-A3)
- Rescan method (P6-A4)
- Any changes to `backend/main.rs` or `anvilml-server` handlers (later tasks)
- Migration files (already created by P6-A1 prerequisite)
- Scanner changes (already completed in P6-A1)

## Approach

1. **Create `crates/anvilml-registry/src/store.rs`:**
   - Define `pub struct ModelRegistry { pool: SqlitePool }`
   - Implement `impl ModelRegistry { pub fn new(pool: SqlitePool) -> Self { Self { pool } } }`
   - Implement `async fn upsert(&self, meta: &ModelMeta) -> Result<()>`:
     - Use `sqlx::query!` macro with the models table columns in schema order: `id`, `name`, `path`, `kind`, `size_bytes`, `dtype_hint`, `vram_estimate_mib`, `scanned_at`
     - SQL: `INSERT OR REPLACE INTO models (id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`
     - Convert `ModelMeta` fields to SQL types: `String`, `String`, `String` (PathBuf → display), `&str` (enum repr), `i64` (u64 cast), `&str`, `i64` (u32 cast), `&str` (chrono DateTime → RFC3339 string)
     - Return `Result<(), AnvilError>` via `.await.map_err(sqlx_error)?`
   - Implement `async fn get(&self, id: &str) -> Result<Option<ModelMeta>>`:
     - Use `sqlx::query_as!` or `sqlx::query_as` with `SELECT * FROM models WHERE id = ?`
     - Since `ModelMeta` derives `Serialize/Deserialize` but not `FromRow`, use manual column extraction: query returns `(id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at)` tuple
     - Convert each column back to the corresponding `ModelMeta` field (string → PathBuf via `PathBuf::from`, string → enum via `serde_json::from_str`, etc.)
     - Return `Result<Option<ModelMeta>, AnvilError>`

2. **Update `crates/anvilml-registry/src/lib.rs`:**
   - Add `pub mod store;`
   - Add `pub use store::ModelRegistry;`

3. **Create `crates/anvilml-registry/tests/store_get.rs`:**
   - Use `tempfile::NamedTempFile` for a unique DB path per test run
   - Call `db::open(path)` to get pool, then `ModelRegistry::new(pool)`
   - Test 1: Create a `ModelMeta`, upsert it, get by ID → verify all fields match
   - Test 2: Get by non-existent ID → verify returns `None`

4. **Run tests:** `cargo test -p anvilml-registry -- store_get` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-registry/src/store.rs` | ModelRegistry struct with upsert and get methods |
| Edit   | `crates/anvilml-registry/src/lib.rs` | Add `pub mod store;` and `pub use store::ModelRegistry;` |
| Create | `crates/anvilml-registry/tests/store_get.rs` | Integration tests: upsert+get roundtrip, get missing returns None |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/store_get.rs` | `test_upsert_then_get_returns_equal_meta` | Upsert a ModelMeta, then get by ID — all 8 fields match exactly |
| `tests/store_get.rs` | `test_get_missing_returns_none` | Get a non-existent model ID — returns None |

## CI Impact

No CI changes required. The task only adds source code and tests within the existing `anvilml-registry` crate. The standard CI matrix (`cargo test --workspace --features mock-hardware`) will automatically include the new tests. No new jobs, gates, or CI file modifications are needed.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `ModelMeta` does not derive `sqlx::FromRow`, so `query_as!` cannot be used directly | Use manual column extraction: `sqlx::query_as` on a tuple of the 8 columns, then reconstruct `ModelMeta` from the tuple in Rust code |
| `PathBuf` serialization/deserialization with SQLite TEXT column | Convert `PathBuf` to/from `String` via `.to_string_lossy()` on insert and `PathBuf::from()` on select |
| `chrono::DateTime<Utc>` ↔ SQLite TEXT conversion | Store as RFC3339/ISO 8601 string (e.g. `"2026-06-04T00:45:00Z"`), parse back with `DateTime::parse_from_rfc3339` and `.with_timezone(&Utc)` |
| Enum variants (`ModelKind`, `DType`) stored as TEXT in DB | Use the same serde string representation (e.g. `"Diffusion"`, `"F16"`) for both insert and select; deserialize via `serde_json::from_str` |
| Test isolation — concurrent tests sharing a temp file | Each test function creates its own `NamedTempFile` with a unique path; sqlx pool is created per-test, ensuring no shared state |

## Acceptance Criteria

- [ ] `crates/anvilml-registry/src/store.rs` exists with `ModelRegistry{pool: SqlitePool}` struct
- [ ] `ModelRegistry::new(pool)` constructor compiles and returns the struct
- [ ] `async fn upsert(&self, &ModelMeta) -> Result<()>` performs INSERT OR REPLACE INTO models
- [ ] `async fn get(&self, id: &str) -> Result<Option<ModelMeta>>` maps all 8 columns and returns None for missing IDs
- [ ] `ModelRegistry` is re-exported from `lib.rs` (`pub use store::ModelRegistry`)
- [ ] `cargo test -p anvilml-registry -- store_get` exits 0, verifying upsert+get roundtrip and get-missing-returns-none
