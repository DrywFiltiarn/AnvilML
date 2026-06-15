/// Integration tests for `seed_loader.rs`.
///
/// These tests verify the SHA256-gated seed loading behavior:
/// - First run applies a new seed file (inserts into seed_history and device_capabilities).
/// - Second run skips the same seed file (SHA256 match, no additional rows).
///
/// Each test uses its own `open_in_memory()` pool for complete database isolation.
use sha2::Digest;

use anvilml_registry::open_in_memory;
use anvilml_registry::seed_loader::run;

/// Verifies that `seed_loader::run()` applies a new seed file on first run.
///
/// Creates a temporary directory with a single `.sql` seed file containing
/// `INSERT OR IGNORE INTO device_capabilities` rows, calls `run()`, and then
/// verifies:
/// - A row exists in `seed_history` for the seed file.
/// - The `device_capabilities` table contains rows from the seed SQL.
///
/// Uses a unique temp directory and in-memory pool for complete isolation.
#[tokio::test]
async fn test_seed_loader_applies_new_seed() {
    // Create a temporary directory for seed files.
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    // Write a seed SQL file with a few device capability INSERTs.
    // Using a small subset instead of the full devices.sql to keep tests fast.
    let seed_sql = r#"INSERT OR IGNORE INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 6912, 'NVIDIA TITAN X', '6.1', 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 6914, 'NVIDIA TITAN Xp', '6.1', 0, 0, 0, 0, 0, 0);
INSERT OR IGNORE INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4098, 29857, 'AMD Instinct MI300X', 'gfx942', 1, 1, 1, 1, 0, 1);"#;

    let seed_path = tmpdir.path().join("test_devices.sql");
    std::fs::write(&seed_path, seed_sql).expect("write seed file");

    // Open an in-memory database pool (runs migrations including seed_history table).
    let pool = open_in_memory().await.expect("open in-memory database");

    // Run the seed loader — this should apply the new seed file.
    run(&pool, tmpdir.path())
        .await
        .expect("seed_loader run should succeed");

    // Verify that seed_history has exactly one row for our seed file.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seed_history")
        .fetch_one(&pool)
        .await
        .expect("query seed_history count");

    assert_eq!(
        count, 1,
        "seed_history should have exactly 1 row after applying one seed"
    );

    // Verify that the device_capabilities table has 3 rows from our seed.
    let device_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM device_capabilities")
        .fetch_one(&pool)
        .await
        .expect("query device_capabilities count");

    assert_eq!(
        device_count, 3,
        "device_capabilities should have 3 rows from seed INSERTs"
    );

    // Verify the SHA256 stored in seed_history matches what we computed.
    let stored_sha256: String =
        sqlx::query_scalar("SELECT sha256 FROM seed_history WHERE file = ?")
            .bind(seed_path.canonicalize().unwrap().to_string_lossy().as_ref())
            .fetch_one(&pool)
            .await
            .expect("query stored sha256");

    // Compute expected SHA256 using sha2 crate (same as seed_loader).
    let expected_sha256 = format!("{:x}", sha2::Sha256::digest(seed_sql.as_bytes()));
    assert_eq!(
        stored_sha256, expected_sha256,
        "stored SHA256 must match computed SHA256 of seed content"
    );

    // Verify applied_at is a valid RFC3339 timestamp.
    let applied_at: String = sqlx::query_scalar("SELECT applied_at FROM seed_history")
        .fetch_one(&pool)
        .await
        .expect("query applied_at");

    // RFC3339 timestamps parse without error.
    chrono::DateTime::parse_from_rfc3339(&applied_at)
        .expect("applied_at must be valid RFC3339 timestamp");
}

/// Verifies that `seed_loader::run()` skips a seed file on second run
/// when the SHA256 matches.
///
/// Runs `run()` twice on the same temp directory with the same seed file.
/// The first run applies the seed; the second run should skip it.
/// After both runs, `seed_history` should still have exactly one row
/// (no duplicate entries).
///
/// Uses a unique temp directory and in-memory pool for complete isolation.
#[tokio::test]
async fn test_seed_loader_skips_up_to_date() {
    // Create a temporary directory for seed files.
    let tmpdir = tempfile::tempdir().expect("create temp dir");

    // Write a seed SQL file.
    let seed_sql = r#"INSERT OR IGNORE INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 6912, 'NVIDIA TITAN X', '6.1', 0, 0, 0, 0, 0, 0);"#;

    let seed_path = tmpdir.path().join("test_devices.sql");
    std::fs::write(&seed_path, seed_sql).expect("write seed file");

    // Open an in-memory database pool.
    let pool = open_in_memory().await.expect("open in-memory database");

    // First run — should apply the seed.
    run(&pool, tmpdir.path())
        .await
        .expect("first run should succeed");

    let count_after_first: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seed_history")
        .fetch_one(&pool)
        .await
        .expect("query seed_history count after first run");

    assert_eq!(
        count_after_first, 1,
        "seed_history should have 1 row after first run"
    );

    // Second run — should skip (up-to-date).
    run(&pool, tmpdir.path())
        .await
        .expect("second run should succeed");

    // seed_history should still have exactly 1 row (no duplicate).
    let count_after_second: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM seed_history")
        .fetch_one(&pool)
        .await
        .expect("query seed_history count after second run");

    assert_eq!(
        count_after_second, 1,
        "seed_history should still have 1 row after second run (no duplicate)"
    );

    // device_capabilities should still have exactly 1 row.
    let device_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM device_capabilities")
        .fetch_one(&pool)
        .await
        .expect("query device_capabilities count after second run");

    assert_eq!(
        device_count, 1,
        "device_capabilities should still have 1 row (seed was skipped)"
    );
}
