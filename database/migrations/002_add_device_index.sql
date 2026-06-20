-- Migration 002: add device_index column to jobs table.
--
-- The device_index stores the GPU device index (0-based) that was used
-- to dispatch a job. This allows the event loop to query the correct
-- device when releasing VRAM reservations on job completion or failure.
--
-- The column is nullable because jobs created before this migration
-- (and jobs not yet dispatched) will have NULL.

ALTER TABLE jobs ADD COLUMN device_index INTEGER;
