//! Windows Job Object wrapper for orphan process cleanup.
//!
//! This module provides `JobObjectGuard`, a RAII guard that wraps a Win32 Job Object
//! with the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` limit. When the guard is dropped,
//! all processes assigned to the job object are force-killed, preventing orphaned
//! worker subprocesses when the supervisor process dies unexpectedly.
//!
//! On non-Windows targets, this module is not compiled (`#[cfg(windows)]`), leaving
//! Linux/macOS orphan cleanup for a future implementation.

use windows::Win32::Foundation::{CloseHandle, DuplicateHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows::Win32::System::Threading::{GetCurrentProcess, PROCESS_ALL_ACCESS};

use anvilml_core::AnvilError;

/// RAII guard for a Win32 Job Object with kill-on-close semantics.
///
/// When dropped, all processes assigned to this job object are force-killed by the OS,
/// preventing orphaned worker subprocesses when the supervisor process dies.
///
/// The inner `HANDLE` is owned by the job object — this struct does not close it on drop
/// because the OS automatically terminates the job object when the last handle is closed.
pub struct JobObjectGuard {
    /// Opaque handle to the Win32 job object.
    /// The handle is valid for the lifetime of this guard.
    handle: HANDLE,
}

impl JobObjectGuard {
    /// Create a new anonymous job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` enabled.
    ///
    /// The job object is created via `CreateJobObjectW` and configured with the
    /// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` limit, which causes all processes in the
    /// job to be terminated when the job object is closed (i.e., when this guard drops).
    ///
    /// # Errors
    /// Returns `AnvilError::Io` if the job object cannot be created or if the limit
    /// cannot be configured.
    pub fn new() -> Result<Self, AnvilError> {
        // Create an anonymous job object (no name, no security attributes).
        // CreateJobObjectW returns Result<HANDLE> — Err on failure.
        // We pass None for security attributes and None for the name (anonymous).
        let handle = unsafe {
            CreateJobObjectW(None, None)
                .map_err(|_| std::io::Error::other("CreateJobObjectW failed"))?
        };

        // Configure the job object to kill all assigned processes when the job
        // object is closed. This is the core orphan-prevention guarantee: if the
        // supervisor process dies, the job object is closed and all child workers
        // are force-killed by the OS.
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // Set the limit flag — JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE tells the OS
        // to terminate all processes in this job when the job handle is closed.
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        // Apply the extended limit information to the job object.
        // JobObjectExtendedLimitInformation tells the OS to read the
        // JOBOBJECT_EXTENDED_LIMIT_INFORMATION struct we just populated.
        let success = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(info) as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if success.is_err() {
            // SetInformationJobObject failed — convert the Win32 error to
            // std::io::Error, then to AnvilError::Io.
            let win_err = windows::core::Error::from_win32();
            return Err(std::io::Error::other(format!(
                "SetInformationJobObject failed: {win_err}"
            ))
            .into());
        }

        tracing::debug!(job_object = ?handle, "created job object with kill-on-job-close");

        Ok(Self { handle })
    }

    /// Assign a spawned child process to this job object.
    ///
    /// The child process will be force-killed when this guard is dropped (because
    /// the job object's kill-on-close limit is enabled).
    ///
    /// # Errors
    /// Returns `AnvilError::Io` if the process cannot be assigned to the job.
    /// This can happen if:
    /// - The child process is already in another job object (ERROR_ACCESS_DENIED).
    /// - The handle cannot be duplicated (insufficient access rights).
    ///
    /// # Platform
    /// Windows only. On non-Windows targets, this function is not compiled.
    pub fn assign_process(&self, child: &tokio::process::Child) -> Result<(), AnvilError> {
        // Get the raw Windows HANDLE from the child process.
        // raw_handle() returns Option<*mut c_void> — unwrap to get the raw handle.
        // This is the handle that CreateProcess returned, which we need
        // to assign to the job object.
        let raw_handle_ptr = child.raw_handle();
        // SAFETY: raw_handle() returns a valid non-null pointer for a running process.
        // If the process has already exited, this will be null/None, and the
        // subsequent AssignProcessToJobObject call will fail gracefully.
        let raw_handle = HANDLE(raw_handle_ptr.unwrap_or(std::ptr::null_mut()));

        // Duplicate the handle via DuplicateHandle before assigning to the job object.
        // AssignProcessToJobObject requires a handle with PROCESS_ALL_ACCESS, and the
        // raw handle from raw_handle() may not have sufficient access rights.
        // DuplicateHandle with PROCESS_ALL_ACCESS on the target side gives us a handle
        // that meets the requirement, even if the source handle has limited access.
        let mut duplicated = HANDLE::default();
        let current_proc = unsafe { GetCurrentProcess() };

        // DuplicateHandle returns windows_core::Result<()> — check for success.
        let dup_result = unsafe {
            DuplicateHandle(
                current_proc,         // source process (ourselves)
                raw_handle,           // source handle
                current_proc,         // target process (ourselves)
                &mut duplicated,      // receives the duplicated handle
                PROCESS_ALL_ACCESS.0, // desired access on the duplicate
                false,                // do not inherit
                windows::Win32::Foundation::DUPLICATE_HANDLE_OPTIONS(0),
            )
        };

        if let Err(e) = dup_result {
            return Err(std::io::Error::other(format!("DuplicateHandle failed: {e}")).into());
        }

        // Assign the duplicated process handle to the job object.
        // This links the process to the job so it will be terminated when the
        // job object is closed (i.e., when this guard drops).
        let success = unsafe { AssignProcessToJobObject(self.handle, duplicated) };

        // Close the duplicated handle after assignment — the job object holds
        // its own internal reference to the process, so our handle copy is no
        // longer needed. This prevents handle leaks.
        let _ = unsafe { CloseHandle(duplicated) };

        if success.is_err() {
            let win_err = windows::core::Error::from_win32();
            return Err(std::io::Error::other(format!(
                "AssignProcessToJobObject failed: {win_err}"
            ))
            .into());
        }

        tracing::debug!(
            process_id = child.id(),
            job_object = ?self.handle,
            "assigned process to job object"
        );

        Ok(())
    }
}

impl Drop for JobObjectGuard {
    fn drop(&mut self) {
        // The job object is implicitly closed when this guard is dropped,
        // which triggers the JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE limit and
        // force-kills all assigned child processes. No explicit CloseHandle
        // is needed — the OS cleans up the job object when the last handle
        // to it is released.
        tracing::debug!(job_object = ?self.handle, "job object dropped, all child processes will be terminated");
    }
}
