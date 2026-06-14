//! Error types for `anvilml-core`.
//!
//! This module provides `AnvilError`, the unified error enum for the crate.
//! It covers I/O errors, database errors, IPC failures, graph validation errors,
//! worker management errors, and internal errors. Each variant maps to an
//! appropriate HTTP status code when converted via `IntoResponse`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use uuid::Uuid;

/// The unified error type for the AnvilML core domain.
///
/// Every variant implements `thiserror::Error` for display formatting and
/// `IntoResponse` for automatic HTTP error responses. The `status_code()`
/// method returns the HTTP status code that `into_response()` would produce,
/// enabling unit-testable status-code logic without needing an axum test client.
///
/// The 14 variants cover the error categories specified in
/// `ANVILML_DESIGN.md §5.2`:
/// - `Db` — database errors from `sqlx::Error`
/// - `Io` — filesystem I/O errors from `std::io::Error`
/// - `Serde`, `Ipc`, `PayloadTooLarge` — serialization and communication failures
/// - `WorkerNotFound`, `JobNotFound`, `ModelNotFound` — resource-not-found errors
/// - `InvalidGraph`, `CycleDetected` — graph validation failures
/// - `WorkersUnavailable` — all workers busy or dead
/// - `Internal` — unexpected internal failures
/// - `Toml`, `EnvVar` — configuration loading errors (pre-existing)
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    /// Database error from `sqlx::Error`.
    ///
    /// Produced when a database operation fails — connection errors, query
    /// failures, constraint violations, or migration errors. Maps to `500
    /// Internal Server Error` because database failures are unexpected from
    /// the client's perspective.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    /// I/O error reading or writing a config file.
    ///
    /// Produced when `std::fs::read_to_string` or similar operations fail —
    /// e.g. the file exists but is not readable, or the path is a directory.
    /// Maps to `500 Internal Server Error` because I/O errors on server-owned
    /// files indicate a server-side problem.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    ///
    /// Produced when `serde_json` fails to serialise a value or when a value
    /// cannot be deserialised from JSON. Maps to `500 Internal Server Error`
    /// because serialization failures indicate a programming error or
    /// incompatible data shape.
    #[error("serialization error: {0}")]
    Serde(String),

    /// IPC error.
    ///
    /// Produced when inter-process communication with a Python worker fails —
    /// connection lost, timeout, or protocol mismatch. Maps to `500 Internal
    /// Server Error` because IPC failures are server-side operational errors.
    #[error("IPC error: {0}")]
    Ipc(String),

    /// Payload too large.
    ///
    /// Produced when an IPC message or HTTP request body exceeds the configured
    /// maximum payload size (`max_ipc_payload_mib` from `ServerConfig`). Maps
    /// to `413 Payload Too Large` because the client sent data that is too big.
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),

    /// Worker not found.
    ///
    /// Produced when a job dispatch references a worker ID that does not exist
    /// or is not registered. Maps to `404 Not Found` because the resource
    /// (worker) does not exist.
    #[error("worker not found: {0}")]
    WorkerNotFound(String),

    /// Job not found.
    ///
    /// Produced when a job ID is referenced in a request but no matching job
    /// exists in the database. Maps to `404 Not Found` because the resource
    /// (job) does not exist.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// Invalid computation graph.
    ///
    /// Produced when a submitted job graph fails validation — e.g. a node
    /// type is unknown, a connection references a non-existent node, or a
    /// required field is missing. Maps to `400 Bad Request` because the
    /// client submitted invalid data.
    #[error("invalid graph: {0:?}")]
    InvalidGraph(Vec<String>),

    /// Graph cycle detected.
    ///
    /// Produced when a submitted job graph contains a cycle (e.g. A → B → A),
    /// making it impossible to execute topologically. Maps to `400 Bad
    /// Request` because the client submitted an invalid graph structure.
    #[error("graph cycle detected: {0:?}")]
    CycleDetected(Vec<String>),

    /// Model not found.
    ///
    /// Produced when a job references a model that does not exist in any
    /// configured model directory. Maps to `404 Not Found` because the
    /// resource (model) does not exist.
    #[error("model not found: {0}")]
    ModelNotFound(String),

    /// Workers unavailable.
    ///
    /// Produced when all registered workers are busy, dead, or failing
    /// preflight checks, so no worker is available to execute a job. Maps to
    /// `503 Service Unavailable` because the service is temporarily unable
    /// to handle the request.
    #[error("workers unavailable: {0}")]
    WorkersUnavailable(String),

    /// Internal error.
    ///
    /// Produced for unexpected internal failures that do not fit any other
    /// category — e.g. a `panic!` catch, an unhandled `match` arm, or a
    /// invariant violation. Maps to `500 Internal Server Error` because
    /// internal errors indicate a bug in the server.
    #[error("internal error: {0}")]
    Internal(String),

    /// TOML deserialisation error.
    ///
    /// Produced when `toml::from_str` fails to parse a config file into
    /// `ServerConfig` — e.g. invalid TOML syntax or type mismatch on a field.
    /// Maps to `400 Bad Request` because the config file is malformed.
    #[error("TOML deserialisation error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Invalid environment variable value.
    ///
    /// Produced when an `ANVILML_*` environment variable is set to a value
    /// that cannot be parsed into the expected type (e.g. `ANVILML_PORT=abc`).
    /// The `name` field identifies the variable; `value` holds the raw string.
    /// Maps to `400 Bad Request` because the environment is misconfigured.
    #[error("Invalid env var {name}: {value}")]
    EnvVar { name: String, value: String },
}

impl AnvilError {
    /// Returns the HTTP status code for this error variant.
    ///
    /// This is the same status code that `into_response()` produces,
    /// enabling unit-testable status-code logic without needing an axum
    /// test client.
    ///
    /// # Mapping
    /// - `Db`, `Io`, `Serde`, `Ipc`, `Internal` → `500 Internal Server Error`
    /// - `PayloadTooLarge` → `413 Payload Too Large`
    /// - `WorkerNotFound`, `JobNotFound`, `ModelNotFound` → `404 Not Found`
    /// - `InvalidGraph`, `CycleDetected`, `Toml`, `EnvVar` → `400 Bad Request`
    /// - `WorkersUnavailable` → `503 Service Unavailable`
    pub fn status_code(&self) -> StatusCode {
        match self {
            // These are server-side failures — the client cannot fix them.
            AnvilError::Db(_)
            | AnvilError::Io(_)
            | AnvilError::Serde(_)
            | AnvilError::Ipc(_)
            | AnvilError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,

            // Client sent data that is too large — they need to reduce it.
            AnvilError::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,

            // Resource not found — the client referenced a non-existent ID.
            AnvilError::WorkerNotFound(_)
            | AnvilError::JobNotFound(_)
            | AnvilError::ModelNotFound(_) => StatusCode::NOT_FOUND,

            // Invalid input — the client submitted bad data.
            AnvilError::InvalidGraph(_)
            | AnvilError::CycleDetected(_)
            | AnvilError::Toml(_)
            | AnvilError::EnvVar { .. } => StatusCode::BAD_REQUEST,

            // Temporarily unable to process — all workers are busy/dead.
            AnvilError::WorkersUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl IntoResponse for AnvilError {
    /// Converts this error into an HTTP response with structured JSON body.
    ///
    /// The response body contains three keys:
    /// - `"error"`: a machine-readable error kind string (the variant name)
    /// - `"message"`: a human-readable description of the error
    /// - `"request_id"`: a fresh `uuid::Uuid::v4()` for log correlation
    ///
    /// Generating a new UUID on every call ensures each HTTP error response
    /// has a unique request_id, even if the same error is returned multiple
    /// times. This is important for correlating error responses with server
    /// log entries.
    fn into_response(self) -> Response {
        // Generate a fresh UUID for every error response — ensures unique
        // request_id for log correlation even when the same error is
        // returned multiple times.
        let request_id = Uuid::new_v4();

        let status = self.status_code();

        // Build the structured error body with the three required keys.
        // The error kind is the variant name (lowercase), and the message
        // is the Display output from thiserror's #[error(...)] attribute.
        let body = serde_json::json!({
            "error": self.error_kind(),
            "message": self.to_string(),
            "request_id": request_id.to_string(),
        });

        (status, Json(body)).into_response()
    }
}

impl AnvilError {
    /// Returns the machine-readable error kind string for this variant.
    ///
    /// The kind is the lowercase variant name (e.g. `"job_not_found"`,
    /// `"internal"`), used as the `"error"` field in the JSON response body.
    ///
    /// This is also exposed for test access to verify response body structure.
    pub fn error_kind(&self) -> &'static str {
        match self {
            AnvilError::Db(_) => "database",
            AnvilError::Io(_) => "io",
            AnvilError::Serde(_) => "serialization",
            AnvilError::Ipc(_) => "ipc",
            AnvilError::PayloadTooLarge(_) => "payload_too_large",
            AnvilError::WorkerNotFound(_) => "worker_not_found",
            AnvilError::JobNotFound(_) => "job_not_found",
            AnvilError::InvalidGraph(_) => "invalid_graph",
            AnvilError::CycleDetected(_) => "cycle_detected",
            AnvilError::ModelNotFound(_) => "model_not_found",
            AnvilError::WorkersUnavailable(_) => "workers_unavailable",
            AnvilError::Internal(_) => "internal",
            AnvilError::Toml(_) => "toml",
            AnvilError::EnvVar { .. } => "env_var",
        }
    }
}
