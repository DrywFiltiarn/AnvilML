//! AnvilError — the single error type for the AnvilML domain.
//!
//! All 13 variants are defined per `ANVILML_DESIGN.md §5.2` (plus `ArtifactNotFound`
//! per `ADDENDUM_ARTIFACT_NOT_FOUND.md`). Each variant maps to a specific HTTP status
//! code via the `IntoResponse` implementation so that API handlers can return errors
//! directly without intermediate conversion.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;
use uuid::Uuid;

/// Structured JSON error body returned by every `AnvilError` variant.
///
/// The `error` field is a snake_case identifier of the variant kind, the `message`
/// field carries the human-readable description, and `request_id` is a fresh UUID v4
/// generated per response to allow correlation of error occurrences.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    message: String,
    request_id: Uuid,
}

/// The single error enum for all AnvilML subsystems.
///
/// Every variant derives `Debug` and `thiserror::Error`. The `Db` and `Io` variants
/// additionally derive `#[from]` so that `From<sqlx::Error>` and `From<std::io::Error>`
/// are implemented automatically, enabling `?` propagation from those sources.
///
/// The `IntoResponse` implementation maps each variant to an HTTP status code and a
/// structured JSON error body — see the `impl IntoResponse for AnvilError` block below.
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    /// Database layer failed (e.g. connection lost, constraint violation).
    ///
    /// Mapped to HTTP 500 because the database is an internal dependency — the client
    /// did nothing wrong.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    /// I/O failure (e.g. file read/write, directory creation).
    ///
    /// Mapped to HTTP 500 because I/O errors at the API surface indicate an unexpected
    /// system condition, not a client error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Client sent malformed serialised data (e.g. invalid JSON).
    ///
    /// Mapped to HTTP 400 because the client supplied data that could not be parsed.
    #[error("serialization error: {0}")]
    Serde(String),

    /// Internal IPC communication error between server and worker.
    ///
    /// Mapped to HTTP 400 because it signals a protocol-level issue in the internal
    /// communication channel, typically caused by a malformed request.
    #[error("IPC error: {0}")]
    Ipc(String),

    /// Request body exceeds the configured maximum payload size.
    ///
    /// Mapped to HTTP 413 (Payload Too Large) per the HTTP/1.1 standard for this
    /// condition. The limit is configurable via `max_ipc_payload_mib` in `ServerConfig`.
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),

    /// A named worker is not registered or reachable.
    ///
    /// Mapped to HTTP 404 because the worker resource does not exist in the registry.
    #[error("worker not found: {0}")]
    WorkerNotFound(String),

    /// A named job is not found in the scheduler.
    ///
    /// Mapped to HTTP 404 because the job resource does not exist or has been purged.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// The submitted computation graph is invalid.
    ///
    /// `fields` contains the names of the invalid fields or nodes. Mapped to HTTP 400
    /// because the client submitted a graph that violates structural constraints.
    #[error("invalid graph: {0:?}")]
    InvalidGraph(Vec<String>),

    /// The submitted computation graph contains a cycle.
    ///
    /// `path` contains the node identifiers forming the cycle. Mapped to HTTP 400
    /// because the client submitted a graph that violates the DAG invariant.
    #[error("graph cycle detected: {0:?}")]
    CycleDetected(Vec<String>),

    /// A requested model is not available in the registry.
    ///
    /// Mapped to HTTP 404 because the model resource does not exist.
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// A requested artifact (e.g. generated image, checkpoint) is not found.
    ///
    /// Per `ADDENDUM_ARTIFACT_NOT_FOUND.md`, this variant uses the same HTTP 404
    /// mapping as `ModelNotFound` — the artifact is a resource identified by ID.
    #[error("artifact not found: {0}")]
    ArtifactNotFound(String),

    /// No workers are available to accept jobs (all dead or unresponsive).
    ///
    /// Mapped to HTTP 503 (Service Unavailable) because the condition is temporary —
    /// workers may recover and accept jobs later.
    #[error("workers unavailable: {0}")]
    WorkersUnavailable(String),

    /// An unexpected internal error occurred.
    ///
    /// Mapped to HTTP 500 as a catch-all for errors that do not fit a more specific
    /// variant. Should be rare and always include diagnostic context in the message.
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AnvilError {
    fn into_response(self) -> Response {
        let (status, error_kind, message) = match &self {
            // Internal/system errors → 500
            Self::Db(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                self.to_string(),
            ),
            Self::Io(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                self.to_string(),
            ),
            Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                self.to_string(),
            ),

            // Client errors → 400
            Self::Serde(msg) => (
                StatusCode::BAD_REQUEST,
                "serde_error",
                format!("serialization error: {msg}"),
            ),
            Self::Ipc(msg) => (
                StatusCode::BAD_REQUEST,
                "ipc_error",
                format!("IPC error: {msg}"),
            ),
            Self::InvalidGraph(fields) => (
                StatusCode::BAD_REQUEST,
                "invalid_graph",
                format!("invalid graph: {fields:?}"),
            ),
            Self::CycleDetected(path) => (
                StatusCode::BAD_REQUEST,
                "cycle_detected",
                format!("graph cycle detected: {path:?}"),
            ),

            // Payload limit → 413
            Self::PayloadTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                format!("payload too large: {msg}"),
            ),

            // Not-found resources → 404
            Self::WorkerNotFound(id) => (
                StatusCode::NOT_FOUND,
                "worker_not_found",
                format!("worker not found: {id}"),
            ),
            Self::JobNotFound(id) => (
                StatusCode::NOT_FOUND,
                "job_not_found",
                format!("job not found: {id}"),
            ),
            Self::ModelNotFound(name) => (
                StatusCode::NOT_FOUND,
                "model_not_found",
                format!("model not found: {name}"),
            ),
            Self::ArtifactNotFound(id) => (
                StatusCode::NOT_FOUND,
                "artifact_not_found",
                format!("artifact not found: {id}"),
            ),

            // Temporarily unavailable → 503
            Self::WorkersUnavailable(reason) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "workers_unavailable",
                format!("workers unavailable: {reason}"),
            ),
        };

        let body = ErrorBody {
            error: error_kind.to_string(),
            message,
            // Generate a fresh UUID v4 per response for request correlation.
            request_id: Uuid::new_v4(),
        };

        (status, Json(body)).into_response()
    }
}
