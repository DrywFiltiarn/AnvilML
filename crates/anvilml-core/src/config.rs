use std::path::PathBuf;

/// Configuration for a single model directory entry.
///
/// Used as an element of `ServerConfig::model_dirs`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelDirConfig {
    /// Directory path to scan for models.
    pub path: PathBuf,
    /// Whether to scan subdirectories recursively.
    pub recursive: bool,
    /// Maximum scan depth when `recursive = true`. Caps at
    /// `ServerConfig::model_scan_depth` if both are set.
    pub max_depth: Option<u32>,
}

/// GPU selection preferences.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuSelectionConfig {
    /// Default device selector: `"auto"`, `"cpu"`, or integer device index as string.
    pub default_device: String,
}

/// Resource limits for the scheduler.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LimitsConfig {
    /// Maximum jobs allowed in `Queued` state simultaneously.
    pub max_queued_jobs: u32,
}

/// Optional ROCm configuration overrides.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RocmConfig {
    /// Override `HSA_OVERRIDE_GFX_VERSION` for unsupported GFX targets.
    pub hsa_override_gfx_version: Option<String>,
}

/// Optional hardware override for CI and isolated testing.
///
/// NEVER include in a release build or production config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HardwareOverrideConfig {
    /// Device type: `"cuda"`, `"rocm"`, or `"cpu"`.
    pub device_type: String,
    /// VRAM to report in MiB.
    pub vram_total_mib: u32,
}

/// Top-level server configuration with compiled-in defaults.
///
/// Fields are loaded through a four-layer precedence chain:
/// defaults → TOML → environment variables → CLI flags.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    /// Bind address for the HTTP server.
    pub host: String,
    /// HTTP server port.
    pub port: u16,
    /// SQLite database file path.
    pub db_path: PathBuf,
    /// Directory for generated image artifacts.
    pub artifact_dir: PathBuf,
    /// Python virtualenv root for worker processes.
    pub venv_path: PathBuf,
    /// Non-recursive model scanner depth.
    pub model_scan_depth: u32,
    /// Maximum IPC message payload in MiB.
    pub max_ipc_payload_mib: u32,
    /// Tokio worker thread count. `None` = auto (num_cpus).
    pub num_threads: Option<u32>,
    /// Model directories to scan.
    pub model_dirs: Vec<ModelDirConfig>,
    /// GPU selection preferences.
    pub gpu_selection: GpuSelectionConfig,
    /// Resource limits.
    pub limits: LimitsConfig,
    /// Optional ROCm configuration.
    pub rocm: Option<RocmConfig>,
    /// Optional hardware override for CI/testing.
    pub hardware_override: Option<HardwareOverrideConfig>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8488,
            db_path: PathBuf::from("./anvilml.db"),
            artifact_dir: PathBuf::from("./artifacts"),
            venv_path: PathBuf::from("./worker/.venv"),
            model_scan_depth: 2,
            max_ipc_payload_mib: 256,
            num_threads: None,
            // Nested table defaults (P2-A3):
            model_dirs: Vec::new(),
            gpu_selection: GpuSelectionConfig {
                default_device: "auto".to_string(),
            },
            limits: LimitsConfig {
                max_queued_jobs: 100,
            },
            rocm: None,
            hardware_override: None,
        }
    }
}
