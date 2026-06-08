/// Environment variable builder for Python worker child processes.
///
/// Produces a `HashMap<String, String>` of environment variables to inject into each
/// worker child process, covering device isolation, ROCm performance flags, threading
/// control, worker identity, mock-mode propagation, and the IPC socket path.
use anvilml_core::{DeviceType, GpuDevice, ServerConfig};
use std::collections::HashMap;

/// Build the environment variable map for a worker process.
///
/// The returned map contains:
/// - Device isolation variables (`CUDA_VISIBLE_DEVICES` or `HIP_VISIBLE_DEVICES`)
/// - ROCm-specific flags when applicable
/// - Threading control variables (OpenMP, MKL, OpenBLAS, vecLib, AnvilML)
/// - Worker identity variables
/// - Mock-mode propagation from the parent environment
/// - IPC socket path (`ANVILML_IPC_SOCKET`)
pub fn build_worker_env(
    device: &GpuDevice,
    cfg: &ServerConfig,
    ipc_socket_path: &str,
) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // ── Device-specific isolation vars ────────────────────────────────
    match device.device_type {
        DeviceType::Cuda => {
            env.insert("CUDA_VISIBLE_DEVICES".to_string(), device.index.to_string());
        }
        DeviceType::Rocm => {
            env.insert("HIP_VISIBLE_DEVICES".to_string(), device.index.to_string());

            // ROCm-specific flags (always set, regardless of OS)
            let hipblaslt = if cfg.rocm.use_hipblaslt { "1" } else { "0" };
            env.insert("ROCBLAS_USE_HIPBLASLT".to_string(), hipblaslt.to_string());

            // HSA_OVERRIDE_GFX_VERSION — Unix only (cfg-gated at compile time)
            #[cfg(unix)]
            if let Some(ref ver) = cfg.rocm.hsa_override_gfx_version {
                env.insert("HSA_OVERRIDE_GFX_VERSION".to_string(), ver.clone());
            }
        }
        DeviceType::Cpu => {
            // No device isolation variable for CPU.
        }
    }

    // ── Threading variables (all device types) ────────────────────────
    let threads = cfg.num_threads.to_string();
    env.insert("OMP_NUM_THREADS".to_string(), threads.clone());
    env.insert("MKL_NUM_THREADS".to_string(), threads.clone());
    env.insert("OPENBLAS_NUM_THREADS".to_string(), threads.clone());
    env.insert("VECLIB_MAXIMUM_THREADS".to_string(), threads);

    // ── AnvilML-specific threading variables (all device types) ───────
    env.insert(
        "ANVILML_NUM_THREADS".to_string(),
        cfg.num_threads.to_string(),
    );
    env.insert(
        "ANVILML_NUM_INTEROP_THREADS".to_string(),
        cfg.num_interop_threads.to_string(),
    );

    // ── Worker identity variables (all device types) ──────────────────
    env.insert(
        "ANVILML_WORKER_ID".to_string(),
        format!("worker-{}", device.index),
    );
    env.insert("ANVILML_DEVICE_INDEX".to_string(), device.index.to_string());

    // ── Mock mode propagation ────────────────────────────────────────
    if let Ok(mock_val) = std::env::var("ANVILML_WORKER_MOCK") {
        env.insert("ANVILML_WORKER_MOCK".to_string(), mock_val);
    }

    // ── IPC socket path ──────────────────────────────────────────────
    env.insert(
        "ANVILML_IPC_SOCKET".to_string(),
        ipc_socket_path.to_string(),
    );

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_cuda_device(index: u32) -> GpuDevice {
        GpuDevice {
            index,
            name: "Mock CUDA GPU".to_string(),
            device_type: DeviceType::Cuda,
            vram_total_mib: 8192,
            vram_free_mib: 8000,
            driver_version: "535.0".to_string(),
            pci_vendor_id: 0x10de,
            pci_device_id: 0x20b0,
            arch: Some("8.0".to_string()),
            caps: Default::default(),
            enumeration_source: Default::default(),
            capabilities_source: Default::default(),
            db_group_name: None,
        }
    }

    fn mock_rocm_device(index: u32) -> GpuDevice {
        GpuDevice {
            index,
            name: "Mock ROCm GPU".to_string(),
            device_type: DeviceType::Rocm,
            vram_total_mib: 16384,
            vram_free_mib: 16000,
            driver_version: "6.1.0".to_string(),
            pci_vendor_id: 0x1002,
            pci_device_id: 0x740c,
            arch: Some("gfx1100".to_string()),
            caps: Default::default(),
            enumeration_source: Default::default(),
            capabilities_source: Default::default(),
            db_group_name: None,
        }
    }

    fn mock_cpu_device() -> GpuDevice {
        GpuDevice {
            index: 0,
            name: "Mock CPU".to_string(),
            device_type: DeviceType::Cpu,
            vram_total_mib: 0,
            vram_free_mib: 0,
            driver_version: "n/a".to_string(),
            pci_vendor_id: 0,
            pci_device_id: 0,
            arch: None,
            caps: Default::default(),
            enumeration_source: Default::default(),
            capabilities_source: Default::default(),
            db_group_name: None,
        }
    }

    fn default_config() -> ServerConfig {
        ServerConfig {
            rocm: anvilml_core::RocmConfig {
                use_hipblaslt: true,
                hsa_override_gfx_version: Some("10.3.0".to_string()),
            },
            ..ServerConfig::default()
        }
    }

    #[test]
    fn test_build_env_cuda() {
        let device = mock_cuda_device(0);
        let cfg = ServerConfig::default();
        let env = build_worker_env(&device, &cfg, "");

        // Device isolation
        assert_eq!(
            env.get("CUDA_VISIBLE_DEVICES").map(|v| v.as_str()),
            Some("0"),
            "CUDA_VISIBLE_DEVICES must be set for CUDA devices",
        );
        assert!(
            !env.contains_key("HIP_VISIBLE_DEVICES"),
            "HIP_VISIBLE_DEVICES must not be set for CUDA devices",
        );

        // Threading vars (default cfg: num_threads=14, num_interop_threads=4)
        assert_eq!(env.get("OMP_NUM_THREADS").map(|v| v.as_str()), Some("14"),);
        assert_eq!(env.get("MKL_NUM_THREADS").map(|v| v.as_str()), Some("14"),);
        assert_eq!(
            env.get("OPENBLAS_NUM_THREADS").map(|v| v.as_str()),
            Some("14"),
        );
        assert_eq!(
            env.get("VECLIB_MAXIMUM_THREADS").map(|v| v.as_str()),
            Some("14"),
        );

        // AnvilML threading vars
        assert_eq!(
            env.get("ANVILML_NUM_THREADS").map(|v| v.as_str()),
            Some("14"),
        );
        assert_eq!(
            env.get("ANVILML_NUM_INTEROP_THREADS").map(|v| v.as_str()),
            Some("4"),
        );

        // Worker identity
        assert_eq!(
            env.get("ANVILML_WORKER_ID").map(|v| v.as_str()),
            Some("worker-0"),
        );
        assert_eq!(
            env.get("ANVILML_DEVICE_INDEX").map(|v| v.as_str()),
            Some("0"),
        );
    }

    #[test]
    fn test_build_env_rocm_linux_hsa() {
        let device = mock_rocm_device(0);
        let cfg = default_config(); // use_hipblaslt=true, hsa_override_gfx_version=Some("10.3.0")
        let env = build_worker_env(&device, &cfg, "");

        // Device isolation
        assert_eq!(
            env.get("HIP_VISIBLE_DEVICES").map(|v| v.as_str()),
            Some("0"),
        );
        assert!(
            !env.contains_key("CUDA_VISIBLE_DEVICES"),
            "CUDA_VISIBLE_DEVICES must not be set for ROCm devices",
        );

        // ROCm flags
        assert_eq!(
            env.get("ROCBLAS_USE_HIPBLASLT").map(|v| v.as_str()),
            Some("1"),
        );

        // HSA_OVERRIDE_GFX_VERSION — present on unix cfg
        #[cfg(unix)]
        {
            assert_eq!(
                env.get("HSA_OVERRIDE_GFX_VERSION").map(|v| v.as_str()),
                Some("10.3.0"),
            );
        }

        // Threading vars (default config)
        assert_eq!(env.get("OMP_NUM_THREADS").map(|v| v.as_str()), Some("14"),);

        // Worker identity
        assert_eq!(
            env.get("ANVILML_WORKER_ID").map(|v| v.as_str()),
            Some("worker-0"),
        );
    }

    #[test]
    fn test_build_env_rocm_windows_no_hsa() {
        let device = mock_rocm_device(0);
        let cfg = ServerConfig {
            rocm: anvilml_core::RocmConfig {
                use_hipblaslt: false,
                hsa_override_gfx_version: None,
            },
            ..ServerConfig::default()
        };
        let env = build_worker_env(&device, &cfg, "");

        // Device isolation
        assert_eq!(
            env.get("HIP_VISIBLE_DEVICES").map(|v| v.as_str()),
            Some("0"),
        );
        assert!(
            !env.contains_key("CUDA_VISIBLE_DEVICES"),
            "CUDA_VISIBLE_DEVICES must not be set for ROCm devices",
        );

        // ROCm flags
        assert_eq!(
            env.get("ROCBLAS_USE_HIPBLASLT").map(|v| v.as_str()),
            Some("0"),
        );

        // HSA_OVERRIDE_GFX_VERSION — absent when hsa_override_gfx_version is None
        #[cfg(unix)]
        {
            assert!(
                !env.contains_key("HSA_OVERRIDE_GFX_VERSION"),
                "HSA_OVERRIDE_GFX_VERSION must not be set when config value is None",
            );
        }

        // Threading vars (default config)
        assert_eq!(env.get("OMP_NUM_THREADS").map(|v| v.as_str()), Some("14"),);

        // Worker identity
        assert_eq!(
            env.get("ANVILML_WORKER_ID").map(|v| v.as_str()),
            Some("worker-0"),
        );
    }

    #[test]
    fn test_build_env_cpu() {
        let device = mock_cpu_device();
        let cfg = ServerConfig::default();
        let env = build_worker_env(&device, &cfg, "");

        // No device isolation variable for CPU
        assert!(!env.contains_key("CUDA_VISIBLE_DEVICES"),);
        assert!(!env.contains_key("HIP_VISIBLE_DEVICES"),);

        // Threading vars (default config)
        assert_eq!(env.get("OMP_NUM_THREADS").map(|v| v.as_str()), Some("14"),);
        assert_eq!(env.get("MKL_NUM_THREADS").map(|v| v.as_str()), Some("14"),);
        assert_eq!(
            env.get("OPENBLAS_NUM_THREADS").map(|v| v.as_str()),
            Some("14"),
        );
        assert_eq!(
            env.get("VECLIB_MAXIMUM_THREADS").map(|v| v.as_str()),
            Some("14"),
        );

        // AnvilML threading vars
        assert_eq!(
            env.get("ANVILML_NUM_THREADS").map(|v| v.as_str()),
            Some("14"),
        );
        assert_eq!(
            env.get("ANVILML_NUM_INTEROP_THREADS").map(|v| v.as_str()),
            Some("4"),
        );

        // Worker identity
        assert_eq!(
            env.get("ANVILML_WORKER_ID").map(|v| v.as_str()),
            Some("worker-0"),
        );
        assert_eq!(
            env.get("ANVILML_DEVICE_INDEX").map(|v| v.as_str()),
            Some("0"),
        );
    }

    #[test]
    fn test_build_env_mock_propagation() {
        // Set the parent env var before calling.
        std::env::set_var("ANVILML_WORKER_MOCK", "1");
        let device = mock_cuda_device(0);
        let cfg = ServerConfig::default();

        let env = build_worker_env(&device, &cfg, "");

        assert_eq!(
            env.get("ANVILML_WORKER_MOCK").map(|v| v.as_str()),
            Some("1"),
            "ANVILML_WORKER_MOCK must be propagated from parent env",
        );

        // Clean up: unset the var so other tests are not affected.
        std::env::remove_var("ANVILML_WORKER_MOCK");
    }

    #[test]
    fn test_build_env_ipc_socket_path() {
        let device = mock_cuda_device(0);
        let cfg = ServerConfig::default();
        let env = build_worker_env(&device, &cfg, "/tmp/anvilml-12345/worker-0.sock");

        assert_eq!(
            env.get("ANVILML_IPC_SOCKET").map(|v| v.as_str()),
            Some("/tmp/anvilml-12345/worker-0.sock"),
            "ANVILML_IPC_SOCKET must be set to the passed path",
        );
    }
}
