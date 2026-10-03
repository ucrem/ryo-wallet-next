use std::ffi::OsString;
use std::net::{Ipv4Addr, TcpListener};
use std::time::Duration;

use thiserror::Error;
use tokio::time::{Instant, sleep, timeout};

use super::{BinaryKind, ManagedProcess, ProcessError, VerifiedBinary};
use crate::domain::{Network, NodeConfig};
use crate::rpc::DaemonRpcClient;
use crate::storage::AppPaths;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("local node configuration or storage is invalid")]
    Configuration,
    #[error("local node could not be started or stopped")]
    Process(#[from] ProcessError),
    #[error("local node did not become reachable before the deadline")]
    Startup,
}

pub struct DaemonSession {
    process: ManagedProcess,
    node: NodeConfig,
}

impl DaemonSession {
    pub async fn start(
        binary: &VerifiedBinary,
        paths: &AppPaths,
        node: &NodeConfig,
    ) -> Result<Self, DaemonError> {
        if binary.kind != BinaryKind::Daemon || !node.uses_local() || node.validate().is_err() {
            return Err(DaemonError::Configuration);
        }
        paths
            .ensure_private_dirs()
            .map_err(|_| DaemonError::Configuration)?;
        // Never inherit ~/.ryo/ryo.conf or another wallet application's settings.
        let config = paths.runtime_root().join("ryod-next.conf");
        match std::fs::symlink_metadata(&config) {
            Ok(metadata)
                if metadata.is_file()
                    && !metadata.file_type().is_symlink()
                    && metadata.len() == 0 => {}
            Ok(_) => return Err(DaemonError::Configuration),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&config)
                    .map_err(|_| DaemonError::Configuration)?;
            }
            Err(_) => return Err(DaemonError::Configuration),
        }
        // Use private ports so another wallet's daemon is never adopted or stopped.
        let mut endpoint = node.clone();
        let options = node.options();
        endpoint.port = configured_port(options.rpc_port)?;
        let p2p_port = configured_port(options.p2p_port)?;
        let zmq_port = configured_port(options.zmq_port)?;
        let args = daemon_args(paths, &endpoint, p2p_port, zmq_port)?;
        let working_directory = super::upstream_path::upstream_path(&paths.chain_root())
            .map_err(|_| DaemonError::Configuration)?;
        let mut process = ManagedProcess::start_daemon(binary, &args, &working_directory).await?;
        let daemon =
            DaemonRpcClient::configured(&endpoint).map_err(|_| DaemonError::Configuration)?;
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            if process.has_exited()? {
                return Err(DaemonError::Startup);
            }
            if let Ok(Ok(health)) = timeout(Duration::from_secs(2), daemon.health()).await {
                if health.network == node.network {
                    return Ok(Self {
                        process,
                        node: endpoint,
                    });
                }
                let _ = process.force_stop(Duration::from_secs(2)).await;
                return Err(DaemonError::Startup);
            }
            if Instant::now() >= until {
                let _ = process.force_stop(Duration::from_secs(2)).await;
                return Err(DaemonError::Startup);
            }
            sleep(Duration::from_millis(200)).await;
        }
    }

    pub fn node(&self) -> &NodeConfig {
        &self.node
    }

    pub fn has_exited(&mut self) -> Result<bool, ProcessError> {
        self.process.has_exited()
    }

    pub async fn stop(&mut self) -> Result<(), ProcessError> {
        self.process.request_daemon_exit().await;
        match self.process.wait_for_exit(Duration::from_secs(15)).await {
            Ok(_) => Ok(()),
            Err(ProcessError::ExitTimeout) => self
                .process
                .force_stop(Duration::from_secs(2))
                .await
                .map(|_| ()),
            Err(error) => Err(error),
        }
    }
}

fn free_port() -> Result<u16, DaemonError> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|_| DaemonError::Configuration)
}

fn configured_port(port: u16) -> Result<u16, DaemonError> {
    if port == 0 {
        free_port()
    } else {
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|_| DaemonError::Configuration)?;
        Ok(port)
    }
}

fn daemon_args(
    paths: &AppPaths,
    node: &NodeConfig,
    p2p_port: u16,
    zmq_port: u16,
) -> Result<Vec<OsString>, DaemonError> {
    let runtime_path = |path: std::path::PathBuf| {
        super::upstream_path::upstream_path(&path)
            .map(|path| path.into_os_string())
            .map_err(|_| DaemonError::Configuration)
    };
    let mut args = vec![
        "--config-file".into(),
        runtime_path(paths.runtime_root().join("ryod-next.conf"))?,
        "--check-updates".into(),
        "disabled".into(),
        "--data-dir".into(),
        runtime_path(paths.chain_root())?,
        "--rpc-bind-ip".into(),
        "127.0.0.1".into(),
        "--rpc-bind-port".into(),
        node.port.to_string().into(),
        "--restricted-rpc".into(),
        "--p2p-bind-ip".into(),
        if node.options().public_p2p {
            "0.0.0.0".into()
        } else {
            "127.0.0.1".into()
        },
        "--p2p-bind-port".into(),
        p2p_port.to_string().into(),
        "--zmq-rpc-bind-ip".into(),
        "127.0.0.1".into(),
        "--zmq-rpc-bind-port".into(),
        zmq_port.to_string().into(),
        "--no-igd".into(),
        "--log-file".into(),
        runtime_path(paths.chain_root().join("ryod.log"))?,
    ];
    let options = node.options();
    if !options.public_p2p {
        args.push("--hide-my-port".into());
    }
    for (flag, value) in [
        ("--out-peers", options.out_peers),
        ("--in-peers", options.in_peers),
        ("--limit-rate-up", options.limit_rate_up),
        ("--limit-rate-down", options.limit_rate_down),
    ] {
        if value >= 0 {
            args.extend([flag.into(), value.to_string().into()]);
        }
    }
    args.extend([
        "--log-level".into(),
        options.daemon_log_level.to_string().into(),
        "--log-file-level".into(),
        options.daemon_log_level.to_string().into(),
    ]);
    if let Some(remote) = &node.bootstrap {
        let host = if remote.host.contains(':') {
            format!("[{}]", remote.host)
        } else {
            remote.host.clone()
        };
        args.extend([
            "--bootstrap-daemon-address".into(),
            format!("{host}:{}", remote.port).into(),
        ]);
    }
    match node.network {
        Network::Mainnet => {}
        Network::Testnet => args.push("--testnet".into()),
        Network::Stagenet => args.push("--stagenet".into()),
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hybrid_bootstrap_and_bounded_options_become_structured_arguments() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_owned(), Network::Mainnet).unwrap();
        let mut node = NodeConfig::hybrid(Network::Mainnet, "2001:db8::1".into(), 12211).unwrap();
        node.advanced = Some(crate::domain::NodeOptions {
            out_peers: 12,
            limit_rate_down: 512,
            daemon_log_level: 2,
            ..Default::default()
        });
        let args: Vec<_> = daemon_args(&paths, &node, 21000, 21001)
            .unwrap()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        for (flag, value) in [
            ("--bootstrap-daemon-address", "[2001:db8::1]:12211"),
            ("--out-peers", "12"),
            ("--limit-rate-down", "512"),
            ("--log-file-level", "2"),
            ("--rpc-bind-ip", "127.0.0.1"),
        ] {
            assert!(args.windows(2).any(|pair| pair == [flag, value]));
        }
        assert!(!args.contains(&"--in-peers".into()));
    }

    #[test]
    fn local_daemon_has_isolated_storage_restricted_loopback_rpc_and_no_wallet_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_owned(), Network::Testnet).unwrap();
        let node = NodeConfig::managed_local(Network::Testnet);
        let args = daemon_args(&paths, &node, 23456, 23457).unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(args.contains(&"--restricted-rpc".into()));
        assert!(args.contains(&"--testnet".into()));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--rpc-bind-ip", "127.0.0.1"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--data-dir"
                    && pair[1] == paths.chain_root().to_string_lossy())
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("wallet") || arg.contains("password"))
        );
    }
}
