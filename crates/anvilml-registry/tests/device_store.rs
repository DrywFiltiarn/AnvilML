//! Integration tests for `anvilml_registry::device_store`.

use anvilml_registry::{DeviceCapabilityRow, DeviceCapabilityStore};
use sqlx::SqlitePool;

/// Helper: open a database at the given path and return a ready pool.
async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// Upsert a `DeviceCapabilityRow`, then retrieve it by PCI IDs — all 11 fields must match.
#[tokio::test]
async fn upsert_then_get_roundtrip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let store = DeviceCapabilityStore::new(pool);

    let row = DeviceCapabilityRow {
        vendor_id: 0x10de,
        device_id: 0x2204,
        model_name: "NVIDIA GeForce RTX 3080".to_string(),
        arch: "ampere".to_string(),
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attn: false,
    };

    store.upsert(&row).await.unwrap();

    let retrieved = store.get(0x10de, 0x2204).await.unwrap().unwrap();
    assert_eq!(retrieved.vendor_id, row.vendor_id);
    assert_eq!(retrieved.device_id, row.device_id);
    assert_eq!(retrieved.model_name, row.model_name);
    assert_eq!(retrieved.arch, row.arch);
    assert_eq!(retrieved.fp32, row.fp32);
    assert_eq!(retrieved.fp16, row.fp16);
    assert_eq!(retrieved.bf16, row.bf16);
    assert_eq!(retrieved.fp8, row.fp8);
    assert_eq!(retrieved.fp4, row.fp4);
    assert_eq!(retrieved.nvfp4, row.nvfp4);
    assert_eq!(retrieved.flash_attn, row.flash_attn);
}

/// Get a non-existent PCI ID pair returns `None`.
#[tokio::test]
async fn get_miss_returns_none() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let store = DeviceCapabilityStore::new(pool);

    let result = store.get(0xFFFF, 0xFFFF).await.unwrap();
    assert!(result.is_none(), "expected None for non-existent PCI ID");
}

/// Seeding 3 entries returns count `3` and all entries are retrievable.
#[tokio::test]
async fn seed_returns_correct_count() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let store = DeviceCapabilityStore::new(pool);

    let entries = vec![
        DeviceCapabilityRow {
            vendor_id: 0x10de,
            device_id: 0x2204,
            model_name: "RTX 3080".to_string(),
            arch: "ampere".to_string(),
            fp32: true,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attn: false,
        },
        DeviceCapabilityRow {
            vendor_id: 0x10de,
            device_id: 0x2324,
            model_name: "RTX 3090".to_string(),
            arch: "ampere".to_string(),
            fp32: true,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attn: false,
        },
        DeviceCapabilityRow {
            vendor_id: 0x10de,
            device_id: 0x2406,
            model_name: "RTX 4090".to_string(),
            arch: "ada".to_string(),
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: true,
            fp4: false,
            nvfp4: false,
            flash_attn: true,
        },
    ];

    let count = store.seed(&entries).await.unwrap();
    assert_eq!(count, 3, "seed should return entry count");

    // Verify all entries are retrievable.
    for entry in &entries {
        let retrieved = store
            .get(entry.vendor_id, entry.device_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.model_name, entry.model_name);
    }
}

/// Specific bool values survive upsert → get roundtrip.
#[tokio::test]
async fn bool_flags_roundtrip() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let store = DeviceCapabilityStore::new(pool);

    let row = DeviceCapabilityRow {
        vendor_id: 0x10de,
        device_id: 0x2406,
        model_name: "Ada Gen".to_string(),
        arch: "ada".to_string(),
        fp32: true,       // true
        fp16: false,      // false
        bf16: true,       // true
        fp8: false,       // false
        fp4: true,        // true
        nvfp4: false,     // false
        flash_attn: true, // true
    };

    store.upsert(&row).await.unwrap();

    let retrieved = store.get(0x10de, 0x2406).await.unwrap().unwrap();
    assert_eq!(retrieved.fp32, true);
    assert_eq!(retrieved.fp16, false);
    assert_eq!(retrieved.bf16, true);
    assert_eq!(retrieved.fp8, false);
    assert_eq!(retrieved.fp4, true);
    assert_eq!(retrieved.nvfp4, false);
    assert_eq!(retrieved.flash_attn, true);
}

/// Upserting the same PCI ID twice updates the record (INSERT OR REPLACE).
#[tokio::test]
async fn upsert_overwrites_existing() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let store = DeviceCapabilityStore::new(pool);

    let row1 = DeviceCapabilityRow {
        vendor_id: 0x10de,
        device_id: 0x2204,
        model_name: "RTX 3080".to_string(),
        arch: "ampere".to_string(),
        fp32: true,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attn: false,
    };

    let row2 = DeviceCapabilityRow {
        vendor_id: 0x10de,
        device_id: 0x2204,
        model_name: "RTX 3080 Ti".to_string(),
        arch: "ampere".to_string(),
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attn: false,
    };

    store.upsert(&row1).await.unwrap();
    store.upsert(&row2).await.unwrap();

    let retrieved = store.get(0x10de, 0x2204).await.unwrap().unwrap();
    assert_eq!(retrieved.model_name, "RTX 3080 Ti");
    assert_eq!(retrieved.fp16, true);
}

/// Seed with an empty slice returns count `0`.
#[tokio::test]
async fn seed_empty_returns_zero() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let store = DeviceCapabilityStore::new(pool);

    let count = store.seed(&[]).await.unwrap();
    assert_eq!(count, 0, "seed with empty slice should return 0");
}
