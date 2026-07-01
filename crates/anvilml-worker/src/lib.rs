//! Spawns/supervises Python worker subprocesses.

mod demux;
pub use demux::Demux;

mod env;
pub use env::WorkerEnv;

mod spawn;
pub use spawn::{build_command, spawn_worker};

#[cfg(windows)]
mod job_object;
#[cfg(windows)]
pub use job_object::JobObjectGuard;
