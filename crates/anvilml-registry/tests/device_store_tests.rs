/// Integration tests for `device_store.rs` — `DeviceCapabilityStore` SQLite lookups.
///
/// These tests verify the complete lookup behavior of `DeviceCapabilityStore`:
/// - `get()` returns correct fields for an existing device
/// - `get()` returns `None` for a non-existent device
/// - Boolean flags stored as `1` map to `true`
/// - Boolean flags stored as `0` map to `false`
///
/// Each test creates its own in-memory database via `open_in_memory()`,
/// ensuring complete database isolation between tests.
use anvilml_registry::{open_in_memory, DeviceCapabilityStore};

/// Verifies that `get()` returns `Some(DeviceRow)` with correct fields
/// for a known PCI vendor/device pair.
///
/// Inserts a device row via raw SQL (H100-SXM5-80GB), then calls `get()`
/// and asserts that all fields match the inserted values.
#[tokio::test]
async fn test_get_existing_device() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    // Clone the pool so we can use it for raw SQL inserts while the store
    // also holds a reference to the same pool. With max_connections(1)
    // from open_in_memory(), all operations see the same in-memory DB.
    let store = DeviceCapabilityStore::new(pool.clone()).await;

    // Insert a known device row via raw SQL.
    // Using H100-SXM5-80GB PCI pair (vendor=4318, device=8994).
    // This specific PCI pair may or may not be covered by seed data,
    // so we insert it explicitly to guarantee the row exists.
    sqlx::query(
        "INSERT OR IGNORE INTO device_capabilities \
         (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) \
         VALUES (4318, 8994, 'NVIDIA H100-SXM5-80GB', '9.0', 1, 1, 1, 1, 0, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert test device");

    let result = store
        .get(4318, 8994)
        .await
        .expect("get should succeed")
        .expect("device should exist");

    assert_eq!(result.vendor_id, 4318);
    assert_eq!(result.device_id, 8994);
    assert_eq!(result.name, "NVIDIA H100-SXM5-80GB");
    assert_eq!(result.arch, "9.0");
    assert!(result.fp32);
    assert!(result.fp16);
    assert!(result.bf16);
    assert!(result.fp8);
    assert!(!result.fp4);
    assert!(result.flash_attention);
}

/// Verifies that `get()` returns `Ok(None)` for a non-existent PCI pair.
///
/// Creates a fresh in-memory database with no device rows, then calls
/// `get()` with arbitrary vendor/device IDs that have no matching row.
/// Asserts that the result is `Ok(None)`, not an error.
#[tokio::test]
async fn test_get_not_found() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    let store = DeviceCapabilityStore::new(pool).await;

    let result = store
        .get(9999, 9999)
        .await
        .expect("get should not error for missing device");

    assert!(
        result.is_none(),
        "get for non-existent device must return None, got {:?}",
        result
    );
}

/// Verifies that boolean flags stored as `1` in SQLite map to `true`
/// in the `DeviceRow` struct.
///
/// Inserts a device row with all capability flags set to `1`, then
/// calls `get()` and asserts that every boolean field is `true`.
#[tokio::test]
async fn test_get_all_caps_true() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    // Clone the pool so we can use it for raw SQL inserts while the store
    // also holds a reference to the same pool.
    let store = DeviceCapabilityStore::new(pool.clone()).await;

    // Insert a device with all capability flags set to 1.
    sqlx::query(
        "INSERT OR IGNORE INTO device_capabilities \
         (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) \
         VALUES (4318, 10001, 'Test All-Cap Device', '9.9', 1, 1, 1, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("insert test device");

    let result = store
        .get(4318, 10001)
        .await
        .expect("get should succeed")
        .expect("device should exist");

    assert!(result.fp32, "fp32 must be true");
    assert!(result.fp16, "fp16 must be true");
    assert!(result.bf16, "bf16 must be true");
    assert!(result.fp8, "fp8 must be true");
    assert!(result.fp4, "fp4 must be true");
    assert!(result.flash_attention, "flash_attention must be true");
}

/// Verifies that boolean flags stored as `0` in SQLite map to `false`
/// in the `DeviceRow` struct.
///
/// Inserts a device row with all capability flags set to `0`, then
/// calls `get()` and asserts that every boolean field is `false`.
#[tokio::test]
async fn test_get_all_caps_false() {
    let pool = open_in_memory().await.expect("open in-memory DB");
    // Clone the pool so we can use it for raw SQL inserts while the store
    // also holds a reference to the same pool.
    let store = DeviceCapabilityStore::new(pool.clone()).await;

    // Insert a device with all capability flags set to 0.
    sqlx::query(
        "INSERT OR IGNORE INTO device_capabilities \
         (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) \
         VALUES (4318, 10002, 'Test No-Cap Device', '6.0', 0, 0, 0, 0, 0, 0)",
    )
    .execute(&pool)
    .await
    .expect("insert test device");

    let result = store
        .get(4318, 10002)
        .await
        .expect("get should succeed")
        .expect("device should exist");

    assert!(!result.fp32, "fp32 must be false");
    assert!(!result.fp16, "fp16 must be false");
    assert!(!result.bf16, "bf16 must be false");
    assert!(!result.fp8, "fp8 must be false");
    assert!(!result.fp4, "fp4 must be false");
    assert!(!result.flash_attention, "flash_attention must be false");
}
