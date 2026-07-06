-- Migration 003: Jobs table
--
-- Creates the `jobs` table for persisted Job rows.
-- Columns map from Job (anvilml-core/src/types/job.rs):
--   id, status, graph, settings, created_at, started_at,
--   completed_at, worker_id, error, queue_position
-- graph and settings are TEXT (serialized JSON via serde_json),
-- not normalized columns — the Job struct owns the canonical shape.

CREATE TABLE IF NOT EXISTS jobs (
    id           TEXT PRIMARY KEY,  -- UUID string (stable unique identifier)
    status       TEXT NOT NULL,     -- JobStatus enum value as text ("queued", "running", "completed", "failed", "cancelled")
    graph        TEXT NOT NULL,     -- serialized JSON computation graph (serde_json::to_string)
    settings     TEXT NOT NULL,     -- serialized JSON JobSettings (serde_json::to_string)
    created_at   TEXT NOT NULL,     -- ISO 8601 UTC timestamp when queued
    started_at   TEXT,              -- ISO 8601 UTC timestamp when execution began (null while queued)
    completed_at TEXT,              -- ISO 8601 UTC timestamp when finished (null while running/queued)
    worker_id    TEXT,              -- worker identity string (set after execution begins)
    error        TEXT,              -- failure diagnostic message (set only for Failed jobs)
    queue_position INTEGER          -- position in the queue when Queued (cleared when picked up)
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at);
