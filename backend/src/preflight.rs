//! Python environment preflight checks at server startup.
//!
//! Resolves the Python interpreter path, verifies the interpreter exists,
//! checks the Python version, and (when not in mock mode) verifies PyTorch
//! is importable. Results are returned as an `EnvReport`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anvilml_core::config::ServerConfig;
use anvilml_core::EnvReport;
use tokio::process::Command;

/// Resolve the Python interpreter path from a venv directory.
///
/// - Unix: `{venv_path}/bin/python3`
/// - Windows: `{venv_path}\Scripts\python.exe`
pub fn resolve_interpreter(venv_path: &Path) -> PathBuf {
    if cfg!(windows) {
        venv_path.join("Scripts").join("python.exe")
    } else {
        venv_path.join("bin").join("python3")
    }
}

/// Run preflight checks against the configured Python venv and return an
/// `EnvReport` describing the result.
///
/// Checks performed:
/// 1. Interpreter file exists (returns `python_missing` if not).
/// 2. `python --version` is executed and the version string is parsed
///    (logs a WARN if major.minor is not `3.12`).
/// 3. When `ANVILML_WORKER_MOCK` is _unset_, `import torch` is verified
///    (returns `torch_unavailable` if it fails).
pub async fn run_preflight(cfg: &ServerConfig) -> EnvReport {
    let python_path = resolve_interpreter(Path::new(&cfg.venv_path));
    let python_path_str = python_path.to_string_lossy().into_owned();

    // Check 1: interpreter exists.
    if !python_path.exists() {
        tracing::warn!(
            python_path = %python_path_str,
            "preflight: python interpreter not found"
        );
        return EnvReport {
            python_path: python_path_str,
            python_version: String::new(),
            torch_version: String::new(),
            preflight_ok: false,
            reason: "python_missing".to_string(),
        };
    }

    // Check 2: python --version.
    let python_version = match get_python_version(&python_path).await {
        Ok(version) => version,
        Err(e) => {
            tracing::warn!(
                python_path = %python_path_str,
                error = %e,
                "preflight: failed to get python version"
            );
            return EnvReport {
                python_path: python_path_str,
                python_version: String::new(),
                torch_version: String::new(),
                preflight_ok: false,
                reason: "version_check_failed".to_string(),
            };
        }
    };

    // Warn if not 3.12.x.
    if !is_python_3_12(&python_version) {
        tracing::warn!(
            python_version = %python_version,
            "preflight: expected Python 3.12.x, got {python_version}"
        );
    }

    // Check 3: import torch (only when not in mock mode).
    let torch_version = if std::env::var("ANVILML_WORKER_MOCK").is_err() {
        match get_torch_version(&python_path).await {
            Ok(version) => version,
            Err(e) => {
                tracing::warn!(
                    python_path = %python_path_str,
                    error = %e,
                    "preflight: torch import failed"
                );
                return EnvReport {
                    python_path: python_path_str,
                    python_version,
                    torch_version: String::new(),
                    preflight_ok: false,
                    reason: "torch_unavailable".to_string(),
                };
            }
        }
    } else {
        // Mock mode — skip torch check.
        tracing::info!("preflight: ANVILML_WORKER_MOCK is set, skipping torch check");
        String::new()
    };

    EnvReport {
        python_path: python_path_str,
        python_version,
        torch_version,
        preflight_ok: true,
        reason: String::new(),
    }
}

/// Run `python --version` and parse the version string.
///
/// Expected output format: `Python X.Y.Z` (or `Python X.Y.Z+...`).
/// Returns the version string `X.Y.Z` on success.
async fn get_python_version(python_path: &Path) -> Result<String, String> {
    let output = Command::new(python_path)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("failed to spawn python: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_version_string(&stdout)
}

/// Run `python -c "import torch; print(torch.__version__)"` and parse the
/// torch version.
async fn get_torch_version(python_path: &Path) -> Result<String, String> {
    let code = "import torch; print(torch.__version__)";
    let output = Command::new(python_path)
        .arg("-c")
        .arg(code)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("failed to spawn python: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("torch import failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().to_string())
}

/// Parse a version string like `"Python 3.12.4"` or `"Python 3.12.4+cu121"`
/// into `"3.12.4"`.
fn parse_version_string(input: &str) -> Result<String, String> {
    // Expected: "Python X.Y.Z" possibly with suffix like "+cu121"
    let trimmed = input.trim();

    // Find "Python " prefix and strip it.
    let without_prefix = if let Some(pos) = trimmed.find("Python ") {
        &trimmed[pos + 7..]
    } else if let Some(pos) = trimmed.find("python ") {
        &trimmed[pos + 7..]
    } else {
        trimmed
    };

    // Take only the first token (up to whitespace or end).
    let version = without_prefix
        .split_whitespace()
        .next()
        .ok_or_else(|| "empty version string".to_string())?;

    // Strip any suffix after '+' (e.g. "+cu121" → "3.12.4").
    let version = version.split('+').next().unwrap_or(version);

    // Validate it looks like a version (X.Y.Z).
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 {
        return Err(format!("invalid version format: {version}"));
    }

    Ok(parts.join("."))
}

/// Check if a version string starts with "3.12.".
fn is_python_3_12(version: &str) -> bool {
    version.starts_with("3.12")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn resolve_interpreter_unix() {
        // On non-Windows, expect bin/python3.
        let venv = Path::new("/opt/myvenv");
        let result = resolve_interpreter(venv);
        assert_eq!(result, PathBuf::from("/opt/myvenv/bin/python3"));
    }

    #[test]
    fn resolve_interpreter_windows() {
        // On Windows, expect Scripts/python.exe.
        #[cfg(windows)]
        {
            let venv = Path::new("C:\\Users\\me\\venv");
            let result = resolve_interpreter(venv);
            assert_eq!(
                result,
                PathBuf::from("C:\\Users\\me\\venv\\Scripts\\python.exe")
            );
        }
    }

    #[test]
    fn parse_version_python_3_12_4() {
        let result = parse_version_string("Python 3.12.4").unwrap();
        assert_eq!(result, "3.12.4");
    }

    #[test]
    fn parse_version_with_suffix() {
        let result = parse_version_string("Python 3.12.4+cu121").unwrap();
        assert_eq!(result, "3.12.4");
    }

    #[test]
    fn parse_version_3_11() {
        let result = parse_version_string("Python 3.11.9").unwrap();
        assert_eq!(result, "3.11.9");
    }

    #[test]
    fn is_python_3_12_true() {
        assert!(is_python_3_12("3.12.4"));
        assert!(is_python_3_12("3.12.0"));
    }

    #[test]
    fn is_python_3_12_false() {
        assert!(!is_python_3_12("3.11.9"));
        assert!(!is_python_3_12("3.13.0"));
    }

    #[test]
    fn parse_version_empty_fails() {
        assert!(parse_version_string("").is_err());
    }

    #[test]
    fn parse_version_no_python_prefix() {
        let result = parse_version_string("3.12.4").unwrap();
        assert_eq!(result, "3.12.4");
    }
}
