//! Unified error type for all AnvilML crates.
//!
//! `AnvilError` replaces ad-hoc `Box<dyn Error>` propagation with a single,
//! well-documented enum that carries semantic information about failure modes
//! such as configuration issues, I/O errors, serialization failures, graph
//! validation problems, worker lifecycle events, job/artifact lookup misses,
//! database errors, and IPC payload size violations.

use thiserror::Error;

/// The unified error type for all AnvilML crates.
#[derive(Debug, Error)]
pub enum AnvilError {
    /// A configuration file could not be loaded or parsed.
    #[error("config load failed: {0}")]
    ConfigLoad(String),

    /// An I/O operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A JSON serialization/deserialization error occurred.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// A computation graph failed validation.
    #[error("invalid computation graph: {0}")]
    InvalidGraph(String),

    /// A worker process has died unexpectedly.
    #[error("worker dead: {0}")]
    WorkerDead(String),

    /// A job was not found by its identifier.
    ///
    /// The identifier is stored as a `String` until the `uuid` crate is added
    /// in P2-A3.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// An artifact was not found by its identifier.
    ///
    /// The identifier is stored as a `String` until the `uuid` crate is added
    /// in P2-A3.
    #[error("artifact not found: {0}")]
    ArtifactNotFound(String),

    /// A database operation failed.
    #[error("database error: {0}")]
    DbError(String),

    /// An IPC payload exceeded the configured size limit.
    #[error(
        "payload too large: {} MiB exceeds limit of {} MiB",
        size_mib,
        limit_mib
    )]
    PayloadTooLarge { size_mib: u32, limit_mib: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Display message tests for each variant
    // ------------------------------------------------------------------

    #[test]
    fn display_config_load() {
        let err = AnvilError::ConfigLoad("missing key".into());
        assert_eq!(err.to_string(), "config load failed: missing key");
    }

    #[test]
    fn display_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let err: AnvilError = io_err.into();
        assert_eq!(err.to_string(), "nope");
    }

    #[test]
    fn display_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: AnvilError = json_err.into();
        // thiserror transparent delegates the inner Display
        assert!(
            err.to_string().contains("expected ident")
                || err.to_string().contains("expected value")
                || err.to_string().contains("invalid")
        );
    }

    #[test]
    fn display_invalid_graph() {
        let err = AnvilError::InvalidGraph("cycle detected".into());
        assert_eq!(err.to_string(), "invalid computation graph: cycle detected");
    }

    #[test]
    fn display_worker_dead() {
        let err = AnvilError::WorkerDead("exit code 137".into());
        assert_eq!(err.to_string(), "worker dead: exit code 137");
    }

    #[test]
    fn display_job_not_found() {
        let err = AnvilError::JobNotFound("abc-123".into());
        assert_eq!(err.to_string(), "job not found: abc-123");
    }

    #[test]
    fn display_artifact_not_found() {
        let err = AnvilError::ArtifactNotFound("def-456".into());
        assert_eq!(err.to_string(), "artifact not found: def-456");
    }

    #[test]
    fn display_db_error() {
        let err = AnvilError::DbError("connection refused".into());
        assert_eq!(err.to_string(), "database error: connection refused");
    }

    #[test]
    fn display_payload_too_large() {
        let err = AnvilError::PayloadTooLarge {
            size_mib: 50,
            limit_mib: 10,
        };
        assert_eq!(
            err.to_string(),
            "payload too large: 50 MiB exceeds limit of 10 MiB"
        );
    }

    // ------------------------------------------------------------------
    // From<std::io::Error> conversion
    // ------------------------------------------------------------------

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let anvil_err: AnvilError = io_err.into();
        match anvil_err {
            AnvilError::Io(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("expected Io variant, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------
    // Send + Sync bounds
    // ------------------------------------------------------------------

    #[test]
    fn anvil_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<AnvilError>();
        assert_sync::<AnvilError>();
    }
}
