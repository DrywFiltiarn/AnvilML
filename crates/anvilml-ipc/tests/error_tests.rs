//! Tests for `IpcError` — Display output for every variant and the
//! `From<IpcError> for AnvilError` conversion.
//!
//! All tests use synchronous `#[test]` (no async needed): `Display` and
//! `From` are pure, blocking operations. No env vars, files, or I/O are used.

use anvilml_core::AnvilError;
use anvilml_ipc::IpcError;

/// `IpcError::BindFailed` displays as `"bind failed: <reason>"`.
#[test]
fn test_bind_failed_display() {
    let err = IpcError::BindFailed("address already in use".to_string());
    assert_eq!(err.to_string(), "bind failed: address already in use");
}

/// `IpcError::SendFailed` displays as `"send failed: <reason>"`.
#[test]
fn test_send_failed_display() {
    let err = IpcError::SendFailed("connection closed".to_string());
    assert_eq!(err.to_string(), "send failed: connection closed");
}

/// `IpcError::RecvFailed` displays as `"recv failed: <reason>"`.
#[test]
fn test_recv_failed_display() {
    let err = IpcError::RecvFailed("timeout".to_string());
    assert_eq!(err.to_string(), "recv failed: timeout");
}

/// `IpcError::SerializationFailed` displays as `"serialization failed: <reason>"`.
#[test]
fn test_serialization_failed_display() {
    let err = IpcError::SerializationFailed("unsupported type".to_string());
    assert_eq!(err.to_string(), "serialization failed: unsupported type");
}

/// `IpcError::PayloadTooLarge` displays both `actual` and `max` values.
#[test]
fn test_payload_too_large_display() {
    let err = IpcError::PayloadTooLarge {
        actual: 1024,
        max: 512,
    };
    assert_eq!(err.to_string(), "payload too large: 1024 > 512");
}

/// `IpcError::UnknownWorker` displays as `"unknown worker: <id>"`.
#[test]
fn test_unknown_worker_display() {
    let err = IpcError::UnknownWorker("gpu:3".to_string());
    assert_eq!(err.to_string(), "unknown worker: gpu:3");
}

/// Converting every `IpcError` variant to `AnvilError` produces
/// `AnvilError::Ipc(_)` with the correct message from the variant's Display output.
#[test]
fn test_from_ipc_error_to_anvil_error() {
    let variants: Vec<(IpcError, &'static str)> = vec![
        (
            IpcError::BindFailed("addr".to_string()),
            "bind failed: addr",
        ),
        (
            IpcError::SendFailed("timeout".to_string()),
            "send failed: timeout",
        ),
        (
            IpcError::RecvFailed("closed".to_string()),
            "recv failed: closed",
        ),
        (
            IpcError::SerializationFailed("bad msgpack".to_string()),
            "serialization failed: bad msgpack",
        ),
        (
            IpcError::PayloadTooLarge {
                actual: 1024,
                max: 512,
            },
            "payload too large: 1024 > 512",
        ),
        (
            IpcError::UnknownWorker("gpu:3".to_string()),
            "unknown worker: gpu:3",
        ),
    ];

    for (ipc_err, expected_msg) in variants {
        let anvil_err: AnvilError = ipc_err.into();
        match anvil_err {
            AnvilError::Ipc(msg) => {
                assert_eq!(msg, expected_msg, "message mismatch for variant");
            }
            other => {
                panic!("expected AnvilError::Ipc, got {:?}", other);
            }
        }
    }
}
