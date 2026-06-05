//! DeviceCapabilityStore — SQLite-backed store for GPU device capability metadata.

use sqlx::SqlitePool;

use anvilml_core::error::AnvilError;

/// Convert a `sqlx::Error` into an `AnvilError::DbError`.
fn sqlx_error(err: sqlx::Error) -> AnvilError {
    AnvilError::DbError(err.to_string())
}

/// Tuple representing a single row from the `device_capabilities` table.
type DeviceCapabilityDbRow = (
    i64,    // vendor_id
    i64,    // device_id
    String, // model_name
    String, // arch
    i64,    // fp32
    i64,    // fp16
    i64,    // bf16
    i64,    // fp8
    i64,    // fp4
    i64,    // nvfp4
    i64,    // flash_attn
);

/// GPU device capability record.
///
/// Fields are ordered to match the `device_capabilities` migration column order:
/// `vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn`.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceCapabilityRow {
    pub vendor_id: u16,
    pub device_id: u16,
    pub model_name: String,
    pub arch: String,
    pub fp32: bool,
    pub fp16: bool,
    pub bf16: bool,
    pub fp8: bool,
    pub fp4: bool,
    pub nvfp4: bool,
    pub flash_attn: bool,
}

/// SQLite-backed device capability store.
pub struct DeviceCapabilityStore {
    pool: SqlitePool,
}

impl DeviceCapabilityStore {
    /// Create a new `DeviceCapabilityStore` backed by the given SQLite connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update a device capability record.
    ///
    /// Uses `INSERT OR REPLACE` so that calling this with an existing
    /// `(vendor_id, device_id)` pair updates all columns to the provided values.
    pub async fn upsert(&self, row: &DeviceCapabilityRow) -> Result<(), AnvilError> {
        sqlx::query(
            "INSERT OR REPLACE INTO device_capabilities \
             (vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(row.vendor_id as i64)
        .bind(row.device_id as i64)
        .bind(&row.model_name)
        .bind(&row.arch)
        .bind(if row.fp32 { 1 } else { 0 })
        .bind(if row.fp16 { 1 } else { 0 })
        .bind(if row.bf16 { 1 } else { 0 })
        .bind(if row.fp8 { 1 } else { 0 })
        .bind(if row.fp4 { 1 } else { 0 })
        .bind(if row.nvfp4 { 1 } else { 0 })
        .bind(if row.flash_attn { 1 } else { 0 })
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;

        Ok(())
    }

    /// Look up a device capability record by its PCI vendor and device IDs.
    ///
    /// Returns `Ok(None)` if no row with the given `(vendor_id, device_id)` pair exists, or
    /// `Ok(Some(row))` with all eleven columns deserialized into a [`DeviceCapabilityRow`].
    pub async fn get(
        &self,
        vendor_id: u16,
        device_id: u16,
    ) -> Result<Option<DeviceCapabilityRow>, AnvilError> {
        let row: Option<DeviceCapabilityDbRow> = sqlx::query_as(
            "SELECT vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn \
             FROM device_capabilities WHERE vendor_id = ? AND device_id = ?",
        )
        .bind(vendor_id as i64)
        .bind(device_id as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?;

        match row {
            Some((
                vendor_id,
                device_id,
                model_name,
                arch,
                fp32,
                fp16,
                bf16,
                fp8,
                fp4,
                nvfp4,
                flash_attn,
            )) => Ok(Some(DeviceCapabilityRow {
                vendor_id: vendor_id as u16,
                device_id: device_id as u16,
                model_name,
                arch,
                fp32: fp32 != 0,
                fp16: fp16 != 0,
                bf16: bf16 != 0,
                fp8: fp8 != 0,
                fp4: fp4 != 0,
                nvfp4: nvfp4 != 0,
                flash_attn: flash_attn != 0,
            })),
            None => Ok(None),
        }
    }

    /// Upsert a batch of device capability records within a single transaction.
    ///
    /// Returns the number of entries inserted or updated. If any entry fails,
    /// the entire transaction is rolled back.
    #[cfg(any(test, feature = "seed-util"))]
    pub async fn seed(&self, entries: &[DeviceCapabilityRow]) -> Result<u64, AnvilError> {
        let mut tx = self.pool.begin().await.map_err(sqlx_error)?;

        for row in entries {
            sqlx::query(
                "INSERT OR REPLACE INTO device_capabilities \
                 (vendor_id, device_id, model_name, arch, fp32, fp16, bf16, fp8, fp4, nvfp4, flash_attn) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
            )
            .bind(row.vendor_id as i64)
            .bind(row.device_id as i64)
            .bind(&row.model_name)
            .bind(&row.arch)
            .bind(if row.fp32 { 1 } else { 0 })
            .bind(if row.fp16 { 1 } else { 0 })
            .bind(if row.bf16 { 1 } else { 0 })
            .bind(if row.fp8 { 1 } else { 0 })
            .bind(if row.fp4 { 1 } else { 0 })
            .bind(if row.nvfp4 { 1 } else { 0 })
            .bind(if row.flash_attn { 1 } else { 0 })
            .execute(&mut *tx)
            .await
            .map_err(sqlx_error)?;
        }

        tx.commit().await.map_err(sqlx_error)?;

        Ok(entries.len() as u64)
    }
}

#[cfg(any(test, feature = "seed-util"))]
mod tests {
    use super::*;

    /// Helper: open a database at the given path and return a ready pool.
    async fn open_pool(path: &std::path::Path) -> SqlitePool {
        crate::db::open(path).await.unwrap()
    }

    #[tokio::test]
    async fn test_upsert_then_get_roundtrip() {
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

    #[tokio::test]
    async fn test_get_miss_returns_none() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();

        let pool = open_pool(path).await;
        let store = DeviceCapabilityStore::new(pool);

        let result = store.get(0xFFFF, 0xFFFF).await.unwrap();
        assert!(result.is_none(), "expected None for non-existent PCI ID");
    }
}
