//! Shared model-directory scan trigger.
//!
//! Provides `trigger_model_scan()`, a fire-and-forget async function that spawns
//! a tokio task to scan all configured model directories and write discovered
//! models into the `ModelStore`. Used by both the server startup path in
//! `main.rs` and the `rescan_models()` HTTP handler.

use crate::scanner::ModelScanner;
use anvilml_core::ModelDirConfig;
use tracing::instrument;

/// Trigger a background scan of all configured model directories.
///
/// Spawns a `tokio::spawn` task that constructs a `ModelScanner` and calls
/// `scan_dir()` for each entry in `model_dirs`, using the configured scan depth.
/// Errors are logged at `WARN` level — the caller gets no error propagation,
/// matching the fire-and-forget contract used by the `/v1/models/rescan` handler.
///
/// This is the shared internal trigger reused by both the startup path in `main()`
/// and the `rescan_models()` HTTP handler.
///
/// # Arguments
///
/// * `pool` — A `SqlitePool` for the model store. Cloned into the spawned task.
/// * `model_dirs` — The list of model directories to scan (from `ServerConfig::model_dirs`).
/// * `model_scan_depth` — Default non-recursive scan depth (from `ServerConfig::model_scan_depth`).
#[instrument(skip(pool, model_dirs), fields(dir_count = model_dirs.len()))]
pub fn trigger_model_scan(
    pool: sqlx::SqlitePool,
    model_dirs: Vec<ModelDirConfig>,
    model_scan_depth: u32,
) {
    tracing::info!(
        dir_count = model_dirs.len(),
        "starting model directory scan"
    );

    tokio::spawn(async move {
        let scanner = ModelScanner::new(pool);

        for entry in &model_dirs {
            let depth = if entry.recursive {
                // When recursive is true, use the entry's own max_depth if set,
                // otherwise fall back to the config default model_scan_depth.
                entry.max_depth.unwrap_or(model_scan_depth)
            } else {
                // Non-recursive scan: depth 0 means scan only files directly
                // in the entry's path, no subdirectory traversal.
                0
            };

            tracing::debug!(
                path = %entry.path.display(),
                depth,
                recursive = entry.recursive,
                "scanning model directory"
            );

            match scanner.scan_dir(&entry.path, depth).await {
                Ok(models) => {
                    tracing::debug!(
                        path = %entry.path.display(),
                        count = models.len(),
                        "scan complete for {}: {} models scanned",
                        entry.path.display(),
                        models.len()
                    );
                }
                Err(e) => {
                    // Scan failure is non-fatal — log at WARN and continue
                    // with remaining directories.
                    tracing::warn!(
                        path = %entry.path.display(),
                        error = %e,
                        "scan failed for {}: {e}",
                        entry.path.display()
                    );
                }
            }
        }
    });
}
