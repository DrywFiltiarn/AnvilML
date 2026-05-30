//! NVIDIA CUDA GPU detector via `nvidia-smi`.
//!
//! Invokes the `nvidia-smi --query-gpu=index,name,vram_total,driver_version
//! --format=csv,noheader,nounits` CLI to enumerate NVIDIA GPUs.  Each line is
//! parsed into a `GpuDevice { device_type: Cuda }`.  If `nvidia-smi` is absent
//! or fails the detector returns an empty list (never an error).
//!
//! Inference capabilities are derived from the driver version:
//! - `fp16 = true` always.
//! - `bf16` and `flash_attention` are gated on driver major ≥ 525.

use anvilml_core::types::{DeviceType, GpuDevice};
use anvilml_core::AnvilError;

use crate::DeviceDetector;

// ---------------------------------------------------------------------------
// Spawn / Parse helpers
// ---------------------------------------------------------------------------

/// Run `nvidia-smi` with a fixed CSV query and return stdout as a `String`.
///
/// Returns `Err` when the binary cannot be found or the process exits with a
/// non-zero status.  Callers should treat both cases as "no GPU present" and
/// return an empty device list rather than propagating an error.
pub fn spawn_nvidia_smi() -> Result<String, AnvilError> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,vram_total,driver_version",
            "--format=csv,noheader,nounits",
        ])
        .output()?;

    if !output.status.success() {
        return Err(AnvilError::ConfigLoad(
            "nvidia-smi exited with non-zero status".into(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Parse the raw CSV output of `nvidia-smi --query-gpu=index,name,vram_total,driver_version
/// --format=csv,noheader,nounits` into a vector of `GpuDevice`s.
///
/// Each line has the format:
/// ```text
/// <index>, <name>, <vram_total> MiB, <driver_version>
/// ```
/// Example lines:
/// ```text
/// 0, NVIDIA GeForce RTX 4090, 24576 MiB, 535.129.03
/// 1, NVIDIA A100-SXM4-80GB, 81920 MiB, 535.129.03
/// ```
///
/// This is a **pure** function — it takes no I/O and can be called from tests
/// with fixture strings without spawning a process.
pub fn parse_nvidia_smi_output(raw: &str) -> Vec<GpuDevice> {
    let mut devices = Vec::new();

    for (idx, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split into at most 4 fields.
        let parts: Vec<&str> = line.splitn(4, ',').collect();
        if parts.len() < 4 {
            continue; // malformed line — skip
        }

        let index = parts[0].trim().parse::<u32>().unwrap_or(idx as u32);
        let name = parts[1].trim().to_string();

        // vram_total field: "NNNNN MiB" → strip the unit and parse.
        let vram_total_mib = parse_vram_mib(parts[2]);

        let driver_version = parts[3].trim().to_string();

        devices.push(GpuDevice {
            index,
            name,
            device_type: DeviceType::Cuda,
            vram_total_mib,
            vram_free_mib: 0, // refreshed later via refresh_vram()
            driver_version,
        });
    }

    devices
}

/// Parse a VRAM field like `"24576 MiB"` into a `u32` value.
pub fn parse_vram_mib(field: &str) -> u32 {
    // Strip "MiB" suffix and whitespace, then parse.
    let trimmed = field.trim();
    let without_unit = trimmed
        .strip_suffix(" MiB")
        .or_else(|| trimmed.strip_suffix("mib"))
        .unwrap_or(trimmed);
    without_unit.trim().parse::<u32>().unwrap_or(0)
}

/// Compute inference capabilities from a driver version string.
///
/// Rules:
/// - `fp16` is always `true` for CUDA devices.
/// - `bf16` and `flash_attention` are gated on driver major ≥ 525.
pub fn compute_inference_caps(driver_version: &str) -> anvilml_core::types::InferenceCaps {
    let major = driver_version
        .split('.')
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    let has_bf16_and_flash = major >= 525;

    anvilml_core::types::InferenceCaps {
        fp16: true,
        bf16: has_bf16_and_flash,
        flash_attention: has_bf16_and_flash,
    }
}

// ---------------------------------------------------------------------------
// CudaDetector — implements DeviceDetector trait
// ---------------------------------------------------------------------------

/// Detects NVIDIA CUDA GPU devices by invoking `nvidia-smi`.
///
/// If `nvidia-smi` is not found or returns a non-zero exit code, `detect()`
/// returns an empty device list (not an error).
#[derive(Debug, Clone)]
pub struct CudaDetector;

impl DeviceDetector for CudaDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        let raw = spawn_nvidia_smi().unwrap_or_default();
        Ok(parse_nvidia_smi_output(&raw))
    }

    fn refresh_vram(&self, device_index: u32) -> Result<(u32, u32), AnvilError> {
        // Query VRAM for a single device index.
        let output = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,vram_used,vram_total",
                &format!("--id={device_index}"),
                "--format=csv,noheader,nounits",
            ])
            .output()?;

        if !output.status.success() {
            return Err(AnvilError::ConfigLoad(format!(
                "nvidia-smi exited with non-zero status for device {device_index}"
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let trimmed = stdout.trim().to_string();

        if trimmed.is_empty() {
            return Err(AnvilError::ConfigLoad(format!(
                "no nvidia-smi output for device {device_index}"
            )));
        }

        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() < 3 {
            return Err(AnvilError::ConfigLoad(format!(
                "malformed nvidia-smi output for device {device_index}"
            )));
        }

        let vram_used = parse_vram_mib(parts[1]);
        let vram_total = parse_vram_mib(parts[2]);

        Ok((vram_used, vram_total))
    }
}

// ---------------------------------------------------------------------------
// Tests — pure-parse helpers with fixture strings (no GPU hardware required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture 1: single-GPU output → one device with correct fields.
    #[test]
    fn parse_single_gpu() {
        let raw = "0, NVIDIA GeForce RTX 4090, 24576 MiB, 535.129.03\n";
        let devices = parse_nvidia_smi_output(raw);
        assert_eq!(devices.len(), 1);

        let dev = &devices[0];
        assert_eq!(dev.index, 0);
        assert_eq!(dev.name, "NVIDIA GeForce RTX 4090");
        assert!(matches!(dev.device_type, DeviceType::Cuda));
        assert_eq!(dev.vram_total_mib, 24576);
        assert_eq!(dev.driver_version, "535.129.03");
    }

    // Fixture 2: dual-GPU output → two devices.
    #[test]
    fn parse_dual_gpu() {
        let raw = "0, NVIDIA A100-SXM4-80GB, 81920 MiB, 535.129.03\n1, NVIDIA GeForce RTX 4090, 24576 MiB, 535.129.03\n";
        let devices = parse_nvidia_smi_output(raw);
        assert_eq!(devices.len(), 2);

        assert_eq!(devices[0].index, 0);
        assert_eq!(devices[0].name, "NVIDIA A100-SXM4-80GB");
        assert_eq!(devices[0].vram_total_mib, 81920);

        assert_eq!(devices[1].index, 1);
        assert_eq!(devices[1].name, "NVIDIA GeForce RTX 4090");
        assert_eq!(devices[1].vram_total_mib, 24576);
    }

    // Fixture 3: empty input → no devices.
    #[test]
    fn parse_empty_input() {
        let devices = parse_nvidia_smi_output("");
        assert!(devices.is_empty());
    }

    // Fixture 4: blank lines are skipped.
    #[test]
    fn parse_blank_lines_skipped() {
        let raw = "\n\n0, Test GPU, 8192 MiB, 535.129.03\n\n";
        let devices = parse_nvidia_smi_output(raw);
        assert_eq!(devices.len(), 1);
    }

    // Driver-version gating: major ≥ 525 → bf16 and flash_attention enabled.
    #[test]
    fn inference_caps_bf16_flash_with_new_driver() {
        let caps = compute_inference_caps("535.129.03");
        assert!(caps.fp16);
        assert!(caps.bf16);
        assert!(caps.flash_attention);
    }

    // Driver-version gating: major < 525 → bf16 and flash_attention disabled.
    #[test]
    fn inference_caps_no_bf16_flash_with_old_driver() {
        let caps = compute_inference_caps("470.82.01");
        assert!(caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    // Driver-version gating: exactly 525 → bf16 and flash_attention enabled.
    #[test]
    fn inference_caps_bf16_flash_at_threshold_525() {
        let caps = compute_inference_caps("525.100.00");
        assert!(caps.fp16);
        assert!(caps.bf16);
        assert!(caps.flash_attention);
    }

    // Driver-version gating: 524 → bf16 and flash_attention disabled.
    #[test]
    fn inference_caps_no_bf16_flash_at_524() {
        let caps = compute_inference_caps("524.99.00");
        assert!(caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    // Driver-version gating: unparseable version → defaults applied.
    #[test]
    fn inference_caps_unparseable_driver_version() {
        let caps = compute_inference_caps("unknown");
        assert!(caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    // VRAM parsing: strips "MiB" suffix correctly.
    #[test]
    fn parse_vram_mib_strips_unit() {
        assert_eq!(parse_vram_mib("24576 MiB"), 24576);
        assert_eq!(parse_vram_mib("81920 MiB"), 81920);
        assert_eq!(parse_vram_mib("16384 MiB"), 16384);
    }

    // VRAM parsing: handles unexpected input gracefully.
    #[test]
    fn parse_vram_mib_graceful_fallback() {
        assert_eq!(parse_vram_mib("not_a_number"), 0);
        assert_eq!(parse_vram_mib(""), 0);
        assert_eq!(parse_vram_mib("   "), 0);
    }

    // Malformed lines (too few fields) are skipped.
    #[test]
    fn parse_skips_malformed_lines() {
        let raw =
            "0, Good GPU, 8192 MiB, 535.129.03\nbad_line\n2, Also Good, 4096 MiB, 535.129.03\n";
        let devices = parse_nvidia_smi_output(raw);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].index, 0);
        assert_eq!(devices[1].index, 2);
    }

    // CudaDetector::detect returns empty list when nvidia-smi is absent.
    #[test]
    fn cuda_detect_empty_when_nvidia_smi_absent() {
        let detector = CudaDetector;
        // On a machine without nvidia-smi, detect() should return Ok(empty vec)
        // (the spawn will fail, but we handle it gracefully).
        let devices = detector.detect().unwrap();
        assert!(devices.is_empty());
    }

    // CudaDetector::refresh_vram returns an error when nvidia-smi is absent.
    #[test]
    fn cuda_refresh_vram_error_when_nvidia_smi_absent() {
        let detector = CudaDetector;
        let result = detector.refresh_vram(0);
        assert!(result.is_err());
    }

    // CudaDetector is Send + Sync.
    #[test]
    fn cuda_detector_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CudaDetector>();
        assert_sync::<CudaDetector>();
    }
}
