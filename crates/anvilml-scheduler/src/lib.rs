//! Job queue, VRAM ledger, DAG validation, and dispatch loop.

pub mod dag;
pub mod event_loop;
pub mod ledger;
pub mod queue;
pub mod scheduler;
pub mod types;
pub use dag::validate_graph;
pub use event_loop::{handle_image_ready, map_worker_event, spawn_event_loop};
pub use ledger::VramLedger;
pub use queue::JobQueue;
pub use scheduler::CancelOutcome;
pub use scheduler::JobScheduler;
pub use types::GraphError;
pub use types::ValidatedGraph;
