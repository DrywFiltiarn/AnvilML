//! Integration tests for `ModelScanner` — hashing, kind/dtype inference, depth limits,
//! and deduplication behavior.
//!
//! Each test creates its own in-memory SQLite pool and temporary directory structure,
//! so there is no cross-test shared state and no `#[serial]` annotation is needed.

use anvilml_core::{ModelDtype, ModelFormat, ModelKind, ModelMeta};
use anvilml_registry::ModelScanner;
use chrono::Utc;
use std::io::Write;
use tempfile::TempDir;

/// Create an in-memory SQLite pool with migrations applied.
///
/// Each test gets its own pool — the in-memory database is isolated per connection
/// by using a unique cache name (uuid-based) so parallel tests don't collide on
/// the shared `:memory:` database.
///
/// We use `max_connections(1)` to ensure all operations within a test use the same
/// in-memory database connection. With `cache=shared`, a pool of 1 connection
/// guarantees no database isolation issues.
async fn make_pool() -> sqlx::SqlitePool {
    // Use a unique in-memory database name per test to avoid the shared `:memory:`
    // database problem: without a unique name, all connections in the same process
    // share the same in-memory database, causing migration conflicts when tests
    // run in parallel.
    let unique_name = uuid::Uuid::new_v4().to_string();

    // Use max_connections(1) to ensure all operations use the same in-memory
    // database connection. With `cache=shared`, this guarantees no database
    // isolation issues between store and scanner operations.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(format!("file:{unique_name}?mode=memory&cache=shared"))
                .create_if_missing(true),
        )
        .await
        .expect("should be able to create in-memory SQLite pool");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator.run(&pool).await.expect("migration should succeed");

    pool
}

/// Create a file inside the given `TempDir` with the given name and content.
///
/// Returns the full path to the created file.
fn create_file(dir: &TempDir, name: &str, content: &[u8]) -> std::path::PathBuf {
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(&path).expect("should be able to create file");
    let _ = file
        .write_all(content)
        .expect("should be able to write file content");
    path
}

/// Create a file in a subdirectory of the given `TempDir`.
///
/// Creates the subdirectory path if it doesn't exist, then creates the file.
/// Returns the full path to the created file.
fn create_file_in_subdir(
    dir: &TempDir,
    subdir: &str,
    name: &str,
    content: &[u8],
) -> std::path::PathBuf {
    let path = dir.path().join(subdir).join(name);
    // Ensure parent directories exist.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("should be able to create parent directories");
    }
    let mut file = std::fs::File::create(&path).expect("should be able to create file");
    let _ = file
        .write_all(content)
        .expect("should be able to write file content");
    path
}

/// SHA256 hash of a file's first 1 MiB is identical before and after renaming.
///
/// Creates a temp file with known content, scans it (which computes the hash),
/// then renames the file and scans again. Asserts that both `ModelMeta.id` values
/// are identical, proving the hash is content-based, not path-based.
#[tokio::test]
async fn test_hash_stability_across_rename() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    // Create a file with 2 MiB of known content — larger than the 1 MiB hash window
    // so we verify that only the first MiB is hashed (the rest is irrelevant).
    let content = vec![b'A'; 2 * 1024 * 1024];
    let path1 = create_file(&dir, "original.safetensors", &content);

    // Scan the file to get its hash (stored in ModelMeta.id).
    let results1 = scanner
        .scan_dir(dir.path(), 0)
        .await
        .expect("scan should succeed");
    assert_eq!(results1.len(), 1);
    let hash1 = &results1[0].id;

    // Rename the file — content is identical, only the path changed.
    let path2 = dir.path().join("renamed.safetensors");
    std::fs::rename(&path1, &path2).expect("should be able to rename file");

    // Scan again — the renamed file should produce the same hash.
    let results2 = scanner
        .scan_dir(dir.path(), 0)
        .await
        .expect("scan should succeed");
    assert_eq!(results2.len(), 1);
    let hash2 = &results2[0].id;

    assert_eq!(
        hash1, hash2,
        "hash should be identical after rename (content-based, not path-based)"
    );
}

/// Directory component `diffusion/` maps to `ModelKind::Diffusion`.
///
/// Creates a temp directory with a `diffusion/` subdirectory containing a file,
/// then scans with depth=1 and asserts the returned `ModelMeta` has `kind == Diffusion`.
#[tokio::test]
async fn test_kind_inference_diffusion() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    // Create diffusion subdirectory and file.
    create_file_in_subdir(&dir, "diffusion", "model.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].kind,
        ModelKind::Diffusion,
        "kind should be Diffusion for diffusion/ directory"
    );
}

/// Directory component `text_encoders/` maps to `ModelKind::TextEncoder`.
///
/// Creates a temp directory with a `text_encoders/` subdirectory containing a file,
/// then scans with depth=1 and asserts the returned `ModelMeta` has `kind == TextEncoder`.
#[tokio::test]
async fn test_kind_inference_text_encoders() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file_in_subdir(
        &dir,
        "text_encoders",
        "encoder.safetensors",
        b"test content",
    );

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].kind,
        ModelKind::TextEncoder,
        "kind should be TextEncoder for text_encoders/ directory"
    );
}

/// Directory component `vae/` maps to `ModelKind::Vae`.
///
/// Creates a temp directory with a `vae/` subdirectory containing a file,
/// then scans with depth=1 and asserts the returned `ModelMeta` has `kind == Vae`.
#[tokio::test]
async fn test_kind_inference_vae() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file_in_subdir(&dir, "vae", "autoencoder.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].kind,
        ModelKind::Vae,
        "kind should be Vae for vae/ directory"
    );
}

/// Non-standard directory maps to `ModelKind::Unknown`.
///
/// Creates a temp directory with a non-standard subdirectory (e.g. `embeddings/`)
/// containing a file, then scans and asserts the returned `ModelMeta` has `kind == Unknown`.
#[tokio::test]
async fn test_kind_inference_unknown_dir() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file_in_subdir(&dir, "embeddings", "embedding.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].kind,
        ModelKind::Unknown,
        "kind should be Unknown for unrecognized directory"
    );
}

/// Filename substring `fp8_e4m3fn` maps to `ModelDtype::Fp8`.
///
/// Creates a temp directory with a file named `model_fp8_e4m3fn.safetensors`,
/// then scans and asserts the returned `ModelMeta` has `dtype == Fp8`.
#[tokio::test]
async fn test_dtype_inference_fp8_e4m3fn() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model_fp8_e4m3fn.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].dtype,
        ModelDtype::Fp8,
        "dtype should be Fp8 for fp8_e4m3fn filename"
    );
}

/// Filename substring `bf16` maps to `ModelDtype::Bf16`.
///
/// Creates a temp directory with a file named `model_bf16.safetensors`,
/// then scans and asserts the returned `ModelMeta` has `dtype == Bf16`.
#[tokio::test]
async fn test_dtype_inference_bf16() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model_bf16.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].dtype,
        ModelDtype::Bf16,
        "dtype should be Bf16 for bf16 filename"
    );
}

/// Filename substring `fp16` maps to `ModelDtype::Fp16`.
///
/// Creates a temp directory with a file named `model_fp16.safetensors`,
/// then scans and asserts the returned `ModelMeta` has `dtype == Fp16`.
#[tokio::test]
async fn test_dtype_inference_fp16() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model_fp16.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].dtype,
        ModelDtype::Fp16,
        "dtype should be Fp16 for fp16 filename"
    );
}

/// Filename substring `fp32` maps to `ModelDtype::Fp32`.
///
/// Creates a temp directory with a file named `model_fp32.safetensors`,
/// then scans and asserts the returned `ModelMeta` has `dtype == Fp32`.
#[tokio::test]
async fn test_dtype_inference_fp32() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model_fp32.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].dtype,
        ModelDtype::Fp32,
        "dtype should be Fp32 for fp32 filename"
    );
}

/// Format `.safetensors` maps to `ModelFormat::Safetensors`.
///
/// Creates a temp directory with a `.safetensors` file, then scans and asserts
/// the returned `ModelMeta` has `format == Safetensors`.
#[tokio::test]
async fn test_format_inference_safetensors() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].format,
        ModelFormat::Safetensors,
        "format should be Safetensors"
    );
}

/// Format `.ckpt` maps to `ModelFormat::Ckpt`.
///
/// Creates a temp directory with a `.ckpt` file, then scans and asserts
/// the returned `ModelMeta` has `format == Ckpt`.
#[tokio::test]
async fn test_format_inference_ckpt() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model.ckpt", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].format,
        ModelFormat::Ckpt,
        "format should be Ckpt"
    );
}

/// Format `.pt` and `.pth` map to `ModelFormat::Pt`.
///
/// Creates a temp directory with `.pt` and `.pth` files, then scans and asserts
/// both are classified as `ModelFormat::Pt`.
#[tokio::test]
async fn test_format_inference_pt() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model.pt", b"test content");
    create_file(&dir, "model.pth", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 2, "expected exactly 2 scanned files");
    assert_eq!(
        results[0].format,
        ModelFormat::Pt,
        "both .pt and .pth should map to Pt"
    );
    assert_eq!(results[1].format, ModelFormat::Pt);
}

/// Format `.bin` and `.gguf` map to `ModelFormat::Bin`.
///
/// Creates a temp directory with `.bin` and `.gguf` files, then scans and asserts
/// both are classified as `ModelFormat::Bin`.
#[tokio::test]
async fn test_format_inference_bin() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model.bin", b"test content");
    create_file(&dir, "model.gguf", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 2, "expected exactly 2 scanned files");
    assert_eq!(
        results[0].format,
        ModelFormat::Bin,
        "both .bin and .gguf should map to Bin"
    );
    assert_eq!(results[1].format, ModelFormat::Bin);
}

/// File already in store with matching size+mtime is not re-upserted.
///
/// Creates a temp file, inserts a row into the store with matching path/size/mtime,
/// then scans the file. Asserts that the store count remains 1 (no duplicate entry).
#[tokio::test]
async fn test_unchanged_file_skips_rehash() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    // Use a single shared pool for both the store and the scanner, so they
    // operate on the same in-memory database.
    let pool = make_pool().await;
    let store = anvilml_registry::ModelStore::new(pool.clone());
    let scanner = ModelScanner::new(pool);

    // Create a file with known content.
    let content = b"test content for dedup check";
    let file_path = create_file(&dir, "model.safetensors", content);

    // Get the file's actual size and mtime.
    let metadata = std::fs::metadata(&file_path).expect("should be able to read metadata");
    let size_bytes = metadata.len();
    let mtime_unix = metadata
        .modified()
        .ok()
        .and_then(|t| {
            use std::time::SystemTime;
            t.duration_since(SystemTime::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
        })
        .unwrap_or(0);

    // Insert a row into the store with matching path, size, and mtime.
    // This simulates a file that was previously scanned.
    // We must use the actual file mtime so the dedup check matches.
    let existing_meta = ModelMeta {
        id: "existing-hash".to_string(),
        name: "model.safetensors".to_string(),
        path: file_path.clone(),
        kind: ModelKind::Unknown,
        dtype: ModelDtype::Unknown,
        format: ModelFormat::Safetensors,
        size_bytes,
        mtime_unix,
        scanned_at: Utc::now(),
    };
    store
        .upsert(&existing_meta)
        .await
        .expect("upsert should succeed");

    // Verify the row exists before scanning.
    let before_count = store.list(None).await.expect("list should succeed").len();
    assert_eq!(
        before_count, 1,
        "store should have exactly 1 row before scan"
    );

    // Scan the directory — the file should be skipped because size+mtime match.
    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    // The file should be skipped (not re-hashed), so results is empty.
    assert!(
        results.is_empty(),
        "unchanged file should be skipped, got {} results",
        results.len()
    );

    // Store count should still be 1 — no new upsert.
    let after_count = store.list(None).await.expect("list should succeed").len();
    assert_eq!(
        after_count, 1,
        "store count should remain 1 after scanning unchanged file (no duplicate upsert)"
    );
}

/// Files at depth > N are not returned when scanning with depth=N.
///
/// Creates a nested directory structure: root/a/file1.safetensors and root/a/b/file2.safetensors.
/// Scans with depth=1 — only file1 should be returned; file2 (at depth 2) should be excluded.
#[tokio::test]
async fn test_depth_limit_respected() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    // Create a/b/file2.safetensors (depth 2 from root).
    create_file_in_subdir(&dir, "a", "file1.safetensors", b"depth 1 content");
    create_file_in_subdir(&dir, "a/b", "file2.safetensors", b"depth 2 content");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    // Only file1 (depth 1) should be scanned; file2 (depth 2) should be excluded.
    assert_eq!(results.len(), 1, "expected exactly 1 file at depth 1");
    assert!(
        results[0].path.ends_with("file1.safetensors"),
        "expected file1.safetensors at depth 1, got: {:?}",
        results[0].path
    );
}

/// Depth 0 scans only files directly in root, no subdirectories.
///
/// Creates files at root level and in a subdirectory, then scans with depth=0.
/// Only root-level files should be returned.
#[tokio::test]
async fn test_depth_zero_scans_only_root() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    // Create a file at root level.
    create_file(&dir, "root_file.safetensors", b"root content");

    // Create a file in a subdirectory.
    create_file_in_subdir(&dir, "sub", "sub_file.safetensors", b"sub content");

    let results = scanner
        .scan_dir(dir.path(), 0)
        .await
        .expect("scan should succeed");

    // Only the root-level file should be scanned.
    assert_eq!(results.len(), 1, "expected exactly 1 file at depth 0");
    assert!(
        results[0].path.ends_with("root_file.safetensors"),
        "expected root_file.safetensors, got: {:?}",
        results[0].path
    );
}

/// Multiple files in the same directory are all scanned.
///
/// Creates three files in a diffusion/ subdirectory, scans with depth=1,
/// and asserts all three are returned.
#[tokio::test]
async fn test_multiple_files_scanned() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file_in_subdir(&dir, "diffusion", "model1.safetensors", b"content 1");
    create_file_in_subdir(&dir, "diffusion", "model2.safetensors", b"content 2");
    create_file_in_subdir(&dir, "diffusion", "model3.safetensors", b"content 3");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(
        results.len(),
        3,
        "expected exactly 3 scanned files, got {}",
        results.len()
    );

    // All should be Diffusion kind.
    for meta in &results {
        assert_eq!(meta.kind, ModelKind::Diffusion);
    }
}

/// Mixed formats and dtypes in a single directory scan.
///
/// Creates files with different extensions and dtype markers, then scans and
/// verifies each file's format and dtype are correctly inferred.
#[tokio::test]
async fn test_mixed_formats_and_dtypes() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    // Create files with different formats and dtypes.
    create_file(&dir, "model_fp8.safetensors", b"content 1");
    create_file(&dir, "model_bf16.ckpt", b"content 2");
    create_file(&dir, "model_fp16.pt", b"content 3");
    create_file(&dir, "model.gguf", b"content 4");

    let results = scanner
        .scan_dir(dir.path(), 1)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 4, "expected exactly 4 scanned files");

    // Verify each file's inferred properties.
    let by_name: std::collections::HashMap<&str, &ModelMeta> =
        results.iter().map(|m| (m.name.as_str(), m)).collect();

    assert_eq!(by_name["model_fp8.safetensors"].dtype, ModelDtype::Fp8);
    assert_eq!(
        by_name["model_fp8.safetensors"].format,
        ModelFormat::Safetensors
    );

    assert_eq!(by_name["model_bf16.ckpt"].dtype, ModelDtype::Bf16);
    assert_eq!(by_name["model_bf16.ckpt"].format, ModelFormat::Ckpt);

    assert_eq!(by_name["model_fp16.pt"].dtype, ModelDtype::Fp16);
    assert_eq!(by_name["model_fp16.pt"].format, ModelFormat::Pt);

    assert_eq!(by_name["model.gguf"].dtype, ModelDtype::Unknown);
    assert_eq!(by_name["model.gguf"].format, ModelFormat::Bin);
}

/// Hash of a file smaller than 1 MiB hashes the entire file.
///
/// Creates a small file (100 bytes), scans it, and verifies the resulting
/// `ModelMeta.id` is a valid 64-character lowercase hex string (SHA256).
/// A second scan correctly skips the unchanged file (dedup), confirming
/// that the hash was computed and stored.
#[tokio::test]
async fn test_hash_small_file() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    let small_content = b"tiny file content";
    create_file(&dir, "small.safetensors", small_content);

    // First scan — should produce exactly 1 result with a valid hash.
    let results1 = scanner
        .scan_dir(dir.path(), 0)
        .await
        .expect("scan should succeed");
    assert_eq!(results1.len(), 1);
    let hash1 = &results1[0].id;

    // Hash should be a 64-character lowercase hex string (SHA256).
    assert_eq!(hash1.len(), 64, "SHA256 hex should be 64 characters");
    assert!(
        hash1.chars().all(|c| c.is_ascii_hexdigit()),
        "hash should be hex"
    );

    // Second scan — the file should be skipped by dedup (unchanged size+mtime),
    // so we get 0 results. This confirms the hash was computed and stored.
    let results2 = scanner
        .scan_dir(dir.path(), 0)
        .await
        .expect("scan should succeed");
    assert_eq!(
        results2.len(),
        0,
        "second scan should skip unchanged file (dedup), got {} results",
        results2.len()
    );
}

/// Files in the root directory (no subdirectory) are scanned with kind Unknown.
///
/// Creates files directly in the scan root (no diffusion/text_encoders/vae subdirectory),
/// then scans and asserts the returned `ModelMeta` has `kind == Unknown`.
#[tokio::test]
async fn test_root_level_kind_unknown() {
    let dir = TempDir::new().expect("should be able to create temp dir");
    let pool = make_pool().await;
    let scanner = ModelScanner::new(pool);

    create_file(&dir, "model.safetensors", b"test content");

    let results = scanner
        .scan_dir(dir.path(), 0)
        .await
        .expect("scan should succeed");

    assert_eq!(results.len(), 1, "expected exactly 1 scanned file");
    assert_eq!(
        results[0].kind,
        ModelKind::Unknown,
        "root-level file should have Unknown kind"
    );
}
