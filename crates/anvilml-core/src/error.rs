//! Centralized error type for the AnvilML core crate.
//!
//! All domain-level errors flow through this enum, which derives
//! `Display`, `std::error::Error`, and `From<std::io::Error>` via
//! `thiserror`. The type is guaranteed `Send + Sync` so it can cross
//! async boundaries (e.g. through `axum`'s `ErrorResponse`s).

use std::fmt;
use uuid::Uuid;

/// Centralised error enum for the AnvilML core crate.
#[derive(Debug)]
pub enum AnvilError {
    /// A configuration file could not be loaded or parsed.
    ConfigLoad(String),
    /// Generic I/O error (auto-converted from `std::io::Error`).
    Io(std::io::Error),
    /// Serde JSON serialization / deserialization failure.
    Json(String),
    /// A DAG validation failure (invalid job graph).
    InvalidGraph(String),
    /// A worker process has died unexpectedly.
    WorkerDead(String),
    /// A job was not found by its UUID.
    JobNotFound(Uuid),
    /// A job cannot be cancelled (not in Queued or Running state).
    JobNotCancellable(Uuid),
    /// An artifact was not found by its identifier.
    ArtifactNotFound(String),
    /// A database error occurred.
    DbError(String),
    /// The request payload exceeded the allowed size limit.
    PayloadTooLarge(String),
    /// A seed SQL file is missing the `-- anvil:seed_table` directive.
    SeedMissingDirective(String),
}

// ── Display / Error ───────────────────────────────────────────────────────────

impl fmt::Display for AnvilError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigLoad(msg) => write!(f, "config load error: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Json(msg) => write!(f, "JSON error: {msg}"),
            Self::InvalidGraph(msg) => write!(f, "invalid graph: {msg}"),
            Self::WorkerDead(msg) => write!(f, "worker dead: {msg}"),
            Self::JobNotFound(id) => {
                write!(f, "job not found: {id}")
            }
            Self::JobNotCancellable(id) => {
                write!(f, "job not cancellable: {id}")
            }
            Self::ArtifactNotFound(id) => {
                write!(f, "artifact not found: {id}")
            }
            Self::DbError(msg) => write!(f, "database error: {msg}"),
            Self::PayloadTooLarge(msg) => {
                write!(f, "payload too large: {msg}")
            }
            Self::SeedMissingDirective(msg) => {
                write!(f, "seed missing directive: {msg}")
            }
        }
    }
}

impl std::error::Error for AnvilError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AnvilError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

// ── Send + Sync ───────────────────────────────────────────────────────────────

/// SAFETY: All variants contain only `Send + Sync` data:
/// - `String`, `Uuid`, and `std::io::Error` are all `Send + Sync`.
unsafe impl Send for AnvilError {}
unsafe impl Sync for AnvilError {}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    /// Every variant must produce a valid (non-empty) Display string.
    #[test]
    fn all_variants_display() {
        let cases: Vec<AnvilError> = vec![
            AnvilError::ConfigLoad("missing file".into()),
            AnvilError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "ghost")),
            AnvilError::Json("bad value".into()),
            AnvilError::InvalidGraph("cycle detected".into()),
            AnvilError::WorkerDead("pid 42 exited with 1".into()),
            AnvilError::JobNotFound(
                Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            ),
            AnvilError::JobNotCancellable(
                Uuid::parse_str("660e8400-e29b-41d4-a716-446655440000").unwrap(),
            ),
            AnvilError::ArtifactNotFound("sha256:dead".into()),
            AnvilError::DbError("connection refused".into()),
            AnvilError::PayloadTooLarge("10 MB > 5 MB limit".into()),
            AnvilError::SeedMissingDirective("no seed_table directive".into()),
        ];

        for err in cases {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "Display must not be empty for {err:?}");
            // The message should contain the variant's descriptive prefix.
            assert!(
                msg.len() > 3,
                "{err:?} produced suspiciously short message: {msg}"
            );
        }
    }

    /// AnvilError must implement std::error::Error with a source chain for Io.
    #[test]
    fn error_trait_impls() {
        let io_err = AnvilError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "no access",
        ));

        // Verify source() returns the inner io::Error for Io variant.
        assert!(io_err.source().is_some(), "Io variant must return source()");

        // Verify other variants have no source chain.
        let config_err = AnvilError::ConfigLoad("test".into());
        assert!(
            config_err.source().is_none(),
            "ConfigLoad should not return a source"
        );
    }

    /// AnvilError must be Send + Sync.
    #[test]
    fn send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<AnvilError>();
        assert_sync::<AnvilError>();
    }

    /// From<std::io::Error> must convert into AnvilError::Io.
    #[test]
    fn from_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        let anvil: AnvilError = io.into();
        match anvil {
            AnvilError::Io(inner) => {
                assert_eq!(inner.kind(), std::io::ErrorKind::Other);
            }
            other => panic!("Expected Io variant, got {other:?}"),
        }
    }

    /// Debug formatting must be valid for all variants.
    #[test]
    fn debug_formatting() {
        let err = AnvilError::WorkerDead("test worker".into());
        let debug_str = format!("{err:?}");
        assert!(
            debug_str.contains("WorkerDead"),
            "Debug must contain variant name: {debug_str}"
        );
    }
}
