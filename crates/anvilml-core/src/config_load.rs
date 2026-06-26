//! Config loading — layered precedence for `ServerConfig`.
//!
//! Implements all four layers of the four-layer config precedence chain
//! defined in `ANVILML_DESIGN.md §15`:
//! 1. Compiled-in defaults (`ServerConfig::default()`)
//! 2. `anvilml.toml` file (optional, field-by-field override)
//! 3. `ANVILML_*` environment variables (with `__` nested-field convention)
//! 4. CLI flags (`CliOverrides` struct, highest precedence)

use std::path::{Path, PathBuf};

use crate::config::{HardwareOverrideConfig, ModelDirConfig, RocmConfig};
use crate::{AnvilError, ServerConfig};

/// CLI flag overrides for config loading.
///
/// Applied as the final (highest-precedence) layer after environment variables.
/// Only `host` and `port` are exposed here — other fields are overridden via env vars.
///
/// Constructed by the binary's CLI parser (P2-A6) and passed to `load()` as the
/// fourth layer of the config precedence chain.
#[derive(Debug, Clone)]
pub struct CliOverrides {
    /// HTTP bind address override. `None` means no override.
    pub host: Option<String>,
    /// HTTP port override. `None` means no override.
    pub port: Option<u16>,
}

/// Load `ServerConfig` by merging compiled-in defaults with an optional TOML file,
/// then applying environment variable overrides, and finally CLI flag overrides.
///
/// This implements the complete four-layer config precedence chain:
/// defaults → TOML → environment variables → CLI flags. Missing fields in each
/// layer retain their value from the prior layer — the merge is field-by-field,
/// not structural replacement.
///
/// # Arguments
///
/// * `toml_path` — Optional path to a TOML config file. If `None`, resolves to
///   `./anvilml.toml` relative to the process working directory. If that file
///   does not exist, the function returns the defaults without error.
/// * `cli_overrides` — Optional CLI flag overrides applied as the final layer.
///   Only `host` and `port` fields are applied; all other fields are overridden
///   via environment variables.
///
/// # Environment Variables
///
/// Each `ANVILML_*` variable overrides the matching config field. Nested fields
/// use double underscores (`__`) to separate struct and field names (e.g.
/// `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` → `gpu_selection.default_device`).
/// Unset variables leave the prior layer's value intact.
///
/// # Errors
///
/// Returns `AnvilError::Io` if the TOML file cannot be read, or `AnvilError::Serde`
/// if the TOML is syntactically invalid or contains fields that cannot be deserialized
/// into the matching `ServerConfig` type. Env var parse failures are silently skipped
/// (the prior layer's value is retained).
pub fn load(
    toml_path: Option<&Path>,
    cli_overrides: Option<CliOverrides>,
) -> Result<ServerConfig, AnvilError> {
    // Start with compiled-in defaults as the base layer.
    let mut config = ServerConfig::default();

    // Resolve the TOML file path: use the caller-provided path or the default.
    let path = toml_path
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from("./anvilml.toml"));

    // If the TOML file exists, read and apply it. If not, skip to env var overrides.
    if path.exists() {
        // Read the file contents; I/O errors propagate via `?` into `AnvilError::Io`.
        let contents = std::fs::read_to_string(&path)?;

        // Parse into an untyped `toml::Value` first so we can inspect which fields
        // were explicitly present in the TOML. This avoids the pitfall of `toml`
        // deserializing missing fields to their type defaults (e.g. `""` for String,
        // `0` for integers), which would overwrite the compiled-in defaults.
        let value: toml::Value =
            toml::from_str(&contents).map_err(|e| AnvilError::Serde(e.to_string()))?;

        let table = value
            .as_table()
            .expect("root of a TOML file must be a table");

        // Override scalar fields only if explicitly present in the TOML file.
        if let Some(host) = table.get("host").and_then(|v| v.as_str()) {
            config.host = host.to_string();
        }
        if let Some(port) = table.get("port").and_then(|v| v.as_integer()) {
            config.port = port as u16;
        }
        if let Some(db_path) = table.get("db_path").and_then(|v| v.as_str()) {
            config.db_path = PathBuf::from(db_path);
        }
        if let Some(artifact_dir) = table.get("artifact_dir").and_then(|v| v.as_str()) {
            config.artifact_dir = PathBuf::from(artifact_dir);
        }
        if let Some(venv_path) = table.get("venv_path").and_then(|v| v.as_str()) {
            config.venv_path = PathBuf::from(venv_path);
        }
        if let Some(model_scan_depth) = table.get("model_scan_depth").and_then(|v| v.as_integer()) {
            config.model_scan_depth = model_scan_depth as u32;
        }
        if let Some(max_ipc_payload_mib) = table
            .get("max_ipc_payload_mib")
            .and_then(|v| v.as_integer())
        {
            config.max_ipc_payload_mib = max_ipc_payload_mib as u32;
        }
        if let Some(num_threads) = table.get("num_threads").and_then(|v| v.as_integer()) {
            config.num_threads = Some(num_threads as u32);
        }

        // Override nested structs only if present in the TOML.
        apply_model_dirs(table, &mut config);
        apply_gpu_selection(table, &mut config);
        apply_limits(table, &mut config);
        apply_rocm(table, &mut config);
        apply_hardware_override(table, &mut config);
    }

    // Layer 3: Apply `ANVILML_*` environment variable overrides.
    // Unset variables silently leave the prior layer's value (defaults or TOML).
    apply_env_vars(&mut config);

    // Layer 4: Apply CLI flag overrides as the highest-precedence layer.
    apply_cli_overrides(&mut config, cli_overrides);

    Ok(config)
}

/// Apply `ANVILML_*` environment variable overrides to `config`.
///
/// Reads each variable from `std::env::var()`, parses `__`-separated nested keys,
/// and overrides the matching `ServerConfig` field. Only the env vars listed in
/// `ENVIRONMENT.md §3` are recognized; all others are silently ignored.
///
/// Parse failures (e.g. `ANVILML_PORT = "abc"`) are silently skipped — the prior
/// layer's value is retained. This is intentional: a misconfigured env var should
/// not crash the server; it should fall back to the TOML or default value.
fn apply_env_vars(config: &mut ServerConfig) {
    // Read `ANVILML_HOST` — string field, no parse needed.
    if let Ok(value) = std::env::var("ANVILML_HOST") {
        config.host = value;
    }

    // Read `ANVILML_PORT` — u16 parse; skip on failure (retain prior layer).
    if let Ok(value) = std::env::var("ANVILML_PORT")
        && let Ok(port) = value.parse::<u16>()
    {
        config.port = port;
    }

    // Read `ANVILML_DB_PATH` — path from string.
    if let Ok(value) = std::env::var("ANVILML_DB_PATH") {
        config.db_path = PathBuf::from(value);
    }

    // Read `ANVILML_ARTIFACT_DIR` — path from string.
    if let Ok(value) = std::env::var("ANVILML_ARTIFACT_DIR") {
        config.artifact_dir = PathBuf::from(value);
    }

    // Read `ANVILML_VENV_PATH` — path from string.
    if let Ok(value) = std::env::var("ANVILML_VENV_PATH") {
        config.venv_path = PathBuf::from(value);
    }

    // Read `ANVILML_MODEL_SCAN_DEPTH` — u32 parse; skip on failure.
    if let Ok(value) = std::env::var("ANVILML_MODEL_SCAN_DEPTH")
        && let Ok(depth) = value.parse::<u32>()
    {
        config.model_scan_depth = depth;
    }

    // Read `ANVILML_MAX_IPC_PAYLOAD_MIB` — u32 parse; skip on failure.
    if let Ok(value) = std::env::var("ANVILML_MAX_IPC_PAYLOAD_MIB")
        && let Ok(mib) = value.parse::<u32>()
    {
        config.max_ipc_payload_mib = mib;
    }

    // Read `ANVILML_NUM_THREADS` — Option<u32> parse; skip on failure.
    if let Ok(value) = std::env::var("ANVILML_NUM_THREADS")
        && let Ok(threads) = value.parse::<u32>()
    {
        config.num_threads = Some(threads);
    }

    // Read `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` — nested field via `__` separator.
    // The `__` convention: first part is the uppercase struct name, second is the
    // uppercase field name. We check explicitly to avoid false positives from
    // unrelated `ANVILML_*` env vars containing `__`.
    if let Ok(value) = std::env::var("ANVILML_GPU_SELECTION__DEFAULT_DEVICE") {
        config.gpu_selection.default_device = value;
    }
}

/// Apply CLI flag overrides as the highest-precedence config layer.
///
/// Only `host` and `port` fields are overridden from `CliOverrides`. All other
/// fields are already set by the prior layers (defaults, TOML, env vars).
fn apply_cli_overrides(config: &mut ServerConfig, cli_overrides: Option<CliOverrides>) {
    // Apply CLI overrides only if provided — `None` means no CLI flags were set,
    // so the env var / TOML / default value is the correct final value.
    if let Some(overrides) = cli_overrides {
        // Override host if the caller explicitly set a CLI --host flag.
        if let Some(host) = overrides.host {
            config.host = host;
        }
        // Override port if the caller explicitly set a CLI --port flag.
        if let Some(port) = overrides.port {
            config.port = port;
        }
    }
}

/// Apply `[[model_dirs]]` array entries from the TOML table into `config`.
///
/// Each array element is a table with `path` (required), `recursive` (default `false`),
/// and `max_depth` (optional). Only present fields are set; absent fields use defaults.
fn apply_model_dirs(table: &toml::Table, config: &mut ServerConfig) {
    if let Some(model_dirs_val) = table.get("model_dirs").and_then(|v| v.as_array()) {
        for item in model_dirs_val {
            if let Some(item_table) = item.as_table() {
                let path = if let Some(p) = item_table.get("path").and_then(|v| v.as_str()) {
                    PathBuf::from(p)
                } else {
                    continue; // Skip entries without a path — not valid.
                };
                let recursive = item_table
                    .get("recursive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let max_depth = item_table
                    .get("max_depth")
                    .and_then(|v| v.as_integer())
                    .map(|v| v as u32);

                config.model_dirs.push(ModelDirConfig {
                    path,
                    recursive,
                    max_depth,
                });
            }
        }
    }
}

/// Apply `[gpu_selection]` fields from the TOML table into `config`.
///
/// Only the `default_device` field is overridden; all other GPU selection fields
/// (currently none beyond `default_device`) keep their defaults.
fn apply_gpu_selection(table: &toml::Table, config: &mut ServerConfig) {
    // Collapse nested if-let for gpu_selection.default_device override.
    if let Some(gpu) = table.get("gpu_selection").and_then(|v| v.as_table())
        && let Some(default_device) = gpu.get("default_device").and_then(|v| v.as_str())
    {
        config.gpu_selection.default_device = default_device.to_string();
    }
}

/// Apply `[limits]` fields from the TOML table into `config`.
///
/// Only the `max_queued_jobs` field is overridden; missing fields keep defaults.
fn apply_limits(table: &toml::Table, config: &mut ServerConfig) {
    // Collapse nested if-let for limits.max_queued_jobs override.
    if let Some(limits) = table.get("limits").and_then(|v| v.as_table())
        && let Some(max_queued_jobs) = limits.get("max_queued_jobs").and_then(|v| v.as_integer())
    {
        config.limits.max_queued_jobs = max_queued_jobs as u32;
    }
}

/// Apply optional `[rocm]` section from the TOML table into `config`.
///
/// If the `[rocm]` section is absent, `config.rocm` remains `None`. If present,
/// the `hsa_override_gfx_version` field is set; if absent within the section,
/// it remains `None`.
fn apply_rocm(table: &toml::Table, config: &mut ServerConfig) {
    if let Some(rocm) = table.get("rocm").and_then(|v| v.as_table()) {
        // Create a RocmConfig with the field from TOML (or None if absent).
        let hsa_override = rocm
            .get("hsa_override_gfx_version")
            .and_then(|v| v.as_str())
            .map(String::from);
        config.rocm = Some(RocmConfig {
            hsa_override_gfx_version: hsa_override,
        });
    }
}

/// Apply optional `[hardware_override]` section from the TOML table into `config`.
///
/// If the `[hardware_override]` section is absent, `config.hardware_override`
/// remains `None`. If present, both `device_type` and `vram_total_mib` are
/// read from the TOML; missing fields within the section use defaults.
fn apply_hardware_override(table: &toml::Table, config: &mut ServerConfig) {
    if let Some(hw) = table.get("hardware_override").and_then(|v| v.as_table()) {
        let device_type = hw
            .get("device_type")
            .and_then(|v| v.as_str())
            .unwrap_or("cpu")
            .to_string();
        let vram_total_mib = hw
            .get("vram_total_mib")
            .and_then(|v| v.as_integer())
            .unwrap_or(0) as u32;
        config.hardware_override = Some(HardwareOverrideConfig {
            device_type,
            vram_total_mib,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `load(None, None)` with a nonexistent default path returns `Ok(defaults)`.
    #[test]
    fn test_load_none_path_missing_file_returns_defaults() {
        let result = load(None, None);
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8488);
    }
}
