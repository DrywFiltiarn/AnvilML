//! Integration tests for `anvilml_registry::db::open`.

#[tokio::test]
async fn test_open_creates_tables() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = anvilml_registry::db::open(path).await.unwrap();

    // Verify all three tables exist in sqlite_master.
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type='table' AND name IN ('jobs','models','artifacts')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count, 3, "expected jobs, models, and artifacts tables");

    // Verify each table individually.
    for table in ["jobs", "models", "artifacts"] {
        let exists: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(exists, 1, "{table} table should exist");
    }
}
