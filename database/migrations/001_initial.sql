-- Migration 001: Initial schema
--
-- Creates the `models` table for persisted ModelMeta rows and the
-- `device_capabilities` PCI-ID capability hint table used for
-- pre-spawn scheduling hints.
--
-- All boolean columns use INTEGER 0/1 (SQLite has no native BOOLEAN type).

-- Models table — one row per discovered model file on disk.
-- Columns map from ModelMeta (anvilml-core/src/types/model.rs):
--   id, name, path, kind, dtype, format, size_bytes, scanned_at
-- Plus mtime_unix (populated by the scanner, not a field on ModelMeta itself).
CREATE TABLE IF NOT EXISTS models (
    id           TEXT PRIMARY KEY,  -- SHA256 hex of first 1 MiB of the file
    name         TEXT NOT NULL,     -- human-readable model name
    path         TEXT NOT NULL UNIQUE,  -- filesystem path (unique to prevent duplicate registrations)
    kind         TEXT NOT NULL,     -- ModelKind enum value as text ("diffusion", "text_encoder", etc.)
    dtype        TEXT NOT NULL,     -- ModelDtype enum value as text ("fp32", "bf16", etc.)
    format       TEXT NOT NULL,     -- ModelFormat enum value as text ("safetensors", "ckpt", etc.)
    size_bytes   INTEGER NOT NULL,  -- file size in bytes
    mtime_unix   INTEGER NOT NULL,  -- last modification time as Unix timestamp (populated by scanner)
    scanned_at   TEXT NOT NULL      -- ISO 8601 UTC timestamp when the model was scanned
);

-- Device capabilities table — PCI-ID based capability hints for pre-spawn scheduling.
-- Columns map directly from InferenceCaps (anvilml-core/src/types/hardware.rs):
--   fp32, fp16, bf16, fp8, fp4, flash_attention
-- Boolean fields are stored as INTEGER 0/1.
-- The Rust DeviceRow struct maps bool ↔ i64 at the store boundary.
CREATE TABLE IF NOT EXISTS device_capabilities (
    vendor_id       INTEGER NOT NULL,
    device_id       INTEGER NOT NULL,
    name            TEXT    NOT NULL,
    arch            TEXT    NOT NULL,
    fp32            INTEGER NOT NULL DEFAULT 0,
    fp16            INTEGER NOT NULL DEFAULT 0,
    bf16            INTEGER NOT NULL DEFAULT 0,
    fp8             INTEGER NOT NULL DEFAULT 0,
    fp4             INTEGER NOT NULL DEFAULT 0,
    flash_attention INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (vendor_id, device_id)
);

-- Unique index on the composite PCI-ID key for fast lookups.
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_capabilities_pci
    ON device_capabilities(vendor_id, device_id);
