/// Integration tests for model directory scan at server startup.
///
/// These tests spawn the built `anvilml` binary in its default (server) path,
/// which triggers a background model directory scan during startup (P18-C3)
/// before binding the TCP listener.
///
/// Preconditions: the `anvilml` binary has been compiled
/// (e.g. `cargo build -p anvilml`).
#[cfg(test)]
mod tests {
    use std::io::BufRead;
    use std::process::Command;
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::time::timeout;

    /// Resolve the seed file path relative to the repo root.
    /// `CARGO_MANIFEST_DIR` is the directory containing the backend crate's
    /// Cargo.toml (`backend/`); the seed file is at
    /// `database/seeds/devices.sql` relative to the repo root.
    fn seed_path() -> String {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        // CARGO_MANIFEST_DIR is an absolute path (e.g., /home/user/repo/backend).
        // Go up one level to reach the repo root, then into database/seeds/.
        format!("{manifest_dir}/../database/seeds/devices.sql")
    }

    /// Minimal safetensors file header bytes.
    ///
    /// Format: 8-byte magic (nulls) + 8-byte LE u64 header length + JSON header.
    /// The JSON header `{}` is valid and produces an empty model metadata object.
    /// The scanner only hashes the file content and infers metadata from the
    /// path and filename — it does not validate the safetensors structure.
    fn minimal_safetensors_bytes() -> Vec<u8> {
        // 8 null bytes (safetensors magic)
        let mut bytes = vec![0u8; 8];
        // Header length = 2 (`{}`) as little-endian u64
        bytes.extend_from_slice(&2u64.to_le_bytes());
        // JSON header
        bytes.extend_from_slice(b"{}");
        bytes
    }

    /// Poll the `/v1/models` endpoint from a blocking thread.
    ///
    /// `reqwest::blocking::get` requires a blocking context, so we use
    /// `std::thread::spawn` to run it outside the async runtime.
    /// Returns `Some(Vec<serde_json::Value>)` if the request succeeds and
    /// parses as a JSON array, `None` otherwise.
    fn poll_models(url: &str) -> Option<Vec<serde_json::Value>> {
        let url = url.to_string();
        std::thread::spawn(move || match reqwest::blocking::get(&url) {
            Ok(resp) => match resp.text() {
                Ok(body) => serde_json::from_str::<Vec<serde_json::Value>>(&body).ok(),
                Err(_) => None,
            },
            Err(_) => None,
        })
        .join()
        .ok()
        .flatten()
    }

    /// Verify that spawning the binary with a temp model_dir containing a
    /// planted model file results in that model being listed via
    /// `GET /v1/models` within a bounded poll window, with no explicit
    /// `/v1/models/rescan` call made.
    ///
    /// Creates a temp directory with a model file placed directly in it
    /// (not in a subdirectory) since the scanner uses `recursive=false`
    /// with `depth=0`, which only scans the root of each configured
    /// model directory.
    ///
    /// Spawns the binary with a TOML config pointing to this directory,
    /// waits for startup, then polls `GET /v1/models` to confirm the
    /// planted model appears. The temp directory is dropped at the end
    /// of the test, cleaning up all files.
    #[tokio::test]
    async fn test_startup_scan_displays_planted_model() {
        // Create a unique temp directory for this test.
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        // Place the model file directly in the scanned directory root.
        // The scanner's non-recursive scan (depth=0) only examines files
        // directly in the root, not in subdirectories.
        // The filename `model_fp8.safetensors` causes the scanner to infer
        // `ModelDtype::Fp8` and `ModelFormat::Safetensors`.
        let safetensors_path = dir.path().join("model_fp8.safetensors");
        std::fs::write(&safetensors_path, minimal_safetensors_bytes())
            .expect("failed to write safetensors file");

        // Write a TOML config pointing to the temp directory.
        // Use a unique DB path inside the temp dir so tests don't share state.
        let db_path = dir.path().join("test_anvilml.db");
        // db_path/model_dir are embedded as TOML *literal* strings (single
        // quotes), not basic strings (double quotes). On Windows,
        // tempfile::tempdir() paths look like
        // `C:\Users\runneradmin\AppData\Local\Temp\...` — inside a TOML
        // basic string, `\U` is parsed as the start of an 8-hex-digit
        // Unicode escape, and the literal text following it isn't valid
        // hex, so config_load::load() fails to parse the TOML on Windows
        // and the binary exits before ever logging "listening" (this is
        // what actually made these tests fail on rust-windows CI — not a
        // genuine 5-second startup delay). TOML literal strings take their
        // content verbatim with no escape processing, which is exactly
        // right for a filesystem path that may itself contain backslashes.
        let config_content = format!(
            r#"
host = "127.0.0.1"
port = 8488
db_path = '{}'
artifact_dir = "./artifacts"
venv_path = "./worker/.venv"
model_scan_depth = 2
max_ipc_payload_mib = 256

[[model_dirs]]
path = '{}'
recursive = false
"#,
            db_path.display(),
            dir.path().display()
        );
        let config_path = dir.path().join("anvilml.toml");
        std::fs::write(&config_path, config_content).expect("failed to write temp config");

        // Use a fixed port to make it easy to construct the HTTP URL.
        // Port 19000 is unlikely to be in use during tests.
        let port = 19000u16;

        // Spawn the binary with the temp config via --config CLI flag.
        // ANVILML_CONFIG is not a valid env var — only --config sets the
        // TOML config path (per ENVIRONMENT.md §4).
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .arg("--config")
            .arg(&config_path)
            .env("ANVILML_PORT", &port.to_string())
            .env("ANVILML_SEED_PATH", &seed_path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

        // Wait for the "listening" log line on stderr.
        // This confirms the binary has completed startup including
        // the model directory scan.
        let started = timeout(Duration::from_secs(5), async {
            let stderr = child
                .stderr
                .take()
                .expect("stderr was not piped — test setup error");
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let line = line.expect("failed to read stderr line");
                if line.contains("listening") {
                    return true;
                }
            }
            false
        });

        let started = started.await;

        // Do NOT kill the process yet — we need it running to serve HTTP
        // requests during the polling phase below.
        assert!(
            started.is_ok() && started.unwrap(),
            "binary did not print 'listening' within 5 seconds"
        );

        // Poll the /v1/models endpoint to wait for the background scan
        // to complete. The scan runs in a spawned tokio task, so it may
        // not have finished by the time the "listening" log line appears.
        let models_url = format!("http://127.0.0.1:{port}/v1/models");
        let mut found_model = false;

        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Some(arr) = poll_models(&models_url) {
                if !arr.is_empty() {
                    found_model = true;
                    break;
                }
            }
        }

        // Now terminate the process to free the port and clean up.
        let _ = child.kill();
        let _ = child.wait();

        // Assert the planted model was discovered by the startup scan.
        assert!(
            found_model,
            "GET /v1/models did not return any models after 5 polling attempts \
             (500ms interval) — startup scan may have failed or not completed"
        );
    }

    /// Verify that spawning the binary with an empty temp model_dir results
    /// in `GET /v1/models` returning an empty array.
    ///
    /// Creates a temp directory with an empty `models/` subdirectory.
    /// Spawns the binary with a TOML config pointing to this directory,
    /// waits for startup, then polls `GET /v1/models` to confirm the
    /// response is an empty JSON array `[]`. The temp directory is
    /// dropped at the end of the test.
    #[tokio::test]
    async fn test_startup_scan_empty_dir_lists_no_models() {
        // Create a unique temp directory for this test.
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        // Create an empty model directory.
        let model_dir = dir.path().join("models");
        std::fs::create_dir_all(&model_dir).expect("failed to create models directory");

        // Write a TOML config pointing to the empty temp model directory.
        // Use a unique DB path inside the temp dir so tests don't share state.
        let db_path = dir.path().join("test_anvilml.db");
        // See the identical comment in test_startup_scan_displays_planted_model
        // above: literal (single-quoted) TOML strings avoid the Windows
        // `C:\Users\...` → `\U` invalid-escape TOML parse failure.
        let config_content = format!(
            r#"
host = "127.0.0.1"
port = 8488
db_path = '{}'
artifact_dir = "./artifacts"
venv_path = "./worker/.venv"
model_scan_depth = 2
max_ipc_payload_mib = 256

[[model_dirs]]
path = '{}'
recursive = false
"#,
            db_path.display(),
            model_dir.display()
        );
        let config_path = dir.path().join("anvilml.toml");
        std::fs::write(&config_path, config_content).expect("failed to write temp config");

        // Use a fixed port to make it easy to construct the HTTP URL.
        let port = 19001u16;

        // Spawn the binary with the temp config via --config CLI flag.
        let mut child = Command::new(env!("CARGO_BIN_EXE_anvilml"))
            .arg("--config")
            .arg(&config_path)
            .env("ANVILML_PORT", &port.to_string())
            .env("ANVILML_SEED_PATH", &seed_path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn anvilml binary");

        // Wait for the "listening" log line on stderr.
        let started = timeout(Duration::from_secs(5), async {
            let stderr = child
                .stderr
                .take()
                .expect("stderr was not piped — test setup error");
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let line = line.expect("failed to read stderr line");
                if line.contains("listening") {
                    return true;
                }
            }
            false
        });

        let started = started.await;

        // Do NOT kill the process yet — we need it running to serve HTTP
        // requests during the polling phase below.
        assert!(
            started.is_ok() && started.unwrap(),
            "binary did not print 'listening' within 5 seconds"
        );

        // Poll the /v1/models endpoint. Even with an empty model dir,
        // the startup scan should have completed and the endpoint should
        // return an empty array.
        let models_url = format!("http://127.0.0.1:{port}/v1/models");
        let mut got_empty = false;

        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Some(arr) = poll_models(&models_url) {
                if arr.is_empty() {
                    got_empty = true;
                    break;
                }
            }
        }

        // Now terminate the process to free the port and clean up.
        let _ = child.kill();
        let _ = child.wait();

        // Assert the response was an empty array.
        assert!(
            got_empty,
            "GET /v1/models did not return an empty array after 5 polling \
             attempts — expected [] for empty model directory"
        );
    }
}
