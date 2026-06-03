use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind};
use anvilml_registry::scanner::scan_dirs;

#[tokio::test]
async fn test_scan_dirs_two_files() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let path = tmp.path();

    // Create first model file.
    let f1_path = path.join("model-fp16.safetensors");
    std::fs::write(&f1_path, b"some content for model fp16").unwrap();

    // Create second model file.
    let f2_path = path.join("weights-q8.pt");
    std::fs::write(&f2_path, b"different content for weights q8").unwrap();

    let dirs = vec![ModelDirConfig {
        path: path.to_path_buf(),
        kind: Some(ModelKind::Diffusion),
    }];

    let results = scan_dirs(&dirs).await;

    assert_eq!(results.len(), 2, "expected exactly 2 model entries");

    // Find entries by name.
    let fp16_entry = results
        .iter()
        .find(|m| m.name == "model-fp16")
        .expect("should find model-fp16 entry");
    let q8_entry = results
        .iter()
        .find(|m| m.name == "weights-q8")
        .expect("should find weights-q8 entry");

    // Verify fp16 entry.
    assert_eq!(fp16_entry.kind, ModelKind::Diffusion);
    assert_eq!(fp16_entry.dtype_hint, DType::F16);
    assert!(
        fp16_entry.vram_estimate_mib > 0,
        "vram_estimate_mib should be positive for fp16 model"
    );
    assert!(!fp16_entry.id.is_empty());
    assert_eq!(fp16_entry.id.len(), 16);
    assert!(
        fp16_entry.id.chars().all(|c| c.is_ascii_hexdigit()),
        "id should be 16 hex chars"
    );
    assert!(
        fp16_entry.path.exists(),
        "path should exist: {:?}",
        fp16_entry.path
    );

    // Verify q8 entry.
    assert_eq!(q8_entry.kind, ModelKind::Diffusion);
    assert_eq!(q8_entry.dtype_hint, DType::Q8);
    assert!(
        q8_entry.vram_estimate_mib > 0,
        "vram_estimate_mib should be positive for q8 model"
    );
    assert!(!q8_entry.id.is_empty());
    assert_eq!(q8_entry.id.len(), 16);
    assert!(
        q8_entry.id.chars().all(|c| c.is_ascii_hexdigit()),
        "id should be 16 hex chars"
    );
    assert!(
        q8_entry.path.exists(),
        "path should exist: {:?}",
        q8_entry.path
    );

    // IDs must be different for different files.
    assert_ne!(fp16_entry.id, q8_entry.id);
}
