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
    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table main_schema\nINSERT INTO foo VALUES (1);\n",
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
    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table main_schema\n-- anvil:seed_strategy merge\n\
         INSERT INTO foo VALUES (1);\n",
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

/// First run with a seed file creates a tracking row. Second run with the
/// unchanged file skips execution (hash comparison path is exercised).
#[tokio::test]
async fn test_sha256_skip_unchanged() {
    let pool: SqlitePool = open_in_memory().await.unwrap();
    let dir = setup_seeds_dir(&[(
        "init.sql",
        "-- anvil:seed_table main_schema\nINSERT INTO foo VALUES (1);\n",
    )]);

    // First run — creates the tracking row.
    run(&pool, dir.path()).await.unwrap();

    // Verify row exists.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seed_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Second run — file is unchanged, hash should match, execution stub skipped.
    run(&pool, dir.path()).await.unwrap();

    // Still exactly one row (no duplicate).
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seed_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
