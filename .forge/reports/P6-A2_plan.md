# Plan Report: P6-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A2                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-registry: ModelStore SQLite CRUD    |
| Depends on  | P6-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T16:56:05Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-registry/src/store.rs` implementing `ModelStore{pool: SqlitePool}` with four CRUD methods — `upsert`, `get`, `list`, `delete` — backed by the `models` table in SQLite. This gives the model registry persistent storage so that scanner results survive server restarts. When complete, `cargo test -p anvilml-registry -- store` exits 0 with ≥ 6 tests, proving the store correctly persists, retrieves, lists, and deletes `ModelMeta` records.

## Scope

### In Scope
- Create `crates/anvilml-registry/src/store.rs` — `ModelStore` struct with `pool: SqlitePool`, methods:
  - `pub async fn new(pool: SqlitePool) -> Self`
  - `pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>`
  - `pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>`
  - `pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>`
  - `pub async fn delete(&self, id: &str) -> Result<bool, AnvilError>`
- Create `crates/anvilml-registry/tests/store_tests.rs` — ≥ 6 tests
- Update `crates/anvilml-registry/src/lib.rs` — add `pub mod store; pub use store::ModelStore;`
- Update `crates/anvilml-registry/Cargo.toml` — bump patch version 0.1.4 → 0.1.5
- Add `sqlx::FromRow` derive to `ModelMeta` in `crates/anvilml-core/src/types/model.rs` (required by `query_as!` macro in `upsert`)

### Out of Scope
- `list_by_kind` as a separate method (task context mentions it but acceptance criteria only requires `list` with `Option<ModelKind>` filter)
- DeviceCapabilityStore (P6-A3)
- REST handlers for model endpoints (P6-B1, P6-B2)
- Migration for new tables (the `models` table already exists in `001_initial.sql`)
- Scanner integration with the store (future tasks will wire scanner output into store.upsert)

## Existing Codebase Assessment

The `anvilml-registry` crate already has `db.rs` (providing `open()` and `open_in_memory()`), `scanner.rs` (directory walk producing `Vec<ModelMeta>`), and `seed_loader.rs` (SHA256-gated SQL seed runner). The `lib.rs` declares three modules: `db`, `scanner`, `seed_loader`. No `store.rs` exists yet.

The `models` table is already defined in `backend/migrations/001_initial.sql` with columns matching `ModelMeta` exactly: `id TEXT PRIMARY KEY`, `name TEXT NOT NULL`, `path TEXT NOT NULL`, `kind TEXT NOT NULL`, `dtype TEXT NOT NULL`, `format TEXT NOT NULL`, `size_bytes INTEGER NOT NULL`, `scanned_at TEXT NOT NULL`.

Test patterns established in `db_tests.rs` and `scanner_tests.rs`:
- Tests are async `#[tokio::test]` functions with doc comments explaining what they verify.
- Tests use `open_in_memory()` from `anvilml_registry` for database isolation.
- Temp file tests use `tempfile::tempdir()` for unique paths.
- Raw SQL queries use `sqlx::query()` with `.bind()` for parameters and `row.get::<T>()` for column extraction.
- `serial_test = "3.5"` is available in dev-dependencies for env-var isolation (not needed here since tests don't mutate env vars).

The `ModelMeta` struct in `anvilml-core/src/types/model.rs` derives `Debug, Clone, Serialize, Deserialize, ToSchema` but **not** `FromRow`. Adding `FromRow` is required because `query_as!` (used in `upsert`) needs it.

## Resolved Dependencies

| Type   | Name         | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| crate  | sqlx        | 0.9.0           | Cargo.lock     | runtime-tokio, sqlite, json |
| crate  | serial_test | 3.5             | Cargo.lock     | n/a                     |

**Notes:**
- `sqlx = "0.9.0"` is already declared in the workspace `Cargo.toml` with features `runtime-tokio`, `sqlite`, `json`. No new dependency is introduced.
- `serial_test = "3.5"` is already in `anvilml-registry` dev-dependencies. No new dependency.
- `FromRow` derive macro is provided by `sqlx-core` and is available when the `sqlite` feature is enabled. Verified: the `sqlx::FromRow` trait and its derive macro exist in sqlx 0.9.0.

## Approach

1. **Add `FromRow` derive to `ModelMeta`** in `crates/anvilml-core/src/types/model.rs`.
   - Append `sqlx::FromRow` to the derive list: `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]`.
   - Rationale: `query_as!` requires `FromRow` to map SQL rows to the target type. `ModelMeta` fields all map to SQLite-compatible types (`String` → TEXT, `i64` → INTEGER for `DateTime<Utc>` via chrono's sqlite integration).

2. **Create `crates/anvilml-registry/src/store.rs`** with the following structure:
   - Module-level doc comment describing the crate's ownership of model CRUD.
   - `ModelStore` struct: `pub pool: SqlitePool` (or private with getter; private is more idiomatic — use `pool: SqlitePool` without `pub`, matching the pattern where the struct owns the connection).
   - `impl ModelStore`:
     - `pub async fn new(pool: SqlitePool) -> Self` — constructor, no-op body.
     - `pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>` — use `sqlx::query_as!` with `INSERT OR REPLACE INTO models (...) VALUES (...)`. The `INSERT OR REPLACE` ensures idempotent upserts: if the `id` (PRIMARY KEY) already exists, the old row is deleted and the new one inserted. Log the operation at DEBUG with `id=` and `kind=` fields.
     - `pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` — use `sqlx::query()` with `.bind(id)`, `fetch_optional()`, then construct `ModelMeta` from row fields using `row.get()`. Return `None` if not found.
     - `pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>` — use `sqlx::query()` with conditional WHERE clause: if `kind` is `Some`, append `AND kind = ?`; otherwise no WHERE. Use `fetch_all()`, then construct `ModelMeta` from each row.
     - `pub async fn delete(&self, id: &str) -> Result<bool, AnvilError>` — use `sqlx::query()` with `.bind(id)`, `execute()`, check `rows_affected() > 0`.
   - All methods return `Result<T, AnvilError>`, converting `sqlx::Error` via `?` (the `#[from]` on `AnvilError::Db` handles this automatically).

3. **Create `crates/anvilml-registry/tests/store_tests.rs`** with ≥ 7 tests:
   - `test_upsert_and_get` — upsert a `ModelMeta`, then get it back, verify all fields match.
   - `test_upsert_overwrites` — upsert same ID twice with different data, verify second data is returned on get.
   - `test_get_not_found` — get a non-existent ID, verify `None`.
   - `test_list_all` — upsert multiple models, list without filter, verify all returned.
   - `test_list_filter_by_kind` — upsert Diffusion + Vae models, list with `Some(ModelKind::Vae)`, verify only Vae returned.
   - `test_delete_existing` — upsert then delete, verify `true` returned and get returns `None`.
   - `test_delete_not_found` — delete a non-existent ID, verify `false` returned.
   - Each test uses `open_in_memory()`, has a doc comment, and is a `#[tokio::test]`.

4. **Update `crates/anvilml-registry/src/lib.rs`** — add `pub mod store;` and `pub use store::ModelStore;` after the existing module declarations.

5. **Bump `crates/anvilml-registry/Cargo.toml`** patch version: `0.1.4` → `0.1.5`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `ModelStore` | struct | `anvilml_registry::ModelStore` | `pub struct ModelStore { pool: SqlitePool }` |
| `ModelStore::new` | fn | `anvilml_registry::ModelStore` | `pub async fn new(pool: SqlitePool) -> Self` |
| `ModelStore::upsert` | fn | `anvilml_registry::ModelStore` | `pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>` |
| `ModelStore::get` | fn | `anvilml_registry::ModelStore` | `pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>` |
| `ModelStore::list` | fn | `anvilml_registry::ModelStore` | `pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>` |
| `ModelStore::delete` | fn | `anvilml_registry::ModelStore` | `pub async fn delete(&self, id: &str) -> Result<bool, AnvilError>` |
| `ModelMeta::FromRow` | derive | `anvilml_core::ModelMeta` | Added `sqlx::FromRow` to existing derive list (not a new pub item, but a required trait impl) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/store.rs` | ModelStore struct with upsert/get/list/delete CRUD methods |
| CREATE | `crates/anvilml-registry/tests/store_tests.rs` | ≥ 7 integration tests for ModelStore |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod store;` and `pub use store::ModelStore;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |
| MODIFY | `crates/anvilml-core/src/types/model.rs` | Add `sqlx::FromRow` to `ModelMeta` derive |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/store_tests.rs` | `test_upsert_and_get` | Upsert a `ModelMeta`, then retrieve it with `get()`, all fields match | In-memory DB with models table | Valid `ModelMeta` | `get()` returns `Some(meta)` with matching fields | `cargo test -p anvilml-registry -- store test_upsert_and_get` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_upsert_overwrites` | Upsert same ID twice with different data; second upsert overwrites first | In-memory DB | Two `ModelMeta` with same ID, different name | `get()` returns second version | `cargo test -p anvilml-registry -- store test_upsert_overwrites` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_get_not_found` | `get()` for non-existent ID returns `None` | In-memory DB, no models inserted | Non-existent ID string | `get()` returns `None` | `cargo test -p anvilml-registry -- store test_get_not_found` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_list_all` | `list(None)` returns all upserted models | In-memory DB, 3 models upserted | No filter | `list()` returns 3 items | `cargo test -p anvilml-registry -- store test_list_all` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_list_filter_by_kind` | `list(Some(kind))` returns only matching models | In-memory DB, Diffusion + Vae models upserted | `kind = Some(ModelKind::Vae)` | `list()` returns 1 Vae model | `cargo test -p anvilml-registry -- store test_list_filter_by_kind` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_delete_existing` | `delete()` for existing ID returns `true`, subsequent `get()` returns `None` | In-memory DB, one model upserted | Existing model ID | `delete()` returns `true`, `get()` returns `None` | `cargo test -p anvilml-registry -- store test_delete_existing` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | `test_delete_not_found` | `delete()` for non-existent ID returns `false` | In-memory DB, no models inserted | Non-existent ID | `delete()` returns `false` | `cargo test -p anvilml-registry -- store test_delete_not_found` exits 0 |

## CI Impact

No CI changes required. The new test file follows the established pattern (`crates/{name}/tests/*_tests.rs`) which `cargo test` picks up automatically. No CI workflow files are modified. The `rust-linux` and `rust-windows` CI jobs will run the new tests as part of `cargo test --workspace --features mock-hardware`.

## Platform Considerations

None identified. The SQLite operations are platform-neutral — WAL mode, parameter binding, and `INSERT OR REPLACE` behave identically on Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The `ModelMeta::path` field is a `String` (not `PathBuf`), so no platform-specific path serialization is involved.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ModelMeta` fields may not map cleanly to SQLite column types for `query_as!` — specifically, `DateTime<Utc>` requires chrono's sqlite integration feature, and enum fields stored as `String` need correct column type mapping. | Medium | High | Verify that `chrono`'s `sqlite` feature is enabled in the workspace `Cargo.toml` (it is: `chrono = { version = "0.4.45", features = ["serde"] }`). The `DateTime<Utc>` maps to SQLite TEXT via chrono's built-in sqlite type mapping. If `query_as!` fails, fall back to manual row construction in `upsert` using `query()` instead of `query_as!`. |
| Adding `FromRow` to `ModelMeta` in `anvilml-core` may cause compilation issues in crates that don't depend on `sqlx` (e.g., `anvilml-server` depends on `anvilml-core` but doesn't directly use sqlx). | Low | Medium | The `FromRow` derive is a zero-cost trait impl — it only adds a trait bound, not runtime code. Crates that don't use `query_as!` on `ModelMeta` are unaffected. If compilation fails due to missing `sqlx` in `anvilml-core`'s dependencies, add `sqlx` as an optional dependency to `anvilml-core` and gate the derive behind it. |
| `INSERT OR REPLACE` semantics may produce unexpected behavior if the `models` table has foreign keys or triggers that fire on DELETE before INSERT. | Low | Medium | The `models` table has no foreign keys or triggers in `001_initial.sql`. `INSERT OR REPLACE` simply deletes the old row and inserts the new one. This is safe for the current schema. |
| Tests may fail due to SQLite connection pooling in `open_in_memory()` — in-memory databases are per-connection, not per-pool. If `SqlitePool::connect("sqlite::memory:")` creates multiple connections, each gets a separate database. | Medium | High | Use `SqlitePool::builder()` with `max_connections(1)` for test pools to ensure a single connection. Alternatively, use `sqlx::SqliteConnectOptions::memory(":test")` (named in-memory DB) which shares state across connections in the same pool. The existing `open_in_memory()` in `db.rs` already uses `SqlitePool::connect("sqlite::memory:")` — if this has the per-connection issue, it would affect all tests, not just new ones. Verify at test time. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- store` exits 0 with ≥ 6 tests
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings from store.rs or model.rs changes)
- [ ] `grep "^pub " crates/anvilml-registry/src/store.rs | wc -l` returns ≥ 6 (new `pub` items: struct + 4 methods + 1 constructor)
- [ ] `grep "ModelStore" crates/anvilml-registry/src/lib.rs` returns the `pub use store::ModelStore;` line
