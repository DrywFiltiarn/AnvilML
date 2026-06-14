/// Integration tests for `config_load.rs` — the `load()` function and
/// `ConfigOverrides` struct.
///
/// Verifies the four-level config precedence chain:
/// defaults → TOML file → env vars → CLI overrides.
use anvilml_core::config::ServerConfig;
use anvilml_core::{load, ConfigOverrides};

/// Verifies that when the TOML file does not exist, `load()` returns
/// `ServerConfig::default()` with all compiled-in defaults intact.
///
/// This is the baseline test — it confirms the function does not panic
/// or error on a missing file, and that defaults are preserved.
#[test]
fn test_missing_file_uses_defaults() {
    // Clear ANVILML_PORT to ensure the test sees the compiled-in default
    // rather than a value leaked from a sibling test binary.
    // Capture the prior state so we can restore it unconditionally.
    let prior = std::env::var("ANVILML_PORT").ok();
    std::env::remove_var("ANVILML_PORT");

    let cfg = load(
        std::path::Path::new("/nonexistent/path.toml"),
        &ConfigOverrides::default(),
    );

    assert!(cfg.is_ok());
    let result = cfg.unwrap();
    assert_eq!(result, ServerConfig::default());

    // Restore prior env state unconditionally.
    match prior {
        Some(v) => std::env::set_var("ANVILML_PORT", v),
        None => std::env::remove_var("ANVILML_PORT"),
    }
}

/// Verifies that an `ANVILML_*` environment variable overrides the same
/// field from the TOML file — env takes precedence over TOML.
///
/// Writes a TOML file with `port = 9001`, sets `ANVILML_PORT=8080`,
/// and asserts that the final config has `port == 8080`.
#[test]
fn test_env_var_beats_toml() {
    let prior = std::env::var("ANVILML_PORT").ok();

    // Create a temporary TOML file with a non-default port.
    // TOML fields map directly to ServerConfig struct fields (no [server] section).
    let toml_content = "host = \"127.0.0.1\"\nport = 9001\n";
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    std::fs::write(tmp.path(), toml_content).expect("write temp file");

    // Set the env var to override the TOML port.
    std::env::set_var("ANVILML_PORT", "8080");

    let cfg = load(tmp.path(), &ConfigOverrides::default()).expect("load config");

    // Env var should beat TOML: port is 8080, not 9001.
    assert_eq!(cfg.port, 8080);

    // Restore prior env state unconditionally.
    match prior {
        Some(v) => std::env::set_var("ANVILML_PORT", v),
        None => std::env::remove_var("ANVILML_PORT"),
    }
}

/// Verifies that `ConfigOverrides.port` (CLI override) takes precedence
/// over both the TOML file and the `ANVILML_PORT` environment variable.
///
/// Writes a TOML file with `port = 9001`, sets `ANVILML_PORT=8080`,
/// and passes `overrides.port = Some(7070)`. Asserts final port is 7070.
#[test]
fn test_cli_override_beats_env() {
    let prior = std::env::var("ANVILML_PORT").ok();

    // Same TOML as test_env_var_beats_toml.
    let toml_content = "host = \"127.0.0.1\"\nport = 9001\n";
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    std::fs::write(tmp.path(), toml_content).expect("write temp file");

    std::env::set_var("ANVILML_PORT", "8080");

    let overrides = ConfigOverrides {
        host: None,
        port: Some(7070),
    };

    let cfg = load(tmp.path(), &overrides).expect("load config");

    // CLI override beats env: port is 7070, not 8080 or 9001.
    assert_eq!(cfg.port, 7070);

    match prior {
        Some(v) => std::env::set_var("ANVILML_PORT", v),
        None => std::env::remove_var("ANVILML_PORT"),
    }
}

/// Verifies that double-underscore nesting works for nested config fields.
///
/// Writes a TOML file without a `gpu_selection` section, sets
/// `ANVILML_GPU_SELECTION__DEFAULT_DEVICE=cpu`, and asserts that
/// `cfg.gpu_selection.default_device == "cpu"`.
#[test]
fn test_nested_env_var() {
    let prior = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE").ok();

    // TOML file without gpu_selection — should use default ("auto").
    let toml_content = "host = \"127.0.0.1\"\nport = 8488\n";
    let tmp = tempfile::NamedTempFile::new().expect("create temp file");
    std::fs::write(tmp.path(), toml_content).expect("write temp file");

    // Set the nested env var via double-underscore.
    std::env::set_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE", "cpu");

    let cfg = load(tmp.path(), &ConfigOverrides::default()).expect("load config");

    // The env var should override the default.
    assert_eq!(cfg.gpu_selection.default_device, "cpu");

    match prior {
        Some(v) => std::env::set_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE", v),
        None => std::env::remove_var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE"),
    }
}
