use std::collections::HashMap;

use anvilml_core::DeviceType;

/// Builder for the environment variable map injected into every Python worker subprocess.
///
/// Produces a `HashMap` of `ANVILML_*` variables that the supervisor passes to
/// `Command::envs()` when spawning the worker process. See `ANVILML_DESIGN.md §9.7`
/// for the complete variable contract.
pub struct WorkerEnv;

impl WorkerEnv {
    /// Build the environment variable map for a Python worker subprocess.
    ///
    /// Returns a `HashMap` containing all `ANVILML_*` variables that should be
    /// injected into the worker's subprocess `Command`. See `ANVILML_DESIGN.md §9.7`.
    ///
    /// # Arguments
    /// * `ipc_port` — TCP port of the ROUTER socket.
    /// * `worker_id` — Bare device index as a string (e.g. `"0"`).
    /// * `device_index` — GPU device index.
    /// * `device_type` — Compute backend (`cuda`, `rocm`, or `cpu`).
    /// * `mock` — Whether the mock-hardware cargo feature is active.
    /// * `log_level` — Forwarded from server config.
    /// * `max_ipc_payload_mib` — Maximum IPC message size in MiB.
    pub fn build(
        ipc_port: u16,
        worker_id: &str,
        device_index: u32,
        device_type: DeviceType,
        mock: bool,
        log_level: &str,
        max_ipc_payload_mib: u32,
    ) -> HashMap<String, String> {
        let mut map = HashMap::new();

        map.insert("ANVILML_IPC_PORT".to_string(), ipc_port.to_string());
        map.insert("ANVILML_WORKER_ID".to_string(), worker_id.to_string());
        map.insert("ANVILML_DEVICE_INDEX".to_string(), device_index.to_string());
        map.insert(
            "ANVILML_DEVICE_TYPE".to_string(),
            device_type_to_str(device_type).to_string(),
        );

        // ANVILML_WORKER_MOCK is only injected when mock mode is active;
        // its absence signals real-mode hardware execution.
        if mock {
            map.insert("ANVILML_WORKER_MOCK".to_string(), "1".to_string());
        }

        map.insert("ANVILML_LOG_LEVEL".to_string(), log_level.to_string());
        map.insert(
            "ANVILML_MAX_IPC_PAYLOAD_MIB".to_string(),
            max_ipc_payload_mib.to_string(),
        );

        // NOTE: The force-mock env var is NOT set by this builder. That
        // variable is handled separately by the caller (the supervisor) as
        // an independent trigger that works regardless of the mock-hardware
        // cargo feature. See ENVIRONMENT.md §3.5 for its semantics.

        map
    }
}

/// Convert a `DeviceType` to its lowercase snake_case string form.
///
/// Mirrors the `#[serde(rename_all = "snake_case")]` serialization,
/// producing `"cuda"`, `"rocm"`, or `"cpu"` — the exact strings the
/// Python worker expects in `ANVILML_DEVICE_TYPE`.
fn device_type_to_str(device_type: DeviceType) -> &'static str {
    match device_type {
        DeviceType::Cuda => "cuda",
        DeviceType::Rocm => "rocm",
        DeviceType::Cpu => "cpu",
    }
}
