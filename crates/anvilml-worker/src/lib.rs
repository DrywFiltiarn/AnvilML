//! Spawns/supervises Python worker subprocesses.

mod env;
pub use env::WorkerEnv;

mod spawn;
pub use spawn::{build_command, spawn_worker};
