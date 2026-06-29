-- Migration 002: Artifacts table
--
-- Creates the `artifacts` table for persisted ArtifactMeta rows.
-- Columns map from ArtifactMeta (anvilml-core/src/types/artifact.rs):
--   hash, job_id, width, height, seed, steps, created_at, file_path
-- Plus an index on job_id for the future list() query (P6-B3).

CREATE TABLE IF NOT EXISTS artifacts (
    hash        TEXT PRIMARY KEY,  -- SHA-256 hex content address
    job_id      TEXT NOT NULL,     -- UUID string of the originating job
    width       INTEGER NOT NULL,  -- image width in pixels
    height      INTEGER NOT NULL,  -- image height in pixels
    seed        INTEGER NOT NULL,  -- random seed (i64, supports negative seeds)
    steps       INTEGER NOT NULL,  -- diffusion steps
    created_at  TEXT NOT NULL,     -- ISO 8601 UTC timestamp
    file_path   TEXT NOT NULL      -- filesystem path to the PNG file
);

CREATE INDEX idx_artifacts_job_id ON artifacts(job_id);
