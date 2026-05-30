//! Model registry for AnvilML.

pub mod db;

#[cfg(test)]
mod tests {
    use super::db;
    use anvilml_core::AnvilError;
    use tempfile::NamedTempFile;

    /// Opens a temporary database, runs migrations, and asserts all three
    /// tables exist in `sqlite_master`.
    #[tokio::test]
    async fn test_migrations_create_tables() -> Result<(), AnvilError> {
        // Create a temporary file for the SQLite database.
        let tmp = NamedTempFile::new().expect("create temp file");
        let path = tmp.path();

        // Open and migrate.
        let pool = db::open(path).await?;

        // Verify all three tables exist via sqlite_master.
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('jobs','models','artifacts') ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| AnvilError::DbError(format!("sqlite_master query failed: {e}")))?;

        let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        assert_eq!(names, vec!["artifacts", "jobs", "models"]);

        Ok(())
    }
}
