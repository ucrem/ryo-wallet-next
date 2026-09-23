use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use thiserror::Error;
use tokio::time::{Instant, sleep};

use crate::domain::NodeConfig;
use crate::rpc::{RpcError, WalletRpcClient};
use crate::storage::AppPaths;

use super::{
    CredentialFileError, ManagedProcess, ProcessError, VerifiedBinary, WalletRpcLaunch,
    WalletRpcLaunchError, read_generated_login,
};

const DEFAULT_STARTUP_DEADLINE: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const FORCE_STOP_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum WalletRpcStartupError {
    #[error("wallet RPC launch configuration is invalid")]
    Launch(#[source] WalletRpcLaunchError),
    #[error("wallet RPC process could not be started")]
    Process(#[source] ProcessError),
    #[error("wallet RPC generated an invalid credential file")]
    Credentials(#[source] CredentialFileError),
    #[error("wallet RPC did not become ready before the deadline")]
    Deadline,
    #[error("wallet RPC readiness check failed")]
    Readiness(#[source] RpcError),
}

/// An authenticated, app-owned `ryo-wallet-rpc` process. It can only be
/// created by successful binary verification and authenticated readiness.
pub struct WalletRpcSession {
    process: ManagedProcess,
    client: WalletRpcClient,
}

impl WalletRpcSession {
    pub async fn start(
        binary: &VerifiedBinary,
        paths: &AppPaths,
        node: &NodeConfig,
        rpc_port: u16,
    ) -> Result<Self, WalletRpcStartupError> {
        Self::start_with_deadline(binary, paths, node, rpc_port, DEFAULT_STARTUP_DEADLINE).await
    }

    pub async fn start_with_deadline(
        binary: &VerifiedBinary,
        paths: &AppPaths,
        node: &NodeConfig,
        rpc_port: u16,
        deadline: Duration,
    ) -> Result<Self, WalletRpcStartupError> {
        let launch = WalletRpcLaunch::prepare(paths, node, rpc_port)
            .map_err(WalletRpcStartupError::Launch)?;
        let mut process = ManagedProcess::start(binary, &launch.args, &launch.working_directory)
            .await
            .map_err(WalletRpcStartupError::Process)?;
        let startup = wait_for_readiness(&launch, rpc_port, deadline).await;
        match startup {
            Ok(client) => Ok(Self { process, client }),
            Err(error) => {
                let _ = process.force_stop(FORCE_STOP_DEADLINE).await;
                Err(error)
            }
        }
    }

    pub fn client(&self) -> &WalletRpcClient {
        &self.client
    }

    /// A lock is complete only once the key-bearing child has exited. If its
    /// graceful RPC stop does not lead to exit, use the bounded OS fallback.
    pub async fn stop(mut self, deadline: Duration) -> Result<(), ProcessError> {
        let _ = self.client.stop_wallet().await;
        match self.process.wait_for_exit(deadline).await {
            Ok(_) => Ok(()),
            Err(ProcessError::ExitTimeout) => self
                .process
                .force_stop(FORCE_STOP_DEADLINE)
                .await
                .map(|_| ()),
            Err(error) => Err(error),
        }
    }
}

async fn wait_for_readiness(
    launch: &WalletRpcLaunch,
    rpc_port: u16,
    deadline: Duration,
) -> Result<WalletRpcClient, WalletRpcStartupError> {
    let until = Instant::now() + deadline;
    loop {
        let credentials = match read_generated_login(&launch.working_directory, rpc_port) {
            Ok(credentials) => credentials,
            Err(CredentialFileError::NotReady) if Instant::now() < until => {
                sleep(POLL_INTERVAL).await;
                continue;
            }
            Err(CredentialFileError::NotReady) => return Err(WalletRpcStartupError::Deadline),
            Err(error) => return Err(WalletRpcStartupError::Credentials(error)),
        };
        let client = WalletRpcClient::new(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, rpc_port),
            credentials,
        )
        .map_err(WalletRpcStartupError::Readiness)?;
        match client.languages().await {
            Ok(_) => return Ok(client),
            Err(RpcError::Transport) if Instant::now() < until => sleep(POLL_INTERVAL).await,
            Err(RpcError::Transport) => return Err(WalletRpcStartupError::Deadline),
            Err(error) => return Err(WalletRpcStartupError::Readiness(error)),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::domain::Network;
    use crate::process::{BinaryDigest, BinaryKind};

    #[tokio::test]
    async fn a_sidecar_that_never_generates_login_fails_startup_and_is_cleaned_up() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("no-login-sidecar");
        let contents = b"#!/bin/sh\nwhile :; do sleep 1; done\n";
        fs::write(&script, contents).unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).unwrap();
        let digest = Sha256::digest(contents);
        let digest = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let binary = VerifiedBinary::verify(
            BinaryKind::WalletRpc,
            &script,
            BinaryDigest::parse_hex(&digest).unwrap(),
        )
        .unwrap();
        let paths = AppPaths::new(temp.path().join("app-data"), Network::Mainnet).unwrap();
        let result = WalletRpcSession::start_with_deadline(
            &binary,
            &paths,
            &NodeConfig::managed_local(Network::Mainnet),
            23456,
            Duration::from_millis(100),
        )
        .await;
        assert!(matches!(result, Err(WalletRpcStartupError::Deadline)));
    }
}
