//! Windows Job Object orphan cleanup for Python worker subprocesses.
//!
//! On Linux, [`crate::spawn::build_command`] sets `PR_SET_PDEATHSIG` so a
//! worker dies automatically if the supervisor (`anvilml`) dies — by any
//! means, including an abrupt kill. Windows has no equivalent `prctl`
//! mechanism; a worker spawned as a plain child process survives its
//! parent's death indefinitely, including when the parent is terminated
//! abruptly (e.g. Task Manager "End task", a crash, or an external
//! `taskkill /F`) rather than shut down gracefully.
//!
//! This module closes that gap with a Windows Job Object: each worker is
//! assigned to its own job, created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
//! That flag means the OS kills every process still assigned to the job the
//! moment the job's last handle closes — and a job's handle closes whenever
//! the process holding it exits, by any means, exactly like a normal file or
//! socket handle. No explicit shutdown code path is required for the
//! abrupt-kill case: the OS itself tears down the worker when `anvilml.exe`
//! disappears, because `anvilml.exe` is the only process holding the job
//! handle that keeps the job (and the kill-on-close behaviour) alive.
//!
//! One job per worker — not one shared job for the whole pool — so that
//! killing or dropping a single worker's job (e.g. during a normal,
//! policy-driven respawn) cannot affect any sibling worker. This mirrors
//! the existing per-worker granularity of `POST /v1/workers/{id}/restart`.
//!
//! The graceful shutdown and respawn paths in `managed.rs` are unaffected:
//! `child.kill().await` continues to be the primary, intentional teardown
//! mechanism on those paths. The job object is a backstop for the case
//! those paths cannot run at all — when the supervisor itself is gone.

#[cfg(windows)]
use std::io;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// An owned handle to a per-worker Windows Job Object.
///
/// Holding this value alive keeps the job's `KILL_ON_JOB_CLOSE` behaviour
/// armed: the assigned worker process is killed by the OS as soon as this
/// handle closes, whether that happens via [`Drop`] (normal teardown) or
/// because the process holding it — `anvilml.exe` itself — terminates for
/// any reason, including an abrupt external kill. Closing this handle does
/// not, by itself, fail or block; the actual worker termination is carried
/// out by the OS as a side effect of the close.
///
/// `#[cfg(windows)]`-only: there is no Unix equivalent, and Unix orphan
/// cleanup is handled separately via `PR_SET_PDEATHSIG` in `spawn.rs`.
#[cfg(windows)]
#[derive(Debug)]
pub struct WorkerJobHandle(HANDLE);

// SAFETY: a Win32 HANDLE is an opaque, OS-managed identifier with no
// thread-affinity semantics — unlike e.g. GDI handles, it carries no
// thread-local state and may be passed to CloseHandle (or any other Win32
// call that accepts it) from any thread. WorkerJobHandle's only operations
// are CreateJobObjectW/SetInformationJobObject/AssignProcessToJobObject (in
// `new`) and CloseHandle (in `Drop`), none of which require the calling
// thread to match the creating thread. This type must move across threads:
// it lives inside `ManagedWorker`, which is owned by a `tokio::spawn`'d
// `run()` future and is therefore required to be `Send`.
#[cfg(windows)]
unsafe impl Send for WorkerJobHandle {}

// SAFETY: for the same reason as the Send impl above — CloseHandle and the
// Win32 job-object calls are safe to invoke concurrently from multiple
// threads against the same handle value (the OS serialises access
// internally); this type exposes no interior mutability and holds the
// handle for its entire lifetime, so &WorkerJobHandle carries no risk of a
// data race.
#[cfg(windows)]
unsafe impl Sync for WorkerJobHandle {}

#[cfg(windows)]
impl WorkerJobHandle {
    /// Create a new, unnamed Job Object configured with
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, then assign `process_handle` to
    /// it.
    ///
    /// `process_handle` must be a valid, currently-open handle to a running
    /// process — the `RawHandle` returned by
    /// `tokio::process::Child::raw_handle()` while the child has not yet
    /// exited satisfies this. The handle is borrowed only for the duration
    /// of the assignment call; this function does not take ownership of it
    /// or close it.
    ///
    /// # Errors
    ///
    /// Returns the OS error if job creation, limit configuration, or
    /// process assignment fails. On any error path, any job handle already
    /// created is closed before returning, so no handle is leaked.
    ///
    /// # Safety
    ///
    /// `process_handle` must be a valid `HANDLE` to a process that has not
    /// yet been closed by the caller. This function performs no validation
    /// of the handle beyond what the underlying Win32 calls perform.
    pub unsafe fn new(process_handle: HANDLE) -> io::Result<Self> {
        // SAFETY: CreateJobObjectW with null attributes and null name
        // creates an unnamed job object with default security, accessible
        // only via the returned handle. This is the documented, safe usage
        // pattern for a process-local job with no cross-process sharing
        // requirement.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }

        // Configure JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: when the job's last
        // handle closes, the OS terminates every process still assigned to
        // it. All other fields are zeroed — only this one limit flag is
        // requested, nothing else about the assigned process is restricted
        // (no CPU/memory caps, no UI restrictions).
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation = JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..unsafe { std::mem::zeroed() }
        };

        // SAFETY: `info` is a validly-initialized
        // JOBOBJECT_EXTENDED_LIMIT_INFORMATION matching the
        // JobObjectExtendedLimitInformation class, and `job` was just
        // created above and is not null.
        let set_ok = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if set_ok == 0 {
            let err = io::Error::last_os_error();
            // Close the job we just created — returning early here must
            // not leak the handle.
            unsafe { CloseHandle(job) };
            return Err(err);
        }

        // SAFETY: `job` is a valid job object handle and `process_handle`
        // is a valid process handle per this function's safety contract.
        let assign_ok = unsafe { AssignProcessToJobObject(job, process_handle) };
        if assign_ok == 0 {
            let err = io::Error::last_os_error();
            unsafe { CloseHandle(job) };
            return Err(err);
        }

        Ok(Self(job))
    }
}

#[cfg(windows)]
impl Drop for WorkerJobHandle {
    fn drop(&mut self) {
        // SAFETY: self.0 was created by CreateJobObjectW in `new` and is
        // not closed anywhere else — WorkerJobHandle owns it exclusively.
        // Closing it here, on normal teardown, triggers the same
        // KILL_ON_JOB_CLOSE behaviour as an abrupt process death would;
        // by that point the worker has typically already been killed via
        // child.kill().await on the same teardown path, so this is usually
        // a no-op against an already-dead process, not a surprise kill.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
