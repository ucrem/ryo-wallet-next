use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use super::Network;

/// The two mutually exclusive node modes supported by the product contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum NodeMode {
    Local,
    Remote,
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
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NodeConfigError {
    #[error(
        "node host must be an IPv4, IPv6, or DNS host name without a scheme, path, or credentials"
    )]
    InvalidHost,
    #[error("node port must be between 1 and 65535")]
    InvalidPort,
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
        })
    }

    pub fn validate(&self) -> Result<(), NodeConfigError> {
        if self.port == 0 {
            return Err(NodeConfigError::InvalidPort);
        }
        match (self.mode, self.trust) {
            (NodeMode::Local, NodeTrust::ManagedLocal) => {
                if self.host != "127.0.0.1" {
                    Err(NodeConfigError::InvalidHost)
                } else if self.port != self.network.default_daemon_rpc_port() {
                    Err(NodeConfigError::InvalidPort)
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

        let mut wrong_port = NodeConfig::managed_local(Network::Mainnet);
        wrong_port.port = 12212;
        assert_eq!(wrong_port.validate(), Err(NodeConfigError::InvalidPort));
    }
}
