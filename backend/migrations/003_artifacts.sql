-- migrations/003_artifacts.sql
CREATE TABLE IF NOT EXISTS artifacts (
    hash        TEXT PRIMARY KEY,
    job_id      TEXT NOT NULL,
    width       INTEGER NOT NULL,
    height      INTEGER NOT NULL,
    format      TEXT NOT NULL DEFAULT 'png',
    seed        INTEGER NOT NULL,
    steps       INTEGER NOT NULL,
    prompt      TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_artifacts_job_id ON artifacts(job_id);
