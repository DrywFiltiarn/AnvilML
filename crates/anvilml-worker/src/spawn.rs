//! Subprocess Command construction for Python worker processes.
//!
//! Produces a `tokio::process::Command` configured to launch the Python worker
//! as a module (`-m worker.worker_main`) via the venv interpreter, with environment
//! variables injected, stdout/stderr piped for log capture, and on Linux
//! `PR_SET_PDEATHSIG` set so the worker is orphan-cleaned if the parent
//! supervisor dies.

use anvilml_core::{GpuDevice, ServerConfig};
use std::process::Stdio;
use tokio::process::Command;

use crate::env::build_worker_env;

/// Build a `tokio::process::Command` to launch the Python worker subprocess.
///
/// The command uses the venv Python interpreter (platform-specific path),
/// passes `-m worker.worker_main` as the module invocation, injects all
/// `ANVILML_*` environment variables via `build_worker_env()`, and pipes
/// stdout/stderr for log capture.
///
/// On Linux, sets `PR_SET_PDEATHSIG` so the worker is killed if the parent
/// supervisor dies.
///
/// # Arguments
///
/// * `cfg` — The server configuration (provides venv path and IPC payload cap).
/// * `device` — The GPU device this worker will operate on.
/// * `port` — The TCP port the worker should connect to for IPC.
///
/// # Returns
///
/// A `tokio::process::Command` ready to be spawned.
pub fn build_command(cfg: &ServerConfig, device: &GpuDevice, port: u16) -> Command {
    // Determine the venv Python interpreter path. The path layout differs
    // between Unix (`bin/python3`) and Windows (`Scripts/python.exe`) virtual
    // environments; PathBuf::join handles platform-native separators.
    let interpreter = if cfg!(target_os = "windows") {
        cfg.venv_path.join("Scripts").join("python.exe")
    } else {
        cfg.venv_path.join("bin").join("python3")
    };

    let mut cmd = Command::new(interpreter);

    // The worker script is at `worker/worker_main.py` relative to the
    // repository root, which is the working directory at server startup.
    // Forward slashes work on both Unix and Windows in the tokio Command API.
    cmd.args(["-m", "worker.worker_main"]);

    // Inject all ANVILML_* environment variables required by the worker
    // runtime: IPC port, worker ID, device info, log level, payload cap.
    cmd.envs(build_worker_env(device, cfg, port));

    // Pipe stdout and stderr so the supervisor can capture worker logs
    // and surface them through the server's log channel. Stdin is left
    // as the default (inherited from parent) since the worker is
    // non-interactive.
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    // On Linux, set PR_SET_PDEATHSIG so the worker receives SIGTERM if the
    // parent supervisor process dies unexpectedly. This is the standard
    // Linux orphan cleanup mechanism — without it, a dead supervisor leaves
    // a zombie worker consuming GPU memory.
    //
    // Safety: prctl(PR_SET_PDEATHSIG, SIGTERM) is a well-documented Linux
    // syscall that sets the parent death signal for the calling process.
    // The return value is ignored — if prctl fails, the process simply
    // won't have orphan cleanup, which is acceptable.
    #[cfg(target_os = "linux")]
    {
        // Safety: pre_exec is unsafe because the closure runs in the child
        // process after fork but before exec. The prctl call with
        // PR_SET_PDEATHSIG is a well-documented Linux syscall that sets
        // the parent death signal. It is safe to call here because:
        // (1) it only uses libc primitives, (2) it does not access shared
        // memory, (3) it does not call any functions with undefined behavior.
        unsafe {
            cmd.pre_exec(|| {
                // Set the parent death signal to SIGTERM.
                // If the parent (Rust supervisor) dies, the child (Python worker)
                // receives SIGTERM and exits.
                // prctl returns 0 on success, -1 on error; we propagate the error
                // because the child should not start without orphan cleanup.
                let ret = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                if ret < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }

    cmd
}
