//! Environment variable builder for Python worker subprocesses.
//!
//! Produces a `HashMap` of `ANVILML_*` variables that the Rust supervisor
//! injects into the Python worker process environment at spawn time.

use anvilml_core::{DeviceType, GpuDevice, ServerConfig};
use std::collections::HashMap;

/// Build the environment variable map for a Python worker subprocess.
///
/// Returns a `HashMap` containing all `ANVILML_*` variables required by the
/// worker runtime:
/// - `ANVILML_IPC_PORT` — TCP port of the ROUTER socket
/// - `ANVILML_WORKER_ID` — logical worker identity (device index)
/// - `ANVILML_DEVICE_INDEX` — GPU device index
/// - `ANVILML_DEVICE_TYPE` — lowercase backend name (`"cuda"`, `"rocm"`, `"cpu"`)
/// - `ANVILML_LOG_LEVEL` — forwarded from server config
/// - `ANVILML_MAX_IPC_PAYLOAD_MIB` — forwarded from server config
/// - `ANVILML_WORKER_MOCK` — `"1"` when compiled with `mock-hardware` feature
///
/// # Arguments
///
/// * `device` — The GPU device this worker will operate on.
/// * `cfg` — The server configuration (provides log level and IPC payload cap).
/// * `port` — The TCP port the worker should connect to for IPC.
///
/// # Returns
///
/// A populated `HashMap` ready to be passed to `std::process::Command::envs()`.
pub fn build_worker_env(
    device: &GpuDevice,
    cfg: &ServerConfig,
    port: u16,
) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // IPC port the worker will dial into the ROUTER socket.
    env.insert("ANVILML_IPC_PORT".into(), port.to_string());

    // Worker identity is the device index — used for identification in logs and
    // heartbeat messages.
    env.insert("ANVILML_WORKER_ID".into(), device.index.to_string());

    // Device index as a separate variable for the Python worker's own tracking.
    env.insert("ANVILML_DEVICE_INDEX".into(), device.index.to_string());

    // Map the enum variant to lowercase for the worker (e.g. "cuda", "rocm", "cpu").
    env.insert(
        "ANVILML_DEVICE_TYPE".into(),
        device_type_label(&device.device_type).to_string(),
    );

    // Forward the server's configured log level so the worker matches it.
    env.insert("ANVILML_LOG_LEVEL".into(), cfg.log_level.clone());

    // Forward the IPC payload size cap so the worker enforces the same limit.
    env.insert(
        "ANVILML_MAX_IPC_PAYLOAD_MIB".into(),
        cfg.max_ipc_payload_mib.to_string(),
    );

    // When compiled with mock-hardware, signal the Python worker to use mock
    // hardware paths (no torch import, sentinel outputs).
    #[cfg(feature = "mock-hardware")]
    env.insert("ANVILML_WORKER_MOCK".into(), "1".into());

    env
}

/// Map a `DeviceType` enum variant to its lowercase string label.
///
/// Returns `"cuda"`, `"rocm"`, or `"cpu"` — the values the Python worker
/// expects for backend identification. This is a manual mapping because
/// the `#[serde(rename_all = "snake_case")]` attribute only applies during
/// serialisation, not during plain value inspection.
fn device_type_label(device_type: &DeviceType) -> &'static str {
    match device_type {
        // NVIDIA CUDA backend label.
        DeviceType::Cuda => "cuda",
        // AMD ROCm backend label.
        DeviceType::Rocm => "rocm",
        // Generic CPU execution label.
        DeviceType::Cpu => "cpu",
    }
}
