//! Job queue, VRAM ledger, DAG validation, and dispatch loop.

pub mod dag;
// INTERIM-P14-PATCH — manual retrofit, pre-Phase-16. Delete this module and
// this line when P16-A1/A2 land — see interim_job_completion.rs's own
// module doc comment for the full replacement checklist.
pub mod event_loop;
pub mod interim_job_completion;
pub mod ledger;
pub mod queue;
pub mod scheduler;
pub mod types;
pub use dag::validate_graph;
pub use event_loop::{handle_image_ready, map_worker_event, spawn_event_loop};
pub use interim_job_completion::spawn_interim_job_completion_listener;
pub use ledger::VramLedger;
pub use queue::JobQueue;
pub use scheduler::JobScheduler;
pub use types::GraphError;
pub use types::ValidatedGraph;
