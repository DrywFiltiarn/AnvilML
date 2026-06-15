//! SHA256-gated SQL seed file runner.
//!
//! Discovers `.sql` files in a configurable directory, computes SHA256 of each
//! file's content, compares against the `seed_history` table, and either skips
//! (up-to-date) or executes + records (changed or new).
//!
//! This module is the SHA256-gated seed loader described in `ANVILML_DESIGN.md`.
//! It is invoked at server startup by the database initialisation path.

use std::path::Path;

use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tracing::info;

use anvilml_core::AnvilError;

/// Run all pending SQL seed files from *seeds_path*.
///
/// Discovers `.sql` files in the given directory, computes a SHA256 hex digest
/// of each file's content, and compares it against the `seed_history` table:
/// - If no row exists for the file's canonical path, or the SHA256 differs:
///   executes the SQL and records the hash and timestamp.
/// - If a row exists and the SHA256 matches: skips the file (up-to-date).
///
/// # Arguments
///
/// * `pool` — A `SqlitePool` connected to the AnvilML database. The pool must
///   have already run migrations (including `001_initial.sql`) so that the
///   `seed_history` and target tables exist.
///
/// * `seeds_path` — Filesystem path to the directory containing `.sql` seed
///   files. The directory is read at call time; files added after a prior run
///   will be discovered and applied.
///
/// # Errors
///
/// Returns `AnvilError::Io` if the seeds directory cannot be read or a seed
/// file cannot be read. Returns `AnvilError::Db` if any SQL query or execution
/// fails.
#[tracing::instrument(skip(pool, seeds_path), fields(seeds_path = %seeds_path.display()))]
pub async fn run(pool: &SqlitePool, seeds_path: &Path) -> Result<(), AnvilError> {
    // Read the seeds directory entries. If the directory does not exist or is
    // unreadable, propagate the I/O error — the caller (server startup) should
    // treat this as a fatal configuration problem.
    let mut entries = std::fs::read_dir(seeds_path)
        .map_err(AnvilError::Io)?
        .filter_map(|e| e.ok()) // Skip entries that no longer exist (TOCTOU)
        .filter(|e| {
            // Only process `.sql` files. Other file types (e.g. `.bak`, `.tmp`)
            // are silently ignored to avoid accidental execution of non-SQL files.
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
        })
        .collect::<Vec<_>>();

    // Sort entries by file name so that seed execution order is deterministic
    // across runs. SQLite DDL seeds (like device_capabilities) should run
    // before DML seeds to avoid constraint violations.
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let file_path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();

        // Read the entire file content. Seed files are small (typically < 1 MB)
        // so reading into memory is appropriate and avoids streaming complexity.
        let content = std::fs::read(&file_path).map_err(AnvilError::Io)?;

        // Compute SHA256 hex digest of the file content. The `Sha256::digest`
        // method consumes the input and returns a `GenericArray<u8, U32>` which
        // we format as a lowercase hex string for storage and comparison.
        let sha256_hex = format!("{:x}", Sha256::digest(&content));

        // Query the seed_history table to check if this file has been applied
        // before and whether its hash matches. The file column stores the
        // canonical path so that file renames are detected as new seeds.
        let file_canonical = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone());

        let existing: Option<String> =
            sqlx::query_scalar("SELECT sha256 FROM seed_history WHERE file = ?")
                .bind(file_canonical.to_string_lossy().as_ref())
                .fetch_optional(pool)
                .await
                .map_err(AnvilError::Db)?;

        match existing {
            Some(ref stored_hash) if *stored_hash == sha256_hex => {
                // SHA256 matches — the seed file is unchanged since last run.
                // Skip execution to avoid redundant work and potential side effects.
                // This is a mandatory INFO log point per ENVIRONMENT.md §9.
                info!(file = %file_name, status = "up-to-date", "seed skipped");
            }
            _ => {
                // No prior record or hash mismatch — execute the SQL and record.
                // We use `INSERT OR REPLACE` so that if the row already exists with
                // a different hash, it is atomically replaced with the new one.
                info!(
                    file = %file_name,
                    sha256 = %sha256_hex,
                    "seed applied"
                );

                // Execute the seed SQL text. We use `sqlx::query` with the text
                // as a string because `sqlx::query_file!` is a compile-time
                // macro that requires the SQL path at build time — we need
                // runtime-discovered files. The seed SQL may contain multiple
                // statements (e.g. batch INSERTs) which SQLite handles natively.
                // `AssertSqlSafe` wraps the dynamic SQL string to assert it has
                // been manually audited for injection — seed files are trusted
                // SQL text from the project's backend/seeds/ directory.
                let sql_text = std::str::from_utf8(&content)
                    .map_err(|e| AnvilError::Internal(e.to_string()))?;
                sqlx::query(sqlx::AssertSqlSafe(sql_text))
                    .execute(pool)
                    .await
                    .map_err(AnvilError::Db)?;

                // Record this seed in the history table with the current timestamp.
                // `INSERT OR REPLACE` ensures idempotency: if the row already exists
                // (e.g. from a previous run with a different hash), it is replaced.
                let applied_at = Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT OR REPLACE INTO seed_history (file, sha256, applied_at) \
                     VALUES (?, ?, ?)",
                )
                .bind(file_canonical.to_string_lossy().as_ref())
                .bind(&sha256_hex)
                .bind(&applied_at)
                .execute(pool)
                .await
                .map_err(AnvilError::Db)?;
            }
        }
    }

    Ok(())
}
