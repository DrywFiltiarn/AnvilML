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

use anvilml_core::AnvilError;
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
}
