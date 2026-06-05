//! Integration tests for `seed_loader` module.

use std::io::Write;

use anvilml_registry::{open_in_memory, run};
use sqlx::SqlitePool;
use tempfile::TempDir;

/// Helper to create a temp seeds directory and write seed files into it.
fn setup_seeds_dir(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        drop(f);
    }
    dir
}

/// Calling `run()` twice with the same seeds dir must not fail — the
/// seed_history table is created idempotently.
#[tokio::test]
async fn test_table_bootstrap_idempotent() {
    let pool: SqlitePool = open_in_memory().await.unwrap();

    // Create the target table so INSERT has a destination.
    sqlx::query("CREATE TABLE main_schema (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table main_schema\nINSERT INTO main_schema VALUES (1);\n",
    )]);

    // First run — bootstraps table, processes file.
    run(&pool, dir.path()).await.unwrap();

    // Second run — should not fail even though table already exists.
    run(&pool, dir.path()).await.unwrap();
}

/// A seed file with both `-- anvil:seed_table` and `-- anvil:seed_strategy`
/// directives is accepted and a seed_history row is created.
#[tokio::test]
async fn test_directive_parsing_hit() {
    let pool: SqlitePool = open_in_memory().await.unwrap();

    // Create the target table so INSERT has a destination.
    sqlx::query("CREATE TABLE main_schema (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table main_schema\n-- anvil:seed_strategy merge\n\
         INSERT INTO main_schema VALUES (1);\n",
    )]);

    run(&pool, dir.path()).await.unwrap();

    // Verify seed_history row was created.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seed_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "expected exactly one seed_history row");

    // Verify the filename and hash are stored.
    let (filename, _hash): (String, String) =
        sqlx::query_as("SELECT filename, sha256 FROM seed_history LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(filename, "init.sql");
}

/// A seed file missing the `-- anvil:seed_table` directive must cause
/// `run()` to return `Err(AnvilError::SeedMissingDirective)`.
#[tokio::test]
async fn test_directive_parsing_miss() {
    let pool: SqlitePool = open_in_memory().await.unwrap();
    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_strategy merge\n\
         INSERT INTO foo VALUES (1);\n",
    )]);

    let result = run(&pool, dir.path()).await;
    assert!(
        result.is_err(),
        "expected an error for missing seed_table directive"
    );

    match result.unwrap_err() {
        anvilml_core::error::AnvilError::SeedMissingDirective(_) => {}
        other => panic!("expected SeedMissingDirective, got {:?}", other),
    }
}

/// When the file hash matches the stored value, execute_seed is never called —
/// the target table retains only the rows that were inserted during the first run.
#[tokio::test]
async fn sha256_skip_does_not_execute() {
    let pool: SqlitePool = open_in_memory().await.unwrap();

    // Create the target table so INSERT has a destination.
    sqlx::query("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table foo\nINSERT INTO foo VALUES (1);\n",
    )]);

    // First run — creates tracking row and inserts one row into foo.
    run(&pool, dir.path()).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "first run should insert exactly one row");

    // Second run — hash matches, execute_seed is skipped.
    run(&pool, dir.path()).await.unwrap();

    // Still exactly one row — seed was not re-executed.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "second run should not duplicate rows");
}

/// Pre-existing rows in the target table are deleted by `DELETE FROM`;
/// only rows from the seed file remain after execution.
#[tokio::test]
async fn replace_all_replaces_table_content() {
    let pool: SqlitePool = open_in_memory().await.unwrap();

    // Create the target table and insert a pre-existing row.
    sqlx::query("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO foo VALUES (99)")
        .execute(&pool)
        .await
        .unwrap();

    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table foo\n-- anvil:seed_strategy replace_all\n\
         INSERT INTO foo VALUES (1);\n",
    )]);

    run(&pool, dir.path()).await.unwrap();

    // Pre-existing row (99) should be gone; only seed row (1) remains.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "only seed rows should remain");

    let val: Option<i64> = sqlx::query_scalar("SELECT id FROM foo WHERE id = 99")
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(val.is_none(), "pre-existing row must be deleted");
}

/// Rows inserted before merge execution that are not in the seed file
/// remain untouched after the merge completes.
#[tokio::test]
async fn merge_preserves_unreferenced_rows() {
    let pool: SqlitePool = open_in_memory().await.unwrap();

    // Create the target table and insert a pre-existing row.
    sqlx::query("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO foo VALUES (99)")
        .execute(&pool)
        .await
        .unwrap();

    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table foo\n-- anvil:seed_strategy merge\n\
         INSERT OR REPLACE INTO foo VALUES (1);\n",
    )]);

    run(&pool, dir.path()).await.unwrap();

    // Both pre-existing row and seed row should exist.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "both pre-existing and seed rows should exist");

    let val: Option<i64> = sqlx::query_scalar("SELECT id FROM foo WHERE id = 99")
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert_eq!(val, Some(99), "pre-existing row must be preserved");
}

/// After modifying a seed file's content (changing its hash), the next run
/// re-executes and the target table reflects the new content.
#[tokio::test]
async fn changed_sha256_reruns_seed() {
    let pool: SqlitePool = open_in_memory().await.unwrap();

    // Create the target table.
    sqlx::query("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .unwrap();

    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table foo\nINSERT INTO foo VALUES (1);\n",
    )]);

    // First run — inserts one row.
    run(&pool, dir.path()).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Modify the seed file content.
    let path = dir.path().join("init.sql");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    writeln!(f, "-- anvil:seed_table foo").unwrap();
    writeln!(f, "INSERT INTO foo VALUES (2);").unwrap();

    // Second run — hash changed, seed re-executed.
    run(&pool, dir.path()).await.unwrap();

    // replace_all strategy: DELETE FROM foo first, then INSERT the new row.
    // So only the new row (id=2) should exist.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "replace_all should have only the new row");

    // Verify the original row was deleted and the new row exists.
    let has_one: bool = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM foo WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap()
        > 0;
    let has_two: bool = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM foo WHERE id = 2")
        .fetch_one(&pool)
        .await
        .unwrap()
        > 0;
    assert!(!has_one, "original row should be deleted by replace_all");
    assert!(has_two, "new row should have been inserted");
}
