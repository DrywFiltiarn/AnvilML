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
use axum::extract::Query;
use axum::extract::State;
use serde::Deserialize;
use uuid::Uuid;

use crate::state::AppState;

/// Query parameters for `GET /v1/artifacts`.
///
/// All fields are optional:
/// - `job_id` filters artifacts to those produced by a specific job.
#[derive(Debug, Deserialize)]
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
///
/// # Response
///
/// Returns `200 OK` with a JSON array of `ArtifactMeta` objects.
/// The array is empty (not `null`) when no artifacts exist or none
/// match the filter.
///
/// # Errors
///
/// Returns `AnvilError::Db` if the database query fails, which maps
/// to HTTP 500 Internal Server Error.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `ArtifactStore` through `state.artifact_store`.
#[tracing::instrument(skip(state), fields(job_id = ?params.job_id))]
pub(crate) async fn list_artifacts(
    State(state): State<AppState>,
    Query(params): Query<ListArtifactsParams>,
) -> Result<Json<Vec<ArtifactMeta>>, AnvilError> {
    // Delegate to the artifact store's list method. It ensures the
    // artifacts table exists and queries all matching rows. The
    // handler adds no business logic — per ANVILML_DESIGN.md §3.3.
    state.artifact_store.list(params.job_id).await.map(Json)
}

// The `make_test_state()` helper lives in the integration test file
// (tests/artifacts_tests.rs) where it is actually used by tests.
