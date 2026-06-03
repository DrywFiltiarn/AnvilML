-- migrations/002_models.sql
CREATE TABLE IF NOT EXISTS models (
    id                TEXT PRIMARY KEY,
    name              TEXT NOT NULL,
    path              TEXT NOT NULL UNIQUE,
    kind              TEXT NOT NULL,
    size_bytes        INTEGER NOT NULL,
    dtype_hint        TEXT NOT NULL,
    vram_estimate_mib INTEGER NOT NULL,
    scanned_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_models_kind ON models(kind);
