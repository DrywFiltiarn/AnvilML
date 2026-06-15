//! Device capability store — read-only lookup for GPU capability metadata.
//!
//! This module provides `DeviceCapabilityStore`, the persistent storage layer
//! for device capability rows. It reads from the `device_capabilities` table
//! which is populated by `SeedLoader` from `backend/seeds/devices.sql`.
//!
//! The `device_capabilities` table schema (from `001_initial.sql`):
//! - `vendor_id INTEGER PRIMARY KEY` — PCI vendor ID (e.g. 4318 = NVIDIA)
//! - `device_id INTEGER PRIMARY KEY` — PCI device ID (e.g. 8994 = H100)
//! - `name TEXT NOT NULL` — human-readable device name
//! - `arch TEXT NOT NULL` — architecture identifier (e.g. "9.0", "gfx942")
//! - `fp32 INTEGER NOT NULL` — supports FP32 (0 or 1)
//! - `fp16 INTEGER NOT NULL` — supports FP16 (0 or 1)
//! - `bf16 INTEGER NOT NULL` — supports BF16 (0 or 1)
//! - `fp8 INTEGER NOT NULL` — supports FP8 (0 or 1)
//! - `fp4 INTEGER NOT NULL` — supports FP4 (0 or 1)
//! - `flash_attention INTEGER NOT NULL` — supports Flash Attention (0 or 1)

use sqlx::{Row, SqlitePool};
use tracing::{debug, instrument};

use anvilml_core::AnvilError;

/// A single row from the `device_capabilities` table.
///
/// Mirrors GPU inference capability flags with the addition of PCI vendor/device
/// identifiers and architecture string. Boolean fields are mapped from SQLite
/// `INTEGER 0/1` at the store boundary.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    /// PCI vendor ID (e.g. 4318 for NVIDIA, 4098 for AMD).
    pub vendor_id: u16,
    /// PCI device ID (e.g. 8994 for H100-SXM5-80GB).
    pub device_id: u16,
    /// Human-readable device name (e.g. "NVIDIA H100-SXM5-80GB").
    pub name: String,
    /// Architecture identifier (e.g. "9.0" for CUDA, "gfx942" for ROCm).
    pub arch: String,
    /// Whether the device supports FP32 compute.
    pub fp32: bool,
    /// Whether the device supports FP16 compute.
    pub fp16: bool,
    /// Whether the device supports BF16 compute.
    pub bf16: bool,
    /// Whether the device supports FP8 compute.
    pub fp8: bool,
    /// Whether the device supports FP4 compute.
    pub fp4: bool,
    /// Whether the device supports Flash Attention.
    pub flash_attention: bool,
}

/// Persistent storage for device capability rows backed by SQLite.
///
/// Wraps a `SqlitePool` and provides lookup by PCI vendor/device ID pair.
/// The underlying `device_capabilities` table is populated by `SeedLoader`
/// from `backend/seeds/devices.sql`.
pub struct DeviceCapabilityStore {
    pool: SqlitePool,
}

impl DeviceCapabilityStore {
    /// Create a new `DeviceCapabilityStore` backed by the given SQLite connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already been configured with WAL mode
    ///   and has the `device_capabilities` table created (via migrations).
    ///
    /// # Returns
    ///
    /// A new `DeviceCapabilityStore` instance. This constructor performs no I/O.
    pub async fn new(pool: SqlitePool) -> Self {
        DeviceCapabilityStore { pool }
    }

    /// Look up a device capability row by PCI vendor and device ID.
    ///
    /// # Arguments
    ///
    /// * `vendor_id` — PCI vendor ID (e.g. `0x10de` = 4318 for NVIDIA).
    /// * `device_id` — PCI device ID (e.g. 8994 for H100-SXM5-80GB).
    ///
    /// # Returns
    ///
    /// `Some(DeviceRow)` if a matching row exists, `None` if no row matches
    /// the given vendor/device pair. Returns `AnvilError::Db` only on
    /// query failure (connection lost, schema mismatch, etc.).
    #[instrument(skip(self), fields(vendor_id, device_id))]
    pub async fn get(
        &self,
        vendor_id: u16,
        device_id: u16,
    ) -> Result<Option<DeviceRow>, AnvilError> {
        debug!(vendor_id, device_id, "looking up device capabilities");

        // Use raw query() instead of query_as!() — we don't need the result
        // row, and query_as! requires DATABASE_URL for online mode. The
        // fetch_optional returns None when no row matches, which is the
        // correct behavior for a lookup that may not find a device.
        //
        // Map INTEGER 0/1 columns to bool via != 0 — SQLite stores booleans
        // as integers, and sqlx maps INTEGER to i64 by default.
        let row = sqlx::query(
            "SELECT vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention \
             FROM device_capabilities \
             WHERE vendor_id = ? AND device_id = ?",
        )
        .bind(vendor_id as i64)
        .bind(device_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let device = DeviceRow {
                    vendor_id: row.get("vendor_id"),
                    device_id: row.get("device_id"),
                    name: row.get("name"),
                    arch: row.get("arch"),
                    fp32: row.get::<i64, _>("fp32") != 0,
                    fp16: row.get::<i64, _>("fp16") != 0,
                    bf16: row.get::<i64, _>("bf16") != 0,
                    fp8: row.get::<i64, _>("fp8") != 0,
                    fp4: row.get::<i64, _>("fp4") != 0,
                    flash_attention: row.get::<i64, _>("flash_attention") != 0,
                };
                Ok(Some(device))
            }
            None => Ok(None),
        }
    }
}
