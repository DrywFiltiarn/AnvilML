//! Spawns/supervises Python worker subprocesses.

mod bridge;
pub use bridge::spawn_bridge;

mod demux;
pub use demux::Demux;

mod env;
pub use env::WorkerEnv;

pub mod keepalive;
pub use keepalive::KeepaliveWatchdog;

mod spawn;
pub use spawn::{ProcessWorkerSpawner, WorkerSpawner, build_command, spawn_worker};

#[cfg(windows)]
mod job_object;
#[cfg(windows)]
pub use job_object::JobObjectGuard;

mod managed;
pub use managed::{
    DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT, DEFAULT_INIT_TIMEOUT, DEFAULT_WATCHDOG_PING_INTERVAL,
    DEFAULT_WATCHDOG_PONG_TIMEOUT, ManagedWorker, ManagedWorkerConfig, RunOutcome, WorkerHandle,
};

mod pool;
pub use pool::WorkerPool;

mod respawn;
pub use respawn::RespawnPolicy;
