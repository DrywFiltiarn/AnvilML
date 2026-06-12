use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind};
use anvilml_registry::scanner::scan_dirs;

/// Integration test: safetensors header dtype detection overrides filename.
///
/// Creates a temp directory with a `.safetensors` file whose header declares
/// F16 tensors but the filename contains "f32". The scanner should detect
/// F16 from the header, not F32 from the filename.
#[tokio::test]
async fn test_safetensors_header_dtype_detection() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let path = tmp.path();

    // Build a safetensors header with F16 dtype strings.
    let header = serde_json::json!({
        "model.layers.0.attn.weight": "F16",
        "model.layers.0.attn.bias": "F16",
        "model.layers.1.attn.weight": "F16",
        "model.layers.1.mlp.weight": "F16",
    });
    let header_bytes = serde_json::to_vec(&header).expect("serialize header");
    let header_len = (header_bytes.len() as u64).to_le_bytes();

    let mut data = Vec::with_capacity(8 + header_bytes.len());
    data.extend_from_slice(&header_len);
    data.extend_from_slice(&header_bytes);

    // Filename says "f32" but header says "F16" — header should win.
    let file_path = path.join("model-f32.safetensors");
    std::fs::write(&file_path, &data).expect("write safetensors file");

    let dirs = vec![ModelDirConfig {
        path: path.to_path_buf(),
        kind: Some(ModelKind::Diffusion),
    }];

    let results = scan_dirs(&dirs).await;

    assert_eq!(results.len(), 1, "expected exactly 1 model entry");

    let entry = &results[0];
    assert_eq!(
        entry.dtype_hint,
        DType::F16,
        "header F16 should win over filename f32"
    );
}
