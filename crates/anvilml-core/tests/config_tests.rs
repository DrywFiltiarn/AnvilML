//! Tests for `ServerConfig` default values — each test asserts one scalar
//! field of `ServerConfig::default()` against the compiled-in default from
//! ENVIRONMENT.md §4.

use anvilml_core::ServerConfig;
use std::path::PathBuf;

/// `ServerConfig::default().host` equals `"127.0.0.1"`.
#[test]
fn test_host_default() {
    let config = ServerConfig::default();
    assert_eq!(config.host, "127.0.0.1");
}

/// `ServerConfig::default().port` equals `8488`.
#[test]
fn test_port_default() {
    let config = ServerConfig::default();
    assert_eq!(config.port, 8488);
}

/// `ServerConfig::default().db_path` equals `PathBuf::from("./anvilml.db")`.
#[test]
fn test_db_path_default() {
    let config = ServerConfig::default();
    assert_eq!(config.db_path, PathBuf::from("./anvilml.db"));
}

/// `ServerConfig::default().artifact_dir` equals `PathBuf::from("./artifacts")`.
#[test]
fn test_artifact_dir_default() {
    let config = ServerConfig::default();
    assert_eq!(config.artifact_dir, PathBuf::from("./artifacts"));
}

/// `ServerConfig::default().venv_path` equals `PathBuf::from("./worker/.venv")`.
#[test]
fn test_venv_path_default() {
    let config = ServerConfig::default();
    assert_eq!(config.venv_path, PathBuf::from("./worker/.venv"));
}

/// `ServerConfig::default().model_scan_depth` equals `2`.
#[test]
fn test_model_scan_depth_default() {
    let config = ServerConfig::default();
    assert_eq!(config.model_scan_depth, 2);
}

/// `ServerConfig::default().max_ipc_payload_mib` equals `256`.
#[test]
fn test_max_ipc_payload_mib_default() {
    let config = ServerConfig::default();
    assert_eq!(config.max_ipc_payload_mib, 256);
}

/// `ServerConfig::default().num_threads` is `None` (auto = num_cpus).
#[test]
fn test_num_threads_default() {
    let config = ServerConfig::default();
    assert!(config.num_threads.is_none());
}
