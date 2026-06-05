-- migrations/004_device_capabilities.sql
CREATE TABLE IF NOT EXISTS device_capabilities (
    vendor_id       INTEGER NOT NULL,
    device_id       INTEGER NOT NULL,
    model_name      TEXT    NOT NULL,
    arch            TEXT    NOT NULL,
    fp32            INTEGER NOT NULL DEFAULT 0,
    fp16            INTEGER NOT NULL DEFAULT 0,
    bf16            INTEGER NOT NULL DEFAULT 0,
    fp8             INTEGER NOT NULL DEFAULT 0,
    fp4             INTEGER NOT NULL DEFAULT 0,
    nvfp4           INTEGER NOT NULL DEFAULT 0,
    flash_attn      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (vendor_id, device_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_capabilities_pci
    ON device_capabilities(vendor_id, device_id);
