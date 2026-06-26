use std::path::PathBuf;

/// Top-level server configuration with compiled-in defaults.
///
/// Fields are loaded through a four-layer precedence chain:
/// defaults → TOML → environment variables → CLI flags.
/// Only the scalar fields are defined here; nested tables
/// (`model_dirs`, `gpu_selection`, `limits`, `rocm`, `hardware_override`)
/// are added by P2-A3.
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
        }
    }
}
