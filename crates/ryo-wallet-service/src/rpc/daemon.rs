use std::net::SocketAddrV4;

use serde::Deserialize;
use serde_json::json;

use super::transport::{JsonRpcTransport, RpcCredentials, RpcError};
use crate::domain::Network;

pub struct DaemonRpcClient {
    transport: JsonRpcTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeHealth {
    pub network: Network,
    pub height: u64,
    pub target_height: u64,
    pub ready: bool,
    pub offline: bool,
    pub untrusted: bool,
}

impl DaemonRpcClient {
    pub fn local(
        address: SocketAddrV4,
        credentials: Option<RpcCredentials>,
    ) -> Result<Self, RpcError> {
        Ok(Self {
            transport: JsonRpcTransport::local(address, credentials)?,
        })
    }

    /// A local daemon is a health source. Remote daemon transport is deliberately
    /// absent until the separate trust and transport policy is implemented.
    pub async fn health(&self) -> Result<NodeHealth, RpcError> {
        #[derive(Deserialize)]
        struct Info {
            height: u64,
            target_height: u64,
            mainnet: bool,
            testnet: bool,
            stagenet: bool,
            is_ready: bool,
            offline: bool,
            untrusted: bool,
        }
        let info: Info = self.transport.call("get_info", &json!({})).await?;
        let network = match (info.mainnet, info.testnet, info.stagenet) {
            (true, false, false) => Network::Mainnet,
            (false, true, false) => Network::Testnet,
            (false, false, true) => Network::Stagenet,
            _ => return Err(RpcError::InvalidResponse),
        };
        Ok(NodeHealth {
            network,
            height: info.height,
            target_height: info.target_height,
            ready: info.is_ready,
            offline: info.offline,
            untrusted: info.untrusted,
        })
    }
}
