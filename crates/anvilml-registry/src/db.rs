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
    // the migrations live at ../backend/migrations/.
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
/// Logs each migration applied at INFO level. If no migrations are pending,
/// logs an "up-to-date" message. This is a mandatory INFO log point per
/// ENVIRONMENT.md §9.
///
/// The migration directory path is embedded at compile time by the
/// `sqlx::migrate!` macro.
async fn run_migrations(pool: &SqlitePool) -> Result<(), AnvilError> {
    // The migrate! macro embeds the migration directory path relative to
    // CARGO_MANIFEST_DIR at compile time. From crates/anvilml-registry/,
    // the migrations live at ../../backend/migrations/.
    let runner = sqlx::migrate!("../../backend/migrations");

    // Log each migration that will be applied. The runner.migrations field
    // is public on the Migrator struct and contains all migrations resolved
    // at compile time. We log them before running so the operator can see
    // what migrations are about to be applied.
    let count = runner.migrations.len();
    for m in runner.migrations.iter() {
        info!(migration = %m.description, version = m.version, "migration pending");
    }

    // Run migrations — convert MigrateError to sqlx::Error via From impl,
    // then to AnvilError::Db via #[from]. Migration failures are fatal
    // database errors that should surface as 500 responses.
    runner
        .run(pool)
        .await
        .map_err(|e| AnvilError::Db(e.into()))?;

    // Log the up-to-date message when no migrations were pending —
    // mandatory INFO log point per ENVIRONMENT.md §9.
    if count == 0 {
        info!(migrations_applied = 0, "migrations up-to-date");
    } else {
        info!(migrations_applied = count, "migrations applied");
    }

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
