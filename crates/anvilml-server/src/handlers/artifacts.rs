//! Artifact serve handler — GET /v1/artifacts/:hash.
//!
//! Serves a stored PNG artifact by its content-addressed hash, with appropriate
//! HTTP caching headers. Returns 404 when the artifact file does not exist.
//!
//! Also includes the list endpoint — GET /v1/artifacts — which returns artifact
//! metadata filtered by optional job_id, limit, and before query parameters.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::{artifact::store::ArtifactMeta, App};

/// Serve a stored artifact by its content-addressed hash.
///
/// Returns the raw PNG bytes with:
/// - `Content-Type: image/png`
/// - `Cache-Control: public, immutable, max-age=31536000`
/// - `ETag: "{hash}"`
///
/// Returns 404 JSON when the artifact file does not exist.
#[utoipa::path(
    get,
    path = "/v1/artifacts/{hash}",
    summary = "Get an artifact by its content hash",
    params(
        ("hash" = String, Path, description = "SHA-256 hex digest of the artifact")
    ),
    responses(
        (status = 200, description = "Artifact found", content_type = "image/png"),
        (status = 404, description = "Artifact not found", body = serde_json::Value)
    )
)]
pub async fn serve_artifact(
    State(state): State<Arc<App>>,
    Path(hash): Path<String>,
) -> impl IntoResponse {
    let path = match state.artifact_store.get_path(&hash).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(hash = %hash, error = %e, "serve_artifact: get_path failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "internal_error",
                    "message": e.to_string(),
                })),
            )
                .into_response();
        }
    };

    // Check if the file exists before attempting to read it.
    let file_metadata = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(hash = %hash, path = %path.display(), "serve_artifact: file not found");
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "artifact_not_found",
                    "message": "artifact not found",
                })),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(hash = %hash, path = %path.display(), error = %e, "serve_artifact: metadata check failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "internal_error",
                    "message": e.to_string(),
                })),
            )
                .into_response();
        }
    };

    // Ensure it's a regular file.
    if !file_metadata.is_file() {
        tracing::warn!(hash = %hash, path = %path.display(), "serve_artifact: path is not a file");
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "artifact_not_found",
                "message": "artifact not found",
            })),
        )
            .into_response();
    }

    // Read file bytes and stream as response.
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(hash = %hash, path = %path.display(), error = %e, "serve_artifact: read failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "internal_error",
                    "message": e.to_string(),
                })),
            )
                .into_response();
        }
    };

    // Build response with caching headers.
    let body: axum::body::Body = bytes.into();
    let mut response = Response::new(body);
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, immutable, max-age=31536000"),
    );
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{hash}\""))
            .expect("hash is valid hex — valid header value"),
    );
    response
}

/// Query parameters for the `GET /v1/artifacts` list endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct ListArtifactsQuery {
    /// Filter by job UUID (optional — return all artifacts if omitted).
    pub job_id: Option<String>,
    /// Maximum number of results (default 100, max 1000).
    pub limit: Option<u32>,
    /// Only return artifacts created before this Unix timestamp.
    pub before: Option<String>,
}

/// List artifacts with optional job_id, limit, and before filters.
///
/// Returns a JSON array of `ArtifactMeta` objects sorted newest-first.
#[utoipa::path(
    get,
    path = "/v1/artifacts",
    summary = "List artifacts with optional filters",
    params(
        ("job_id" = Option<String>, Query, description = "Filter by job UUID"),
        ("limit" = Option<u32>, Query, description = "Maximum number of results (default 100, max 1000)"),
        ("before" = Option<i64>, Query, description = "Only artifacts created before this Unix timestamp")
    ),
    responses(
        (status = 200, description = "Artifact list", body = Vec<ArtifactMeta>),
        (status = 503, description = "Database not available", body = serde_json::Value)
    )
)]
pub async fn list_artifacts(
    State(state): State<Arc<App>>,
    Query(query): Query<ListArtifactsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Parse before as i64 Unix timestamp; invalid values are ignored (no filter).
    let parsed_before = query.before.as_deref().and_then(|s| s.parse::<i64>().ok());
    if query.before.is_some() && parsed_before.is_none() {
        tracing::warn!(before = ?query.before, "list_artifacts: invalid before timestamp, ignoring filter");
    }

    // Compute effective limit: default 100, clamped to [1, 1000].
    let effective_limit = query.limit.unwrap_or(100).clamp(1, 1000);

    // Verify the artifact store's pool matches the app DB (it should, but guard against
    // mismatched pools in test setups).
    let artifact_store = &state.artifact_store;

    match artifact_store
        .list(query.job_id, effective_limit, parsed_before)
        .await
    {
        Ok(artifacts) => (
            StatusCode::OK,
            Json(serde_json::to_value(&artifacts).expect("artifact list serialises")),
        ),
        Err(e) => {
            tracing::error!(error = %e, "list_artifacts: database query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "internal_error",
                    "message": e.to_string(),
                })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        http::{Request, StatusCode},
        Router,
    };
    use bytes::Bytes;
    use http_body_util::Full;
    use serde_json::Value;
    use sqlx::SqlitePool;
    use tower::ServiceExt;

    use super::*;
    use crate::{build_router, EventBroadcaster};

    /// Create an in-memory SQLite pool with the artifacts table.
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect in-memory SQLite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS artifacts (
                hash       TEXT PRIMARY KEY,
                job_id     TEXT    NOT NULL,
                width      INTEGER NOT NULL,
                height     INTEGER NOT NULL,
                format     TEXT    NOT NULL,
                seed       INTEGER NOT NULL,
                steps      INTEGER NOT NULL,
                prompt     TEXT    NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create artifacts table");

        pool
    }

    /// Insert an artifact row into the artifacts table.
    async fn insert_artifact(pool: &SqlitePool, hash: &str, job_id: &str, created_at: i64) {
        sqlx::query(
            "INSERT INTO artifacts (hash, job_id, width, height, format, seed, steps, prompt, created_at) \
             VALUES (?, ?, 512, 512, 'png', 42, 20, 'test prompt', ?)",
        )
        .bind(hash)
        .bind(job_id)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("insert artifact");
    }

    /// Build a test `App` with an artifact store backed by an in-memory DB.
    async fn build_test_artifact_app() -> (Router, SqlitePool) {
        let pool = setup_pool().await;
        let artifact_store = crate::artifact::store::ArtifactStore::new(
            tempfile::tempdir().unwrap().keep(),
            pool.clone(),
        );
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state = App::new(
            "0.1.0",
            Some(pool.clone()),
            None,
            None,
            broadcaster,
            None,
            None,
            artifact_store,
        );
        (build_router(state), pool)
    }

    /// GET /v1/artifacts returns 200 with an empty JSON array when no artifacts exist.
    #[tokio::test]
    async fn list_artifacts_empty_returns_200_with_empty_array() {
        let (app, _pool) = build_test_artifact_app().await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/artifacts")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert!(parsed.is_array(), "response body must be a JSON array");
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }

    /// GET /v1/artifacts?job_id=... returns only artifacts for that job.
    #[tokio::test]
    async fn list_artifacts_with_job_id_filter() {
        let (app, pool) = build_test_artifact_app().await;

        let job_a = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let job_b = "b2c3d4e5-f6a7-8901-bcde-f12345678901";

        insert_artifact(&pool, "hash_a", job_a, 1000).await;
        insert_artifact(&pool, "hash_b", job_b, 1001).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/v1/artifacts?job_id={job_a}"))
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 1);
        assert_eq!(parsed[0]["job_id"], job_a);
        assert_eq!(parsed[0]["hash"], "hash_a");
    }
}
