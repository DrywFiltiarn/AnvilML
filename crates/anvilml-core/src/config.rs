//! Configuration types for the AnvilML server.
//!
//! Defines all configuration structs and enums specified in ANVILML_DESIGN.md §3.1
//! with proper Default implementations, serde derives, and #[serde(default)] annotations.

use std::net::IpAddr;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Kind of a model file in the model directory.
pub type Url = url::Url;

/// Kind of a model file in the model directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Clip,
    Diffusion,
    Vae,
    Lora,
    ControlNet,
    Unet,
    #[default]
    Upscale,
}

/// Hardware device type for inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Cuda,
    Rocm,
    #[default]
    Cpu,
}

/// Configuration for model directories.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelDirConfig {
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default)]
    pub kind: Option<ModelKind>,
}

/// ROCm-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocmConfig {
    /// Use hipBLASLT for ROCm operations. Maps to ROCBLAS_USE_HIPBLASLT=1.
    #[serde(default = "default_rocm_use_hipblaslt")]
    pub use_hipblaslt: bool,
    /// Override HSA gfx version string (e.g. "10.3.0") for unsupported GPUs.
    #[serde(default)]
    pub hsa_override_gfx_version: Option<String>,
}

impl Default for RocmConfig {
    fn default() -> Self {
        Self {
            use_hipblaslt: true,
            hsa_override_gfx_version: None,
        }
    }
}

fn default_rocm_use_hipblaslt() -> bool {
    true
}

/// Hardware override configuration (bypasses auto-detection).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HardwareOverrideConfig {
    #[serde(default)]
    pub device_type: DeviceType,
    #[serde(default)]
    pub vram_total_mib: u32,
}

/// Frontend serving mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FrontendMode {
    /// Serve static files from a local directory (for a custom/third-party frontend).
    /// Not used for BloomeryUI, which SindriStudio runs as a separate server.
    Local { path: PathBuf },
    /// Reverse-proxy non-API requests to a remote frontend dev server / host.
    Remote { url: Url },
    /// Serve no frontend; API-only.
    #[default]
    Headless,
}

/// Frontend configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FrontendConfig {
    #[serde(default)]
    pub mode: FrontendMode,
}

/// GPU selection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSelectionConfig {
    /// "auto" = scheduler fitness algorithm; "cpu" = force CPU worker; else a device index.
    #[serde(default = "default_gpu_selection")]
    pub default_device: String,
}

impl Default for GpuSelectionConfig {
    fn default() -> Self {
        Self {
            default_device: "auto".to_string(),
        }
    }
}

fn default_gpu_selection() -> String {
    "auto".to_string()
}

/// Resource limits configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    #[serde(default = "default_max_ipc_payload_mib")]
    pub max_ipc_payload_mib: u32,
    #[serde(default = "default_list_default_limit")]
    pub list_default_limit: u32,
    #[serde(default = "default_list_max_limit")]
    pub list_max_limit: u32,
    #[serde(default = "default_ws_broadcast_capacity")]
    pub ws_broadcast_capacity: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_ipc_payload_mib: 64,
            list_default_limit: 100,
            list_max_limit: 1000,
            ws_broadcast_capacity: 256,
        }
    }
}

fn default_max_ipc_payload_mib() -> u32 {
    64
}

fn default_list_default_limit() -> u32 {
    100
}

fn default_list_max_limit() -> u32 {
    1000
}

fn default_ws_broadcast_capacity() -> usize {
    256
}

/// Top-level server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub model_dirs: Vec<ModelDirConfig>,
    #[serde(default = "default_artifact_dir")]
    pub artifact_dir: PathBuf,
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    #[serde(default = "default_venv_path")]
    pub venv_path: PathBuf,
    #[serde(default)]
    pub rocm: RocmConfig,
    #[serde(default)]
    pub hardware_override: Option<HardwareOverrideConfig>,
    #[serde(default = "default_worker_log_dir")]
    pub worker_log_dir: Option<PathBuf>,
    #[serde(default = "default_num_threads")]
    pub num_threads: usize,
    #[serde(default = "default_num_interop_threads")]
    pub num_interop_threads: usize,
    #[serde(default)]
    pub frontend: FrontendConfig,
    #[serde(default)]
    pub gpu_selection: GpuSelectionConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
}

fn default_host() -> IpAddr {
    "127.0.0.1".parse().unwrap()
}

fn default_port() -> u16 {
    8488
}

fn default_artifact_dir() -> PathBuf {
    PathBuf::from("./artifacts")
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./anvilml.db")
}

fn default_venv_path() -> PathBuf {
    PathBuf::from("./venv")
}

fn default_worker_log_dir() -> Option<PathBuf> {
    Some(PathBuf::from("./logs"))
}

fn default_num_threads() -> usize {
    14
}

fn default_num_interop_threads() -> usize {
    4
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            model_dirs: Vec::new(),
            artifact_dir: default_artifact_dir(),
            db_path: default_db_path(),
            venv_path: default_venv_path(),
            rocm: RocmConfig::default(),
            hardware_override: None,
            worker_log_dir: default_worker_log_dir(),
            num_threads: default_num_threads(),
            num_interop_threads: default_num_interop_threads(),
            frontend: FrontendConfig::default(),
            gpu_selection: GpuSelectionConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_roundtrip() {
        let config = ServerConfig {
            host: "192.168.1.10".parse().unwrap(),
            port: 9999,
            model_dirs: vec![
                ModelDirConfig {
                    path: PathBuf::from("./models/diffusion"),
                    kind: Some(ModelKind::Diffusion),
                },
                ModelDirConfig {
                    path: PathBuf::from("./models/vae"),
                    kind: Some(ModelKind::Vae),
                },
            ],
            artifact_dir: PathBuf::from("./artifacts"),
            db_path: PathBuf::from("./anvilml.db"),
            venv_path: PathBuf::from("./venv"),
            rocm: RocmConfig {
                use_hipblaslt: false,
                hsa_override_gfx_version: Some("10.3.0".to_string()),
            },
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cuda,
                vram_total_mib: 16384,
            }),
            worker_log_dir: Some(PathBuf::from("./logs")),
            num_threads: 8,
            num_interop_threads: 2,
            frontend: FrontendConfig {
                mode: FrontendMode::Headless,
            },
            gpu_selection: GpuSelectionConfig {
                default_device: "cpu".to_string(),
            },
            limits: LimitsConfig {
                max_ipc_payload_mib: 128,
                list_default_limit: 50,
                list_max_limit: 500,
                ws_broadcast_capacity: 512,
            },
        };

        // Serialize to TOML
        let toml_str =
            toml::ser::to_string_pretty(&config).expect("serialize ServerConfig to TOML");

        // Deserialize back
        let parsed: ServerConfig =
            toml::from_str(&toml_str).expect("deserialize ServerConfig from TOML");

        // Verify all fields round-trip correctly
        assert_eq!(parsed.host, config.host);
        assert_eq!(parsed.port, config.port);
        assert_eq!(parsed.model_dirs.len(), config.model_dirs.len());
        assert_eq!(parsed.model_dirs[0].path, config.model_dirs[0].path);
        assert_eq!(parsed.model_dirs[0].kind, config.model_dirs[0].kind);
        assert_eq!(parsed.model_dirs[1].path, config.model_dirs[1].path);
        assert_eq!(parsed.model_dirs[1].kind, config.model_dirs[1].kind);
        assert_eq!(parsed.artifact_dir, config.artifact_dir);
        assert_eq!(parsed.db_path, config.db_path);
        assert_eq!(parsed.venv_path, config.venv_path);
        assert_eq!(parsed.rocm.use_hipblaslt, config.rocm.use_hipblaslt);
        assert_eq!(
            parsed.rocm.hsa_override_gfx_version,
            config.rocm.hsa_override_gfx_version
        );
        assert_eq!(
            parsed.hardware_override.as_ref().map(|h| h.device_type),
            config.hardware_override.as_ref().map(|h| h.device_type)
        );
        assert_eq!(
            parsed.hardware_override.as_ref().map(|h| h.vram_total_mib),
            config.hardware_override.as_ref().map(|h| h.vram_total_mib)
        );
        assert_eq!(parsed.worker_log_dir, config.worker_log_dir);
        assert_eq!(parsed.num_threads, config.num_threads);
        assert_eq!(parsed.num_interop_threads, config.num_interop_threads);
        assert_eq!(parsed.frontend.mode, config.frontend.mode);
        assert_eq!(
            parsed.gpu_selection.default_device,
            config.gpu_selection.default_device
        );
        assert_eq!(
            parsed.limits.max_ipc_payload_mib,
            config.limits.max_ipc_payload_mib
        );
        assert_eq!(
            parsed.limits.list_default_limit,
            config.limits.list_default_limit
        );
        assert_eq!(parsed.limits.list_max_limit, config.limits.list_max_limit);
        assert_eq!(
            parsed.limits.ws_broadcast_capacity,
            config.limits.ws_broadcast_capacity
        );
    }

    #[test]
    fn test_default_server_config() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(config.port, 8488);
        assert!(config.model_dirs.is_empty());
        assert_eq!(config.artifact_dir, PathBuf::from("./artifacts"));
        assert_eq!(config.db_path, PathBuf::from("./anvilml.db"));
        assert_eq!(config.venv_path, PathBuf::from("./venv"));
        assert!(config.rocm.use_hipblaslt);
        assert!(config.hardware_override.is_none());
        assert_eq!(config.worker_log_dir, Some(PathBuf::from("./logs")));
        assert_eq!(config.num_threads, 14);
        assert_eq!(config.num_interop_threads, 4);
        assert!(matches!(config.frontend.mode, FrontendMode::Headless));
        assert_eq!(config.gpu_selection.default_device, "auto");
        assert_eq!(config.limits.max_ipc_payload_mib, 64);
        assert_eq!(config.limits.list_default_limit, 100);
        assert_eq!(config.limits.list_max_limit, 1000);
        assert_eq!(config.limits.ws_broadcast_capacity, 256);
    }

    #[test]
    fn test_empty_toml_uses_defaults() {
        let empty = "";
        let config: ServerConfig = toml::from_str(empty).expect("empty TOML parses");
        assert_eq!(config.host, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(config.port, 8488);
        assert!(config.model_dirs.is_empty());
    }

    #[test]
    fn test_model_kind_default() {
        let kind: ModelKind = Default::default();
        assert_eq!(kind, ModelKind::Upscale);
    }

    #[test]
    fn test_device_type_default() {
        let dtype: DeviceType = Default::default();
        assert_eq!(dtype, DeviceType::Cpu);
    }
}
