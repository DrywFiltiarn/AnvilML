//! Integration tests for `DeviceCapabilityStore` — PCI-ID lookup on the `device_capabilities` table.
//!
//! Each test creates its own in-memory SQLite pool with migrations applied,
//! so there is no cross-test shared state and no `#[serial]` annotation is needed.

use anvilml_registry::DeviceCapabilityStore;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Create an in-memory SQLite pool with migrations applied.
///
/// Each test gets its own pool — the in-memory database is isolated per connection
/// by using a unique cache name (uuid-based) so parallel tests don't collide on
/// the shared `:memory:` database.
///
/// The migration from `database/migrations/001_initial.sql` is applied so the
/// `device_capabilities` table exists before any lookup.
async fn make_pool() -> SqlitePool {
    // Use a unique in-memory database name per test to avoid the shared `:memory:`
    // database problem: without a unique name, all connections in the same process
    // share the same in-memory database, causing cross-test interference.
    let unique_name = uuid::Uuid::new_v4().to_string();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(format!("file:{unique_name}?mode=memory&cache=shared"))
                .create_if_missing(true),
        )
        .await
        .expect("should be able to create in-memory SQLite pool");

    // Apply the migration so the `device_capabilities` table exists.
    // sqlx::migrate!() embeds the migration at compile time; .run() applies
    // any pending migrations (idempotent — running against an already-migrated
    // database is a no-op).
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator.run(&pool).await.expect("migration should succeed");

    pool
}

/// Insert a row into `device_capabilities` for testing lookups.
///
/// All boolean columns are set explicitly; the `name` and `arch` columns
/// are set to placeholder values since they are not used by the lookup.
async fn insert_device_caps(
    pool: &SqlitePool,
    vendor_id: i64,
    device_id: i64,
    fp32: i64,
    fp16: i64,
    bf16: i64,
    fp8: i64,
    fp4: i64,
    flash_attention: i64,
) {
    sqlx::query(
        "INSERT INTO device_capabilities \
         (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(vendor_id)
    .bind(device_id)
    .bind("Test Device")
    .bind("Test Arch")
    .bind(fp32)
    .bind(fp16)
    .bind(bf16)
    .bind(fp8)
    .bind(fp4)
    .bind(flash_attention)
    .execute(pool)
    .await
    .expect("insert should succeed");
}

/// `lookup` on a known PCI-ID pair with all-true caps returns `Some(InferenceCaps)`.
///
/// Inserts a row with vendor_id=0x10DE, device_id=0x2684, all capability columns=1,
/// then looks it up and asserts that every bool field is `true`.
#[tokio::test]
async fn test_lookup_known_pciid_returns_caps() {
    let pool = make_pool().await;

    // Insert the row before creating the store — the store takes ownership of the pool.
    insert_device_caps(&pool, 0x10DE, 0x2684, 1, 1, 1, 1, 0, 1).await;

    let store = DeviceCapabilityStore::new(pool);

    let result = store
        .lookup(0x10DE, 0x2684)
        .await
        .expect("lookup should succeed");

    let caps = result.expect("row should exist for known PCI-ID");
    assert!(caps.fp32, "fp32 should be true");
    assert!(caps.fp16, "fp16 should be true");
    assert!(caps.bf16, "bf16 should be true");
    assert!(caps.fp8, "fp8 should be true");
    assert!(!caps.fp4, "fp4 should be false");
    assert!(caps.flash_attention, "flash_attention should be true");
}

/// `lookup` on an unknown PCI-ID pair returns `Ok(None)`, never `Err`.
///
/// Does not insert any rows; directly queries for a nonexistent PCI-ID pair
/// and asserts that the result is `None` rather than an error.
#[tokio::test]
async fn test_lookup_unknown_pciid_returns_none() {
    let pool = make_pool().await;
    let store = DeviceCapabilityStore::new(pool);

    let result = store
        .lookup(0xFFFF, 0xFFFF)
        .await
        .expect("lookup should not error for missing PCI-ID");

    assert!(
        result.is_none(),
        "expected None for unknown PCI-ID, got {:?}",
        result
    );
}

/// `lookup` on boundary value vendor_id=0xFFFF, device_id=0xFFFF returns `None`.
///
/// Verifies that the maximum u16 values are handled correctly — no row exists at
/// that ID, so the result should be `None`. This exercises the u16→i64 cast path.
#[tokio::test]
async fn test_lookup_boundary_0xffff() {
    let pool = make_pool().await;
    let store = DeviceCapabilityStore::new(pool);

    let result = store
        .lookup(0xFFFF, 0xFFFF)
        .await
        .expect("lookup should succeed at boundary values");

    assert!(
        result.is_none(),
        "expected None at boundary 0xFFFF/0xFFFF, got {:?}",
        result
    );
}

/// INTEGER 0/1 columns correctly map to `false`/`true` in `InferenceCaps`.
///
/// Inserts a row with mixed 0/1 values (fp32=1, fp16=0, bf16=1, fp8=0, fp4=0, flash=1)
/// and asserts that the `row_to_caps` conversion produces the correct bool values.
#[tokio::test]
async fn test_lookup_integer_to_bool_mapping() {
    let pool = make_pool().await;

    // Insert the row before creating the store — the store takes ownership of the pool.
    insert_device_caps(&pool, 0x1234, 0x5678, 1, 0, 1, 0, 0, 1).await;

    let store = DeviceCapabilityStore::new(pool);

    let result = store
        .lookup(0x1234, 0x5678)
        .await
        .expect("lookup should succeed");

    let caps = result.expect("row should exist");
    assert!(caps.fp32, "fp32 should be true (stored as 1)");
    assert!(!caps.fp16, "fp16 should be false (stored as 0)");
    assert!(caps.bf16, "bf16 should be true (stored as 1)");
    assert!(!caps.fp8, "fp8 should be false (stored as 0)");
    assert!(!caps.fp4, "fp4 should be false (stored as 0)");
    assert!(
        caps.flash_attention,
        "flash_attention should be true (stored as 1)"
    );
}

/// Multiple PCI-ID rows do not cause cross-contamination — each lookup returns its own caps.
///
/// Inserts three rows with different PCI-IDs and different capability values, then
/// verifies that each lookup returns only its own row's values.
#[tokio::test]
async fn test_lookup_multiple_ids_no_interference() {
    let pool = make_pool().await;

    // Insert all rows before creating the store — the store takes ownership of the pool.
    insert_device_caps(&pool, 0x1001, 0x1111, 1, 1, 0, 0, 0, 0).await;
    insert_device_caps(&pool, 0x1002, 0x2222, 0, 0, 1, 1, 0, 0).await;
    insert_device_caps(&pool, 0x10DE, 0x3333, 0, 0, 0, 0, 1, 1).await;

    let store = DeviceCapabilityStore::new(pool);

    // Look up each PCI-ID and verify it returns its own caps.
    let caps_a = store
        .lookup(0x1001, 0x1111)
        .await
        .expect("lookup A should succeed")
        .expect("row A should exist");
    assert!(caps_a.fp32);
    assert!(caps_a.fp16);
    assert!(!caps_a.bf16);

    let caps_b = store
        .lookup(0x1002, 0x2222)
        .await
        .expect("lookup B should succeed")
        .expect("row B should exist");
    assert!(!caps_b.fp32);
    assert!(!caps_b.fp16);
    assert!(caps_b.bf16);
    assert!(caps_b.fp8);

    let caps_c = store
        .lookup(0x10DE, 0x3333)
        .await
        .expect("lookup C should succeed")
        .expect("row C should exist");
    assert!(!caps_c.fp32);
    assert!(!caps_c.bf16);
    assert!(caps_c.fp4);
    assert!(caps_c.flash_attention);
}
