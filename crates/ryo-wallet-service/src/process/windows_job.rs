use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::null;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

/// Anonymous, non-inheritable ownership for a sidecar and its descendants.
/// The kernel closes it even when the application exits without Rust drops.
pub(super) struct ProcessJob(OwnedHandle);

impl ProcessJob {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: unnamed job, non-inheritable handle, correctly sized native structure.
        unsafe {
            let handle = CreateJobObjectW(null(), null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Self(OwnedHandle::from_raw_handle(handle));
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.0.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }
    }

    pub(super) fn assign(&self, child: HANDLE) -> io::Result<()> {
        // SAFETY: the caller holds a live process handle; the job handle is owned here.
        if child.is_null()
            || unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child) } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INVALID_PARAMETER, GetLastError, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    #[test]
    #[ignore = "subprocess fixture; called by owner_exit_closes_job_and_terminates_child"]
    fn exiting_owner_fixture() {
        let report = std::env::var_os("RYO_TEST_JOB_PID_REPORT").expect("fixture report path");
        let ping = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/ping.exe");
        let mut child = Command::new(ping)
            .args(["-n", "180", "127.0.0.1"])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let job = ProcessJob::new().unwrap();
        if let Err(error) = job.assign(child.as_raw_handle()) {
            let _ = child.kill();
            panic!("fixture job assignment failed: {error}");
        }
        std::fs::write(report, child.id().to_string()).unwrap();
        // Deliberately bypass all Rust destructors, as in an abrupt owner exit.
        std::process::exit(0);
    }

    #[test]
    fn owner_exit_closes_job_and_terminates_child() {
        let temp = tempfile::tempdir().unwrap();
        let report = temp.path().join("pid");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "process::windows_job::tests::exiting_owner_fixture",
            ])
            .env("RYO_TEST_JOB_PID_REPORT", &report)
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let pid: u32 = std::fs::read_to_string(report).unwrap().parse().unwrap();
        // SAFETY: read-only synchronization handle. No PID-based termination is used.
        unsafe {
            let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !process.is_null() {
                let result = WaitForSingleObject(process, 5000);
                CloseHandle(process);
                assert_eq!(result, WAIT_OBJECT_0, "owned child survived owner exit");
            } else {
                assert_eq!(
                    GetLastError(),
                    ERROR_INVALID_PARAMETER,
                    "could not observe child exit"
                );
            }
        }
    }
}
