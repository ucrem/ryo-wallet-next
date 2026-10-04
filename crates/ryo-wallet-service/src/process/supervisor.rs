use std::ffi::OsString;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::time::timeout;

use super::VerifiedBinary;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("process working directory must be an absolute directory")]
    UnsafeWorkingDirectory,
    #[error("verified sidecar could not be started")]
    Start(#[source] std::io::Error),
    #[error("sidecar did not exit before the deadline")]
    ExitTimeout,
    #[error("sidecar exit could not be observed")]
    Wait(#[source] std::io::Error),
    #[error("sidecar could not be terminated")]
    Terminate(#[source] std::io::Error),
}

/// An app-owned child process. This type has no public constructor: a future
/// lifecycle adapter can create it only after manifest verification succeeds.
pub struct ManagedProcess {
    child: Child,
}

impl ManagedProcess {
    /// Starts an already-verified sidecar with no shell, inherited stdin, or
    /// inherited environment. Arguments remain structured OS strings.
    pub async fn start(
        binary: &VerifiedBinary,
        args: &[OsString],
        working_directory: &Path,
    ) -> Result<Self, ProcessError> {
        Self::spawn(binary, args, working_directory, false, false).await
    }

    pub(crate) async fn start_wallet(
        binary: &VerifiedBinary,
        args: &[OsString],
        working_directory: &Path,
    ) -> Result<Self, ProcessError> {
        Self::spawn(binary, args, working_directory, false, true).await
    }

    pub(crate) fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.child.stdout.take()
    }

    pub(crate) async fn start_daemon(
        binary: &VerifiedBinary,
        args: &[OsString],
        working_directory: &Path,
    ) -> Result<Self, ProcessError> {
        Self::spawn(binary, args, working_directory, true, false).await
    }

    async fn spawn(
        binary: &VerifiedBinary,
        args: &[OsString],
        working_directory: &Path,
        console: bool,
        capture_wallet: bool,
    ) -> Result<Self, ProcessError> {
        if !working_directory.is_absolute() || !working_directory.is_dir() {
            return Err(ProcessError::UnsafeWorkingDirectory);
        }
        let mut command = Command::new(binary.path());
        command
            .args(args)
            .current_dir(working_directory)
            .env_clear()
            .env("LANG", "C")
            .stdin(if console {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(if capture_wallet {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW: sidecars never open a console.
            // Windows DNS/system libraries require this OS directory. No other
            // parent environment (including credentials or PATH) is inherited.
            if let Some(system_root) = std::env::var_os("SystemRoot") {
                command.env("SystemRoot", system_root);
            }
        }
        let child = command.spawn().map_err(ProcessError::Start)?;
        Ok(Self { child })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub fn has_exited(&mut self) -> Result<bool, ProcessError> {
        self.child
            .try_wait()
            .map(|status| status.is_some())
            .map_err(ProcessError::Wait)
    }

    /// The fixed console command belongs to our child, never an arbitrary RPC endpoint.
    pub(crate) async fn request_daemon_exit(&mut self) {
        if let Some(mut stdin) = self.child.stdin.take() {
            let _ = timeout(Duration::from_secs(1), stdin.write_all(b"exit\n")).await;
        }
    }

    /// OS-level fallback after a graceful wallet RPC shutdown has failed.
    pub async fn force_stop(&mut self, deadline: Duration) -> Result<ExitStatus, ProcessError> {
        self.child.start_kill().map_err(ProcessError::Terminate)?;
        self.wait_for_exit(deadline).await
    }

    pub async fn wait_for_exit(&mut self, deadline: Duration) -> Result<ExitStatus, ProcessError> {
        timeout(deadline, self.child.wait())
            .await
            .map_err(|_| ProcessError::ExitTimeout)?
            .map_err(ProcessError::Wait)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::process::{BinaryDigest, BinaryKind};

    fn verified_script(temp: &tempfile::TempDir, contents: &[u8]) -> VerifiedBinary {
        let script = temp.path().join("fake-sidecar");
        fs::write(&script, contents).unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).unwrap();
        let digest = Sha256::digest(contents);
        let hex = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        VerifiedBinary::verify(
            BinaryKind::WalletRpc,
            &script,
            BinaryDigest::parse_hex(&hex).unwrap(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn only_a_verified_binary_can_be_supervised_and_it_can_be_stopped() {
        let temp = tempfile::tempdir().unwrap();
        let binary = verified_script(
            &temp,
            b"#!/bin/sh\ntrap 'exit 0' TERM\nwhile :; do sleep 1; done\n",
        );
        let mut child = ManagedProcess::start(&binary, &[], temp.path())
            .await
            .unwrap();
        assert!(child.pid().is_some());
        child.force_stop(Duration::from_secs(2)).await.unwrap();
    }

    #[tokio::test]
    async fn relative_working_directory_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let binary = verified_script(&temp, b"#!/bin/sh\nexit 0\n");
        assert!(matches!(
            ManagedProcess::start(&binary, &[], Path::new("runtime")).await,
            Err(ProcessError::UnsafeWorkingDirectory)
        ));
    }
}
