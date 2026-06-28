/// Config-drift integration test.
///
/// Asserts that the checked-in `anvilml.toml` at the repo root deserialises
/// via `config_load::load()` into a `ServerConfig` where every field equals
/// `ServerConfig::default()`. This proves the config file and the type
/// definition never silently diverge — the `config-drift` CI job's actual
/// implementation.
///
/// Preconditions: `anvilml.toml` exists at the repo root with all fields at
/// their documented defaults.
/// Expected output: every loaded field matches the compiled-in default.
#[cfg(test)]
mod tests {
    use anvilml_core::ServerConfig;
    use anvilml_core::config_load::load;
    use std::path::Path;

    /// `config_load::load("../anvilml.toml")` returns a `ServerConfig` where
    /// every field equals `ServerConfig::default()`.
    ///
    /// Loads the checked-in TOML file from the repo root, asserts the result
    /// is `Ok(config)`, then verifies each scalar and nested field against
    /// the compiled-in default. On mismatch the default `assert_eq!` message
    /// names the field and both values for clear diagnosis.
    #[test]
    fn config_reference_matches_defaults() {
        // Load the repo-root config file. Cargo runs integration tests with
        // the crate root (`backend/`) as CWD, so `../anvilml.toml` resolves
        // to the repo root's `anvilml.toml`.
        let config = load(Some(Path::new("../anvilml.toml")), None)
            .expect("anvilml.toml should load successfully");

        let defaults = ServerConfig::default();

        // Verify every scalar field matches the compiled-in default.
        assert_eq!(config.host, defaults.host, "host mismatch");
        assert_eq!(config.port, defaults.port, "port mismatch");
        assert_eq!(config.db_path, defaults.db_path, "db_path mismatch");
        assert_eq!(
            config.artifact_dir, defaults.artifact_dir,
            "artifact_dir mismatch"
        );
        assert_eq!(config.venv_path, defaults.venv_path, "venv_path mismatch");
        assert_eq!(
            config.model_scan_depth, defaults.model_scan_depth,
            "model_scan_depth mismatch"
        );
        assert_eq!(
            config.max_ipc_payload_mib, defaults.max_ipc_payload_mib,
            "max_ipc_payload_mib mismatch"
        );
        assert_eq!(
            config.num_threads, defaults.num_threads,
            "num_threads mismatch"
        );

        // Verify every nested/optional field matches the compiled-in default.
        assert!(config.model_dirs.is_empty(), "model_dirs should be empty");
        assert_eq!(
            config.gpu_selection.default_device, defaults.gpu_selection.default_device,
            "gpu_selection.default_device mismatch"
        );
        assert_eq!(
            config.limits.max_queued_jobs, defaults.limits.max_queued_jobs,
            "limits.max_queued_jobs mismatch"
        );
        assert!(config.rocm.is_none(), "rocm should be None");
        assert!(
            config.hardware_override.is_none(),
            "hardware_override should be None"
        );
    }
}
