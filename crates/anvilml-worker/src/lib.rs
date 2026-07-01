//! Spawns/supervises Python worker subprocesses.

mod env;
pub use env::WorkerEnv;

mod spawn;
pub use spawn::{build_command, spawn_worker};

#[cfg(windows)]
mod job_object;
#[cfg(windows)]
pub use job_object::JobObjectGuard;
