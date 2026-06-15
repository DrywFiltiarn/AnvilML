-- Initial migration: creates all five tables required by the AnvilML registry.
--
-- Table overview:
--   jobs              — persisted job records (UUID hex primary key)
--   models            — scanned model metadata (SHA256 hex primary key)
--   artifacts         — files produced by job execution (content-addressed by hash)
--   seed_history      — SHA256-gated seed file tracking
--   device_capabilities — per-device hardware capability flags

-- ============================================================================
-- jobs: persisted job records
-- ============================================================================
CREATE TABLE IF NOT EXISTS jobs (
    id               TEXT PRIMARY KEY,       -- UUID hex string
    status           TEXT    NOT NULL,        -- Queued | Running | Completed | Failed | Cancelled
    graph            TEXT    NOT NULL,        -- JSON computation graph
    settings         TEXT    NOT NULL,        -- JSON job settings
    created_at       TEXT    NOT NULL,        -- ISO 8601 UTC timestamp
    started_at       TEXT,                    -- ISO 8601 UTC, null if not yet started
    completed_at     TEXT,                    -- ISO 8601 UTC, null if not yet completed
    worker_id        TEXT,                    -- Worker ID that executed this job
    error            TEXT,                    -- Error message if failed
    queue_position   INTEGER                  -- Position in queue (null once dispatched)
);

-- Index for filtering jobs by status (e.g. SELECT * FROM jobs WHERE status = 'Queued')
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);

-- ============================================================================
-- models: scanned model metadata
-- ============================================================================
CREATE TABLE IF NOT EXISTS models (
    id               TEXT PRIMARY KEY,        -- SHA256 hex digest of model file
    name             TEXT    NOT NULL,        -- Human-readable model name
    path             TEXT    NOT NULL,        -- Filesystem path to model file/directory
    kind             TEXT    NOT NULL,        -- ModelKind enum (snake_case)
    dtype            TEXT    NOT NULL,        -- ModelDtype enum (snake_case)
    format           TEXT    NOT NULL,        -- ModelFormat enum (snake_case)
    size_bytes       INTEGER NOT NULL,        -- File size in bytes
    scanned_at       TEXT    NOT NULL         -- ISO 8601 UTC timestamp
);

-- ============================================================================
-- artifacts: files produced by job execution
-- ============================================================================
CREATE TABLE IF NOT EXISTS artifacts (
    id               INTEGER PRIMARY KEY AUTOINCREMENT, -- Stable internal PK
    job_id           TEXT    NOT NULL,                 -- FK → jobs.id
    hash             TEXT    NOT NULL UNIQUE,           -- SHA-256 hex digest (content-addressed)
    path             TEXT    NOT NULL,                  -- Filesystem path to artifact
    size_bytes       INTEGER NOT NULL,                  -- File size in bytes
    created_at       TEXT    NOT NULL                  -- ISO 8601 UTC timestamp
);

-- Index for looking up all artifacts of a job
CREATE INDEX IF NOT EXISTS idx_artifacts_job_id ON artifacts(job_id);

-- ============================================================================
-- seed_history: SHA256-gated seed file tracking
-- ============================================================================
CREATE TABLE IF NOT EXISTS seed_history (
    file        TEXT PRIMARY KEY,       -- Seed file path
    sha256      TEXT    NOT NULL,       -- SHA-256 hex digest of file contents
    applied_at  TEXT    NOT NULL        -- ISO 8601 UTC timestamp
);

-- ============================================================================
-- device_capabilities: per-device hardware capability flags
-- ============================================================================
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

-- Unique index on PCI vendor/device pair
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_capabilities_pci
    ON device_capabilities(vendor_id, device_id);
