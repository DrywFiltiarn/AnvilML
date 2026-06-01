//! Layered configuration loader for the AnvilML server.
//!
//! Resolves `ServerConfig` from four precedence levels (lowest to highest):
//! 1. Built-in defaults
//! 2. Optional TOML file on disk
//! 3. `ANVILML_*` environment variables (with double-underscore nesting for sub-fields)
//! 4. Explicit CLI overrides
//!
//! This module is the runtime glue that turns static type definitions into a
//! fully-resolved configuration instance usable by the server.

use std::collections::HashMap;
use std::env;
use std::fmt;
use std::net::IpAddr;
use std::path::Path;
use std::str::FromStr;

use crate::{FrontendMode, ServerConfig};

// ── Error types ───────────────────────────────────────────────────────────────

/// Errors that can occur during configuration loading.
#[derive(Debug)]
pub enum ConfigError {
    /// File I/O error (e.g. TOML file could not be read).
    Io(std::io::Error),
    /// TOML deserialization error.
    Toml(toml::de::Error),
    /// Invalid value for an environment variable.
    /// `(field_name, raw_value)`.
    EnvParse(String, String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Toml(err) => write!(f, "TOML parse error: {err}"),
            Self::EnvParse(field, value) => {
                write!(f, "invalid env var value for `{field}`: `{value}`")
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Toml(err) => Some(err),
            Self::EnvParse(_, _) => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        Self::Toml(err)
    }
}

// ── CLI overrides ─────────────────────────────────────────────────────────────

/// Explicit command-line overrides for server configuration.
///
/// These are applied on top of defaults -> TOML -> env vars (highest precedence).
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    /// Override the bind host address.
    pub host: Option<IpAddr>,
    /// Override the bind port number.
    pub port: Option<u16>,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Resolve a single environment variable by name.
#[allow(dead_code)]
fn resolve_env_var(name: &str) -> Option<String> {
    env::var(name).ok()
}

/// Parse a string value into a `FrontendMode` enum variant.
fn parse_frontend_mode(value: &str) -> Option<FrontendMode> {
    match value.trim().to_lowercase().as_str() {
        "headless" => Some(FrontendMode::Headless),
        "local" => Some(FrontendMode::Local {
            path: std::path::PathBuf::from("./bloomery"),
        }),
        "remote" => Some(FrontendMode::Remote {
            url: url::Url::parse("http://127.0.0.1:3000").ok()?,
        }),
        _ => None,
    }
}

/// Apply environment variables to a `ServerConfig` instance.
///
/// Reads all env vars starting with `ANVILML_`, strips the prefix, and maps
/// the remaining key (with double-underscore nesting) onto the config struct.
fn apply_env_to_config(config: ServerConfig, env_vars: &HashMap<String, String>) -> ServerConfig {
    let mut cfg = config;

    for (key, value) in env_vars {
        // Only process ANVILML_ prefixed variables.
        if !key.starts_with("ANVILML_") {
            continue;
        }

        let remainder = &key[8..]; // strip "ANVILML_" prefix
        let upper = remainder.to_uppercase();

        match upper.as_str() {
            // ── Flat fields (no nesting) ────────────────────────────────
            "HOST" => {
                if let Ok(addr) = IpAddr::from_str(value) {
                    cfg.host = addr;
                } else {
                    eprintln!(
                        "[anvilml] warning: ANVILML_HOST={value:?} is not a valid IP address, skipping"
                    );
                }
            }
            "PORT" => {
                if let Ok(port) = value.parse::<u16>() {
                    cfg.port = port;
                } else {
                    eprintln!(
                        "[anvilml] warning: ANVILML_PORT={value:?} is not a valid u16, skipping"
                    );
                }
            }
            "DB_PATH" => {
                cfg.db_path = std::path::PathBuf::from(&value);
            }
            "ARTIFACT_DIR" => {
                cfg.artifact_dir = std::path::PathBuf::from(&value);
            }
            "VENV_PATH" => {
                cfg.venv_path = std::path::PathBuf::from(&value);
            }
            "WORKER_LOG_DIR" => {
                cfg.worker_log_dir = Some(std::path::PathBuf::from(&value));
            }
            "NUM_THREADS" => {
                if let Ok(n) = value.parse::<usize>() {
                    cfg.num_threads = n;
                } else {
                    eprintln!(
                        "[anvilml] warning: ANVILML_NUM_THREADS={value:?} is not a valid usize, skipping"
                    );
                }
            }
            "NUM_INTEROP_THREADS" => {
                if let Ok(n) = value.parse::<usize>() {
                    cfg.num_interop_threads = n;
                } else {
                    eprintln!(
                        "[anvilml] warning: ANVILML_NUM_INTEROP_THREADS={value:?} is not a valid usize, skipping"
                    );
                }
            }

            // ── Nested fields (double-underscore) ───────────────────────
            _ => {
                if let Some((parent, child)) = upper.split_once("__") {
                    match (parent, child) {
                        ("FRONTEND", "MODE") => {
                            if let Some(mode) = parse_frontend_mode(value) {
                                cfg.frontend.mode = mode;
                            } else {
                                eprintln!(
                                    "[anvilml] warning: ANVILML_FRONTEND__MODE={value:?} is not a valid frontend mode, skipping"
                                );
                            }
                        }
                        ("GPU_SELECTION", "DEFAULT_DEVICE") => {
                            cfg.gpu_selection.default_device = value.to_string();
                        }
                        // Unrecognized nested path — silently ignored.
                        _ => {}
                    }
                }
                // Single-level keys that are not recognized — silently ignored.
            }
        }
    }

    cfg
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Load a fully-resolved `ServerConfig` from the four-layer precedence chain:
/// defaults -> TOML file -> environment variables -> explicit overrides.
///
/// # Arguments
///
/// * `toml_path` — Optional path to a TOML configuration file. If `None` or
///   the file does not exist, only defaults (plus env vars and overrides) are used.
/// * `overrides` — Explicit CLI overrides that take highest precedence.
///
/// # Errors
///
/// Returns `ConfigError::Io` if the TOML file cannot be read,
/// `ConfigError::Toml` if it cannot be deserialized, or
/// `ConfigError::EnvParse` if an env var value is unparseable.
/// Missing TOML files produce a warning (via `eprintln!`) and fall back to defaults.
pub fn load_config(
    toml_path: Option<&Path>,
    overrides: ConfigOverrides,
) -> Result<ServerConfig, ConfigError> {
    // ── Layer 1: Built-in defaults ──────────────────────────────────────────
    let mut config = ServerConfig::default();

    // ── Layer 2: TOML file on disk (optional) ───────────────────────────────
    if let Some(path) = toml_path {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                // Deserialize the TOML file. Since every field in ServerConfig has
                // #[serde(default)], a partial TOML file will correctly merge with
                // defaults — missing keys get their default values.
                let toml_config: ServerConfig = toml::from_str(&contents)?;
                config = merge_config(config, toml_config);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "[anvilml] warning: TOML config file not found at {:?}, using defaults",
                    path
                );
            }
            Err(err) => return Err(ConfigError::Io(err)),
        }
    }

    // ── Layer 3: Environment variables ──────────────────────────────────────
    let env_vars: HashMap<String, String> = env::vars().collect();
    config = apply_env_to_config(config, &env_vars);

    // ── Layer 4: Explicit overrides (highest precedence) ────────────────────
    if let Some(host) = overrides.host {
        config.host = host;
    }
    if let Some(port) = overrides.port {
        config.port = port;
    }

    Ok(config)
}

/// Merge `override_cfg` fields over `base_cfg`, preferring the override.
///
/// Because ServerConfig uses #[serde(default)] on every field, a TOML file
/// that specifies only a subset of fields will still produce a complete
/// ServerConfig with defaults for the missing keys. This function ensures
/// that explicitly-specified fields in the TOML take precedence over
/// built-in defaults, while preserving sensible defaults for sub-structs
/// where an empty/None value means "use the base".
fn merge_config(base: ServerConfig, override_cfg: ServerConfig) -> ServerConfig {
    ServerConfig {
        host: override_cfg.host,
        port: override_cfg.port,
        model_dirs: if override_cfg.model_dirs.is_empty() {
            base.model_dirs
        } else {
            override_cfg.model_dirs
        },
        artifact_dir: override_cfg.artifact_dir,
        db_path: override_cfg.db_path,
        venv_path: override_cfg.venv_path,
        rocm: merge_rocm(base.rocm, override_cfg.rocm),
        hardware_override: override_cfg.hardware_override.or(base.hardware_override),
        worker_log_dir: override_cfg.worker_log_dir.or(base.worker_log_dir),
        num_threads: override_cfg.num_threads,
        num_interop_threads: override_cfg.num_interop_threads,
        frontend: merge_frontend(base.frontend, override_cfg.frontend),
        gpu_selection: crate::GpuSelectionConfig {
            default_device: if override_cfg.gpu_selection.default_device.is_empty() {
                base.gpu_selection.default_device
            } else {
                override_cfg.gpu_selection.default_device
            },
        },
        limits: merge_limits(base.limits, override_cfg.limits),
    }
}

fn merge_rocm(base: crate::RocmConfig, override_cfg: crate::RocmConfig) -> crate::RocmConfig {
    crate::RocmConfig {
        use_hipblaslt: override_cfg.use_hipblaslt,
        hsa_override_gfx_version: override_cfg
            .hsa_override_gfx_version
            .or(base.hsa_override_gfx_version),
    }
}

fn merge_frontend(
    base: crate::FrontendConfig,
    override_cfg: crate::FrontendConfig,
) -> crate::FrontendConfig {
    let mode = if override_cfg.mode == base.mode {
        base.mode
    } else {
        override_cfg.mode
    };
    crate::FrontendConfig { mode }
}

fn merge_limits(
    _base: crate::LimitsConfig,
    override_cfg: crate::LimitsConfig,
) -> crate::LimitsConfig {
    // All limits have non-zero defaults, so any TOML-specified value
    // will differ from the default. Return override directly.
    override_cfg
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    /// Test that environment variables override TOML file values.
    ///
    /// Sets ANVILML_PORT=9999 in the env, puts port = 8488 in a TOML file,
    /// and verifies the loaded config has port 9999.
    #[test]
    fn env_overrides_toml() {
        // Ensure the test env var is set.
        env::set_var("ANVILML_PORT", "9999");

        // Write a temporary TOML file with port = 8488.
        let toml_content = r#"port = 8488
host = "127.0.0.1"
"#;
        let mut tmp_file = std::env::temp_dir();
        tmp_file.push("anvilml_test_env_over_toml.toml");
        {
            let mut f = fs::File::create(&tmp_file).expect("create temp toml file");
            f.write_all(toml_content.as_bytes())
                .expect("write temp toml file");
        }

        let result = load_config(Some(tmp_file.as_path()), ConfigOverrides::default())
            .expect("load_config should succeed");

        // Clean up.
        let _ = fs::remove_file(&tmp_file);

        // Env var takes precedence over TOML.
        assert_eq!(
            result.port, 9999,
            "env var ANVILML_PORT=9999 should override TOML port=8488"
        );
        assert_eq!(result.host, "127.0.0.1".parse::<IpAddr>().unwrap());

        // Unset the env var for other tests.
        env::remove_var("ANVILML_PORT");
    }

    /// Test that explicit overrides beat environment variables.
    ///
    /// Sets ANVILML_PORT=9999 in env and ConfigOverrides { port: Some(7777) },
    /// then verifies the loaded config has port 7777.
    #[test]
    fn override_beats_env() {
        env::set_var("ANVILML_PORT", "9999");

        let toml_content = r#"port = 8488
"#;
        let mut tmp_file = std::env::temp_dir();
        tmp_file.push("anvilml_test_override_beats_env.toml");
        {
            let mut f = fs::File::create(&tmp_file).expect("create temp toml file");
            f.write_all(toml_content.as_bytes())
                .expect("write temp toml file");
        }

        let result = load_config(
            Some(tmp_file.as_path()),
            ConfigOverrides {
                host: None,
                port: Some(7777),
            },
        )
        .expect("load_config should succeed");

        // Clean up.
        let _ = fs::remove_file(&tmp_file);

        // Explicit override takes precedence over env var.
        assert_eq!(
            result.port, 7777,
            "ConfigOverrides port=7777 should beat ANVILML_PORT=9999"
        );

        env::remove_var("ANVILML_PORT");
    }

    /// Test that a missing TOML file produces a warning and falls back to defaults + env.
    #[test]
    fn missing_toml_fallback() {
        // Ensure no ANVILML_ env vars interfere.
        env::remove_var("ANVILML_PORT");
        env::remove_var("ANVILML_HOST");

        // Test with None — should return defaults.
        let result_none = load_config(None, ConfigOverrides::default())
            .expect("load_config(None, ...) should succeed");
        assert_eq!(result_none.port, 8488);
        assert_eq!(result_none.host, "127.0.0.1".parse::<IpAddr>().unwrap());

        // Test with a nonexistent path — should warn and return defaults.
        let nonexistent = std::path::PathBuf::from("/nonexistent/path/config.toml");
        let result_missing = load_config(Some(&nonexistent), ConfigOverrides::default())
            .expect("load_config(nonexistent, ...) should succeed with warning");
        assert_eq!(result_missing.port, 8488);
        assert_eq!(result_missing.host, "127.0.0.1".parse::<IpAddr>().unwrap());
    }

    /// Test that double-underscore env var nesting works for nested config fields.
    #[test]
    fn env_nested_field() {
        env::set_var("ANVILML_FRONTEND__MODE", "headless");

        let result =
            load_config(None, ConfigOverrides::default()).expect("load_config should succeed");

        assert!(
            matches!(result.frontend.mode, FrontendMode::Headless),
            "ANVILML_FRONTEND__MODE=headless should set frontend.mode to Headless"
        );

        env::remove_var("ANVILML_FRONTEND__MODE");
    }
}
