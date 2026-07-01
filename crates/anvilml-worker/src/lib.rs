//! Spawns/supervises Python worker subprocesses.

mod demux;
pub use demux::Demux;

mod env;
pub use env::WorkerEnv;

pub mod keepalive;
pub use keepalive::KeepaliveWatchdog;

mod spawn;
pub use spawn::{build_command, spawn_worker};

#[cfg(windows)]
mod job_object;
#[cfg(windows)]
pub use job_object::JobObjectGuard;

mod managed;
pub use managed::{ManagedWorker, WorkerHandle};

mod respawn;
pub use respawn::RespawnPolicy;
