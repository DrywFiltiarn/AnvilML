//! Tests for `config_load::load()` — layered precedence (defaults + TOML).
//!
//! Each test verifies one aspect of the two-layer config loading:
//! missing file fallback, partial override, malformed TOML error,
//! full round-trip, default path resolution, and nested struct override.

use anvilml_core::ServerConfig;
use anvilml_core::config_load::load;
use std::path::{Path, PathBuf};

/// `load(Some(Path::new("/nonexistent.toml")))` returns `Ok` with all defaults.
///
/// Verifies that a missing TOML file does not produce an error — the function
/// falls back cleanly to compiled-in defaults.
#[test]
fn test_load_missing_file_falls_back_to_defaults() {
    let result = load(Some(Path::new("/nonexistent/path.toml")));
    assert!(result.is_ok());
    let config = result.unwrap();
    let defaults = ServerConfig::default();
    assert_eq!(config.host, defaults.host);
    assert_eq!(config.port, defaults.port);
    assert_eq!(config.db_path, defaults.db_path);
    assert_eq!(config.artifact_dir, defaults.artifact_dir);
    assert_eq!(config.venv_path, defaults.venv_path);
    assert_eq!(config.model_scan_depth, defaults.model_scan_depth);
    assert_eq!(config.max_ipc_payload_mib, defaults.max_ipc_payload_mib);
    assert_eq!(config.num_threads, defaults.num_threads);
    assert!(config.model_dirs.is_empty());
    assert_eq!(
        config.gpu_selection.default_device,
        defaults.gpu_selection.default_device
    );
    assert_eq!(
        config.limits.max_queued_jobs,
        defaults.limits.max_queued_jobs
    );
    assert!(config.rocm.is_none());
    assert!(config.hardware_override.is_none());
}

/// `load(Some(&temp_path))` with a partial TOML overrides only specified fields.
///
/// Writes a temporary TOML file containing only `host` and `port`, calls `load`,
/// and asserts that the two specified fields are overridden while every other
/// field (including nested structs) retains its default value.
#[test]
fn test_load_partial_toml_overrides_only_specified_fields() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a4_partial.toml");
    let toml_content = r#"
host = "0.0.0.0"
port = 9999
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path));
    assert!(result.is_ok());
    let config = result.unwrap();

    // Overridden fields:
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9999);

    // All other fields must retain defaults:
    let defaults = ServerConfig::default();
    assert_eq!(config.db_path, defaults.db_path);
    assert_eq!(config.artifact_dir, defaults.artifact_dir);
    assert_eq!(config.venv_path, defaults.venv_path);
    assert_eq!(config.model_scan_depth, defaults.model_scan_depth);
    assert_eq!(config.max_ipc_payload_mib, defaults.max_ipc_payload_mib);
    assert_eq!(config.num_threads, defaults.num_threads);
    assert!(config.model_dirs.is_empty());
    assert_eq!(
        config.gpu_selection.default_device,
        defaults.gpu_selection.default_device
    );
    assert_eq!(
        config.limits.max_queued_jobs,
        defaults.limits.max_queued_jobs
    );
    assert!(config.rocm.is_none());
    assert!(config.hardware_override.is_none());

    // Clean up temp file.
    let _ = std::fs::remove_file(&temp_path);
}

/// `load(Some(&temp_path))` with malformed TOML returns `Err(AnvilError::Serde)`.
///
/// Writes a temporary TOML file with invalid syntax (trailing comma), calls `load`,
/// and asserts that the error variant is `Serde`.
#[test]
fn test_load_malformed_toml_returns_err() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a4_malformed.toml");
    // Trailing comma is invalid TOML.
    let toml_content = r#"
host = "127.0.0.1",
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path));
    assert!(result.is_err());
    match result {
        Err(anvilml_core::AnvilError::Serde(_)) => {} // Expected.
        other => panic!("expected AnvilError::Serde, got {:?}", other),
    }

    // Clean up temp file.
    let _ = std::fs::remove_file(&temp_path);
}

/// `load(Some(&temp_path))` with a full TOML round-trips all fields.
///
/// Writes a temporary TOML file with every `ServerConfig` field set to a
/// non-default value, calls `load`, and asserts that every loaded field
/// matches the TOML values exactly. This proves the merge covers all fields
/// including nested structs and optional sections.
#[test]
fn test_load_full_toml_roundtrips_all_fields() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a4_full.toml");
    let toml_content = r#"
host = "192.168.1.1"
port = 3000
db_path = "/var/lib/anvilml/anvilml.db"
artifact_dir = "/var/lib/anvilml/artifacts"
venv_path = "/opt/anvilml/worker/.venv"
model_scan_depth = 5
max_ipc_payload_mib = 512
num_threads = 8

[[model_dirs]]
path = "/models/checkpoints"
recursive = true
max_depth = 3

[gpu_selection]
default_device = "cpu"

[limits]
max_queued_jobs = 200

[rocm]
hsa_override_gfx_version = "gfx90a"

[hardware_override]
device_type = "cuda"
vram_total_mib = 24576
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path));
    assert!(result.is_ok());
    let config = result.unwrap();

    // Scalar fields:
    assert_eq!(config.host, "192.168.1.1");
    assert_eq!(config.port, 3000);
    assert_eq!(config.db_path, PathBuf::from("/var/lib/anvilml/anvilml.db"));
    assert_eq!(
        config.artifact_dir,
        PathBuf::from("/var/lib/anvilml/artifacts")
    );
    assert_eq!(config.venv_path, PathBuf::from("/opt/anvilml/worker/.venv"));
    assert_eq!(config.model_scan_depth, 5);
    assert_eq!(config.max_ipc_payload_mib, 512);
    assert_eq!(config.num_threads, Some(8));

    // model_dirs:
    assert_eq!(config.model_dirs.len(), 1);
    assert_eq!(
        config.model_dirs[0].path,
        PathBuf::from("/models/checkpoints")
    );
    assert_eq!(config.model_dirs[0].recursive, true);
    assert_eq!(config.model_dirs[0].max_depth, Some(3));

    // gpu_selection:
    assert_eq!(config.gpu_selection.default_device, "cpu");

    // limits:
    assert_eq!(config.limits.max_queued_jobs, 200);

    // rocm:
    assert!(config.rocm.is_some());
    assert_eq!(
        config.rocm.as_ref().unwrap().hsa_override_gfx_version,
        Some("gfx90a".to_string())
    );

    // hardware_override:
    assert!(config.hardware_override.is_some());
    let hw = config.hardware_override.as_ref().unwrap();
    assert_eq!(hw.device_type, "cuda");
    assert_eq!(hw.vram_total_mib, 24576);

    // Clean up temp file.
    let _ = std::fs::remove_file(&temp_path);
}

/// `load(None)` resolves to the default `./anvilml.toml` path.
///
/// The checked-in `./anvilml.toml` at the repo root contains only `host` and `port`,
/// so this test verifies that the default path resolution works and that the two
/// present fields are loaded while all others retain defaults.
#[test]
fn test_load_default_path_resolves_anvilml_toml() {
    let result = load(None);
    assert!(result.is_ok());
    let config = result.unwrap();

    // The checked-in anvilml.toml has host and port.
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8488);

    // All other fields must retain defaults (since the toml has no other keys).
    let defaults = ServerConfig::default();
    assert_eq!(config.db_path, defaults.db_path);
    assert_eq!(config.artifact_dir, defaults.artifact_dir);
    assert_eq!(config.venv_path, defaults.venv_path);
    assert_eq!(config.model_scan_depth, defaults.model_scan_depth);
    assert_eq!(config.max_ipc_payload_mib, defaults.max_ipc_payload_mib);
    assert_eq!(config.num_threads, defaults.num_threads);
    assert!(config.model_dirs.is_empty());
    assert_eq!(
        config.gpu_selection.default_device,
        defaults.gpu_selection.default_device
    );
    assert_eq!(
        config.limits.max_queued_jobs,
        defaults.limits.max_queued_jobs
    );
    assert!(config.rocm.is_none());
    assert!(config.hardware_override.is_none());
}

/// `load(Some(&temp_path))` with only `[gpu_selection]` overrides just that nested struct.
///
/// Writes a temporary TOML file containing only a `[gpu_selection]` section with
/// `default_device = "cpu"`, calls `load`, and asserts that only
/// `gpu_selection.default_device` is overridden while all other nested structs
/// (`limits`, `rocm`, `hardware_override`) retain their default `None`/zero values.
#[test]
fn test_load_nested_struct_partial_override() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a4_gpu.toml");
    let toml_content = r#"
[gpu_selection]
default_device = "cpu"
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path));
    assert!(result.is_ok());
    let config = result.unwrap();

    // Only gpu_selection.default_device is overridden:
    assert_eq!(config.gpu_selection.default_device, "cpu");

    // All other nested structs retain defaults:
    let defaults = ServerConfig::default();
    assert_eq!(
        config.limits.max_queued_jobs,
        defaults.limits.max_queued_jobs
    );
    assert!(config.rocm.is_none());
    assert!(config.hardware_override.is_none());
    assert!(config.model_dirs.is_empty());

    // Scalar fields also retain defaults:
    assert_eq!(config.host, defaults.host);
    assert_eq!(config.port, defaults.port);

    // Clean up temp file.
    let _ = std::fs::remove_file(&temp_path);
}
