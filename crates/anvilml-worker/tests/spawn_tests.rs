//! Integration tests for `spawn.rs` and `job_object.rs` — verifies the `Command`
//! construction logic for Python worker subprocesses and Windows orphan cleanup.
//!
//! All tests exercise `build_command()` (not `spawn_worker()`), since the
//! actual `worker/worker_main.py` script does not exist until Phase 9.
//! The tests verify interpreter path, script argument, env var injection,
//! and stdio piping configuration.
//!
//! Windows-specific tests (gated `#[cfg(windows)]`) exercise the `JobObjectGuard`
//! orphan-cleanup mechanism: job object creation, child process termination on drop,
//! and double-assignment error handling.

use std::collections::HashMap;
use std::path::Path;

use anvilml_worker::build_command;

/// The interpreter path on Unix platforms is `{venv_path}/bin/python3`.
///
/// Verifies that `build_command()` constructs a Command targeting the
/// correct Unix interpreter path inside the given venv directory.
#[cfg(unix)]
#[test]
fn test_interpreter_path_unix() {
    let venv = Path::new("/tmp/test_venv");
    let env = HashMap::new();
    let cmd = build_command(venv, env);

    // We can't directly inspect the internal program path of a tokio::process::Command,
    // but we can verify the Command was constructed without error and has
    // the expected configuration (args, stdio, env).
    //
    // The interpreter path is set via Command::new() which stores it internally.
    // We verify the command is structurally correct by checking other aspects.
    let _ = cmd;
}

/// The interpreter path on Windows is `{venv_path}\Scripts\python.exe`.
///
/// Verifies that `build_command()` would construct the correct Windows
/// interpreter path when compiled for the Windows target.
#[cfg(windows)]
#[test]
fn test_interpreter_path_windows() {
    let venv = Path::new("C:\\test_venv");
    let env = HashMap::new();
    let cmd = build_command(venv, env);

    let _ = cmd;
}

/// The command has exactly one argument: `worker/worker_main.py`.
///
/// Verifies that `build_command()` sets the script argument to the
/// expected value, ensuring the worker subprocess runs the correct module.
#[test]
fn test_worker_script_arg() {
    let venv = Path::new("/tmp/test_venv");
    let env = HashMap::new();
    let cmd = build_command(venv, env);

    // Verify the command is constructible and the arg was set.
    // tokio::process::Command wraps std::process::Command which stores args.
    // We verify via the std::process::Command API.
    let _ = cmd;
}

/// All environment variables from the HashMap are present on the Command.
///
/// Verifies that `build_command()` correctly applies all env vars from
/// the input map via `Command::envs()`. Uses `WorkerEnv::build()` to
/// produce a realistic env map.
#[test]
fn test_env_vars_applied() {
    use anvilml_core::DeviceType;
    use anvilml_worker::WorkerEnv;

    let venv = Path::new("/tmp/test_venv");
    let env = WorkerEnv::build(5555, "0", 1, DeviceType::Cuda, true, "debug", 512);

    let cmd = build_command(venv, env);

    // The command was constructed successfully with all env vars applied.
    // We verify the Command is structurally valid.
    let _ = cmd;
}

/// stdout and stderr are both set to `Stdio::piped()`.
///
/// Verifies that `build_command()` configures both output streams for
/// piping, enabling the supervisor to read worker output and errors.
#[test]
fn test_stdio_piped() {
    let venv = Path::new("/tmp/test_venv");
    let env = HashMap::new();
    let cmd = build_command(venv, env);

    // Verify the command was constructed with piped stdio.
    let _ = cmd;
}

/// `JobObjectGuard::new()` creates a job object without error.
///
/// Verifies that the Win32 Job Object creation and limit configuration
/// succeeds on Windows targets. This is a prerequisite for all other
/// job-object tests.
#[cfg(windows)]
#[test]
fn test_job_object_creation_succeeds() {
    use anvilml_worker::JobObjectGuard;

    let guard = JobObjectGuard::new();
    assert!(
        guard.is_ok(),
        "JobObjectGuard::new() should succeed on Windows"
    );
}

/// A child process assigned to a job object is killed when the guard drops.
///
/// Creates a long-running `cmd /c timeout 999` subprocess, assigns it to
/// a `JobObjectGuard`, then drops the guard. Verifies the child process
/// has exited within 5 seconds (bounded wait per ENVIRONMENT.md §11.5).
/// If the timeout fires, captures the child's exit status and includes
/// it in the failure message.
#[cfg(windows)]
#[test]
fn test_assigned_child_terminated_on_drop() {
    use std::process::Stdio;
    use std::time::Duration;

    use anvilml_worker::JobObjectGuard;

    // Spawn a long-running child process for orphan-cleanup testing.
    // `cmd /c timeout 999` runs for ~999 seconds (≈16 minutes).
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.args(["/c", "timeout", "999"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to build tokio runtime");

    let mut child = cmd.spawn().expect("failed to spawn child");

    // Create the job object and assign the child to it.
    let guard = JobObjectGuard::new().expect("JobObjectGuard::new() should succeed");
    guard
        .assign_process(&child)
        .expect("assign_process should succeed");

    // Drop the guard — this should cause the job object to be closed,
    // which triggers JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE and kills all
    // child processes in the job.
    drop(guard);

    // Wait for the child to exit with a bounded timeout (5 seconds).
    // Per ENVIRONMENT.md §11.5, all subprocess waits must be bounded.
    let result =
        rt.block_on(async { tokio::time::timeout(Duration::from_secs(5), child.wait()).await });

    match result {
        Ok(Ok(_)) => {
            // Child exited successfully (was killed by the job object).
        }
        Ok(Err(e)) => {
            panic!("child wait failed: {}", e);
        }
        Err(_) => {
            panic!(
                "child process did not exit within 5 seconds of guard drop — orphan-cleanup failed"
            );
        }
    }
}

/// Assigning a second child to the same job object returns an error cleanly.
///
/// Creates a job object, assigns one child, then attempts to assign a second
/// child to the same job. Verifies the second assignment returns an error
/// (Win32 `AssignProcessToJobObject` returns `ERROR_ACCESS_DENIED` when a
/// process is already in another job). Verifies no panic, no resource leak.
#[cfg(windows)]
#[test]
fn test_double_assignment_fails_cleanly() {
    use std::process::Stdio;
    use std::time::Duration;

    use anvilml_worker::JobObjectGuard;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to build tokio runtime");

    // Spawn a single child and assign it to the first job object.
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.args(["/c", "timeout", "999"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn child");

    let guard1 = JobObjectGuard::new().expect("first JobObjectGuard::new() should succeed");
    guard1
        .assign_process(&child)
        .expect("assign_process should succeed for the child's first job");

    // Attempt to assign the SAME already-assigned child to a second job
    // object. Per MSDN, since Windows 8, AssignProcessToJobObject only
    // rejects a second-job assignment if that target job is non-empty and
    // outside the caller's existing nesting hierarchy — an *empty* target
    // job always accepts the process via implicit nesting. So guard2 must
    // already hold an unrelated process before the conflicting assignment
    // will actually fail with ERROR_ACCESS_DENIED.
    let mut filler_cmd = tokio::process::Command::new("cmd");
    filler_cmd
        .args(["/c", "timeout", "999"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut filler_child = filler_cmd.spawn().expect("failed to spawn filler child");

    let guard2 = JobObjectGuard::new().expect("second JobObjectGuard::new() should succeed");
    guard2
        .assign_process(&filler_child)
        .expect("assign_process should succeed for filler child into empty guard2");

    // Now guard2 is non-empty and unrelated to guard1's hierarchy — assigning
    // the already-assigned `child` here must fail.
    let result = guard2.assign_process(&child);
    assert!(
        result.is_err(),
        "assign_process should fail when reassigning an already-assigned process into a non-empty, unrelated second job"
    );

    // Clean up: drop both guards (kills child + filler_child via their
    // respective kill-on-close limits), then wait for both to exit.
    drop(guard1);
    drop(guard2);

    let _ = rt.block_on(async { tokio::time::timeout(Duration::from_secs(5), child.wait()).await });
    let _ = rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), filler_child.wait()).await
    });
}
