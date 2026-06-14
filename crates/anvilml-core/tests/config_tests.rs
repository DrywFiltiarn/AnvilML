/// Tests for `config.rs` — `ServerConfig` and all nested structs.
///
/// Verifies:
/// - Default values match documented defaults from `ENVIRONMENT.md §4`.
/// - `Serialize`/`Deserialize` roundtrip preserves all fields including `PathBuf`.
/// - Non-default values (including `Option::Some` variants) survive roundtrip.
use anvilml_core::config::*;

/// Verifies that `ServerConfig::default()` fields match all documented
/// defaults from `ENVIRONMENT.md §4`.
///
/// This is the acceptance gate for correctness of the `Default` impl.
/// Every field is asserted against its documented default value.
#[test]
fn test_default_values() {
    let cfg = ServerConfig::default();

    // Top-level scalar fields
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.port, 8488);
    assert_eq!(cfg.db_path, std::path::PathBuf::from("./anvilml.db"));
    assert_eq!(cfg.artifact_dir, std::path::PathBuf::from("./artifacts"));
    assert_eq!(cfg.num_threads, None);
    assert_eq!(cfg.venv_path, std::path::PathBuf::from("./worker/.venv"));
    assert_eq!(cfg.max_ipc_payload_mib, 256);
    assert_eq!(cfg.seeds_path, std::path::PathBuf::from("./backend/seeds"));

    // model_dirs is empty — no directories configured by default
    assert!(cfg.model_dirs.is_empty());

    // gpu_selection defaults
    assert_eq!(cfg.gpu_selection.default_device, "auto");

    // limits defaults
    assert_eq!(cfg.limits.max_queued_jobs, 100);
    assert_eq!(cfg.limits.max_concurrent_jobs, 1);

    // Optional sections default to None
    assert!(cfg.rocm.is_none());
    assert!(cfg.hardware_override.is_none());
}

/// Verifies that `ServerConfig` serialises to JSON and deserialises back
/// to an identical value — including `PathBuf` fields (which round-trip
/// as strings) and `Option` fields.
///
/// This tests that `Serialize`/`Deserialize` derives produce correct
/// field mappings and that the `path_as_string` helper works correctly.
#[test]
fn test_serialisation_roundtrip() {
    let cfg = ServerConfig::default();

    // Serialize to JSON string
    let json = serde_json::to_string(&cfg).expect("serialize ServerConfig to JSON");

    // Deserialize back
    let restored: ServerConfig =
        serde_json::from_str(&json).expect("deserialize JSON back to ServerConfig");

    // All fields must be equal — roundtrip is lossless
    assert_eq!(cfg, restored);
}

/// Verifies that non-default values — including `Option::Some` variants —
/// survive a serialisation roundtrip.
///
/// This constructs a `ServerConfig` mimicking what environment variable
/// overrides would produce (e.g. `host = "0.0.0.0"`, `port = 9001`,
/// `rocm = Some(...)`), then asserts all overridden values are preserved.
#[test]
fn test_env_override_values() {
    // Build a config with non-default values that mimic env var overrides.
    let cfg = ServerConfig {
        host: "0.0.0.0".to_string(),
        port: 9001,
        db_path: std::path::PathBuf::from("/custom/anvilml.db"),
        artifact_dir: std::path::PathBuf::from("/custom/artifacts"),
        num_threads: Some(4),
        venv_path: std::path::PathBuf::from("/custom/worker/.venv"),
        max_ipc_payload_mib: 512,
        model_dirs: vec![ModelDirConfig {
            path: std::path::PathBuf::from("/models"),
            recursive: true,
            max_depth: Some(3),
        }],
        gpu_selection: GpuSelectionConfig {
            default_device: "0".to_string(),
        },
        limits: LimitsConfig {
            max_queued_jobs: 200,
            max_concurrent_jobs: 4,
        },
        rocm: Some(RocmConfig {
            hsa_override_gfx_version: Some("gfx942".into()),
        }),
        hardware_override: Some(HardwareOverrideConfig {
            device_type: "cuda".to_string(),
            vram_total_mib: 16384,
        }),
        seeds_path: std::path::PathBuf::from("/custom/seeds"),
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&cfg).expect("serialize ServerConfig to JSON");
    let restored: ServerConfig =
        serde_json::from_str(&json).expect("deserialize JSON back to ServerConfig");

    // All overridden values must be preserved
    assert_eq!(restored.host, "0.0.0.0");
    assert_eq!(restored.port, 9001);
    assert_eq!(
        restored.db_path,
        std::path::PathBuf::from("/custom/anvilml.db")
    );
    assert_eq!(
        restored.artifact_dir,
        std::path::PathBuf::from("/custom/artifacts")
    );
    assert_eq!(restored.num_threads, Some(4));
    assert_eq!(
        restored.venv_path,
        std::path::PathBuf::from("/custom/worker/.venv")
    );
    assert_eq!(restored.max_ipc_payload_mib, 512);

    // model_dirs
    assert_eq!(restored.model_dirs.len(), 1);
    assert_eq!(
        restored.model_dirs[0].path,
        std::path::PathBuf::from("/models")
    );
    assert!(restored.model_dirs[0].recursive);
    assert_eq!(restored.model_dirs[0].max_depth, Some(3));

    // gpu_selection
    assert_eq!(restored.gpu_selection.default_device, "0");

    // limits
    assert_eq!(restored.limits.max_queued_jobs, 200);
    assert_eq!(restored.limits.max_concurrent_jobs, 4);

    // rocm (Option::Some variant)
    assert!(restored.rocm.is_some());
    assert_eq!(
        restored.rocm.as_ref().unwrap().hsa_override_gfx_version,
        Some("gfx942".into())
    );

    // hardware_override (Option::Some variant)
    assert!(restored.hardware_override.is_some());
    assert_eq!(
        restored.hardware_override.as_ref().unwrap().device_type,
        "cuda"
    );
    assert_eq!(
        restored.hardware_override.as_ref().unwrap().vram_total_mib,
        16384
    );

    // seeds_path
    assert_eq!(
        restored.seeds_path,
        std::path::PathBuf::from("/custom/seeds")
    );
}
