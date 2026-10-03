use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use super::Network;

/// Hybrid runs a local daemon with an explicitly chosen bootstrap node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum NodeMode {
    Local,
    Remote,
    Hybrid,
}

/// Trust is explicit rather than inferred from a host name or port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum NodeTrust {
    ManagedLocal,
    ExplicitRemote,
}

/// A validated node selection. A host is kept separately from transport so a
/// caller cannot smuggle a URL, path, credentials, or HTTPS downgrade through
/// this configuration type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NodeConfig {
    pub mode: NodeMode,
    pub network: Network,
    pub host: String,
    pub port: u16,
    pub trust: NodeTrust,
    #[serde(default)]
    #[ts(optional)]
    pub bootstrap: Option<RemoteNode>,
    #[serde(default)]
    #[ts(optional)]
    pub advanced: Option<NodeOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(deny_unknown_fields)]
pub struct RemoteNode {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, deny_unknown_fields)]
pub struct NodeOptions {
    pub daemon_log_level: u8,
    pub wallet_log_level: u8,
    pub in_peers: i32,
    pub out_peers: i32,
    pub limit_rate_up: i32,
    pub limit_rate_down: i32,
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub zmq_port: u16,
    pub wallet_rpc_port: u16,
    pub public_p2p: bool,
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            daemon_log_level: 0,
            wallet_log_level: 0,
            in_peers: -1,
            out_peers: -1,
            limit_rate_up: -1,
            limit_rate_down: -1,
            p2p_port: 0,
            rpc_port: 0,
            zmq_port: 0,
            wallet_rpc_port: 0,
            public_p2p: false,
        }
    }
}

impl NodeOptions {
    pub fn validate(&self) -> Result<(), NodeConfigError> {
        if self.daemon_log_level > 4
            || self.wallet_log_level > 4
            || [
                self.in_peers,
                self.out_peers,
                self.limit_rate_up,
                self.limit_rate_down,
            ]
            .iter()
            .any(|value| !(-1..=65535).contains(value))
        {
            return Err(NodeConfigError::InvalidOptions);
        }
        let ports = [
            self.p2p_port,
            self.rpc_port,
            self.zmq_port,
            self.wallet_rpc_port,
        ];
        for (index, port) in ports.iter().enumerate() {
            if *port != 0 && (*port < 1024 || ports[..index].contains(port)) {
                return Err(NodeConfigError::InvalidOptions);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NodeConfigError {
    #[error(
        "node host must be an IPv4, IPv6, or DNS host name without a scheme, path, or credentials"
    )]
    InvalidHost,
    #[error("node port must be between 1 and 65535")]
    InvalidPort,
    #[error("node options have invalid limits, log levels or conflicting ports")]
    InvalidOptions,
}

impl NodeConfig {
    /// The app-owned daemon is always loopback-bound and uses the network's
    /// documented default RPC port until the supervised process selects it.
    pub fn managed_local(network: Network) -> Self {
        Self {
            mode: NodeMode::Local,
            network,
            host: "127.0.0.1".to_owned(),
            port: network.default_daemon_rpc_port(),
            trust: NodeTrust::ManagedLocal,
            bootstrap: None,
            advanced: None,
        }
    }

    /// Constructs an explicit remote-daemon selection. This only validates
    /// configuration; remote transport is a later, separately reviewed adapter.
    pub fn remote(network: Network, host: String, port: u16) -> Result<Self, NodeConfigError> {
        if !valid_host(&host) {
            return Err(NodeConfigError::InvalidHost);
        }
        if port == 0 {
            return Err(NodeConfigError::InvalidPort);
        }
        Ok(Self {
            mode: NodeMode::Remote,
            network,
            host,
            port,
            trust: NodeTrust::ExplicitRemote,
            bootstrap: None,
            advanced: None,
        })
    }

    pub fn hybrid(network: Network, host: String, port: u16) -> Result<Self, NodeConfigError> {
        Self::remote(network, host.clone(), port)?;
        let mut config = Self::managed_local(network);
        config.mode = NodeMode::Hybrid;
        config.bootstrap = Some(RemoteNode { host, port });
        Ok(config)
    }

    pub fn uses_local(&self) -> bool {
        self.mode != NodeMode::Remote
    }
    pub fn options(&self) -> NodeOptions {
        self.advanced.clone().unwrap_or_default()
    }

    pub fn validate(&self) -> Result<(), NodeConfigError> {
        self.options().validate()?;
        match (&self.bootstrap, self.mode) {
            (Some(remote), NodeMode::Hybrid) if valid_host(&remote.host) && remote.port != 0 => {}
            (None, NodeMode::Local | NodeMode::Remote) => {}
            _ => return Err(NodeConfigError::InvalidHost),
        }
        if self.port == 0 {
            return Err(NodeConfigError::InvalidPort);
        }
        match (self.mode, self.trust) {
            (NodeMode::Local | NodeMode::Hybrid, NodeTrust::ManagedLocal) => {
                if self.host != "127.0.0.1" {
                    Err(NodeConfigError::InvalidHost)
                } else {
                    Ok(())
                }
            }
            (NodeMode::Remote, NodeTrust::ExplicitRemote) if valid_host(&self.host) => Ok(()),
            _ => Err(NodeConfigError::InvalidHost),
        }
    }
}

fn valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    if host.contains(['/', '@', ':']) {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_configs_load_and_hybrid_limits_are_validated() {
        let old: NodeConfig = serde_json::from_str(r#"{"mode":"local","network":"mainnet","host":"127.0.0.1","port":12211,"trust":"managed_local"}"#).unwrap();
        assert!(old.validate().is_ok());
        assert_eq!(old.options(), NodeOptions::default());
        let mut hybrid =
            NodeConfig::hybrid(Network::Mainnet, "bootstrap.example.org".into(), 12211).unwrap();
        assert!(hybrid.validate().is_ok());
        let options = NodeOptions {
            rpc_port: 21000,
            p2p_port: 21000,
            ..NodeOptions::default()
        };
        hybrid.advanced = Some(options);
        assert_eq!(hybrid.validate(), Err(NodeConfigError::InvalidOptions));
        hybrid.advanced = None;
        hybrid.bootstrap = None;
        assert!(hybrid.validate().is_err());
    }

    #[test]
    fn local_and_remote_configs_have_explicit_different_trust() {
        let local = NodeConfig::managed_local(Network::Mainnet);
        assert_eq!(local.mode, NodeMode::Local);
        assert_eq!(local.host, "127.0.0.1");
        assert_eq!(local.port, 12211);
        assert_eq!(local.trust, NodeTrust::ManagedLocal);

        let remote =
            NodeConfig::remote(Network::Testnet, "node.example.org".to_owned(), 13311).unwrap();
        assert_eq!(remote.mode, NodeMode::Remote);
        assert_eq!(remote.trust, NodeTrust::ExplicitRemote);
    }

    #[test]
    fn remote_host_cannot_be_a_url_or_credential_container() {
        for host in [
            "",
            "http://node.example.org",
            "https://node.example.org",
            "node.example.org/rpc",
            "user@node.example.org",
            "node..example.org",
            "-node.example.org",
            "node.example.org-",
        ] {
            assert_eq!(
                NodeConfig::remote(Network::Mainnet, host.to_owned(), 12211),
                Err(NodeConfigError::InvalidHost)
            );
        }
        assert_eq!(
            NodeConfig::remote(Network::Mainnet, "node.example.org".to_owned(), 0),
            Err(NodeConfigError::InvalidPort)
        );
    }

    #[test]
    fn deserialized_node_configuration_must_preserve_mode_and_trust() {
        let mut config = NodeConfig::managed_local(Network::Mainnet);
        config.trust = NodeTrust::ExplicitRemote;
        assert_eq!(config.validate(), Err(NodeConfigError::InvalidHost));

        let mut runtime_endpoint = NodeConfig::managed_local(Network::Mainnet);
        runtime_endpoint.port = 23456;
        assert_eq!(runtime_endpoint.validate(), Ok(()));
    }
}
