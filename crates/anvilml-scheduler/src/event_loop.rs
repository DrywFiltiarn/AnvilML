/// Event loop module for processing `WorkerEvent` variants from workers.
///
/// This module owns the handler for `WorkerEvent::ImageReady` — the first real
/// consumer of worker events in the scheduler. It base64-decodes the image payload,
/// constructs an `ArtifactMeta`, and calls `artifact_store.save()` to persist the
/// decoded PNG bytes under their content hash.
///
/// The design separates the event variant matching from the handler function so that
/// callers (the interim job completion listener, or a future real event broadcaster)
/// can pattern-match on the event to extract the variant before calling the handler.
/// This avoids forcing the caller to destructure the event.
use std::path::PathBuf;
use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{AnvilError, ArtifactMeta};
use anvilml_ipc::WorkerEvent;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
use uuid::Uuid;

/// Handle a `WorkerEvent::ImageReady` event by decoding the base64 image payload,
/// constructing an `ArtifactMeta`, and persisting the decoded bytes to the artifact
/// store.
///
/// The caller is responsible for pattern-matching on the event to ensure it is
/// `ImageReady` before calling this function. The function accepts `WorkerEvent`
/// and `job_id` as separate parameters to avoid forcing the caller to destructure.
///
/// # Arguments
///
/// * `artifact_store` — The artifact storage backend where the decoded image will be saved.
/// * `event` — The worker event. The caller must ensure this is `ImageReady` before calling.
/// * `job_id` — The UUID of the job that produced this artifact.
///
/// # Returns
///
/// On success, returns the SHA-256 hex digest of the saved artifact.
///
/// # Errors
///
/// Returns `AnvilError::Serde` if the base64 decoding fails.
/// Returns `AnvilError::Io` if the filesystem write fails.
/// Returns `AnvilError::Db` if the metadata persistence fails.
#[tracing::instrument(fields(job_id), skip(artifact_store))]
pub async fn handle_image_ready(
    artifact_store: Arc<ArtifactStore>,
    event: WorkerEvent,
    job_id: Uuid,
) -> Result<String, AnvilError> {
    // Log at DEBUG that we are processing this event. The tracing::instrument
    // macro already creates a span with job_id, so this log entry will be
    // nested under that span.
    tracing::debug!(job_id = %job_id, "processing ImageReady event");

    // Extract the ImageReady fields from the event. The caller should have
    // already verified this is ImageReady, but we match here to extract the
    // fields and provide a clear error if the event is unexpected.
    let WorkerEvent::ImageReady {
        image_b64,
        width,
        height,
        format: _, // format is recorded in the event but not stored in ArtifactMeta
        seed,
        steps,
        job_id: _, // job_id is passed separately as a parameter — ignore the event's own copy
    } = event
    else {
        // This should never happen if the caller follows the contract of
        // matching on ImageReady before calling this function. However,
        // we return a clear error rather than panicking.
        return Err(AnvilError::Serde(
            "expected ImageReady event, got different variant".into(),
        ));
    };

    // Decode the base64-encoded image payload. The STANDARD engine uses
    // the standard base64 alphabet (A-Z, a-z, 0-9, +, /) with padding (=).
    // This matches the encoding used by the Python worker.
    let png_bytes = STANDARD
        .decode(&image_b64)
        .map_err(|err| AnvilError::Serde(format!("base64 decode failed: {err}")))?;

    // Construct the ArtifactMeta for this artifact. The `hash` and `file_path`
    // fields are ignored by `save()` — it computes both from the PNG bytes:
    // hash is SHA-256 of the bytes, file_path is {artifact_dir}/{hash}.png.
    // The remaining fields (job_id, width, height, seed, steps) are persisted
    // in the database and are used for querying artifacts by generation params.
    let meta = ArtifactMeta {
        hash: String::new(), // ignored by save() — computed from bytes
        job_id,
        width,
        height,
        seed,
        steps,
        created_at: Utc::now(),
        file_path: PathBuf::from(""), // ignored by save() — computed from dir + hash
    };

    // Save the decoded PNG bytes to the artifact store. This computes the
    // SHA-256 hash, writes the file, and persists the metadata row. Returns
    // the computed hash on success. The save() method is idempotent — calling
    // it twice with the same bytes returns the same hash without duplicating.
    let hash = artifact_store.save(&png_bytes, &meta).await?;

    // Log the successful artifact save with structured fields for log aggregation.
    tracing::info!(
        job_id = %job_id,
        hash = %hash,
        width = width,
        height = height,
        "artifact saved from ImageReady"
    );

    Ok(hash)
}
