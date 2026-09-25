use std::net::SocketAddrV4;

use serde::{Deserialize, Serialize};
use serde_json::json;
use zeroize::Zeroizing;

use super::transport::{JsonRpcTransport, RpcCredentials, RpcError};
use crate::domain::AtomicAmount;
use crate::storage::WalletId;

pub struct WalletRpcClient {
    transport: JsonRpcTransport,
}

/// Secret recovery material returned after wallet creation. It intentionally
/// has no `Debug`, `Display`, `Clone`, or serialization implementation.
pub struct RecoveryPhrase(Zeroizing<String>);

impl RecoveryPhrase {
    /// Consumes the one-time value so a caller must explicitly decide when to
    /// hand it to the dedicated backup UI.
    pub fn into_secret(self) -> Zeroizing<String> {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Balance {
    pub total: AtomicAmount,
    pub unlocked: AtomicAmount,
    pub multisig_import_needed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletScope {
    pub account_count: usize,
    pub multisig: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiveAddress {
    pub address_index: u32,
    pub address: String,
    pub label: String,
    pub used: bool,
}

impl WalletRpcClient {
    pub fn new(address: SocketAddrV4, credentials: RpcCredentials) -> Result<Self, RpcError> {
        Ok(Self {
            transport: JsonRpcTransport::local(address, Some(credentials))?,
        })
    }

    pub async fn languages(&self) -> Result<Vec<String>, RpcError> {
        #[derive(Deserialize)]
        struct Languages {
            languages: Vec<String>,
        }
        let result: Languages = self.transport.call("get_languages", &json!({})).await?;
        if result.languages.is_empty() {
            return Err(RpcError::InvalidResponse);
        }
        Ok(result.languages)
    }

    /// Creates a long-address wallet in an already allocated app-owned wallet
    /// directory. Recovery material is not returned by this RPC; it must be
    /// obtained through the separate one-time backup flow after creation.
    pub async fn create_wallet(
        &self,
        wallet_id: &WalletId,
        password: Zeroizing<String>,
        language: &str,
    ) -> Result<(), RpcError> {
        if language.is_empty()
            || language.len() > 128
            || language.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(RpcError::InvalidResponse);
        }
        #[derive(Serialize)]
        struct CreateWallet<'a> {
            filename: String,
            password: &'a str,
            language: &'a str,
            short_address: bool,
        }
        #[derive(Deserialize)]
        struct Empty {}

        let request = CreateWallet {
            filename: wallet_filename(wallet_id),
            password: password.as_str(),
            language,
            short_address: false,
        };
        self.transport
            .call::<_, Empty>("create_wallet", &request)
            .await
            .map(|_| ())
    }

    /// Restores a long or legacy seed into an already allocated app-owned
    /// directory. Seed and password remain zeroizing Rust values throughout the
    /// caller-controlled part of the request lifetime.
    pub async fn restore_wallet(
        &self,
        wallet_id: &WalletId,
        password: Zeroizing<String>,
        seed: Zeroizing<String>,
        refresh_start_height: u64,
    ) -> Result<(), RpcError> {
        #[derive(Serialize)]
        struct RestoreWallet<'a> {
            filename: String,
            password: &'a str,
            seed: &'a str,
            refresh_start_height: u64,
        }
        #[derive(Deserialize)]
        struct Empty {}

        let request = RestoreWallet {
            filename: wallet_filename(wallet_id),
            password: password.as_str(),
            seed: seed.as_str(),
            refresh_start_height,
        };
        self.transport
            .call::<_, Empty>("restore_wallet", &request)
            .await
            .map(|_| ())
    }

    /// Requests only the mnemonic recovery phrase; spend and view keys are
    /// deliberately not reachable through this client API.
    pub async fn query_mnemonic(&self) -> Result<RecoveryPhrase, RpcError> {
        #[derive(Deserialize)]
        struct KeyResponse {
            key: String,
        }
        let result: KeyResponse = self
            .transport
            .call("query_key", &json!({ "key_type": "mnemonic" }))
            .await?;
        if result.key.is_empty()
            || result.key.len() > 4_096
            || result.key.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(RpcError::InvalidResponse);
        }
        Ok(RecoveryPhrase(Zeroizing::new(result.key)))
    }

    /// Opens only an app-owned imported wallet. The renderer never supplies a
    /// filename or path; the sidecar's configured wallet directory is expected
    /// to be the app-owned `wallets` directory.
    pub async fn open_imported_wallet(
        &self,
        wallet_id: &WalletId,
        password: Zeroizing<String>,
    ) -> Result<(), RpcError> {
        #[derive(Serialize)]
        struct OpenWallet<'a> {
            filename: String,
            password: &'a str,
        }
        #[derive(Deserialize)]
        struct Empty {}

        let request = OpenWallet {
            filename: wallet_filename(wallet_id),
            password: password.as_str(),
        };
        self.transport
            .call::<_, Empty>("open_wallet", &request)
            .await
            .map(|_| ())
            .map_err(|error| match error {
                RpcError::Remote(-22) => RpcError::IncorrectPassword,
                other => other,
            })
    }

    /// Closes a wallet backend while leaving the RPC process available.
    pub async fn close_wallet(&self) -> Result<(), RpcError> {
        self.empty_call("close_wallet").await
    }

    /// Saves and requests shutdown of the wallet RPC process. The caller must
    /// still observe the owned child process exit before reporting a lock.
    pub async fn stop_wallet(&self) -> Result<(), RpcError> {
        self.empty_call("stop_wallet").await
    }

    pub async fn height(&self) -> Result<u64, RpcError> {
        #[derive(Deserialize)]
        struct Height {
            height: u64,
        }
        Ok(self
            .transport
            .call::<_, Height>("get_height", &json!({}))
            .await?
            .height)
    }

    pub async fn balance(&self) -> Result<Balance, RpcError> {
        #[derive(Deserialize)]
        struct RawBalance {
            balance: u64,
            unlocked_balance: u64,
            multisig_import_needed: bool,
        }
        let raw: RawBalance = self
            .transport
            .call("get_balance", &json!({ "account_index": 0 }))
            .await?;
        if raw.unlocked_balance > raw.balance {
            return Err(RpcError::InvalidResponse);
        }
        Ok(Balance {
            total: AtomicAmount::from_atomic(raw.balance),
            unlocked: AtomicAmount::from_atomic(raw.unlocked_balance),
            multisig_import_needed: raw.multisig_import_needed,
        })
    }

    pub async fn primary_address(&self) -> Result<String, RpcError> {
        #[derive(Deserialize)]
        struct Address {
            address: String,
        }
        let result: Address = self
            .transport
            .call(
                "get_address",
                &json!({ "account_index": 0, "address_index": [0] }),
            )
            .await?;
        if !valid_wallet_address(&result.address) {
            return Err(RpcError::InvalidResponse);
        }
        Ok(result.address)
    }

    pub async fn receive_addresses(&self) -> Result<Vec<ReceiveAddress>, RpcError> {
        #[derive(Deserialize)]
        struct AddressList {
            address: String,
            addresses: Vec<ReceiveAddress>,
        }
        let result: AddressList = self
            .transport
            .call("get_address", &json!({ "account_index": 0 }))
            .await?;
        if result.addresses.is_empty()
            || result.address != result.addresses[0].address
            || result.addresses.iter().enumerate().any(|(index, entry)| {
                entry.address_index as usize != index
                    || !valid_wallet_address(&entry.address)
                    || entry.label.len() > 256
                    || entry.label.chars().any(char::is_control)
            })
        {
            return Err(RpcError::InvalidResponse);
        }
        Ok(result.addresses)
    }

    /// Creates an account-zero subaddress and saves it before exposing it to the UI.
    pub async fn create_receive_address(&self) -> Result<ReceiveAddress, RpcError> {
        #[derive(Deserialize)]
        struct CreatedAddress {
            address: String,
            address_index: u32,
        }
        let created: CreatedAddress = self
            .transport
            .call(
                "create_address",
                &json!({ "account_index": 0, "label": "" }),
            )
            .await?;
        if created.address_index == 0 || !valid_wallet_address(&created.address) {
            return Err(RpcError::InvalidResponse);
        }
        self.empty_call("store").await?;
        self.receive_addresses()
            .await?
            .into_iter()
            .find(|entry| {
                entry.address_index == created.address_index && entry.address == created.address
            })
            .ok_or(RpcError::InvalidResponse)
    }

    /// Reports only scope metadata needed to enforce the MVP's account-zero,
    /// non-multisig boundary. It never requests spend or view keys.
    pub async fn scope(&self) -> Result<WalletScope, RpcError> {
        #[derive(Deserialize)]
        struct Account {
            account_index: u32,
        }
        #[derive(Deserialize)]
        struct Accounts {
            subaddress_accounts: Vec<Account>,
        }
        #[derive(Deserialize)]
        struct Multisig {
            multisig: bool,
        }
        let accounts: Accounts = self
            .transport
            .call("get_accounts", &json!({ "tag": "" }))
            .await?;
        if accounts.subaddress_accounts.is_empty()
            || accounts
                .subaddress_accounts
                .iter()
                .enumerate()
                .any(|(index, account)| account.account_index != index as u32)
        {
            return Err(RpcError::InvalidResponse);
        }
        let multisig: Multisig = self.transport.call("is_multisig", &json!({})).await?;
        Ok(WalletScope {
            account_count: accounts.subaddress_accounts.len(),
            multisig: multisig.multisig,
        })
    }

    /// Uses a registered method because upstream `validate_address` is not in its RPC map.
    /// Requires an open wallet and inherits that wallet's network rules.
    pub async fn validate_address(&self, address: &str) -> Result<(), RpcError> {
        if address.is_empty()
            || address.len() > 256
            || !address.bytes().all(|c| c.is_ascii_alphanumeric())
        {
            return Err(RpcError::InvalidAddress);
        }
        #[derive(Deserialize)]
        struct ParsedUri {
            uri: ParsedAddress,
        }
        #[derive(Deserialize)]
        struct ParsedAddress {
            address: String,
        }
        let parsed: ParsedUri = self
            .transport
            .call("parse_uri", &json!({ "uri": format!("ryo:{address}") }))
            .await
            .map_err(|error| match error {
                RpcError::Remote(-11) | RpcError::Remote(-2) => RpcError::InvalidAddress,
                other => other,
            })?;
        if parsed.uri.address != address {
            return Err(RpcError::InvalidResponse);
        }
        Ok(())
    }

    async fn empty_call(&self, method: &'static str) -> Result<(), RpcError> {
        #[derive(Deserialize)]
        struct Empty {}
        self.transport
            .call::<_, Empty>(method, &json!({}))
            .await
            .map(|_| ())
    }
}

fn wallet_filename(wallet_id: &WalletId) -> String {
    format!("{}/wallet", wallet_id.as_str())
}

fn valid_wallet_address(address: &str) -> bool {
    !address.is_empty()
        && address.len() <= 256
        && address.bytes().all(|byte| byte.is_ascii_alphanumeric())
}
