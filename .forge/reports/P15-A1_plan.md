# Plan Report: P15-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-A1                                      |
| Phase       | 015 — Artifact Storage                      |
| Description | anvilml-server: artifact/store.rs content-addressed PNG storage |
| Depends on  | none (Phase 014 prerequisite satisfied by phase ordering) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-20T13:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `ArtifactStore` in `crates/anvilml-server/src/artifact/store.rs` — the persistence layer for PNG artifacts. `ArtifactStore` wraps a `PathBuf` (artifact directory) and a `SqlitePool`, implements SHA-256 content-addressed storage with idempotent saves, and provides `save`, `get`, and `list` async methods. This task also adds `width` and `height` fields to the `ArtifactMeta` type and creates the `artifact` module skeleton (`mod.rs`). Tests verify save+get roundtrip, deterministic hashing, list returns saved artifact, and save idempotency (≥ 4 tests).

## Scope

### In Scope
- Create `crates/anvilml-server/src/artifact/mod.rs` — `pub mod store; pub use store::ArtifactStore;`
- Create `crates/anvilml-server/src/artifact/store.rs` — `ArtifactStore` struct with `save`, `get`, `list` methods
- Add `width: u32` and `height: u32` fields to `ArtifactMeta` in `anvilml-core/src/types/artifact.rs`
- Add `pub mod artifact;` to `crates/anvilml-server/src/lib.rs`
- Add `sha2 = "0.10"` dependency to `crates/anvilml-server/Cargo.toml`
- Add `serial_test = "3.5"` dev-dependency to `crates/anvilml-server/Cargo.toml`
- Bump `anvilml-server` patch version `0.1.22 → 0.1.23`
- Integration tests in `crates/anvilml-server/tests/artifact_store_tests.rs`

### Out of Scope
- Wiring `ArtifactStore` into `AppState` (handled by P15-A2)
- Persisting `ImageReady` events in the scheduler (handled by P15-A2)
- HTTP handlers for artifact retrieval (handled by P15-A3)
- Migration changes — the `artifacts` table already exists in `001_initial.sql`

## Existing Codebase Assessment

The `artifacts` SQLite table already exists in migration `001_initial.sql` with columns `id INTEGER PRIMARY KEY AUTOINCREMENT`, `job_id TEXT NOT NULL`, `hash TEXT NOT NULL UNIQUE`, `path TEXT NOT NULL`, `size_bytes INTEGER NOT NULL`, `created_at TEXT NOT NULL`, and an index `idx_artifacts_job_id`. No new migration is needed.

The `ArtifactMeta` struct in `anvilml-core/src/types/artifact.rs` currently has fields `id`, `job_id`, `hash`, `path`, `size_bytes`, `created_at` — but is missing the `width` and `height` fields specified in `ANVILML_DESIGN.md §5.5`. These must be added to `ArtifactMeta` before `ArtifactStore::save` can populate them.

The `anvilml-registry` crate already uses `sha2 = "0.10"` for content-addressed model IDs, establishing the exact same hashing pattern (SHA-256 → lowercase hex digest). The `ModelStore` in `anvilml-registry/src/store.rs` provides the established pattern for SQLite CRUD: raw `sqlx::query()` with `?` bind parameters, manual row-to-struct mapping via `row.get()`, and `Result<T, AnvilError>` return types.

The `anvilml-server` crate has no `artifact/` directory yet — only `handlers/`, `ws/`, `lib.rs`, `state.rs`, and `error.rs` exist. The test pattern uses `axum::Router` + `tower::util::ServiceExt::oneshot()` for HTTP tests, but this task writes unit tests that test `ArtifactStore` directly (no HTTP layer), following the simpler pattern used in `anvilml-registry/tests/store_tests.rs`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sha2    | 0.10.9          | Cargo.lock     | n/a                    |

`sha2 = "0.10"` is already declared in `anvilml-registry/Cargo.toml`. The Cargo.lock confirms version 0.10.9. The API shape used is `sha2::{Sha256, Digest}` — `Sha256::new()`, `.update(&data)`, `.finalize()` returning a `GenericArray<u8, OutputSize<Sha256>>`, formatted via `format!("{:x}", result)`. This is the stable API for sha2 0.10.x.

## Approach

1. **Add `width` and `height` fields to `ArtifactMeta`** in `crates/anvilml-core/src/types/artifact.rs`. Add `pub width: u32` and `pub height: u32` fields after `hash`. Update the struct's `Default` derive — since `u32` defaults to `0`, no explicit `Default` impl is needed. The `#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]` attributes remain unchanged. This is a backward-compatible addition because serde deserialisation ignores unknown fields and the new fields have valid defaults.

2. **Create `crates/anvilml-server/src/artifact/mod.rs`**. A two-line module file: `pub mod store;` and `pub use store::ArtifactStore;`. Follows the established pattern from `ws/mod.rs` and `handlers/mod.rs`.

3. **Create `crates/anvilml-server/src/artifact/store.rs`**. The main implementation file. It contains:
   - `ArtifactStore { dir: PathBuf, db: SqlitePool }` struct — owns the artifact directory path and the shared SQLite connection pool.
   - `pub async fn new(dir: PathBuf, db: SqlitePool) -> Self` — creates the store. Calls `std::fs::create_dir_all(&dir)` to ensure the artifact directory exists (idempotent). No tracing instrumentation needed — this is a simple constructor.
   - `pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta>` — the core persistence method:
     - Compute SHA-256 hex digest of `image_bytes` using `sha2::Sha256` and `sha2::Digest`.
     - Build file path as `{dir}/{hash}.png` using `PathBuf::join`.
     - If the file already exists on disk (`std::fs::metadata(path).is_ok()`), skip the write (idempotent) — this avoids unnecessary disk I/O when the same image is produced by multiple jobs.
     - Write `image_bytes` to the file using `tokio::fs::write`.
     - Insert into the `artifacts` table using `INSERT OR IGNORE INTO artifacts (job_id, hash, path, size_bytes, created_at, width, height) VALUES (?, ?, ?, ?, ?, ?, ?)` — the `OR IGNORE` clause ensures idempotency at the database level: if a row with the same `hash` UNIQUE constraint already exists, the INSERT is silently skipped.
     - Query back the inserted row to return `ArtifactMeta` (the `INSERT OR IGNORE` means we must read after write to get the correct metadata).
     - Log at DEBUG: `artifact saved` with `hash`, `job_id`, `size_bytes` fields.
   - `pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>>` — returns the file path if it exists in the database:
     - Query `SELECT path FROM artifacts WHERE hash = ?` with `fetch_optional`.
     - If found, return `Some(PathBuf)`; if not found, return `None`.
   - `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>>` — returns artifact metadata optionally filtered by job:
     - Build a query with optional `WHERE job_id = ?` clause.
     - Fetch all matching rows and map to `Vec<ArtifactMeta>`.
     - Return empty vec if no matches (not an error).
   - Use `anvilml_core::AnvilError` for all error returns. `sqlx::Error` is converted via `#[from]` on the `Db` variant.
   - Apply `#[tracing::instrument(skip(self))]` to `save` and `list` (meaningful async operations); `get` is a simple query and does not need instrumentation.

4. **Add `pub mod artifact;` to `crates/anvilml-server/src/lib.rs`**. Insert after the existing `pub mod ws;` line. Follows the established `lib.rs` discipline (only `pub mod` declarations and re-exports).

5. **Add `sha2 = "0.10"` to `crates/anvilml-server/Cargo.toml` dependencies section**. Add alongside existing workspace dependencies. This is a new direct dependency — `anvilml-server` needs sha2 directly since it performs its own hashing (independent of `anvilml-registry`'s model hashing).

6. **Add `serial_test = "3.5"` to `crates/anvilml-server/Cargo.toml` dev-dependencies**. Used for test isolation on tests that share filesystem state (the artifact directory).

7. **Bump `anvilml-server` patch version** in `crates/anvilml-server/Cargo.toml` from `0.1.22` to `0.1.23`. Per FORGE_AGENT_RULES §14 and ENVIRONMENT.md §12.

8. **Create `crates/anvilml-server/tests/artifact_store_tests.rs`** with ≥ 4 tests:
   - `test_save_and_get_roundtrip`: Creates an `ArtifactStore` with an in-memory pool and a temp directory, calls `save` with known bytes, then calls `get` and verifies the returned path exists on disk and the file content matches.
   - `test_hash_is_deterministic`: Calls `save` twice with identical bytes and verifies the hash is the same both times (deterministic SHA-256).
   - `test_list_returns_saved_artifact`: Calls `save` with a known `job_id`, then calls `list(None)` and verifies the returned `Vec<ArtifactMeta>` contains exactly one entry with the correct `job_id` and `hash`.
   - `test_save_is_idempotent`: Calls `save` twice with identical bytes and verifies the database has exactly one row (the `INSERT OR IGNORE` prevents duplicates).
   - `test_get_returns_none_for_unknown_hash`: Calls `get` with a hash that was never saved and verifies it returns `None`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| Struct | `anvilml-server::artifact::ArtifactStore` | `pub struct ArtifactStore { dir: PathBuf, db: SqlitePool }` |
| Constructor | `anvilml-server::artifact::ArtifactStore::new` | `pub async fn new(dir: PathBuf, db: SqlitePool) -> Self` |
| Save | `anvilml-server::artifact::ArtifactStore::save` | `pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta, AnvilError>` |
| Get | `anvilml-server::artifact::ArtifactStore::get` | `pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>, AnvilError>` |
| List | `anvilml-server::artifact::ArtifactStore::list` | `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError>` |
| New fields | `anvilml-core::types::ArtifactMeta` | Added `width: u32` and `height: u32` fields |

Note: `ArtifactStore` is re-exported via `crates/anvilml-server/src/artifact/mod.rs` as `pub use store::ArtifactStore`. The `anvilml-server` crate's `lib.rs` does not re-export it at the top level (that will be done in P15-A2 when it is added to `AppState`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/artifact/mod.rs` | Module declaration + re-export |
| CREATE | `crates/anvilml-server/src/artifact/store.rs` | `ArtifactStore` struct and all methods |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Add `pub mod artifact;` |
| MODIFY | `crates/anvilml-core/src/types/artifact.rs` | Add `width` and `height` fields to `ArtifactMeta` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Add `sha2` dep, `serial_test` dev-dep, bump version |
| CREATE | `crates/anvilml-server/tests/artifact_store_tests.rs` | Integration tests for `ArtifactStore` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/artifact_store_tests.rs` | `test_save_and_get_roundtrip` | `save` writes file to disk and records row in DB; `get` returns correct path | In-memory SQLite pool, temp artifact dir | Known byte slice `[0x89, 0x50, ...]` (64-byte PNG-like data), job_id `uuid::Uuid::new_v4()` | `get(hash)` returns `Some(path)`; file at path contains exact input bytes | `cargo test -p anvilml-server --features mock-hardware -- test_save_and_get_roundtrip` exits 0 |
| `crates/anvilml-server/tests/artifact_store_tests.rs` | `test_hash_is_deterministic` | SHA-256 hash is deterministic for identical input bytes | In-memory SQLite pool, temp artifact dir | Same byte slice passed to two `save` calls | Both `ArtifactMeta` results have identical `hash` values | `cargo test -p anvilml-server --features mock-hardware -- test_hash_is_deterministic` exits 0 |
| `crates/anvilml-server/tests/artifact_store_tests.rs` | `test_list_returns_saved_artifact` | `list(None)` returns all saved artifacts with correct metadata | In-memory SQLite pool, temp artifact dir | `save(job_id, bytes)` then `list(None)` | `Vec<ArtifactMeta>` contains exactly 1 entry matching saved `job_id` and `hash` | `cargo test -p anvilml-server --features mock-hardware -- test_list_returns_saved_artifact` exits 0 |
| `crates/anvilml-server/tests/artifact_store_tests.rs` | `test_save_is_idempotent` | Second `save` with same bytes does not create duplicate DB rows | In-memory SQLite pool, temp artifact dir | Same byte slice passed to two `save` calls with same `job_id` | DB has exactly 1 row; second `save` returns the same `ArtifactMeta` | `cargo test -p anvilml-server --features mock-hardware -- test_save_is_idempotent` exits 0 |
| `crates/anvilml-server/tests/artifact_store_tests.rs` | `test_get_returns_none_for_unknown_hash` | `get` returns `None` for a hash that was never saved | In-memory SQLite pool, temp artifact dir | `get("nonexistent_hash")` with no prior saves | `Option<PathBuf>` is `None` | `cargo test -p anvilml-server --features mock-hardware -- test_get_returns_none_for_unknown_hash` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-server/tests/` which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new file types, no new gates, no new CI job configuration needed. The `config-drift` and `openapi-drift` CI jobs are unaffected because this task does not modify `ServerConfig` or handler signatures.

## Platform Considerations

None identified. The `tokio::fs::write` and `std::fs::metadata` operations are cross-platform. The `{dir}/{hash}.png` path construction uses `PathBuf::join` which handles platform-specific separators automatically. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ArtifactMeta` currently lacks `width` and `height` fields — adding them changes the struct definition. Existing code that constructs `ArtifactMeta` (if any) may fail to compile. | Low | Medium | Check all `ArtifactMeta` construction sites before writing. Since `Default` derives and the fields are `u32` (defaulting to 0), any existing `..Default::default()` patterns will silently pick up the new fields. Verify with `cargo check` after the change. |
| `INSERT OR IGNORE` on `artifacts.hash UNIQUE` — if the file write succeeds but the DB insert fails (e.g. disk full), the file exists on disk but is not in the DB. On retry, `get` returns `None` but the file exists. | Low | Medium | This is an acceptable trade-off for idempotency. The `INSERT OR IGNORE` prevents duplicate DB rows. If the DB insert fails, the caller gets an `Err` and can retry. The file on disk is harmless (it's the correct content-addressed path). |
| Test uses `tempfile::tempdir()` which requires the `tempfile` crate as a dev-dependency. `tempfile` is already in the workspace dependencies but not in `anvilml-server`'s dev-dependencies. | Low | Low | Add `tempfile = { workspace = true }` to `anvilml-server`'s `[dev-dependencies]` section. This is already available in the workspace. |
| `sha2` 0.10 API — `Digest::finalize()` returns a `GenericArray<u8>`, not a hex string. The plan uses `format!("{:x}", result)` which works for `GenericArray<u8>` via its `Display` impl. | Low | Low | Verified: `sha2` 0.10.x provides `Display` formatting on the finalize result via the `digest` crate's `GenericArray`. No additional crate needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware -- artifact_store_tests` exits 0 with ≥ 5 tests
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0 (all existing tests still pass)
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (no compilation errors from new dependency)
- [ ] `grep -q 'pub mod artifact' crates/anvilml-server/src/lib.rs` — the artifact module is declared
- [ ] `grep -q 'pub use store::ArtifactStore' crates/anvilml-server/src/artifact/mod.rs` — the re-export exists
- [ ] `grep -q 'width: u32' crates/anvilml-core/src/types/artifact.rs` — width field added to ArtifactMeta
- [ ] `grep -q 'height: u32' crates/anvilml-core/src/types/artifact.rs` — height field added to ArtifactMeta
- [ ] `grep -q 'sha2 = "0.10"' crates/anvilml-server/Cargo.toml` — sha2 dependency added
- [ ] `grep -q 'serial_test' crates/anvilml-server/Cargo.toml` — serial_test dev-dependency added
- [ ] `grep 'version = "0.1.23"' crates/anvilml-server/Cargo.toml` — version bumped from 0.1.22
