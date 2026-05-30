//! Configuration types for AnvilML.
//!
//! All types are pure serializable data — no I/O, no async. They match the
//! field names, types, and defaults specified in `ANVILML_DESIGN.md §3.1`.

use std::net::IpAddr;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Forward-compatible placeholder enums (to be replaced by canonical types from
// P2-A3 / P2-A4 when those tasks are implemented).
// ---------------------------------------------------------------------------

/// Model kind — matches the MVP set from ANVILML_DESIGN.md §4.2.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Clip,
    Diffusion,
    Vae,
    Lora,
    ControlNet,
    Unet,
    Upscale,
}

/// Device type — matches the MVP set from ANVILML_DESIGN.md §4.3.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DeviceType {
    Cuda,
    Rocm,
    Cpu,
}

// ---------------------------------------------------------------------------
// Config structs
// ---------------------------------------------------------------------------

/// Top-level server configuration.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
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

/// A model directory entry.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ModelDirConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub kind: Option<ModelKind>,
}

/// ROCm backend settings.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct RocmConfig {
    #[serde(default = "default_rocm_use_hipblaslt")]
    pub use_hipblaslt: bool,
    #[serde(default)]
    pub hsa_override_gfx_version: Option<String>,
}

impl Default for RocmConfig {
    fn default() -> Self {
        Self {
            use_hipblaslt: default_rocm_use_hipblaslt(),
            hsa_override_gfx_version: None,
        }
    }
}

/// Synthetic hardware override for testing / CI.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct HardwareOverrideConfig {
    #[serde(default = "default_device_type")]
    pub device_type: DeviceType,
    #[serde(default = "default_vram_total_mib")]
    pub vram_total_mib: u32,
}

impl Default for HardwareOverrideConfig {
    fn default() -> Self {
        Self {
            device_type: default_device_type(),
            vram_total_mib: default_vram_total_mib(),
        }
    }
}

/// Frontend serving configuration.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct FrontendConfig {
    #[serde(default)]
    pub mode: FrontendMode,
}

/// How the frontend is served.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "mode")]
pub enum FrontendMode {
    /// Serve static files from a local directory (default: ./bloomery).
    Local {
        #[serde(default = "default_frontend_path")]
        path: PathBuf,
    },
    /// Reverse-proxy non-API requests to a remote frontend dev server.
    Remote { url: String },
    /// Serve no frontend; API-only.
    Headless,
}

impl Default for FrontendMode {
    fn default() -> Self {
        FrontendMode::Local {
            path: default_frontend_path(),
        }
    }
}

/// GPU device selection policy.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GpuSelectionConfig {
    #[serde(default = "default_gpu_default_device")]
    pub default_device: String,
}

impl Default for GpuSelectionConfig {
    fn default() -> Self {
        Self {
            default_device: default_gpu_default_device(),
        }
    }
}

/// IPC and API limits.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
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
            max_ipc_payload_mib: default_max_ipc_payload_mib(),
            list_default_limit: default_list_default_limit(),
            list_max_limit: default_list_max_limit(),
            ws_broadcast_capacity: default_ws_broadcast_capacity(),
        }
    }
}

// ---------------------------------------------------------------------------
// Default value helpers
// ---------------------------------------------------------------------------

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

fn default_frontend_path() -> PathBuf {
    PathBuf::from("./bloomery")
}

fn default_rocm_use_hipblaslt() -> bool {
    true
}

fn default_device_type() -> DeviceType {
    DeviceType::Cpu
}

fn default_vram_total_mib() -> u32 {
    8192
}

fn default_gpu_default_device() -> String {
    "auto".to_string()
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip test: construct a ServerConfig with all fields set to
    /// non-default values, serialize to TOML, deserialize back, and assert
    /// equality.
    #[test]
    fn config_round_trip() {
        let config = ServerConfig {
            host: "0.0.0.0".parse().unwrap(),
            port: 9000,
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
            artifact_dir: PathBuf::from("./my_artifacts"),
            db_path: PathBuf::from("./my_server.db"),
            venv_path: PathBuf::from("./my_venv"),
            rocm: RocmConfig {
                use_hipblaslt: false,
                hsa_override_gfx_version: Some("10.3.0".to_string()),
            },
            hardware_override: Some(HardwareOverrideConfig {
                device_type: DeviceType::Cuda,
                vram_total_mib: 16384,
            }),
            worker_log_dir: Some(PathBuf::from("./my_logs")),
            num_threads: 32,
            num_interop_threads: 8,
            frontend: FrontendConfig {
                mode: FrontendMode::Remote {
                    url: "http://localhost:5173".to_string(),
                },
            },
            gpu_selection: GpuSelectionConfig {
                default_device: "0".to_string(),
            },
            limits: LimitsConfig {
                max_ipc_payload_mib: 128,
                list_default_limit: 50,
                list_max_limit: 500,
                ws_broadcast_capacity: 512,
            },
        };

        let toml_str = toml::to_string(&config).expect("serialize ServerConfig to TOML");
        let deserialized: ServerConfig =
            toml::from_str(&toml_str).expect("deserialize ServerConfig from TOML");

        assert_eq!(config.host, deserialized.host);
        assert_eq!(config.port, deserialized.port);
        assert_eq!(config.model_dirs.len(), deserialized.model_dirs.len());
        for (a, b) in config.model_dirs.iter().zip(deserialized.model_dirs.iter()) {
            assert_eq!(a.path, b.path);
            assert_eq!(a.kind, b.kind);
        }
        assert_eq!(config.artifact_dir, deserialized.artifact_dir);
        assert_eq!(config.db_path, deserialized.db_path);
        assert_eq!(config.venv_path, deserialized.venv_path);
        assert_eq!(config.rocm.use_hipblaslt, deserialized.rocm.use_hipblaslt);
        assert_eq!(
            config.rocm.hsa_override_gfx_version,
            deserialized.rocm.hsa_override_gfx_version
        );
        assert_eq!(
            config.hardware_override.as_ref().map(|h| &h.device_type),
            deserialized
                .hardware_override
                .as_ref()
                .map(|h| &h.device_type)
        );
        assert_eq!(
            config.hardware_override.as_ref().map(|h| h.vram_total_mib),
            deserialized
                .hardware_override
                .as_ref()
                .map(|h| h.vram_total_mib)
        );
        assert_eq!(config.worker_log_dir, deserialized.worker_log_dir);
        assert_eq!(config.num_threads, deserialized.num_threads);
        assert_eq!(config.num_interop_threads, deserialized.num_interop_threads);
        match (&config.frontend.mode, &deserialized.frontend.mode) {
            (FrontendMode::Remote { url: a }, FrontendMode::Remote { url: b }) => assert_eq!(a, b),
            _ => panic!("frontend mode mismatch"),
        }
        assert_eq!(
            config.gpu_selection.default_device,
            deserialized.gpu_selection.default_device
        );
        assert_eq!(
            config.limits.max_ipc_payload_mib,
            deserialized.limits.max_ipc_payload_mib
        );
        assert_eq!(
            config.limits.list_default_limit,
            deserialized.limits.list_default_limit
        );
        assert_eq!(
            config.limits.list_max_limit,
            deserialized.limits.list_max_limit
        );
        assert_eq!(
            config.limits.ws_broadcast_capacity,
            deserialized.limits.ws_broadcast_capacity
        );
    }

    /// Minimal TOML (empty table) deserializes into a ServerConfig with all
    /// documented defaults populated.
    #[test]
    fn config_default_deserialize() {
        let toml_str = "";
        let config: ServerConfig = toml::from_str(toml_str).expect("deserialize empty TOML");

        assert_eq!(
            config.host,
            IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
        );
        assert_eq!(config.port, 8488);
        assert!(config.model_dirs.is_empty());
        assert_eq!(config.artifact_dir, PathBuf::from("./artifacts"));
        assert_eq!(config.db_path, PathBuf::from("./anvilml.db"));
        assert_eq!(config.venv_path, PathBuf::from("./venv"));
        assert!(config.hardware_override.is_none());
        assert_eq!(config.worker_log_dir, Some(PathBuf::from("./logs")));
        assert_eq!(config.num_threads, 14);
        assert_eq!(config.num_interop_threads, 4);
        match &config.frontend.mode {
            FrontendMode::Local { path } => assert_eq!(path, &PathBuf::from("./bloomery")),
            other => panic!("expected Local mode, got {:?}", other),
        }
        assert_eq!(config.gpu_selection.default_device, "auto");
        assert_eq!(config.limits.max_ipc_payload_mib, 64);
        assert_eq!(config.limits.list_default_limit, 100);
        assert_eq!(config.limits.list_max_limit, 1000);
        assert_eq!(config.limits.ws_broadcast_capacity, 256);
    }

    /// Each FrontendMode variant round-trips correctly through TOML.
    #[test]
    fn config_frontend_modes() {
        // Local mode
        let local = ServerConfig {
            frontend: FrontendConfig {
                mode: FrontendMode::Local {
                    path: PathBuf::from("./custom"),
                },
            },
            ..Default::default()
        };
        let s = toml::to_string(&local).unwrap();
        let back: ServerConfig = toml::from_str(&s).unwrap();
        match &back.frontend.mode {
            FrontendMode::Local { path } => assert_eq!(path, &PathBuf::from("./custom")),
            _ => panic!("expected Local mode"),
        }

        // Remote mode
        let remote = ServerConfig {
            frontend: FrontendConfig {
                mode: FrontendMode::Remote {
                    url: "http://example.com:3000".to_string(),
                },
            },
            ..Default::default()
        };
        let s = toml::to_string(&remote).unwrap();
        let back: ServerConfig = toml::from_str(&s).unwrap();
        match &back.frontend.mode {
            FrontendMode::Remote { url } => assert_eq!(url, "http://example.com:3000"),
            _ => panic!("expected Remote mode"),
        }

        // Headless mode
        let headless = ServerConfig {
            frontend: FrontendConfig {
                mode: FrontendMode::Headless,
            },
            ..Default::default()
        };
        let s = toml::to_string(&headless).unwrap();
        let back: ServerConfig = toml::from_str(&s).unwrap();
        assert!(matches!(back.frontend.mode, FrontendMode::Headless));
    }
}
