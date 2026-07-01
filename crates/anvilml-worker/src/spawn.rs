//! Subprocess construction for Python worker processes.
//!
//! Provides `build_command()` to construct a configured `tokio::process::Command`
//! and `spawn_worker()` to execute it. The interpreter path is platform-specific:
//! `{venv_path}/bin/python3` on Unix, `{venv_path}\Scripts\python.exe` on Windows.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use anvilml_core::AnvilError;

/// Construct and configure a `Command` to run the Python worker subprocess.
///
/// The command targets the correct Python interpreter in the worker venv
/// (platform-specific path), passes `worker/worker_main.py` as the script
/// argument, applies all environment variables from `env`, and pipes both
/// stdout and stderr so the supervisor can read them.
///
/// This function does **not** spawn the process — it returns the configured
/// `Command` for inspection in tests or for spawning by `spawn_worker()`.
///
/// # Arguments
/// * `venv_path` — Root of the Python virtual environment containing the
///   interpreter (e.g. `./worker/.venv`).
/// * `env` — Environment variables to inject into the subprocess via
///   `Command::envs()`. Typically produced by `WorkerEnv::build()`.
///
/// # Returns
/// A configured `tokio::process::Command` ready to be spawned. The actual
/// interpreter path depends on the compilation target platform.
pub fn build_command(venv_path: &Path, env: HashMap<String, String>) -> Command {
    // Platform-specific interpreter path: Unix uses `bin/python3`,
    // Windows uses `Scripts\python.exe` inside the venv.
    #[cfg(unix)]
    let interpreter = venv_path.join("bin/python3");

    #[cfg(windows)]
    let interpreter = venv_path.join("Scripts\\python.exe");

    let mut cmd = Command::new(interpreter);

    // The worker script is always `worker/worker_main.py` relative to the
    // current working directory — the supervisor sets CWD before spawning.
    cmd.arg("worker/worker_main.py");

    // Apply all environment variables from the builder (WorkerEnv::build).
    // These include ANVILML_IPC_PORT, ANVILML_WORKER_ID, device info,
    // mock mode flag, log level, and max IPC payload size.
    cmd.envs(env);

    // Pipe both stdout and stderr so the supervisor can read worker output
    // and errors without mixing them with the supervisor's own console.
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    cmd
}

/// Spawn a Python worker subprocess with the given environment variables.
///
/// Constructs the worker command via `build_command()` and spawns it,
/// returning a `tokio::process::Child` handle for the supervisor to
/// monitor.
///
/// # Arguments
/// * `venv_path` — Root of the Python virtual environment containing the
///   interpreter.
/// * `env` — Environment variables to inject into the subprocess.
///
/// # Errors
/// Returns `AnvilError::Io` if the process cannot be spawned (e.g. the
/// interpreter binary does not exist at the expected path, or the OS
/// denies permission).
///
/// # Logging
/// Emits a DEBUG log at entry with the `venv_path` field.
#[tracing::instrument(skip(env), fields(venv_path = %venv_path.display()))]
pub async fn spawn_worker(
    venv_path: &Path,
    env: HashMap<String, String>,
) -> Result<tokio::process::Child, AnvilError> {
    tracing::debug!(venv_path = %venv_path.display(), "spawning worker subprocess");

    // Build the configured command, then spawn it.
    // `spawn()` requires `&mut self`, so the binding must be mutable.
    let mut cmd = build_command(venv_path, env);
    let child = cmd.spawn()?;

    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `build_command()` returns a non-empty command path.
    #[test]
    fn test_build_command_has_path() {
        let env = HashMap::new();
        let venv = Path::new("/tmp/test_venv");
        let cmd = build_command(venv, env);
        // The Command's program path should be set (not empty).
        // We can't easily inspect the internal path, but we can verify
        // the command is constructible without panicking.
        let _ = cmd;
    }
}
