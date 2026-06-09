//! Artifact serve handler — GET /v1/artifacts/:hash.
//!
//! Serves a stored PNG artifact by its content-addressed hash, with appropriate
//! HTTP caching headers. Returns 404 when the artifact file does not exist.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::App;

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
