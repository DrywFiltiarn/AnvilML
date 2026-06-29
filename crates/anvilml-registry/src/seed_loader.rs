//! SeedLoader — SHA256-gated seed idempotency checking via the `_seed_log` bookkeeping table.
//!
//! Provides `SeedLoader`, which holds a `SqlitePool` and exposes `already_applied()`:
//! a method that compares a seed file's SHA256 hash against a stored value in the
//! `_seed_log` table. If the hash matches, the seed has already been applied and can
//! be skipped. If the hash differs or the seed is unseen, it must be applied.
//!
//! The `_seed_log` table is created lazily on the first call to `already_applied()`
//! via `CREATE TABLE IF NOT EXISTS` — this is idempotent DDL and a no-op if the
//! table already exists. The table is a bookkeeping concern of this module, not part
//! of the initial migration (which is handled by `001_initial.sql`).
//!
//! This module is the hash-comparison foundation that `run()` (P6-A7) will call to
//! decide whether to skip or re-apply a seed file at server startup.

use std::path::Path;

use anvilml_core::AnvilError;
use digest::Digest;
use sha2::Sha256;
use sqlx::SqlitePool;

/// SQLite-backed seed idempotency checker.
///
/// Wraps a `SqlitePool` and provides `already_applied()`, which compares a seed file's
/// SHA256 hash against the stored value in the `_seed_log` bookkeeping table.
///
/// The `_seed_log` table is created lazily on first use via `CREATE TABLE IF NOT EXISTS`,
/// so callers do not need to ensure the table exists beforehand.
///
/// # Usage
///
/// ```no_run
/// # use anvilml_registry::seed_loader::SeedLoader;
/// # use anvilml_registry::create_pool;
/// # use std::path::Path;
/// # async fn example() -> Result<(), anvilml_core::AnvilError> {
/// let pool = create_pool(Path::new("./anvilml.db")).await?;
/// let loader = SeedLoader::new(pool);
/// let applied = loader.already_applied("devices.sql", "abc123...").await?;
/// # Ok::<_, anvilml_core::AnvilError>(())
/// # }
/// ```
pub struct SeedLoader {
    /// Database connection pool. All methods acquire a connection from this pool.
    pool: SqlitePool,
}

impl SeedLoader {
    /// Construct a new `SeedLoader` backed by the given connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already had migrations applied.
    ///   The pool must be connected to a database; the `_seed_log` table will be
    ///   created lazily on the first `already_applied()` call.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check whether a seed file has already been applied with the given SHA256 hash.
    ///
    /// Compares *sha256* against the stored hash for *seed_name* in the `_seed_log` table.
    /// The `_seed_log` table is created lazily via `CREATE TABLE IF NOT EXISTS` if it does
    /// not yet exist (idempotent DDL).
    ///
    /// # Returns
    ///
    /// - `Ok(true)` — the seed has been applied and the stored hash matches *sha256*.
    ///   The seed can be safely skipped.
    /// - `Ok(false)` — the seed has never been applied (no row in `_seed_log`), or the
    ///   stored hash differs from *sha256* (the seed file has changed since last run).
    ///   The seed should be re-applied.
    /// - `Err(AnvilError::Db)` — a genuine database error occurred (connection failure,
    ///   malformed query, constraint violation).
    ///
    /// # Concurrency
    ///
    /// This method is called at server startup in a single-threaded context, before any
    /// async work begins. Concurrent calls are not expected, so the `CREATE TABLE IF
    /// NOT EXISTS` + `SELECT` sequence does not need a mutex guard.
    #[tracing::instrument(fields(seed_name = %seed_name), skip(self))]
    pub async fn already_applied(&self, seed_name: &str, sha256: &str) -> Result<bool, AnvilError> {
        // Create the bookkeeping table if it doesn't exist. This is idempotent DDL —
        // SQLite silently ignores the statement if the table already exists.
        // The table is local to this module's concern, not part of 001_initial.sql.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _seed_log \
             (seed_name TEXT PRIMARY KEY, sha256 TEXT NOT NULL, applied_at TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await?;

        // Use fetch_optional() which converts "no row found" to Ok(None) rather than
        // Err(RowNotFound). This is the correct pattern for optional-row queries in sqlx.
        let stored_hash: Option<String> =
            sqlx::query_scalar("SELECT sha256 FROM _seed_log WHERE seed_name = ?")
                .bind(seed_name)
                .fetch_optional(&self.pool)
                .await?;

        match stored_hash {
            Some(hash) if hash == sha256 => {
                // Hash matches — the seed was already applied with this exact content.
                tracing::debug!(seed_name = %seed_name, "seed already applied (hash match)");
                Ok(true)
            }
            Some(_) => {
                // Row exists but hash differs — the seed file has changed since last run.
                tracing::debug!(seed_name = %seed_name, "seed already applied (hash mismatch)");
                Ok(false)
            }
            None => {
                // No row found — this seed has never been applied.
                tracing::debug!(seed_name = %seed_name, "seed not yet applied");
                Ok(false)
            }
        }
    }

    /// Execute a seed SQL file against the database, recording the hash and timestamp.
    ///
    /// This method implements idempotent seed application:
    /// 1. Computes the SHA256 hash of the seed file content.
    /// 2. Checks whether a seed with the same name and hash was already applied via
    ///    `already_applied()`.
    /// 3. If already applied, returns `Ok(())` without re-executing (idempotent skip).
    /// 4. If not already applied, executes the seed SQL within a transaction and records
    ///    the hash+timestamp in `_seed_log`.
    ///
    /// The transaction wrapping ensures that if the SQL is malformed, the entire
    /// operation rolls back and no hash+timestamp is recorded. This prevents partial
    /// application — the seed will be re-attempted on the next `run()` call.
    /// Without a transaction, a malformed SQL statement could leave partial state
    /// in the database and a stale hash+timestamp in `_seed_log`.
    ///
    /// # Arguments
    ///
    /// * `seed_name` — A stable identifier for this seed (e.g. `"devices.sql"`).
    ///   Used as the primary key in `_seed_log`.
    /// * `seed_path` — Filesystem path to the SQL seed file. The full file contents
    ///   are read and hashed to detect content changes.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the seed file cannot be read, or
    /// `AnvilError::Db` if the SQL execution or hash recording fails.
    #[tracing::instrument(fields(seed_name = %seed_name, seed_path = %seed_path.display()), skip(self))]
    pub async fn run(&self, seed_name: &str, seed_path: &Path) -> Result<(), AnvilError> {
        // Step A: Read the seed file and compute SHA256 hash of its contents.
        // We hash the full file content so that any change to the SQL (even a
        // whitespace change) produces a different hash and triggers re-application.
        let contents = std::fs::read(seed_path)?;
        let hasher = Sha256::digest(&contents);
        // Convert the 32-byte digest to a lowercase hex string.
        let sha256_hex: String = hasher.iter().map(|b| format!("{:02x}", b)).collect();

        tracing::debug!(seed_name = %seed_name, sha256 = %sha256_hex, "computed seed file hash");

        // Step B: Check if this seed (with this exact content hash) was already applied.
        // The `already_applied()` call also lazily creates the `_seed_log` table
        // if it doesn't exist yet.
        let already = self.already_applied(seed_name, &sha256_hex).await?;

        // Step C: If already applied with this exact hash, skip — this is the
        // idempotent path. The seed was previously applied with identical content.
        if already {
            tracing::debug!(seed_name = %seed_name, "seed already applied, skipping");
            return Ok(());
        }

        // Step D: Not yet applied — execute the seed SQL within a transaction.
        // The transaction ensures atomicity: if any SQL statement fails, the entire
        // operation rolls back (automatic via `Drop` on `Transaction` when `commit()`
        // is not called), and no hash+timestamp is recorded in `_seed_log`.
        // This means the seed will be re-attempted on the next `run()` call.
        let mut tx = self.pool.begin().await?;

        // Execute the seed SQL. Split on `;` to handle multiple statements in the
        // batch, and execute each one individually. This avoids the `'static` lifetime
        // requirement of `raw_sql()` while still supporting multi-statement seed files.
        // Each statement is trimmed and wrapped in `AssertSqlSafe` after auditing that
        // seed files are read from trusted, checked-in paths (not user-supplied input).
        let sql = String::from_utf8_lossy(&contents);
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if trimmed.is_empty() {
                continue; // Skip empty statements from trailing semicolons
            }
            // Convert to owned String so we can wrap with AssertSqlSafe — the trait
            // is only implemented for owned types, not borrowed &str.
            let stmt = trimmed.to_string();
            sqlx::query(sqlx::AssertSqlSafe(stmt))
                .execute(&mut *tx)
                .await?;
        }

        // Record the hash and timestamp in `_seed_log`. Using `INSERT OR REPLACE`
        // ensures idempotency even if this code path is reached twice (which should
        // not happen in normal operation, but protects against race conditions).
        let applied_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO _seed_log (seed_name, sha256, applied_at) \
             VALUES (?, ?, ?)",
        )
        .bind(seed_name)
        .bind(&sha256_hex)
        .bind(applied_at)
        .execute(&mut *tx)
        .await?;

        // Commit the transaction. If this fails, the transaction will be rolled back
        // automatically via `Drop`.
        tx.commit().await?;

        tracing::info!(seed_name = %seed_name, "seed applied successfully");
        Ok(())
    }
}
