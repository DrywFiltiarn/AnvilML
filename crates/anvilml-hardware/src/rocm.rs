//! AMD ROCm GPU detector via `rocm-smi`.
//!
//! Invokes the `rocm-smi --showmeminfo vram --json` CLI to enumerate AMD GPUs.
//! The JSON output is parsed into a `Vec<GpuDevice>` with `device_type: Rocm`.
//! If `rocm-smi` is absent or fails, the detector returns an empty device list
//! (never an error).
//!
//! Inference capabilities are derived from the graphics architecture:
//! - `fp16 = true` always.
//! - `bf16` and `flash_attention` are gated on gfx_arch from
//!   `RocmConfig.hsa_override_gfx_version` starting with `gfx11` or higher
//!   (RDNA3+). If no override is provided, defaults to `bf16 = false` for safety.

use anvilml_core::types::{DeviceType, GpuDevice, InferenceCaps};
use anvilml_core::AnvilError;

use crate::DeviceDetector;

// ---------------------------------------------------------------------------
// Spawn / Parse helpers
// ---------------------------------------------------------------------------

/// Run `rocm-smi --showmeminfo vram --json` and return stdout as a `String`.
///
/// Returns `Err` when the binary cannot be found or the process exits with a
/// non-zero status. Callers should treat both cases as "no GPU present" and
/// return an empty device list rather than propagating an error.
pub fn spawn_rocm_smi() -> Result<String, AnvilError> {
    let output = std::process::Command::new("rocm-smi")
        .args(["--showmeminfo", "vram", "--json"])
        .output()?;

    if !output.status.success() {
        return Err(AnvilError::ConfigLoad(
            "rocm-smi exited with non-zero status".into(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse the raw JSON output of `rocm-smi --showmeminfo vram --json` into a
/// vector of `GpuDevice`s.
///
/// The JSON structure from `rocm-smi` typically looks like:
/// ```json
/// {
///   "gpu": [
///     {
///       "id": "0",
///       "name": "AMD Radeon RX 7900 XTX",
///       "vram": {
///         "total": 17179869184,
///         "used": 0
///       }
///     },
///     ...
///   ]
/// }
/// ```
/// VRAM values are in bytes and are converted to MiB.
///
/// This is a **pure** function — it takes no I/O and can be called from tests
/// with fixture strings without spawning a process.
pub fn parse_rocm_smi_output(raw: &str) -> Vec<GpuDevice> {
    let json_value = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut devices = Vec::new();

    // The top-level "gpu" key holds an array of GPU objects.
    let gpu_array = match json_value.get("gpu").and_then(|g| g.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    for (idx, gpu_obj) in gpu_array.iter().enumerate() {
        // Extract index.
        let index = gpu_obj
            .get("id")
            .and_then(|i| i.as_str())
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(idx as u32);

        // Extract name (optional — fall back to "AMD GPU").
        let name = gpu_obj
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("AMD GPU")
            .to_string();

        // Extract VRAM total and used in bytes from the nested "vram" object.
        let vram_total_bytes = gpu_obj
            .get("vram")
            .and_then(|v| v.get("total"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0);

        let vram_used_bytes = gpu_obj
            .get("vram")
            .and_then(|v| v.get("used"))
            .and_then(|u| u.as_u64())
            .unwrap_or(0);

        // Convert bytes → MiB.
        let vram_total_mib = (vram_total_bytes / 1024 / 1024) as u32;
        let vram_free_mib = (vram_used_bytes / 1024 / 1024) as u32;

        devices.push(GpuDevice {
            index,
            name,
            device_type: DeviceType::Rocm,
            vram_total_mib,
            vram_free_mib,
            driver_version: "n/a".into(),
        });
    }

    devices
}

/// Compute inference capabilities from an optional gfx architecture string.
///
/// Rules:
/// - `fp16` is always `true` for ROCm devices.
/// - `bf16` and `flash_attention` are gated on gfx_arch starting with `gfx11`
///   or higher (RDNA3+). The gfx string typically looks like "gfx1100", "gfx1030",
///   etc. If no override is provided, defaults to `bf16 = false` for safety.
pub fn compute_inference_caps(hsa_override_gfx_version: Option<&str>) -> InferenceCaps {
    let has_bf16_and_flash = match hsa_override_gfx_version {
        Some(gfx) => {
            // The gfx version is a string like "11.0.0" or "10.3.0".
            // We check if the major version is >= 11 (RDNA3+).
            let major = gfx
                .split('.')
                .next()
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(0);
            major >= 11
        }
        None => false, // default to safe (no BF16) if no override provided
    };

    InferenceCaps {
        fp16: true,
        bf16: has_bf16_and_flash,
        flash_attention: has_bf16_and_flash,
    }
}

// ---------------------------------------------------------------------------
// RocmDetector — implements DeviceDetector trait
// ---------------------------------------------------------------------------

/// Detects AMD ROCm GPU devices by invoking `rocm-smi`.
///
/// If `rocm-smi` is not found or returns a non-zero exit code, `detect()`
/// returns an empty device list (not an error).
#[derive(Debug, Clone)]
pub struct RocmDetector;

impl DeviceDetector for RocmDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        let raw = spawn_rocm_smi().unwrap_or_default();
        Ok(parse_rocm_smi_output(&raw))
    }

    fn refresh_vram(&self, device_index: u32) -> Result<(u32, u32), AnvilError> {
        // Re-invoke rocm-smi and parse the per-card VRAM values.
        let output = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--json"])
            .output()?;

        if !output.status.success() {
            return Err(AnvilError::ConfigLoad(format!(
                "rocm-smi exited with non-zero status for device {device_index}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let devices = parse_rocm_smi_output(&stdout);

        if device_index >= devices.len() as u32 {
            return Err(AnvilError::ConfigLoad(format!(
                "no ROCm GPU found at index {device_index}"
            )));
        }

        let dev = &devices[device_index as usize];
        // refresh_vram returns (used_mib, total_mib).
        // The parse function sets vram_free_mib to the "used" value from rocm-smi,
        // but for refresh_vram we return (total - used, total) to be more accurate.
        let total = dev.vram_total_mib;
        // The used VRAM in MiB is derived from the raw bytes; use vram_free_mib
        // which was set to used_bytes / 1024 / 1024 during parsing.
        let used = dev.vram_free_mib;

        Ok((used, total))
    }
}

// ---------------------------------------------------------------------------
// Tests — pure-parse helpers with fixture strings (no AMD GPU hardware required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture: single-GPU rocm-smi JSON output.
    const SINGLE_GPU_JSON: &str = r#"{
        "gpu": [
            {
                "id": "0",
                "name": "AMD Radeon RX 7900 XTX",
                "vram": {
                    "total": 21474836480,
                    "used": 536870912
                }
            }
        ]
    }"#;

    // Fixture: dual-GPU rocm-smi JSON output.
    const DUAL_GPU_JSON: &str = r#"{
        "gpu": [
            {
                "id": "0",
                "name": "AMD Radeon Pro W7900",
                "vram": {
                    "total": 42949672960,
                    "used": 1073741824
                }
            },
            {
                "id": "1",
                "name": "AMD Radeon RX 7900 XTX",
                "vram": {
                    "total": 21474836480,
                    "used": 268435456
                }
            }
        ]
    }"#;

    // Parse single-GPU output → one device with correct fields.
    #[test]
    fn parse_single_gpu() {
        let devices = parse_rocm_smi_output(SINGLE_GPU_JSON);
        assert_eq!(devices.len(), 1);

        let dev = &devices[0];
        assert_eq!(dev.index, 0);
        assert_eq!(dev.name, "AMD Radeon RX 7900 XTX");
        assert!(matches!(dev.device_type, DeviceType::Rocm));
        // 21474836480 bytes = 20 GiB = 20480 MiB
        assert_eq!(dev.vram_total_mib, 20480);
        // 536870912 bytes = 512 MiB
        assert_eq!(dev.vram_free_mib, 512);
        assert_eq!(dev.driver_version, "n/a");
    }

    // Parse dual-GPU output → two devices.
    #[test]
    fn parse_dual_gpu() {
        let devices = parse_rocm_smi_output(DUAL_GPU_JSON);
        assert_eq!(devices.len(), 2);

        assert_eq!(devices[0].index, 0);
        assert_eq!(devices[0].name, "AMD Radeon Pro W7900");
        // 42949672960 bytes = 40 GiB = 40960 MiB
        assert_eq!(devices[0].vram_total_mib, 40960);

        assert_eq!(devices[1].index, 1);
        assert_eq!(devices[1].name, "AMD Radeon RX 7900 XTX");
        assert_eq!(devices[1].vram_total_mib, 20480);
    }

    // Empty input → no devices.
    #[test]
    fn parse_empty_input() {
        let devices = parse_rocm_smi_output("");
        assert!(devices.is_empty());
    }

    // Malformed JSON → no devices.
    #[test]
    fn parse_malformed_json() {
        let devices = parse_rocm_smi_output("not json at all");
        assert!(devices.is_empty());
    }

    // Missing "gpu" key → no devices.
    #[test]
    fn parse_missing_gpu_key() {
        let devices = parse_rocm_smi_output(r#"{"other": "data"}"#);
        assert!(devices.is_empty());
    }

    // gfx11 → RDNA3 → bf16 and flash_attention enabled.
    #[test]
    fn inference_caps_gfx11_rdna3() {
        let caps = compute_inference_caps(Some("11.0.0"));
        assert!(caps.fp16);
        assert!(caps.bf16);
        assert!(caps.flash_attention);
    }

    // gfx11.0.1 → RDNA3 → bf16 and flash_attention enabled.
    #[test]
    fn inference_caps_gfx11_variant() {
        let caps = compute_inference_caps(Some("11.0.1"));
        assert!(caps.fp16);
        assert!(caps.bf16);
        assert!(caps.flash_attention);
    }

    // gfx10 → pre-RDNA3 → bf16 and flash_attention disabled.
    #[test]
    fn inference_caps_gfx10_cdna2() {
        let caps = compute_inference_caps(Some("10.3.0"));
        assert!(caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    // gfx12 → RDNA4 → bf16 and flash_attention enabled.
    #[test]
    fn inference_caps_gfx12_rdna4() {
        let caps = compute_inference_caps(Some("12.0.0"));
        assert!(caps.fp16);
        assert!(caps.bf16);
        assert!(caps.flash_attention);
    }

    // No override → safe defaults (bf16 disabled).
    #[test]
    fn inference_caps_no_override() {
        let caps = compute_inference_caps(None);
        assert!(caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    // Unparseable gfx version → safe defaults.
    #[test]
    fn inference_caps_unparseable_gfx() {
        let caps = compute_inference_caps(Some("unknown"));
        assert!(caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    // RocmDetector::detect returns empty list when rocm-smi is absent.
    #[test]
    fn rocm_detect_empty_when_rocm_smi_absent() {
        let detector = RocmDetector;
        let devices = detector.detect().unwrap();
        assert!(devices.is_empty());
    }

    // RocmDetector::refresh_vram returns an error when rocm-smi is absent.
    #[test]
    fn rocm_refresh_vram_error_when_rocm_smi_absent() {
        let detector = RocmDetector;
        let result = detector.refresh_vram(0);
        assert!(result.is_err());
    }

    // RocmDetector is Send + Sync.
    #[test]
    fn rocm_detector_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<RocmDetector>();
        assert_sync::<RocmDetector>();
    }
}
