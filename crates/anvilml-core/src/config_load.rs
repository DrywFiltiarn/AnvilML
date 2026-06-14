//! Configuration loading with layered precedence.
//!
//! Provides `load()` which implements the four-level config precedence chain:
//! compiled-in defaults → `anvilml.toml` → `ANVILML_*` env vars → `ConfigOverrides` (CLI).
//!
//! Each level starts from the previous result, ensuring correct precedence without
//! manual field-by-field merging.

use std::path::Path;

use crate::config::ServerConfig;
use crate::error::AnvilError;

/// CLI-overridable configuration fields.
///
/// Carries only the fields that can be overridden from the command line.
/// Other `ServerConfig` fields are overridden exclusively via `ANVILML_*`
/// environment variables. Uses `Option` so the struct can be `Default`-
/// initialised when no CLI overrides are provided.
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    /// Override bind address. `None` means use value from lower precedence levels.
    pub host: Option<String>,
    /// Override bind port. `None` means use value from lower precedence levels.
    pub port: Option<u16>,
}

/// Apply `ANVILML_*` environment variable overrides to a config.
///
/// Reads each configured environment variable; if present, replaces the
/// matching field in the config. Unset variables are silently skipped.
///
/// Nested config fields use double-underscore nesting (e.g.
/// `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` → `gpu_selection.default_device`),
/// as documented in `ENVIRONMENT.md §3`.
///
/// Returns the config with all overrides applied, or an `AnvilError::EnvVar`
/// if any variable contains an unparseable value.
fn apply_env_overrides(mut cfg: ServerConfig) -> Result<ServerConfig, AnvilError> {
    if let Ok(val) = std::env::var("ANVILML_HOST") {
        cfg.host = val;
    }

    if let Ok(val) = std::env::var("ANVILML_PORT") {
        cfg.port = val.parse().map_err(|_| AnvilError::EnvVar {
            name: "ANVILML_PORT".to_string(),
            value: val,
        })?;
    }

    if let Ok(val) = std::env::var("ANVILML_DB_PATH") {
        cfg.db_path = std::path::PathBuf::from(val);
    }

    if let Ok(val) = std::env::var("ANVILML_ARTIFACT_DIR") {
        cfg.artifact_dir = std::path::PathBuf::from(val);
    }

    if let Ok(val) = std::env::var("ANVILML_VENV_PATH") {
        cfg.venv_path = std::path::PathBuf::from(val);
    }

    if let Ok(val) = std::env::var("ANVILML_SEEDS_PATH") {
        cfg.seeds_path = std::path::PathBuf::from(val);
    }

    if let Ok(val) = std::env::var("ANVILML_MAX_IPC_PAYLOAD_MIB") {
        cfg.max_ipc_payload_mib = val.parse().map_err(|_| AnvilError::EnvVar {
            name: "ANVILML_MAX_IPC_PAYLOAD_MIB".to_string(),
            value: val,
        })?;
    }

    if let Ok(val) = std::env::var("ANVILML_NUM_THREADS") {
        cfg.num_threads = Some(val.parse().map_err(|_| AnvilError::EnvVar {
            name: "ANVILML_NUM_THREADS".to_string(),
            value: val,
        })?);
    }

    // Nested field: gpu_selection.default_device uses double-underscore nesting.
    if let Ok(val) = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE") {
        cfg.gpu_selection.default_device = val;
    }

    Ok(cfg)
}

/// Load `ServerConfig` from a TOML file with layered precedence.
///
/// Implements the four-level config precedence chain (lowest to highest):
/// 1. **Compiled-in defaults** (`ServerConfig::default()`)
/// 2. **TOML file** at `path` — present fields override defaults; missing fields
///    keep their default values via serde's `#[serde(default)]` handling.
/// 3. **`ANVILML_*` environment variables** — override all fields from levels 1–2.
/// 4. **`ConfigOverrides`** — `host` and `port` from CLI flags override everything.
///
/// If the TOML file does not exist, it is skipped silently and defaults are used.
/// This is intentional: the file is optional, and defaults serve as the base layer.
///
/// # Arguments
///
/// * `path` — Path to the TOML configuration file.
/// * `overrides` — CLI overrides applied at the highest precedence level.
///
/// # Errors
///
/// Returns `AnvilError::EnvVar` if any environment variable contains an
/// unparseable value. Returns `AnvilError::Io` if the TOML file exists but
/// cannot be read.
pub fn load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig, AnvilError> {
    // Level 1: Start with compiled-in defaults.
    let mut cfg = ServerConfig::default();

    // Level 2: Try to read and parse the TOML file.
    // If the file doesn't exist, skip silently — defaults remain.
    if let Ok(content) = std::fs::read_to_string(path) {
        // The toml crate's from_str uses serde's Deserialize impl on ServerConfig.
        // Fields present in the TOML override defaults; missing fields get defaults
        // via serde's #[serde(default)] / Default trait on nested structs.
        cfg = toml::from_str::<ServerConfig>(&content)?;
    }

    // Level 3: Apply environment variable overrides.
    cfg = apply_env_overrides(cfg)?;

    // Level 4: Apply CLI overrides last — highest precedence.
    if let Some(host) = &overrides.host {
        cfg.host = host.clone();
    }
    if let Some(port) = overrides.port {
        cfg.port = port;
    }

    Ok(cfg)
}
