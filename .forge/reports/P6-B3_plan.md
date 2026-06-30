# Plan Report: P6-B3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-B3                                       |
| Phase       | 006 — Model Registry & Artifacts            |
| Description | anvilml-artifacts: ArtifactStore::list by job_id |
| Depends on  | P6-B2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T01:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Add `list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError>` to `ArtifactStore` in `crates/anvilml-artifacts/src/store.rs`, querying the `artifacts` table created by P6-B2. When `job_id` is `None`, all rows are returned; when `Some(job_id)`, only rows matching that job are returned. This completes the `ArtifactStore` API surface (save → get → list) and receives the listing scope that P6-B2 explicitly deferred.

## Scope

### In Scope
- Add `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError>` to `ArtifactStore` in `crates/anvilml-artifacts/src/store.rs`.
- SQL query against the `artifacts` table: `SELECT hash, job_id, width, height, seed, steps, created_at, file_path FROM artifacts` with an optional `WHERE job_id = ?` clause.
- Map each `Row` to `ArtifactMeta` using `sqlx::FromRow`.
- Add 3 new tests in `crates/anvilml-artifacts/tests/store_tests.rs`:
  1. `test_list_with_job_id_filter` — saves artifacts under two different job IDs, calls `list(Some(job_id))`, asserts only matching rows are returned.
  2. `test_list_without_filter_returns_all` — saves artifacts under multiple job IDs, calls `list(None)`, asserts all rows are returned.
  3. `test_list_empty_table_returns_empty_vec` — creates an empty store, calls `list(None)`, asserts `Vec::len() == 0`.
- Bump `anvilml-artifacts` patch version from `0.1.2` to `0.1.3` in `Cargo.toml`.

### Out of Scope
None. The `defers_to` field is `[]` (empty) — no functionality may be deferred. This task implements its full scope, including any part the task context describes as needing confirmation or verification "at ACT time" — that phrase means resolve-then-implement, not skip-and-stub.

## Existing Codebase Assessment

**What already exists:** `ArtifactStore` (in `crates/anvilml-artifacts/src/store.rs`) already implements `save()` (content-addressed PNG write + metadata persistence) and `get()` (filesystem read by hash). The `artifacts` table schema is defined in `database/migrations/002_artifacts.sql` with columns `hash`, `job_id`, `width`, `height`, `seed`, `steps`, `created_at`, `file_path`, plus a `CREATE INDEX idx_artifacts_job_id ON artifacts(job_id)` — the index needed for efficient `list()` filtering already exists. `ArtifactMeta` is defined in `anvilml-core/src/types/artifact.rs` with all fields matching the table columns, and implements `sqlx::FromRow` (via the derive macros and serde compatibility that sqlx uses). The existing test file `store_tests.rs` has 6 tests using the `make_pool()` helper (unique in-memory SQLite pool per test) and `test_meta()` fixture.

**Established patterns:** All tests use per-test in-memory pools with unique cache names to avoid cross-test interference. The `sqlx::query!` or `sqlx::query()` macro is used with `.bind()` calls for parameterized queries. Error propagation uses `?` with `AnvilError` (via `sqlx::Error`'s `Into<AnvilError>` implementation). Async is driven by `#[tokio::test]`. The store uses `#[tracing::instrument]` on public methods. Doc comments follow the `///` format with `# Arguments`, `# Returns`, and `# Errors` sections.

**Gap between design doc and current source:** The design doc's `ensure_artifacts_table()` inline DDL in `save()` is a temporary safety net (documented in the file header as such). The migration `002_artifacts.sql` already exists and creates the same schema plus the `job_id` index. The `list()` method is the only missing piece — no gap, just the next method to implement.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sqlx    | 0.9.0           | Cargo.toml (workspace) | sqlite, runtime-tokio, migrate, chrono |
| crate  | uuid    | 1.23            | Cargo.toml (workspace) | v4 |
| crate  | chrono  | 0.4             | Cargo.toml (workspace) | serde |

No new dependencies are introduced. The `list()` method only uses existing dependencies already declared in `Cargo.toml`: `sqlx` for the SQL query, `uuid` for the `Option<Uuid>` parameter, and `chrono` is already used in `ArtifactMeta`.

## Approach

### Step 1: Implement `list()` in `store.rs`

Add the following method to `impl ArtifactStore` in `crates/anvilml-artifacts/src/store.rs`:

```rust
/// List artifact metadata, optionally filtered by job ID.
///
/// Queries the `artifacts` table and returns all rows, or only rows
/// matching the given `job_id` when `Some(job_id)` is provided.
/// Returns an empty vector when no rows match (not an error).
///
/// # Arguments
///
/// * `job_id` — Optional job UUID to filter by. `None` returns all rows.
///
/// # Returns
///
/// A `Vec<ArtifactMeta>` containing all matching artifact metadata rows,
/// or an empty vector if no rows match.
///
/// # Errors
///
/// Returns `AnvilError::Db` if the database query fails.
#[tracing::instrument(fields(artifact_dir = %self.artifact_dir.display()), skip(self))]
pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError> {
    // Build the SQL query: SELECT all artifact columns from the artifacts table.
    // When job_id is Some, add a WHERE clause to filter by that job.
    // The WHERE clause uses parameter binding (? placeholder) to prevent SQL injection.
    let rows = if let Some(jid) = job_id {
        // Filter by job_id — the WHERE clause uses a bound parameter (?)
        // so the UUID is safely serialised as a TEXT value.
        sqlx::query_as::<_, ArtifactMeta>(
            "SELECT hash, job_id, width, height, seed, steps, created_at, file_path \
             FROM artifacts WHERE job_id = ?",
        )
        .bind(jid.to_string())
        .fetch_all(&self.pool)
        .await?
    } else {
        // No filter — return all rows.
        sqlx::query_as::<_, ArtifactMeta>(
            "SELECT hash, job_id, width, height, seed, steps, created_at, file_path \
             FROM artifacts",
        )
        .fetch_all(&self.pool)
        .await?
    };

    tracing::debug!(count = rows.len(), job_id = ?job_id, "list completed");
    Ok(rows)
}
```

**Rationale for `query_as::<_, ArtifactMeta>`:** `ArtifactMeta` already has all fields matching the table columns in order. sqlx's `query_as` maps columns positionally to struct fields via the `sqlx::FromRow` derive. This avoids manual row-to-struct mapping code and is the standard pattern used by other store implementations in this project (e.g. `anvilml-registry`'s `ModelStore`).

**Rationale for `job_id.to_string()`:** The `job_id` column is `TEXT NOT NULL` (UUID as string). Binding `Uuid` directly would require a feature flag or conversion; `.to_string()` produces the canonical hyphenated UUID string format that matches how `save()` writes it (`meta.job_id.to_string()` in the existing code).

### Step 2: Add 3 new tests in `store_tests.rs`

Add three new test functions to `crates/anvilml-artifacts/tests/store_tests.rs`:

**Test 1: `test_list_with_job_id_filter`**
- Setup: Create tempdir + pool, save two artifacts under different job IDs.
- Action: Call `store.list(Some(job_id_a))`.
- Verify: Returns exactly 1 row with the matching job ID.

**Test 2: `test_list_without_filter_returns_all`**
- Setup: Create tempdir + pool, save three artifacts under two different job IDs.
- Action: Call `store.list(None)`.
- Verify: Returns exactly 3 rows.

**Test 3: `test_list_empty_table_returns_empty_vec`**
- Setup: Create tempdir + pool (no saves).
- Action: Call `store.list(None)`.
- Verify: Returns an empty vector (`Vec::len() == 0`).

### Step 3: Bump version in `Cargo.toml`

Increment `anvilml-artifacts` patch version from `0.1.2` to `0.1.3`.

### Step 4: Run acceptance criteria

Execute `cargo test -p anvilml-artifacts --test store_tests` and confirm it exits 0 with >=9 tests total (6 existing + 3 new).

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `pub async fn list` | `crates/anvilml-artifacts/src/store.rs` | `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError>` |

No new structs, enums, traits, or re-exports. This is a single new public method on the existing `ArtifactStore` type.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-artifacts/src/store.rs` | Add `list()` method (~40 lines) |
| MODIFY | `crates/anvilml-artifacts/tests/store_tests.rs` | Add 3 new test functions (~80 lines) |
| MODIFY | `crates/anvilml-artifacts/Cargo.toml` | Bump patch version 0.1.2 → 0.1.3 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_list_with_job_id_filter` | `list(Some(job_id))` returns only artifacts matching the given job ID | Two artifacts saved under different job IDs via `save()` | `job_id_a` (UUID of first artifact) | `Vec` with exactly 1 `ArtifactMeta`, matching `job_id_a` | `cargo test -p anvilml-artifacts --test store_tests test_list_with_job_id_filter` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_list_without_filter_returns_all` | `list(None)` returns all artifact rows regardless of job ID | Three artifacts saved under two different job IDs via `save()` | `None` | `Vec` with exactly 3 `ArtifactMeta` entries | `cargo test -p anvilml-artifacts --test store_tests test_list_without_filter_returns_all` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_list_empty_table_returns_empty_vec` | `list(None)` on an empty table returns an empty `Vec`, not `None` or error | No artifacts saved; empty `artifacts` table | `None` | Empty `Vec` (`len() == 0`) | `cargo test -p anvilml-artifacts --test store_tests test_list_empty_table_returns_empty_vec` exits 0 |

## CI Impact

No CI changes required. The new tests are in the existing `store_tests.rs` file, which is already picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `list()` method is a pure database query — it uses sqlx's parameter binding which is platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ArtifactMeta` does not derive `sqlx::FromRow` automatically — sqlx requires the struct fields to exactly match the column names and types in the SQL query. If the derive is missing or the column order doesn't match, `query_as` will fail at compile time. | Low | High (compile error) | Verify that `ArtifactMeta` has `#[derive(sqlx::FromRow)]` or that sqlx's compile-time checks accept it. If the derive is missing, add it — this is a trivial addition. The existing codebase inspection shows `ArtifactMeta` has all fields matching the table columns in order. |
| The `job_id` column stores UUIDs as TEXT strings. Binding `Uuid` via `.bind(jid.to_string())` must produce the same string format that `save()` wrote. A mismatch would cause `list(Some(job_id))` to return empty results for valid data. | Low | Medium | Use `.to_string()` consistently — the same format `save()` uses (`meta.job_id.to_string()` in store.rs line 130). Write a roundtrip test (save → list(Some(job_id))) to verify. |
| The inline `ensure_artifacts_table()` DDL and the migration `002_artifacts.sql` both create the `artifacts` table. If the migration runs first, the inline DDL is a no-op. If the inline DDL runs first, it creates the table without the `job_id` index. The `list()` method needs the index for efficiency but works without it. | Low | Low | The `list()` query works regardless of whether the index exists. The index is an optimization, not a correctness requirement. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-artifacts --test store_tests` exits 0 with >=9 tests total
- [ ] `grep -c "async fn test_" crates/anvilml-artifacts/tests/store_tests.rs` outputs >=9
- [ ] `grep -c "pub async fn" crates/anvilml-artifacts/src/store.rs` outputs >=3 (save, get, list)
- [ ] `grep '^version' crates/anvilml-artifacts/Cargo.toml` contains `0.1.3`
