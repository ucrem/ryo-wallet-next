use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Mainnet,
    Testnet,
    Stagenet,
}

impl Network {
    pub const fn default_daemon_rpc_port(self) -> u16 {
        match self {
            Self::Mainnet => 12211,
            Self::Testnet => 13311,
            Self::Stagenet => 14411,
        }
    }
}
