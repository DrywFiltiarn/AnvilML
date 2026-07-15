//! `GET /v1/artifacts` handler — list artifact metadata.
//!
//! Returns `200 OK` with a JSON array of `ArtifactMeta` objects,
//! optionally filtered by `job_id` query parameter.
//!
//! Per `ANVILML_DESIGN.md §13.4`: `GET /v1/artifacts?job_id=<uuid> → 200 [ArtifactMeta, ...]`.
//! The handler is a thin delegation layer — zero business logic, per `ANVILML_DESIGN.md §3.3`.

use anvilml_core::AnvilError;
use anvilml_core::ArtifactMeta;
use axum::Json;
use axum::body::Body;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use serde::Deserialize;
use uuid::Uuid;

use crate::state::AppState;

/// Query parameters for `GET /v1/artifacts`.
///
/// All fields are optional:
/// - `job_id` filters artifacts to those produced by a specific job.
#[derive(Debug, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
pub(crate) struct ListArtifactsParams {
    /// Optional job UUID to filter artifacts — only artifacts from this job are returned.
    /// `None` returns all artifacts regardless of job.
    #[serde(default)]
    pub job_id: Option<Uuid>,
}

/// List artifact metadata, optionally filtered by job ID.
///
/// Accepts an optional `job_id` query parameter. Delegates entirely to
/// `ArtifactStore::list()` which queries the database and returns all
/// matching artifact metadata rows.
#[utoipa::path(
    get,
    path = "/v1/artifacts",
    tag = "Artifacts",
    operation_id = "list_artifacts",
    summary = "List artifacts",
    description = "Lists artifact metadata, optionally filtered by job ID.",
    params(ListArtifactsParams),
    responses(
        (status = 200, description = "List of artifacts", body = Vec<ArtifactMeta>),
        (status = 500, description = "Database error")
    )
)]
#[tracing::instrument(skip(state, params), fields(job_id = ?params.job_id))]
pub(crate) async fn list_artifacts(
    State(state): State<AppState>,
    Query(params): Query<ListArtifactsParams>,
) -> Result<Json<Vec<ArtifactMeta>>, AnvilError> {
    // Delegate to the artifact store's list method. It ensures the
    // artifacts table exists and queries all matching rows. The
    // handler adds no business logic — per ANVILML_DESIGN.md §3.3.
    state.artifact_store.list(params.job_id).await.map(Json)
}

/// Serve raw PNG bytes for a content-addressed artifact by its SHA-256 hash.
///
/// Delegates to `ArtifactStore::get()` which reads the PNG file from the
/// content-addressed directory. Returns the raw bytes with `Content-Type:
/// image/png` on success.
#[utoipa::path(
    get,
    path = "/v1/artifacts/{hash}",
    tag = "Artifacts",
    operation_id = "get_artifact",
    summary = "Get artifact",
    description = "Serves raw PNG bytes for a content-addressed artifact by its SHA-256 hash.",
    params(
        ("hash" = String, Path, description = "SHA-256 content hash of the artifact")
    ),
    responses(
        (status = 200, description = "Raw PNG bytes", content_type = "image/png"),
        (status = 404, description = "Artifact not found"),
        (status = 500, description = "I/O error")
    )
)]
#[tracing::instrument(skip(state), fields(hash = %hash))]
pub(crate) async fn get_artifact(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Response, AnvilError> {
    // Delegate to the artifact store's get method. It constructs the
    // content-addressed file path and reads the PNG bytes, returning
    // Ok(None) for missing files (not an error).
    match state.artifact_store.get(&hash).await {
        Ok(Some(bytes)) => {
            // Construct the response with the raw PNG bytes and set the
            // Content-Type header to image/png so the client knows how
            // to interpret the body.
            let response = Response::builder()
                .status(axum::http::StatusCode::OK)
                .header(CONTENT_TYPE, "image/png")
                .body(Body::from(bytes))
                .expect("valid Response must build — status, header, and body are all valid");
            Ok(response)
        }
        Ok(None) => {
            // The artifact does not exist for this hash — return a 404
            // via the dedicated ArtifactNotFound variant.
            Err(AnvilError::ArtifactNotFound(hash))
        }
        Err(e) => {
            // I/O errors from the store (e.g. permission denied) propagate
            // as AnvilError::Io via the From<std::io::Error> impl, which
            // maps to HTTP 500 Internal Server Error.
            Err(e)
        }
    }
}

// The `make_test_state()` helper lives in the integration test file
// (tests/artifacts_tests.rs) where it is actually used by tests.
