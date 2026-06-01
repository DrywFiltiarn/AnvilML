//! Job domain types for AnvilML.
//!
//! Defines the `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, and
//! `SubmitJobResponse` types specified in ANVILML_DESIGN §4.1.
//!
//! Also includes model and artifact domain types from §4.2,
//! hardware types from §4.3, and worker types from §4.4/§6.1.

pub mod artifact;
pub mod hardware;
pub mod job;
pub mod model;
pub mod worker;
