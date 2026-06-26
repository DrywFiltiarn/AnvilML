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

/// `ServerConfig::default().model_dirs` is an empty vec.
#[test]
fn test_model_dirs_default() {
    let config = ServerConfig::default();
    assert!(config.model_dirs.is_empty());
}

/// `ServerConfig::default().gpu_selection.default_device` equals `"auto"`.
#[test]
fn test_gpu_selection_default() {
    let config = ServerConfig::default();
    assert_eq!(config.gpu_selection.default_device, "auto");
}

/// `ServerConfig::default().limits.max_queued_jobs` equals `100`.
#[test]
fn test_limits_default() {
    let config = ServerConfig::default();
    assert_eq!(config.limits.max_queued_jobs, 100);
}

/// `ServerConfig::default().rocm` is `None`.
#[test]
fn test_rocm_default() {
    let config = ServerConfig::default();
    assert!(config.rocm.is_none());
}

/// `ServerConfig::default().hardware_override` is `None`.
#[test]
fn test_hardware_override_default() {
    let config = ServerConfig::default();
    assert!(config.hardware_override.is_none());
}
