//! Domain types for job management.
//!
//! Contains `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, and `SubmitJobResponse`.

pub mod job;

pub use job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
