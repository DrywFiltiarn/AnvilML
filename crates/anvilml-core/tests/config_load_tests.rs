//! Tests for `config_load::load()` — layered precedence (defaults + TOML + env vars + CLI).
//!
//! Tests cover:
//! - Missing file fallback (defaults)
//! - Partial TOML override
//! - Malformed TOML error
//! - Full round-trip
//! - Default path resolution
//! - Nested struct partial override
//! - Environment variable overrides (layers 3)
//! - CLI flag overrides (layer 4)

use anvilml_core::ServerConfig;
use anvilml_core::config_load::CliOverrides;
use anvilml_core::config_load::load;
use serial_test::serial;
use std::path::{Path, PathBuf};

/// `load(None, None)` returns `Ok` with all defaults.
///
/// Verifies that a missing TOML file does not produce an error — the function
/// falls back cleanly to compiled-in defaults.
#[serial]
#[test]
fn test_load_missing_file_falls_back_to_defaults() {
    let result = load(Some(Path::new("/nonexistent/path.toml")), None);
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

/// `load(Some(&temp_path), None)` with a partial TOML overrides only specified fields.
///
/// Writes a temporary TOML file containing only `host` and `port`, calls `load`,
/// and asserts that the two specified fields are overridden while every other
/// field (including nested structs) retains its default value.
#[serial]
#[test]
fn test_load_partial_toml_overrides_only_specified_fields() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a5_partial.toml");
    let toml_content = r#"
host = "0.0.0.0"
port = 9999
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path), None);
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

/// `load(Some(&temp_path), None)` with malformed TOML returns `Err(AnvilError::Serde)`.
///
/// Writes a temporary TOML file with invalid syntax (trailing comma), calls `load`,
/// and asserts that the error variant is `Serde`.
#[serial]
#[test]
fn test_load_malformed_toml_returns_err() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a5_malformed.toml");
    // Trailing comma is invalid TOML.
    let toml_content = r#"
host = "127.0.0.1",
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path), None);
    assert!(result.is_err());
    match result {
        Err(anvilml_core::AnvilError::Serde(_)) => {} // Expected.
        other => panic!("expected AnvilError::Serde, got {:?}", other),
    }

    // Clean up temp file.
    let _ = std::fs::remove_file(&temp_path);
}

/// `load(Some(&temp_path), None)` with a full TOML round-trips all fields.
///
/// Writes a temporary TOML file with every `ServerConfig` field set to a
/// non-default value, calls `load`, and asserts that every loaded field
/// matches the TOML values exactly. This proves the merge covers all fields
/// including nested structs and optional sections.
#[serial]
#[test]
fn test_load_full_toml_roundtrips_all_fields() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a5_full.toml");
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

    let result = load(Some(&temp_path), None);
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

/// `load(None, None)` resolves to the default `./anvilml.toml` path.
///
/// The checked-in `./anvilml.toml` at the repo root contains only `host` and `port`,
/// so this test verifies that the default path resolution works and that the two
/// present fields are loaded while all others retain defaults.
#[serial]
#[test]
fn test_load_default_path_resolves_anvilml_toml() {
    // Ensure ANVILML_HOST is not set, to avoid interference from prior tests.
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    // Clear all ANVILML_* env vars that might leak from other tests.
    clear_anvilml_env_vars();

    let result = load(None, None);
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

    // Restore prior env state.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
}

/// `load(Some(&temp_path), None)` with only `[gpu_selection]` overrides just that nested struct.
///
/// Writes a temporary TOML file containing only a `[gpu_selection]` section with
/// `default_device = "cpu"`, calls `load`, and asserts that only
/// `gpu_selection.default_device` is overridden while all other nested structs
/// (`limits`, `rocm`, `hardware_override`) retain their default `None`/zero values.
#[serial]
#[test]
fn test_load_nested_struct_partial_override() {
    let temp_path = std::env::temp_dir().join("anvilml_p2_a5_gpu.toml");
    let toml_content = r#"
[gpu_selection]
default_device = "cpu"
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path), None);
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

/// `ANVILML_HOST` env var overrides a TOML-set `host` value.
///
/// Writes a TOML with `host = "0.0.0.0"`, sets `ANVILML_HOST = "10.0.0.1"`,
/// calls `load()`, and asserts `config.host == "10.0.0.1"`. Verifies that
/// environment variables (layer 3) beat TOML values (layer 2).
#[serial]
#[test]
fn test_env_var_overrides_toml_value() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    // Clear other ANVILML_* env vars to avoid interference.
    clear_anvilml_env_vars();
    unsafe { std::env::set_var("ANVILML_HOST", "10.0.0.1") };

    let temp_path = std::env::temp_dir().join("anvilml_p2_a5_env_toml.toml");
    let toml_content = r#"
host = "0.0.0.0"
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path), None);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.host, "10.0.0.1");

    // Env var overrides TOML: host should be "10.0.0.1", not "0.0.0.0".

    // Clean up: restore prior env value and remove temp file.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
    let _ = std::fs::remove_file(&temp_path);
}

/// `ANVILML_PORT` env var overrides the compiled-in default when no TOML is present.
///
/// Calls `load()` with a nonexistent TOML path, `None` CLI overrides, and
/// `ANVILML_PORT = "9999"`, asserting `config.port == 9999`. Verifies that
/// env vars (layer 3) beat defaults (layer 1).
#[serial]
#[test]
fn test_env_var_overrides_default_no_toml() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    clear_anvilml_env_vars();
    unsafe { std::env::set_var("ANVILML_PORT", "9999") };

    let result = load(None, None);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.port, 9999);

    // Env var overrides default: port should be 9999, not 8488.

    // Clean up: restore prior env value.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
}

/// `CliOverrides { host }` overrides an env var-set `host` value.
///
/// Sets `ANVILML_HOST = "10.0.0.1"`, calls `load()` with
/// `Some(CliOverrides { host: Some("127.0.0.2".into()), port: None })`,
/// and asserts `config.host == "127.0.0.2"`. Verifies that CLI overrides
/// (layer 4) beat env vars (layer 3).
#[serial]
#[test]
fn test_cli_override_beats_env_var() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    clear_anvilml_env_vars();
    unsafe { std::env::set_var("ANVILML_HOST", "10.0.0.1") };

    let cli = Some(CliOverrides {
        host: Some("127.0.0.2".into()),
        port: None,
    });

    // Use a nonexistent TOML path so only env vars and CLI matter.
    let result = load(None, cli);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.host, "127.0.0.2");

    // CLI overrides env var: host should be "127.0.0.2", not "10.0.0.1".

    // Clean up: restore prior env value.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
}

/// `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` correctly parses nested field via `__` separator.
///
/// Sets `ANVILML_GPU_SELECTION__DEFAULT_DEVICE = "cuda"`, calls `load()`,
/// and asserts `config.gpu_selection.default_device == "cuda"`. Verifies the
/// `__` nested field convention for env vars.
#[serial]
#[test]
fn test_nested_env_var_gpu_selection() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    clear_anvilml_env_vars();
    unsafe { std::env::set_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE", "cuda") };

    let result = load(None, None);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.gpu_selection.default_device, "cuda");

    // Nested env var overrides default: gpu_selection.default_device should be "cuda".

    // Clean up: restore prior env value.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
}

/// Unset `ANVILML_HOST` preserves the TOML-set value (no override).
///
/// Writes a TOML with `host = "0.0.0.0"`, does NOT set `ANVILML_HOST`,
/// calls `load()`, and asserts `config.host == "0.0.0.0"`. Verifies that
/// unset env vars preserve the prior layer's value (TOML).
#[serial]
#[test]
fn test_unset_env_vars_leave_prior_layer_value() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    clear_anvilml_env_vars();

    let temp_path = std::env::temp_dir().join("anvilml_p2_a5_unset.toml");
    let toml_content = r#"
host = "0.0.0.0"
"#;
    std::fs::write(&temp_path, toml_content).expect("write temp TOML");

    let result = load(Some(&temp_path), None);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.host, "0.0.0.0");

    // Unset env var preserves TOML value: host should be "0.0.0.0".

    // Clean up: restore prior env state and remove temp file.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
    let _ = std::fs::remove_file(&temp_path);
}

/// `ANVILML_PORT` env var parses as `u16` correctly.
///
/// Sets `ANVILML_PORT = "7777"`, calls `load()`, and asserts `config.port == 7777`.
/// Verifies scalar numeric env var parsing.
#[serial]
#[test]
fn test_env_var_port_override() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    clear_anvilml_env_vars();
    unsafe { std::env::set_var("ANVILML_PORT", "7777") };

    let result = load(None, None);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.port, 7777);

    // Env var u16 parse: port should be 7777.

    // Clean up: restore prior env value.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
}

/// `ANVILML_NUM_THREADS` env var parses as `Option<u32>` correctly.
///
/// Sets `ANVILML_NUM_THREADS = "4"`, calls `load()`, and asserts
/// `config.num_threads == Some(4)`. Verifies `Option<u32>` env var parsing.
#[serial]
#[test]
fn test_num_threads_env_var() {
    let prior_host = std::env::var("ANVILML_HOST").ok();
    let prior_port = std::env::var("ANVILML_PORT").ok();
    let prior_gpu = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();
    let prior_threads = std::env::var("ANVILML_NUM_THREADS").ok();
    clear_anvilml_env_vars();
    unsafe { std::env::set_var("ANVILML_NUM_THREADS", "4") };

    let result = load(None, None);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.num_threads, Some(4));

    // Env var u32 parse: num_threads should be Some(4).

    // Clean up: restore prior env value.
    restore_env_vars(prior_host, prior_port, prior_gpu, prior_threads);
}

/// Helper to clear all known ANVILML_* env vars used by config loading.
///
/// This ensures tests don't leak state into each other. Called at the
/// start of every env-var test to guarantee a clean slate.
fn clear_anvilml_env_vars() {
    // Clear all ANVILML_* env vars that config_load reads.
    let vars = [
        "ANVILML_HOST",
        "ANVILML_PORT",
        "ANVILML_DB_PATH",
        "ANVILML_ARTIFACT_DIR",
        "ANVILML_VENV_PATH",
        "ANVILML_MODEL_SCAN_DEPTH",
        "ANVILML_MAX_IPC_PAYLOAD_MIB",
        "ANVILML_NUM_THREADS",
        "ANVILML_GPU_SELECTION__DEFAULT_DEVICE",
    ];
    for var in &vars {
        unsafe { std::env::remove_var(var) };
    }
}

/// Helper to restore env vars after a test that mutated them.
fn restore_env_vars(
    prior_host: Option<String>,
    prior_port: Option<String>,
    prior_gpu: Option<String>,
    prior_threads: Option<String>,
) {
    // Restore ANVILML_HOST.
    match prior_host {
        Some(v) => unsafe { std::env::set_var("ANVILML_HOST", v) },
        None => unsafe { std::env::remove_var("ANVILML_HOST") },
    }
    // Restore ANVILML_PORT.
    match prior_port {
        Some(v) => unsafe { std::env::set_var("ANVILML_PORT", v) },
        None => unsafe { std::env::remove_var("ANVILML_PORT") },
    }
    // Restore ANVILML_GPU_SELECTION__DEFAULT_DEVICE.
    match prior_gpu {
        Some(v) => unsafe { std::env::set_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE", v) },
        None => unsafe { std::env::remove_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE") },
    }
    // Restore ANVILML_NUM_THREADS.
    match prior_threads {
        Some(v) => unsafe { std::env::set_var("ANVILML_NUM_THREADS", v) },
        None => unsafe { std::env::remove_var("ANVILML_NUM_THREADS") },
    }
}
