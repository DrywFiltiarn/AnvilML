//! Seed loader — bootstraps a tracking table and applies `.sql` seed files.
//!
//! On every run it:
//! 1. Creates the `seed_history` tracking table if absent.
//! 2. Enumerates `.sql` files in the seeds directory (sorted by filename).
//! 3. Parses header directives (`-- anvil:seed_table <name>` and
//!    `-- anvil:seed_strategy <replace_all|merge>`) from the top of each file.
//! 4. Computes a SHA256 digest of the file contents.
//! 5. Compares against the stored hash in `seed_history`; if unchanged, skips
//!    execution. Otherwise runs the execution stub and upserts the tracking row.

use std::path::{Path, PathBuf};

use anvilml_core::error::AnvilError;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// Convert a `sqlx::Error` into an `AnvilError::DbError`.
fn sqlx_error(err: sqlx::Error) -> AnvilError {
    AnvilError::DbError(err.to_string())
}

/// Parse the header of a seed SQL file to extract `seed_table` and
/// `seed_strategy` directives.
///
/// Reads the first non-empty lines beginning with `-- anvil:` and looks for:
/// - `-- anvil:seed_table <name>` — required
/// - `-- anvil:seed_strategy <replace_all|merge>` — optional, defaults to `replace_all`
///
/// Returns `(table_name, strategy)` or an error if `seed_table` is missing.
fn parse_header(bytes: &[u8]) -> Result<(String, String), AnvilError> {
    let mut table: Option<String> = None;
    let mut strategy = "replace_all".to_string();

    for line in bytes.split(|&b| b == b'\n') {
        let trimmed = std::str::from_utf8(line).map_err(|e| {
            AnvilError::SeedMissingDirective(format!("invalid UTF-8 in header: {e}"))
        })?;

        let trimmed = trimmed.trim();

        // Only consider lines starting with `-- anvil:`
        if !trimmed.starts_with("-- anvil:") {
            continue;
        }

        // Strip the `-- anvil:` prefix and trim
        let directive = &trimmed["-- anvil:".len()..].trim();

        if let Some(table_val) = directive.strip_prefix("seed_table ") {
            table = Some(table_val.trim().to_string());
        } else if let Some(strategy_val) = directive.strip_prefix("seed_strategy ") {
            strategy = strategy_val.trim().to_string();
        }
    }

    let seed_table = table.ok_or_else(|| {
        AnvilError::SeedMissingDirective(
            "file is missing the required `-- anvil:seed_table` directive".into(),
        )
    })?;

    Ok((seed_table, strategy))
}

/// Compute the SHA256 hex digest of the given bytes.
fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Extract the SQL body from raw file bytes.
///
/// The body is everything after the last `-- anvil:` header line,
/// with leading and trailing whitespace stripped.
fn extract_body(bytes: &[u8]) -> String {
    let mut header_end = None;
    for (i, line) in bytes.split(|&b| b == b'\n').enumerate() {
        let trimmed = std::str::from_utf8(line).unwrap_or("").trim();
        if trimmed.starts_with("-- anvil:") {
            header_end = Some(i);
        }
    }

    let start = header_end.map(|i| i + 1).unwrap_or(0);
    bytes
        .split(|&b| b == b'\n')
        .skip(start)
        .map(|line| std::str::from_utf8(line).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Execute a single seed file against the database.
///
/// Parses the SQL body, splits on semicolons, and executes each statement
/// within a single transaction. On any error the transaction is rolled back.
///
/// - `replace_all`: DELETE FROM <table> first, then INSERT statements from body.
/// - `merge`: execute each INSERT statement from body directly (INSERT OR REPLACE).
async fn execute_seed(
    pool: &SqlitePool,
    table: &str,
    body: &[u8],
    strategy: &str,
) -> Result<(), AnvilError> {
    let body_str = extract_body(body);
    let statements: Vec<String> = body_str
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if statements.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await.map_err(sqlx_error)?;

    if strategy == "replace_all" {
        let delete_sql = format!("DELETE FROM {table}");
        sqlx::query(sqlx::AssertSqlSafe(delete_sql))
            .execute(&mut *tx)
            .await
            .map_err(sqlx_error)?;
    }

    for stmt in statements {
        sqlx::query(sqlx::AssertSqlSafe(stmt))
            .execute(&mut *tx)
            .await
            .map_err(sqlx_error)?;
    }

    tx.commit().await.map_err(sqlx_error)?;

    Ok(())
}

/// Run the seed loader: bootstrap tracking table and apply seed files.
///
/// 1. Creates `seed_history` table if it doesn't exist.
/// 2. Reads `.sql` files from `seeds_dir`, sorted by filename.
/// 3. For each file: parse header, compute SHA256, compare with stored hash.
///    If hash matches, skip. Otherwise execute (stub) and upsert tracking row.
pub async fn run(pool: &SqlitePool, seeds_dir: &Path) -> Result<(), AnvilError> {
    // 1. Bootstrap the seed_history table.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS seed_history (\
         filename TEXT PRIMARY KEY, \
         sha256 TEXT NOT NULL, \
         applied_at INTEGER NOT NULL)",
    )
    .execute(pool)
    .await
    .map_err(sqlx_error)?;

    // 2. Read and sort seed files.
    let entries = std::fs::read_dir(seeds_dir).map_err(AnvilError::from)?;
    let mut sql_files: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(AnvilError::from)?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "sql") {
            sql_files.push(path);
        }
    }
    sql_files.sort_by_key(|p| p.file_name().unwrap_or_default().to_owned());

    // 3. Process each seed file.
    for file_path in &sql_files {
        let filename = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| {
                AnvilError::SeedMissingDirective(format!(
                    "could not extract filename from {:?}",
                    file_path
                ))
            })?;

        let bytes = std::fs::read(file_path).map_err(AnvilError::from)?;

        // Parse header (fails fast on missing directive).
        let (table, strategy) = parse_header(&bytes)?;

        // Compute SHA256 of the full file content.
        let hash = compute_sha256(&bytes);

        // Check if this file has already been applied with the same content.
        let existing: Option<String> =
            sqlx::query_scalar("SELECT sha256 FROM seed_history WHERE filename = ?")
                .bind(&filename)
                .fetch_optional(pool)
                .await
                .map_err(sqlx_error)?;

        if let Some(stored_hash) = existing {
            if stored_hash == hash {
                // Hash matches — skip execution.
                tracing::info!(file = %filename, status = "up-to-date", "seed skipped");
                continue;
            }
        }

        // Execute the seed.
        execute_seed(pool, &table, &bytes, &strategy).await?;

        tracing::info!(file = %filename, sha256 = %hash, "seed applied");

        // Upsert the tracking row.
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT OR REPLACE INTO seed_history (filename, sha256, applied_at) \
             VALUES (?, ?, ?)",
        )
        .bind(&filename)
        .bind(&hash)
        .bind(now)
        .execute(pool)
        .await
        .map_err(sqlx_error)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_header_both_directives() {
        let content = b"-- anvil:seed_table main_schema\n-- anvil:seed_strategy merge\n";
        let (table, strategy) = parse_header(content).unwrap();
        assert_eq!(table, "main_schema");
        assert_eq!(strategy, "merge");
    }

    #[test]
    fn test_parse_header_defaults_strategy() {
        let content = b"-- anvil:seed_table main_schema\n";
        let (table, strategy) = parse_header(content).unwrap();
        assert_eq!(table, "main_schema");
        assert_eq!(strategy, "replace_all");
    }

    #[test]
    fn test_parse_header_missing_table() {
        let content = b"-- anvil:seed_strategy merge\n";
        let result = parse_header(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            AnvilError::SeedMissingDirective(_) => {}
            other => panic!("expected SeedMissingDirective, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_header_empty_file() {
        let content = b"";
        let result = parse_header(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            AnvilError::SeedMissingDirective(_) => {}
            other => panic!("expected SeedMissingDirective, got {:?}", other),
        }
    }

    #[test]
    fn test_compute_sha256_known_value() {
        let bytes = b"hello";
        let hash = compute_sha256(bytes);
        // SHA256 of "hello" is known
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_compute_sha256_empty() {
        let bytes = b"";
        let hash = compute_sha256(bytes);
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
