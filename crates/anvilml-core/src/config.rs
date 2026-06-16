//! Configuration schema for the AnvilML server.
//!
//! Defines `ServerConfig` and all nested configuration structs, with `Default`
//! implementations matching the documented defaults in `ENVIRONMENT.md §4`.
//!
//! `PathBuf` fields use a `path_as_string` helper to serialise as JSON strings,
//! ensuring platform-independent roundtrip through `serde_json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// PathBuf serialisation helpers
//
// `PathBuf` does not implement `serde::Serialize` / `Deserialize` by default.
// We convert to/from `String` so that JSON roundtrips correctly on every
// platform — the JSON string is platform-independent text, and `PathBuf::from`
// parses it back into the platform-native representation.
// ---------------------------------------------------------------------------

mod path_as_string {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::path::PathBuf;

    pub fn serialize<S>(path: &std::path::Path, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert to string for JSON transport; platform-native on roundtrip.
        serializer.serialize_str(path.to_str().unwrap_or_default())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(PathBuf::from(s))
    }
}

/// Configuration for a single model directory.
///
/// Each entry tells the model scanner which filesystem directory to walk
/// and how deeply.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDirConfig {
    /// Directory to scan for models.
    pub path: PathBuf,
    /// Scan subdirectories recursively. Default: `false`.
    #[serde(default)]
    pub recursive: bool,
    /// Maximum scan depth when `recursive = true`. Default: `None` (unlimited).
    #[serde(default)]
    pub max_depth: Option<u32>,
}

/// GPU selection policy.
///
/// Controls which GPU device the server selects for job dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuSelectionConfig {
    /// Device selection mode: `"auto"`, `"cpu"`, or an integer device index as string.
    /// Default: `"auto"`.
    #[serde(default = "default_device")]
    pub default_device: String,
}

fn default_device() -> String {
    "auto".to_string()
}

impl Default for GpuSelectionConfig {
    fn default() -> Self {
        Self {
            default_device: "auto".to_string(),
        }
    }
}

/// Job queue and concurrency limits.
///
/// Controls how many jobs may be queued and how many may run simultaneously
/// across all workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitsConfig {
    /// Maximum jobs allowed in `Queued` state simultaneously. Default: `100`.
    #[serde(default = "default_max_queued_jobs")]
    pub max_queued_jobs: u32,
    /// Maximum jobs dispatched simultaneously (one per GPU). Default: `1`.
    #[serde(default = "default_max_concurrent_jobs")]
    pub max_concurrent_jobs: u32,
}

fn default_max_queued_jobs() -> u32 {
    100
}

fn default_max_concurrent_jobs() -> u32 {
    1
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_queued_jobs: 100,
            max_concurrent_jobs: 1,
        }
    }
}

/// ROCm-specific settings.
///
/// Optional section — present only when explicitly configured. Used to
/// override the `HSA_OVERRIDE_GFX_VERSION` environment variable for GPUs
/// whose GFX version is not yet recognised by the ROCm runtime.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RocmConfig {
    /// Override `HSA_OVERRIDE_GFX_VERSION` for unsupported GFX targets.
    /// Default: `None` (use ROCm's automatic detection).
    #[serde(default)]
    pub hsa_override_gfx_version: Option<String>,
}

/// Hardware override for CI and isolated test environments.
///
/// **NEVER** include this section in a release build or production config.
/// It forces a specific device type and VRAM amount regardless of what
/// the physical hardware reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareOverrideConfig {
    /// Forced device type: `"cuda"`, `"rocm"`, or `"cpu"`. Default: `"cpu"`.
    #[serde(default = "default_device_type")]
    pub device_type: String,
    /// VRAM to report in MiB. Default: `8192`.
    #[serde(default = "default_vram_total_mib")]
    pub vram_total_mib: u32,
}

fn default_device_type() -> String {
    "cpu".to_string()
}

fn default_vram_total_mib() -> u32 {
    8192
}

impl Default for HardwareOverrideConfig {
    fn default() -> Self {
        Self {
            device_type: "cpu".to_string(),
            vram_total_mib: 8192,
        }
    }
}

/// Top-level server configuration.
///
/// Fields map to `anvilml.toml` sections and to `ANVILML_*` environment
/// variable overrides. The `Default` impl provides compiled-in defaults
/// that serve as the base layer in the config precedence chain:
/// defaults < `anvilml.toml` < env vars < CLI flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Bind address. Default: `"127.0.0.1"`.
    pub host: String,
    /// Bind port. Default: `8488`.
    pub port: u16,
    /// SQLite database path. Default: `"./anvilml.db"`.
    #[serde(with = "path_as_string")]
    pub db_path: PathBuf,
    /// Artifact storage directory. Default: `"./artifacts"`.
    #[serde(with = "path_as_string")]
    pub artifact_dir: PathBuf,
    /// Tokio worker thread count. `None` means "use num_cpus". Default: `None`.
    pub num_threads: Option<usize>,
    /// Python venv root path. Default: `"./worker/.venv"`.
    #[serde(with = "path_as_string")]
    pub venv_path: PathBuf,
    /// Maximum IPC message payload size in MiB. Default: `256`.
    pub max_ipc_payload_mib: u32,
    /// Model directory specifications. Empty vec means no directories configured.
    pub model_dirs: Vec<ModelDirConfig>,
    /// GPU selection policy.
    pub gpu_selection: GpuSelectionConfig,
    /// Per-device job queue and concurrency limits.
    pub limits: LimitsConfig,
    /// ROCm-specific settings. `None` when ROCm is not explicitly configured.
    pub rocm: Option<RocmConfig>,
    /// Hardware override for CI/testing. `None` in production builds.
    pub hardware_override: Option<HardwareOverrideConfig>,
    /// SQL seed files directory. Default: `"./database/seeds"`.
    #[serde(with = "path_as_string")]
    pub seeds_path: PathBuf,
    /// Logging level forwarded to worker subprocesses. Default: `"info"`.
    ///
    /// This value is injected as `ANVILML_LOG_LEVEL` into the Python worker
    /// environment so the worker uses the same log level as the server.
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8488,
            db_path: PathBuf::from("./anvilml.db"),
            artifact_dir: PathBuf::from("./artifacts"),
            num_threads: None,
            venv_path: PathBuf::from("./worker/.venv"),
            max_ipc_payload_mib: 256,
            model_dirs: Vec::new(),
            gpu_selection: GpuSelectionConfig::default(),
            limits: LimitsConfig::default(),
            rocm: None,
            hardware_override: None,
            seeds_path: PathBuf::from("./database/seeds"),
            log_level: "info".to_string(),
        }
    }
}
