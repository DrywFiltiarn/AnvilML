//! Artifact handlers for `GET /v1/artifacts` and `GET /v1/artifacts/:hash`.
//!
//! These handlers provide read access to the artifact store: listing artifact
//! metadata (optionally filtered by job ID) and serving raw artifact bytes.

use crate::state::AppState;
use anvilml_core::AnvilError;
use anvilml_core::ArtifactMeta;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::Response;
use axum::Json;
use serde::Deserialize;
use utoipa::ToSchema;

/// Query parameters for the `list_artifacts` endpoint.
///
/// All fields are optional — the handler passes them directly to the
/// `ArtifactStore::list()` method which handles the filtering.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListArtifactsQuery {
    /// Filter by job ID. If omitted, all artifacts are returned.
    pub job_id: Option<uuid::Uuid>,
}

/// List artifact metadata.
///
/// Returns all artifacts (optionally filtered by job ID) as a JSON array.
/// Returns an empty array when no artifacts match — this is not an error.
///
/// # Arguments
///
/// * `state` — Shared application state containing the artifact store.
/// * `params` — Optional filter: `job_id` (UUID string).
///
/// # Returns
///
/// * `200 OK` with a JSON array of `ArtifactMeta` objects.
/// * `500 Internal Server Error` if the database query fails.
#[tracing::instrument(skip(state), fields(job_id = ?params.job_id))]
pub async fn list_artifacts(
    State(state): State<AppState>,
    Query(params): Query<ListArtifactsQuery>,
) -> Result<Json<Vec<ArtifactMeta>>, AnvilError> {
    // Delegate to the artifact store which queries the database. The store
    // returns an empty vec (not an error) when no artifacts match, so this
    // handler simply passes through the result.
    let artifacts = state.artifact_store.list(params.job_id).await?;

    // Log at DEBUG level so operators can track artifact listing activity
    // without cluttering the default INFO log output.
    tracing::debug!(count = artifacts.len(), "artifact list returned");

    Ok(Json(artifacts))
}

/// Serve a raw artifact by its content hash.
///
/// Reads the artifact file from disk and returns it as a binary response
/// with `Content-Type: image/png`. The hash is the SHA-256 hex digest
/// stored in `ArtifactMeta.hash`.
///
/// # Arguments
///
/// * `state` — Shared application state containing the artifact store.
/// * `hash` — The SHA-256 hex digest of the artifact to serve.
///
/// # Returns
///
/// * `200 OK` with the raw artifact bytes and `Content-Type: image/png`.
/// * `404 Not Found` if no artifact with the given hash exists.
/// * `500 Internal Server Error` if reading the file fails.
#[tracing::instrument(skip(state), fields(hash = %hash))]
pub async fn serve_artifact(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Response<Body>, AnvilError> {
    // Look up the artifact path by hash. Returns None if not found —
    // this is a normal case, not a database error.
    let maybe_path = state.artifact_store.get(&hash).await?;

    // If the artifact doesn't exist in the store, return 404.
    // The hash is included in the error message for log correlation.
    let path = match maybe_path {
        Some(p) => p,
        None => return Err(AnvilError::ArtifactNotFound(hash)),
    };

    // Read the artifact file from disk. This is the only I/O operation
    // in this handler — the store lookup was a database query.
    let bytes = tokio::fs::read(&path).await?;

    // Build the response with the artifact bytes as the body and
    // Content-Type set to image/png. The hash is captured in the
    // tracing span so operators can correlate responses with logs.
    // We use Response::builder() to set the content-type header
    // before consuming the body.
    let response = Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(CONTENT_TYPE, "image/png")
        .body(Body::from(bytes))
        // The builder always succeeds here — we're not setting invalid
        // headers or an impossible status code.
        .expect("response builder should not fail with valid parameters");

    Ok(response)
}
