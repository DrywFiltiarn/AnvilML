//! SQLite database connection, migration, and ghost-job reset.
//!
//! Provides `open()` for file-backed pools and `open_in_memory()` for test pools.
//! Both functions enable WAL mode, run the compile-time migration macro, and
//! reset any jobs left in `Queued` or `Running` state (ghost jobs from an
//! unclean server shutdown).

use std::path::Path;

use sqlx::{pool::PoolOptions, sqlite::SqliteConnectOptions, SqlitePool};
use tracing::info;

use anvilml_core::AnvilError;

/// Open a file-backed SQLite connection pool at *path*.
///
/// Creates the database file if it does not exist, enables WAL journal mode
/// for better concurrent read performance, runs all pending migrations, and
/// resets any jobs left in `Queued` or `Running` state (ghost jobs from an
/// unclean shutdown).
///
/// # Arguments
///
/// * `path` — Filesystem path to the SQLite database file. The file is created
///   if it does not exist.
///
/// # Returns
///
/// A `SqlitePool` ready for queries, or an `AnvilError::Db` if connection,
/// migration, or ghost-job reset fails.
///
/// # WAL mode
///
/// WAL (Write-Ahead Logging) mode is enabled because it provides better
/// concurrent read performance and prevents the "database is locked" errors
/// that plague rollback journal mode under concurrent access from multiple
/// tasks. This is important because the registry may handle concurrent job
/// submissions and model scans.
pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError> {
    // Check if the database file already exists before connecting — this
    // determines whether we log a "created" message at INFO level.
    let existed = path.exists();

    // Build connection options with WAL mode and auto-creation.
    // WAL mode provides better concurrent read performance and prevents
    // "database is locked" errors under concurrent access from multiple tasks.
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts).await?;

    // Log that a new database file was created.
    // This is a mandatory INFO log point per ENVIRONMENT.md §9.
    if !existed {
        info!(path = %path.display(), "database created");
    }

    // Run migrations — the compile-time macro embeds the migration directory
    // path relative to CARGO_MANIFEST_DIR. From crates/anvilml-registry/,
    // the migrations live at ../database/migrations/.
    run_migrations(&pool).await?;

    // Reset ghost jobs: jobs left in Queued or Running state from an
    // unclean server shutdown. Set them to Failed with a diagnostic error
    // message so the scheduler can re-queue or discard them.
    reset_ghost_jobs(&pool).await?;

    Ok(pool)
}

/// Open an in-memory SQLite connection pool.
///
/// Creates a transient in-memory database that is discarded when the pool
/// is dropped. Runs the same migrations and ghost-job reset as `open()`,
/// ensuring test behavior matches production behavior.
///
/// # Returns
///
/// A `SqlitePool` backed by an in-memory database, ready for queries, or
/// an `AnvilError::Db` if migration or ghost-job reset fails.
///
/// # Test usage
///
/// This function is intended for tests that need a clean database without
/// touching the filesystem. Each test gets its own pool — no shared
/// connections — ensuring test isolation.
pub async fn open_in_memory() -> Result<SqlitePool, AnvilError> {
    // Use max_connections(1) because SQLite's ":memory:" URL creates a
    // private database per connection. With more than one connection in
    // the pool, different pool members would see different databases.
    // A single-connection pool ensures all operations see the same data.
    let pool = PoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    // Run the same migrations as file-backed open.
    run_migrations(&pool).await?;

    // Run ghost-job reset (no-op on empty tables).
    reset_ghost_jobs(&pool).await?;

    Ok(pool)
}

/// Run all pending migrations from the compiled-in migration directory.
///
/// Logs each migration that is genuinely pending (not yet applied to this
/// database) at INFO level, and logs an "up-to-date" message when nothing
/// needs to run. This is a mandatory INFO log point per ENVIRONMENT.md §9.
///
/// The migration directory path is embedded at compile time by the
/// `sqlx::migrate!` macro.
async fn run_migrations(pool: &SqlitePool) -> Result<(), AnvilError> {
    // The migrate! macro embeds the migration directory path relative to
    // CARGO_MANIFEST_DIR at compile time. From crates/anvilml-registry/,
    // the migrations live at ../../database/migrations/.
    let runner = sqlx::migrate!("../../database/migrations");

    // Query the applied-migration tracking table to determine which
    // migrations have already run. sqlx creates _sqlx_migrations on the
    // first call to run(); on a brand-new database it does not exist yet,
    // so unwrap_or_default() treats that as an empty set rather than an
    // error.
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    // Filter the compile-time migration list down to those whose version
    // number is absent from the applied set. Casting m.version (u64) to
    // i64 is safe — sqlx stores versions as i64 in _sqlx_migrations and
    // migration version numbers are small sequential integers.
    let pending: Vec<_> = runner
        .migrations
        .iter()
        .filter(|m| !applied.contains(&(m.version as i64)))
        .collect();

    // Log each genuinely pending migration before running so the operator
    // can see exactly what is about to be applied.
    if pending.is_empty() {
        info!("migrations up-to-date");
    } else {
        for m in &pending {
            info!(migration = %m.description, version = m.version, "migration pending");
        }
    }

    // Run migrations — convert MigrateError to sqlx::Error via From impl,
    // then to AnvilError::Db via #[from]. Migration failures are fatal
    // database errors that should surface as 500 responses.
    runner
        .run(pool)
        .await
        .map_err(|e| AnvilError::Db(e.into()))?;

    Ok(())
}

/// Reset ghost jobs left in `Queued` or `Running` state from an unclean shutdown.
///
/// Sets their status to `Failed` with `error = 'server_restart'` so the
/// scheduler can re-queue or discard them. Logs the number of affected rows.
///
/// This is a mandatory INFO log point per ENVIRONMENT.md §9.
async fn reset_ghost_jobs(pool: &SqlitePool) -> Result<(), AnvilError> {
    // UPDATE jobs that are still Queued or Running — these are ghost jobs
    // from an unclean shutdown. Set them to Failed with a diagnostic error
    // message so the scheduler can take action.
    let rows = sqlx::query(
        "UPDATE jobs \
         SET status = 'Failed', error = 'server_restart' \
         WHERE status IN ('Queued', 'Running')",
    )
    .execute(pool)
    .await?
    .rows_affected();

    // Log the number of ghost jobs reset — mandatory INFO log point.
    info!(ghost_jobs_reset = rows, "ghost jobs reset");

    Ok(())
}
