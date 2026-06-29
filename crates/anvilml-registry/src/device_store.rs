//! DeviceCapabilityStore — read-only SQLite query layer for PCI-ID capability hints.
//!
//! Wraps a `SqlitePool` and provides `lookup(vendor_id, device_id)`, which queries the
//! `device_capabilities` table created by migration `001_initial.sql` and maps the
//! INTEGER 0/1 boolean columns to `InferenceCaps::bool` fields. Returns `Ok(None)` for
//! unknown PCI-ID pairs — never `Err` for a missing row.

use anvilml_core::InferenceCaps;
use sqlx::SqlitePool;

/// Read-only SQLite-backed query layer for PCI-ID device capability hints.
///
/// Wraps a `SqlitePool` and provides `lookup(vendor_id, device_id)`, which queries the
/// `device_capabilities` table (created by migration `001_initial.sql`). Boolean columns
/// stored as INTEGER 0/1 are mapped to `InferenceCaps` bool fields via `value != 0`.
///
/// Returns `Ok(None)` when no row matches the given PCI-ID pair — this is the correct
/// "not found" semantics, enabling the caller to fall through to
/// `CapabilitySource::Fallback` without an error.
///
/// # Errors
///
/// Returns `AnvilError::Db` only for genuine database errors (connection failure,
/// malformed query, constraint violation). Missing rows are never errors.
pub struct DeviceCapabilityStore {
    /// Database connection pool. All methods acquire a connection from this pool.
    pool: SqlitePool,
}

/// Private helper struct for reading `device_capabilities` table rows.
///
/// Maps SQL column names directly to struct fields via `sqlx::FromRow`.
/// The `name` and `arch` columns are not used by `lookup` but are present
/// in the table schema, so they must be captured to avoid column-mismatch errors.
#[derive(sqlx::FromRow)]
struct DeviceCapsRow {
    #[allow(dead_code)]
    // vendor_id is captured by sqlx::FromRow to match the SQL column but is not used in row_to_caps.
    vendor_id: i64,
    #[allow(dead_code)]
    // device_id is captured by sqlx::FromRow to match the SQL column but is not used in row_to_caps.
    device_id: i64,
    #[allow(dead_code)]
    // name is a human-readable device name (e.g. "NVIDIA GeForce RTX 4090") — not used by lookup.
    name: String,
    #[allow(dead_code)]
    // arch is the architecture string (e.g. "Ada Lovelace") — not used by lookup.
    arch: String,
    fp32: i64,
    fp16: i64,
    bf16: i64,
    fp8: i64,
    fp4: i64,
    flash_attention: i64,
}

impl DeviceCapabilityStore {
    /// Construct a new `DeviceCapabilityStore` backed by the given connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already had migrations applied.
    ///   The pool must be connected to a database containing the `device_capabilities` table.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Look up inference capabilities for a PCI-ID pair.
    ///
    /// Queries the `device_capabilities` table by `vendor_id` and `device_id` (the
    /// composite primary key). Returns `Ok(Some(caps))` when a matching row exists,
    /// `Ok(None)` when no row matches — never `Err` for a missing row.
    ///
    /// The `name` and `arch` columns from the table are ignored; only the boolean
    /// capability columns (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`)
    /// are mapped to `InferenceCaps` fields via `value != 0`.
    ///
    /// # Arguments
    ///
    /// * `vendor_id` — PCI vendor ID (e.g. `0x10de` for NVIDIA).
    /// * `device_id` — PCI device ID (vendor-specific).
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` only for genuine database errors (connection failure,
    /// malformed query). Missing PCI-ID pairs return `Ok(None)`.
    #[tracing::instrument(fields(vendor_id = %vendor_id, device_id = %device_id), skip(self))]
    pub async fn lookup(
        &self,
        vendor_id: u16,
        device_id: u16,
    ) -> Result<Option<InferenceCaps>, anvilml_core::AnvilError> {
        let row = sqlx::query_as::<_, DeviceCapsRow>(
            "SELECT vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention \
             FROM device_capabilities \
             WHERE vendor_id = ? AND device_id = ?",
        )
        .bind(vendor_id as i64)
        .bind(device_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                tracing::debug!(
                    vendor_id = %vendor_id,
                    device_id = %device_id,
                    "found device capability row"
                );
                Ok(Some(self.row_to_caps(r)))
            }
            None => {
                tracing::debug!(
                    vendor_id = %vendor_id,
                    device_id = %device_id,
                    "no device capability row found"
                );
                Ok(None)
            }
        }
    }

    /// Convert a raw `DeviceCapsRow` (INTEGER fields from SQL) into an `InferenceCaps`.
    ///
    /// Each boolean column is mapped via `value != 0`, matching the SQLite convention
    /// documented in `001_initial.sql` ("All boolean columns use INTEGER 0/1").
    fn row_to_caps(&self, row: DeviceCapsRow) -> InferenceCaps {
        InferenceCaps {
            // Map INTEGER 0/1 → bool: non-zero values indicate the capability is supported.
            fp32: row.fp32 != 0,
            fp16: row.fp16 != 0,
            bf16: row.bf16 != 0,
            fp8: row.fp8 != 0,
            fp4: row.fp4 != 0,
            flash_attention: row.flash_attention != 0,
        }
    }
}
