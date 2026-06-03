# Plan Report: P5-A3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P5-A3                                         |
| Phase       | 005 — SQLite Persistence                      |
| Description | anvilml-registry: ghost-job reset query        |
| Depends on  | P5-A2                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-03T18:00:00Z                          |
| Attempt     | 1                                             |

## Objective

Add the `reset_ghost_jobs` function to `crates/anvilml-registry/src/db.rs` so that on server startup, any jobs left in `Running` or `Queued` state from a previous unclean exit are marked as `Failed` with `error='server_restart'`. The function returns the number of rows affected.

## Scope

### In Scope
- Add `pub async fn reset_ghost_jobs(pool: &SqlitePool) -> Result<u64, AnvilError>` to `db.rs`.
- The function executes `UPDATE jobs SET status = 'Failed', error = 'server_restart' WHERE status IN ('Running','Queued')` and returns the count of rows affected.
- Add an inline unit test (`mod tests {}`) in `db.rs` that:
  - Opens a temporary database (reusing P5-A2's `open`).
  - Inserts two jobs with status `'Running'` and one job with status `'Completed'`.
  - Calls `reset_ghost_jobs` and asserts exactly 2 rows were updated.
  - Verifies the `'Completed'` job is untouched (still `status = 'Completed'`, `error IS NULL`).

### Out of Scope
- Calling `reset_ghost_jobs` from `main.rs` — that is task P5-A4.
- Any changes to migration files, lib.rs re-exports, or other crates.
- Any HTTP API endpoint or WebSocket event wiring.
- Any config or environment variable changes.

## Approach

1. **Implement the function** in `crates/anvilml-registry/src/db.rs`, after the existing `open()` function and before the `#[cfg(test)]` block:

   ```rust
   /// Reset any jobs left in Running or Queued state from a previous unclean exit.
   ///
   /// Returns the number of rows updated (ghost jobs that were reset).
   pub async fn reset_ghost_jobs(pool: &SqlitePool) -> Result<u64, AnvilError> {
       let rows = sqlx::query(
           "UPDATE jobs SET status = 'Failed', error = 'server_restart' \
            WHERE status IN ('Running', 'Queued')",
       )
       .execute(pool)
       .await
       .map_err(sqlx_error)?;

       Ok(rows.rows_affected())
   }
   ```

   This uses `sqlx::query` (not `query_as`) because we only need the affected-row count, not row data. The return type `u64` matches the spec and is what `rows_affected()` returns on sqlx 0.9.

2. **Add the unit test** inside the existing `mod tests` block in `db.rs`:

   ```rust
   /// Ghost-job reset marks Running/Queued as Failed, leaves Completed untouched.
   #[tokio::test]
   async fn test_reset_ghost_jobs() {
       let tmp = tempfile::NamedTempFile::new().unwrap();
       let path = tmp.path();

       let pool = open(path).await.unwrap();

       // Insert 2 Running jobs and 1 Completed job.
       let running_id_1 = uuid::Uuid::new_v4();
       let running_id_2 = uuid::Uuid::new_v4();
       let completed_id = uuid::Uuid::new_v4();

       sqlx::query(
           "INSERT INTO jobs (id, status, graph, settings) VALUES (?, 'Running', '{}', '{}')",
       )
       .bind(running_id_1)
       .execute(&pool)
       .await
       .unwrap();

       sqlx::query(
           "INSERT INTO jobs (id, status, graph, settings) VALUES (?, 'Running', '{}', '{}')",
       )
       .bind(running_id_2)
       .execute(&pool)
       .await
       .unwrap();

       sqlx::query(
           "INSERT INTO jobs (id, status, graph, settings) VALUES (?, 'Completed', '{}', '{}')",
       )
       .bind(completed_id)
       .execute(&pool)
       .await
       .unwrap();

       // Call reset.
       let count = reset_ghost_jobs(&pool).await.unwrap();
       assert_eq!(count, 2, "exactly 2 ghost jobs should be reset");

       // Verify Running jobs are now Failed with error='server_restart'.
       for id in [running_id_1, running_id_2] {
           let (status, error): (String, Option<String>) = sqlx::query_as(
               "SELECT status, error FROM jobs WHERE id = ?",
           )
           .bind(id)
           .fetch_one(&pool)
           .await
           .unwrap();
           assert_eq!(status, "Failed", "ghost job should be Failed");
           assert_eq!(error.as_deref(), Some("server_restart"), "error must be server_restart");
       }

       // Verify Completed job is untouched.
       let (status, error): (String, Option<String>) = sqlx::query_as(
           "SELECT status, error FROM jobs WHERE id = ?",
       )
       .bind(completed_id)
       .fetch_one(&pool)
       .await
       .unwrap();
       assert_eq!(status, "Completed", "completed job must not be touched");
       assert!(error.is_none(), "completed job must have no error");
   }
   ```

3. **Verify the test compiles and passes**: `cargo test -p anvilml-registry -- ghost`.

4. **Run the full registry test suite**: `cargo test -p anvilml-registry` to confirm no regression against P5-A2's `test_open_creates_tables`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `crates/anvilml-registry/src/db.rs` | Add `reset_ghost_jobs()` function and unit test |

No new files. No changes to `lib.rs`, `Cargo.toml`, migrations, or other crates.

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/src/db.rs` (inline) | `test_reset_ghost_jobs` | Inserts 2 Running + 1 Completed jobs; calls `reset_ghost_jobs`; asserts 2 rows affected, Running→Failed+server_restart, Completed untouched |

## CI Impact

No CI changes required. The task adds only an inline unit test within the existing `anvilml-registry` crate. The existing CI matrix (`cargo test --workspace --features mock-hardware`) will pick it up automatically. No new dependencies are introduced (uses only existing `sqlx`, `tempfile`, and `tokio`).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| sqlx 0.9 API: `rows_affected()` may have a different method name or return type | Checked against the existing codebase — `sqlx::query().execute()` returns a `sqlx::sqlite::SqliteQueryResult` which exposes `rows_affected() -> u64`. This is consistent with sqlx 0.9 docs. |
| Test depends on P5-A2's `open()` function working correctly | `open()` is already tested by P5-A2's `test_open_creates_tables`; the same `tempfile` + `NamedTempFile` pattern is reused. If `open` were broken, the existing test would also fail. |
| `uuid::Uuid` not in scope for tests | The existing `anvilml-core` dependency re-exports `Uuid`, but the inline test can use `uuid::Uuid::new_v4()` directly since `uuid` is already a transitive dependency through `anvilml-core`. If compilation fails, add `uuid` to `dev-dependencies` in `Cargo.toml`. |
| SQLite WAL mode interacts with temp file path | The temp file lives in the OS temp directory; WAL sidecars will be created alongside it. This is the same pattern used by P5-A2 and is well-tested. No special handling needed. |

## Acceptance Criteria

- [ ] `reset_ghost_jobs` function exists in `crates/anvilml-registry/src/db.rs` with signature `pub async fn reset_ghost_jobs(pool: &SqlitePool) -> Result<u64, AnvilError>`
- [ ] Function executes `UPDATE jobs SET status = 'Failed', error = 'server_restart' WHERE status IN ('Running','Queued')`
- [ ] Returns the count of rows affected
- [ ] Inline unit test `test_reset_ghost_jobs` exists and passes: `cargo test -p anvilml-registry -- ghost` exits 0
- [ ] Full registry suite passes: `cargo test -p anvilml-registry` exits 0
- [ ] No regressions in P5-A2's `test_open_creates_tables`
