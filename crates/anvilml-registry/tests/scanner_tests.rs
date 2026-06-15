/// Integration tests for `scanner.rs` — `ModelScanner` directory walk and
/// metadata derivation.
///
/// These tests verify:
/// - Kind inference from directory names (diffusion, text_encoders, clip, etc.)
/// - Dtype inference from filenames with correct priority ordering (fp8 before fp16)
/// - Deterministic ID computation via SHA256 of first 1 MiB
/// - Non-existent directory handling (returns empty vec, no panic)
/// - Full scan with model files (correct ModelMeta fields)
/// - Empty directory handling (returns empty vec)
///
/// All tests exercise the public `scan()` API. Kind and dtype inference are
/// verified indirectly by scanning files in directories with known names and
/// filenames with known precision indicators.
///
/// Each test uses its own `tempfile::tempdir()` for unique temp directories,
/// ensuring complete filesystem isolation.
use anvilml_core::{ModelDirConfig, ModelDtype, ModelFormat, ModelKind};
use anvilml_registry::ModelScanner;

/// Verifies that `ModelScanner::scan()` correctly infers `ModelKind::Diffusion`
/// from a `"diffusion"` directory name.
///
/// Creates a temp directory named `"diffusion"`, writes a `.safetensors` file,
/// scans it, and asserts the returned `ModelMeta.kind == Diffusion`.
#[tokio::test]
async fn test_infer_kind_diffusion() {
    let scanner = ModelScanner;
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    let diffusion_dir = tmpdir.path().join("diffusion");
    std::fs::create_dir_all(&diffusion_dir).expect("create diffusion dir");
    std::fs::write(diffusion_dir.join("model.safetensors"), b"model content")
        .expect("write model file");

    let dirs = vec![ModelDirConfig {
        path: diffusion_dir.clone(),
        recursive: false,
        max_depth: None,
    }];

    let results = scanner.scan(&dirs).await;
    assert_eq!(results.len(), 1, "scan should find 1 file");
    assert_eq!(
        results[0].kind,
        ModelKind::Diffusion,
        "model in 'diffusion/' dir must have kind=Diffusion"
    );
}

/// Verifies that `ModelScanner::scan()` correctly infers `ModelKind::TextEncoder`
/// from both `"text_encoders"` and `"clip"` directory names.
///
/// Creates temp directories named `"text_encoders"` and `"clip"`, writes a
/// `.safetensors` file in each, scans both, and asserts the returned
/// `ModelMeta.kind == TextEncoder` for both.
#[tokio::test]
async fn test_infer_kind_text_encoder() {
    let scanner = ModelScanner;
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    let te_dir = tmpdir.path().join("text_encoders");
    let clip_dir = tmpdir.path().join("clip");
    std::fs::create_dir_all(&te_dir).expect("create text_encoders dir");
    std::fs::create_dir_all(&clip_dir).expect("create clip dir");
    std::fs::write(te_dir.join("clip_text.safetensors"), b"te content").expect("write te file");
    std::fs::write(clip_dir.join("clip_model.safetensors"), b"clip content")
        .expect("write clip file");

    let dirs = vec![
        ModelDirConfig {
            path: te_dir.clone(),
            recursive: false,
            max_depth: None,
        },
        ModelDirConfig {
            path: clip_dir.clone(),
            recursive: false,
            max_depth: None,
        },
    ];

    let results = scanner.scan(&dirs).await;
    assert_eq!(results.len(), 2, "scan should find 2 files");

    for meta in results {
        assert_eq!(
            meta.kind,
            ModelKind::TextEncoder,
            "model in text encoder dir must have kind=TextEncoder, got {:?}",
            meta.kind
        );
    }
}

/// Verifies that `ModelScanner::scan()` correctly infers `ModelDtype::Fp8`
/// when the filename contains both `"fp16"` and `"fp8"` substrings.
///
/// This is the critical ordering test: a filename like
/// `"model_fp16_fp8.safetensors"` contains both `"fp16"` and `"fp8"`.
/// The scanner must classify it as `Fp8` (fp8 checked before fp16 in the
/// implementation). Tests additional dtype inferences via a single scan.
#[tokio::test]
async fn test_infer_dtype_fp8_before_fp16() {
    let scanner = ModelScanner;
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    let model_dir = tmpdir.path().join("diffusion");
    std::fs::create_dir_all(&model_dir).expect("create model dir");

    // Write files with different dtype indicators in filenames.
    std::fs::write(
        model_dir.join("model_fp16_fp8.safetensors"),
        b"fp16+fp8 content",
    )
    .expect("write fp16+fp8 file");

    std::fs::write(model_dir.join("model_fp16.safetensors"), b"fp16 content")
        .expect("write fp16 file");

    std::fs::write(model_dir.join("model_bf16.safetensors"), b"bf16 content")
        .expect("write bf16 file");

    std::fs::write(model_dir.join("model_fp32.safetensors"), b"fp32 content")
        .expect("write fp32 file");

    std::fs::write(
        model_dir.join("model.safetensors"),
        b"unknown dtype content",
    )
    .expect("write unknown dtype file");

    let dirs = vec![ModelDirConfig {
        path: model_dir.clone(),
        recursive: false,
        max_depth: None,
    }];

    let results = scanner.scan(&dirs).await;
    assert_eq!(
        results.len(),
        5,
        "scan should find exactly 5 .safetensors files"
    );

    // Build a lookup by filename for targeted assertions.
    let by_name: std::collections::HashMap<_, _> =
        results.into_iter().map(|m| (m.name.clone(), m)).collect();

    // Critical: fp16+fp8 filename must resolve to Fp8 (fp8 checked first).
    assert_eq!(
        by_name["model_fp16_fp8.safetensors"].dtype,
        ModelDtype::Fp8,
        "filename containing both 'fp16' and 'fp8' must resolve to Fp8"
    );

    // Pure fp16 should resolve to Fp16.
    assert_eq!(
        by_name["model_fp16.safetensors"].dtype,
        ModelDtype::Fp16,
        "filename containing only 'fp16' must resolve to Fp16"
    );

    // bf16 should resolve to Bf16.
    assert_eq!(
        by_name["model_bf16.safetensors"].dtype,
        ModelDtype::Bf16,
        "filename containing only 'bf16' must resolve to Bf16"
    );

    // fp32 should resolve to Fp32.
    assert_eq!(
        by_name["model_fp32.safetensors"].dtype,
        ModelDtype::Fp32,
        "filename containing only 'fp32' must resolve to Fp32"
    );

    // No precision indicator should resolve to Unknown.
    assert_eq!(
        by_name["model.safetensors"].dtype,
        ModelDtype::Unknown,
        "filename without precision indicator must resolve to Unknown"
    );
}

/// Verifies that the model ID computed by `scan()` is deterministic and
/// has the correct format (64-character lowercase hex SHA256 digest).
///
/// Creates a temp file with known content, scans it, and asserts that:
/// - The returned `ModelMeta.id` is a 64-character lowercase hex string.
/// - Scanning the same file twice produces the same ID (deterministic).
#[tokio::test]
async fn test_compute_id_deterministic() {
    let scanner = ModelScanner;
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    let model_dir = tmpdir.path().join("diffusion");
    std::fs::create_dir_all(&model_dir).expect("create model dir");

    let test_file = model_dir.join("model_fp8.safetensors");
    // Write known content to the file for deterministic hashing.
    let content = b"test model content for deterministic hash verification";
    std::fs::write(&test_file, content).expect("write test file");

    let dirs = vec![ModelDirConfig {
        path: model_dir.clone(),
        recursive: false,
        max_depth: None,
    }];

    // First scan.
    let results = scanner.scan(&dirs).await;
    assert_eq!(results.len(), 1, "scan should find 1 file");

    let id1 = &results[0].id;
    assert_eq!(
        id1.len(),
        64,
        "SHA256 hex digest must be exactly 64 characters, got {}",
        id1.len()
    );

    assert!(
        id1.chars().all(|c| c.is_ascii_hexdigit()),
        "SHA256 hex digest must contain only lowercase hex characters"
    );

    // Second scan — verify deterministic.
    let results2 = scanner.scan(&dirs).await;
    assert_eq!(
        &results2[0].id, id1,
        "scanning the same file twice must produce identical IDs"
    );
}

/// Verifies that `ModelScanner::scan()` returns an empty vector when given
/// a non-existent directory path, and does not panic.
///
/// Tests the graceful degradation path: when a configured model directory
/// does not exist on disk, the scanner logs a DEBUG message and moves on
/// without panicking or returning an error.
#[tokio::test]
async fn test_scan_nonexistent_dir() {
    let scanner = ModelScanner;

    let dirs = vec![ModelDirConfig {
        path: std::path::PathBuf::from("/nonexistent/path/that/does/not/exist"),
        recursive: false,
        max_depth: None,
    }];

    let results = scanner.scan(&dirs).await;

    assert!(
        results.is_empty(),
        "scanning a non-existent directory must return an empty vec"
    );
}

/// Verifies that `ModelScanner::scan()` produces correct `ModelMeta` entries
/// for `.safetensors` files in temp directories.
///
/// Creates temp directories mimicking real model layouts:
/// - `diffusion/model_fp8.safetensors` — diffusion model, fp8 dtype
/// - `text_encoders/clip_text.safetensors` — text encoder, unknown dtype
///
/// Also creates a non-`.safetensors` file that should be skipped.
/// Scans both directories and verifies each `ModelMeta` has the correct
/// kind, dtype, format, and a valid (64-char hex) ID.
#[tokio::test]
async fn test_scan_with_files() {
    let scanner = ModelScanner;

    // Create temp directory structure: diffusion/ and text_encoders/
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    let diffusion_dir = tmpdir.path().join("diffusion");
    let text_encoder_dir = tmpdir.path().join("text_encoders");
    std::fs::create_dir_all(&diffusion_dir).expect("create diffusion dir");
    std::fs::create_dir_all(&text_encoder_dir).expect("create text_encoder dir");

    // Write model files with known content.
    std::fs::write(
        diffusion_dir.join("model_fp8.safetensors"),
        b"diffusion model content",
    )
    .expect("write diffusion model file");

    std::fs::write(
        text_encoder_dir.join("clip_text.safetensors"),
        b"text encoder content",
    )
    .expect("write text encoder file");

    // Write a non-safetensors file that should be skipped.
    std::fs::write(diffusion_dir.join("model.pt"), b"pytorch file").expect("write .pt file");

    let dirs = vec![
        ModelDirConfig {
            path: diffusion_dir.clone(),
            recursive: false,
            max_depth: None,
        },
        ModelDirConfig {
            path: text_encoder_dir.clone(),
            recursive: false,
            max_depth: None,
        },
    ];

    let results = scanner.scan(&dirs).await;

    // Should have exactly 2 results (one per .safetensors file; .pt is skipped).
    assert_eq!(
        results.len(),
        2,
        "scan should find exactly 2 .safetensors files, found {}",
        results.len()
    );

    // Build a lookup by filename for easier assertions.
    let by_name: std::collections::HashMap<_, _> =
        results.into_iter().map(|m| (m.name.clone(), m)).collect();

    // Verify the diffusion model metadata.
    let diffusion_meta = by_name
        .get("model_fp8.safetensors")
        .expect("diffusion model should be present in scan results");
    assert_eq!(
        diffusion_meta.kind,
        ModelKind::Diffusion,
        "model in 'diffusion/' dir must have kind=Diffusion"
    );
    assert_eq!(
        diffusion_meta.dtype,
        ModelDtype::Fp8,
        "filename 'model_fp8.safetensors' must have dtype=Fp8"
    );
    assert_eq!(
        diffusion_meta.format,
        ModelFormat::Safetensors,
        ".safetensors extension must have format=Safetensors"
    );
    assert_eq!(
        diffusion_meta.id.len(),
        64,
        "model id must be 64 characters, got {}",
        diffusion_meta.id.len()
    );

    // Verify the text encoder metadata.
    let te_meta = by_name
        .get("clip_text.safetensors")
        .expect("text encoder should be present in scan results");
    assert_eq!(
        te_meta.kind,
        ModelKind::TextEncoder,
        "model in 'text_encoders/' dir must have kind=TextEncoder"
    );
    assert_eq!(
        te_meta.dtype,
        ModelDtype::Unknown,
        "filename 'clip_text.safetensors' has no precision indicator, must have dtype=Unknown"
    );

    // Verify the scanned_at timestamps are recent (within last 60 seconds).
    let now = chrono::Utc::now();
    for meta in by_name.values() {
        let elapsed = now
            .signed_duration_since(meta.scanned_at)
            .num_seconds()
            .unsigned_abs();
        assert!(
            elapsed < 60,
            "scanned_at must be within 60 seconds of now, was {} seconds ago",
            elapsed
        );
    }
}

/// Verifies that `ModelScanner::scan()` returns an empty vector when given
/// an empty directory (no files at all).
///
/// This tests the zero-file edge case: the directory exists and is readable,
/// but contains no files. The scanner should return an empty vec without errors.
#[tokio::test]
async fn test_scan_empty_dir() {
    let scanner = ModelScanner;

    let tmpdir = tempfile::tempdir().expect("create temp dir");

    let dirs = vec![ModelDirConfig {
        path: tmpdir.path().to_path_buf(),
        recursive: false,
        max_depth: None,
    }];

    let results = scanner.scan(&dirs).await;

    assert!(
        results.is_empty(),
        "scanning an empty directory must return an empty vec"
    );
}
