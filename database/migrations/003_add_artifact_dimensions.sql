-- Migration 003: add width and height columns to artifacts table.
--
-- These columns record the dimensions of PNG image artifacts produced
-- by job execution. They are nullable because artifacts created before
-- this migration (and non-image artifacts) will have NULL values.
--
-- The ArtifactStore save() method populates these fields from the PNG
-- header when the image is saved.

ALTER TABLE artifacts ADD COLUMN width INTEGER;
ALTER TABLE artifacts ADD COLUMN height INTEGER;
