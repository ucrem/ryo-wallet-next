use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;

use thiserror::Error;

use crate::domain::{Network, NodeConfig, NodeConfigError};
use crate::storage::{AppPaths, PathError};

#[derive(Debug, Error)]
pub enum WalletRpcLaunchError {
    #[error("wallet RPC port must be nonzero")]
    InvalidPort,
    #[error("node configuration is invalid")]
    InvalidNode(#[source] NodeConfigError),
    #[error("private runtime directories could not be prepared")]
    Paths(#[source] PathError),
}

/// Structured, non-secret arguments for a `ryo-wallet-rpc` sidecar. Passwords
/// and RPC credentials are never command-line arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletRpcLaunch {
    pub args: Vec<OsString>,
    pub working_directory: PathBuf,
}

impl WalletRpcLaunch {
    pub fn prepare(
        paths: &AppPaths,
        node: &NodeConfig,
        rpc_port: u16,
    ) -> Result<Self, WalletRpcLaunchError> {
        if rpc_port == 0 {
            return Err(WalletRpcLaunchError::InvalidPort);
        }
        node.validate().map_err(WalletRpcLaunchError::InvalidNode)?;
        paths
            .ensure_private_dirs()
            .map_err(WalletRpcLaunchError::Paths)?;
        let mut args = vec![
            "--rpc-bind-ip".into(),
            "127.0.0.1".into(),
            "--rpc-bind-port".into(),
            rpc_port.to_string().into(),
            "--wallet-dir".into(),
            paths.wallets_root().into_os_string(),
            "--daemon-address".into(),
            daemon_address(node).into(),
        ];
        match node.network {
            Network::Mainnet => {}
            Network::Testnet => args.push("--testnet".into()),
            Network::Stagenet => args.push("--stagenet".into()),
        }
        Ok(Self {
            args,
            working_directory: paths.runtime_root(),
        })
    }
}

fn daemon_address(node: &NodeConfig) -> String {
    let host = match node.host.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("[{}]", node.host),
        _ => node.host.clone(),
    };
    format!("http://{host}:{}", node.port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NodeConfig;

    fn app_paths(temp: &tempfile::TempDir, network: Network) -> AppPaths {
        AppPaths::new(temp.path().join("app-data"), network).unwrap()
    }

    #[test]
    fn local_launch_is_loopback_authenticated_and_never_disables_login() {
        let temp = tempfile::tempdir().unwrap();
        let paths = app_paths(&temp, Network::Mainnet);
        let launch =
            WalletRpcLaunch::prepare(&paths, &NodeConfig::managed_local(Network::Mainnet), 23456)
                .unwrap();
        let args = launch
            .args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--rpc-bind-ip", "127.0.0.1"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--daemon-address", "http://127.0.0.1:12211"])
        );
        assert!(!args.contains(&"--disable-rpc-login".into()));
        assert!(launch.working_directory.is_dir());
    }

    #[test]
    fn remote_ipv6_and_network_flag_are_explicit() {
        let temp = tempfile::tempdir().unwrap();
        let paths = app_paths(&temp, Network::Stagenet);
        let node = NodeConfig::remote(Network::Stagenet, "2001:db8::1".to_owned(), 14411).unwrap();
        let launch = WalletRpcLaunch::prepare(&paths, &node, 23456).unwrap();
        let args = launch
            .args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--daemon-address", "http://[2001:db8::1]:14411"])
        );
        assert!(args.contains(&"--stagenet".into()));
    }
}
