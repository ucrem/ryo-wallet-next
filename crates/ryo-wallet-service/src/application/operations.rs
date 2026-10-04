//! Account-zero wallet operations. Signed metadata never crosses IPC.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::domain::{AtomicAmount, NodeConfig};
use crate::rpc::{DaemonRpcClient, RpcError, WalletRpcClient};
use crate::storage::save_wallet_name;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WalletOperation {
    Info,
    SetName {
        name: String,
    },
    History,
    SetNote {
        txid: String,
        note: String,
    },
    Contacts,
    SaveContact {
        id: Option<String>,
        name: String,
        address: String,
        payment_id: String,
        notes: String,
    },
    DeleteContact {
        id: String,
    },
    LabelAddress {
        index: u32,
        label: String,
    },
    AddressBalances,
    MakeRequest {
        address: String,
        amount: String,
        payment_id: String,
        description: String,
    },
    ParseRequest {
        uri: String,
    },
    PrepareSend {
        address: String,
        amount: String,
        sweep: bool,
        payment_id: String,
        priority: u32,
        ring_size: u32,
    },
    ConfirmSend {
        token: String,
    },
    CancelSend {
        token: String,
    },
    SendJournal,
    ChangePassword {
        old_password: Zeroizing<String>,
        new_password: Zeroizing<String>,
    },
    Rescan {
        spent_only: bool,
    },
    Secrets {
        password: Zeroizing<String>,
    },
    ExportKeyImages {
        filename: String,
        password: Zeroizing<String>,
    },
    ImportKeyImages {
        filename: String,
    },
    Authenticate {
        password: Zeroizing<String>,
    },
}

impl WalletOperation {
    pub(crate) fn is_background_read(&self) -> bool {
        matches!(
            self,
            Self::Info | Self::History | Self::Contacts | Self::AddressBalances
        )
    }
}

pub struct SecretMaterial {
    phrase: Zeroizing<String>,
    view_key: Zeroizing<String>,
    spend_key: Zeroizing<String>,
}
impl Serialize for SecretMaterial {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = serializer.serialize_struct("SecretMaterial", 3)?;
        out.serialize_field("phrase", self.phrase.as_str())?;
        out.serialize_field("view_key", self.view_key.as_str())?;
        out.serialize_field("spend_key", self.spend_key.as_str())?;
        out.end()
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum WalletOperationOutput {
    Json(Value),
    Secrets(SecretMaterial),
}

#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("wallet operation failed")]
    Rpc(#[from] RpcError),
    #[error("wallet operation could not be saved; check its current state before retrying")]
    Storage,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contact {
    id: String,
    name: String,
    address: String,
    payment_id: String,
    notes: String,
}

#[derive(Serialize, Deserialize)]
struct RawDestination {
    address: String,
    amount: u64,
}
#[derive(Deserialize)]
struct RawTransfer {
    txid: String,
    payment_id: String,
    height: u64,
    timestamp: u64,
    amount: u64,
    fee: u64,
    note: String,
    #[serde(default)]
    destinations: Vec<RawDestination>,
    #[serde(rename = "type")]
    kind: String,
    unlock_time: u64,
    address: String,
    double_spend_seen: bool,
}
impl RawTransfer {
    fn into_dto(self) -> Result<Value, OperationError> {
        if !is_hash(&self.txid)
            || !["in", "out", "pending", "failed", "pool"].contains(&self.kind.as_str())
        {
            return Err(RpcError::InvalidResponse.into());
        }
        Ok(
            json!({"txid":self.txid,"payment_id":clean_payment_id(&self.payment_id),
            "height":self.height.to_string(),"timestamp":self.timestamp.to_string(),
            "amount":self.amount.to_string(),"fee":self.fee.to_string(),"note":self.note,
            "destinations":self.destinations.into_iter().map(|d| json!({"address":d.address,"amount":d.amount.to_string()})).collect::<Vec<_>>(),
            "type":self.kind,"unlock_time":self.unlock_time.to_string(),"address":self.address,
            "double_spend_seen":self.double_spend_seen}),
        )
    }
}

pub(crate) struct PreparedSend {
    token: String,
    generation: String,
    expires: Instant,
    pub(crate) hashes: Vec<String>,
    metadata: Vec<Zeroizing<String>>,
    summary: Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct JournalEntry {
    pub(crate) txid: String,
    pub(crate) state: String,
}
#[derive(Default)]
pub(crate) struct OperationsState {
    draft: Option<PreparedSend>,
}
impl OperationsState {
    pub(crate) fn expire(&mut self) {
        if self
            .draft
            .as_ref()
            .is_some_and(|d| Instant::now() >= d.expires)
        {
            self.invalidate();
        }
    }
    pub(crate) fn invalidate(&mut self) {
        self.draft = None;
    }

    pub(crate) async fn execute(
        &mut self,
        client: &WalletRpcClient,
        node: &NodeConfig,
        directory: &Path,
        generation: &str,
        operation: WalletOperation,
    ) -> Result<WalletOperationOutput, OperationError> {
        let value = match operation {
            WalletOperation::Info => {
                let name = attribute(client, "next.name").await?.unwrap_or_default();
                // Backfill names from older wallets after a successful authenticated read.
                // A broken display cache must not prevent access to the encrypted wallet.
                let _ = save_wallet_name(directory, &name);
                json!({"name":name})
            }
            WalletOperation::SetName { name } => {
                text(&name, 100, false)?;
                save_attribute(client, "next.name", &name).await?;
                save_wallet_name(directory, &name).map_err(|_| OperationError::Storage)?;
                json!({})
            }
            WalletOperation::History => {
                #[derive(Default, Deserialize)]
                struct Transfers {
                    #[serde(rename = "in", default)]
                    incoming: Vec<RawTransfer>,
                    #[serde(rename = "out", default)]
                    outgoing: Vec<RawTransfer>,
                    #[serde(default)]
                    pending: Vec<RawTransfer>,
                    #[serde(default)]
                    failed: Vec<RawTransfer>,
                    #[serde(default)]
                    pool: Vec<RawTransfer>,
                }
                let raw: Transfers = client.transport.call("get_transfers", &json!({"in":true,"out":true,"pending":true,"failed":true,"pool":true,
                    "filter_by_height":false,"min_height":0,"account_index":0,"subaddr_indices":[]})).await?;
                let mut entries: Vec<RawTransfer> = raw
                    .incoming
                    .into_iter()
                    .chain(raw.outgoing)
                    .chain(raw.pending)
                    .chain(raw.failed)
                    .chain(raw.pool)
                    .collect();
                entries.sort_by(|a, b| {
                    b.timestamp
                        .cmp(&a.timestamp)
                        .then_with(|| b.height.cmp(&a.height))
                });
                json!(
                    entries
                        .into_iter()
                        .map(RawTransfer::into_dto)
                        .collect::<Result<Vec<_>, _>>()?
                )
            }
            WalletOperation::SetNote { txid, note } => {
                if !is_hash(&txid) {
                    return Err(OperationError::Invalid("invalid transaction hash"));
                }
                text(&note, 2000, true)?;
                let _: Value = client
                    .transport
                    .call("set_tx_notes", &json!({"txids":[txid],"notes":[note]}))
                    .await?;
                client.empty_call("store").await?;
                json!({})
            }
            WalletOperation::Contacts => json!(contacts(client).await?),
            WalletOperation::SaveContact {
                id,
                name,
                address,
                payment_id,
                notes,
            } => {
                text(&name, 100, false)?;
                text(&notes, 2000, true)?;
                validate_payment_id(&payment_id)?;
                client.validate_address(&address).await?;
                let mut entries = contacts(client).await?;
                if entries.len() >= 1000 && id.is_none() {
                    return Err(OperationError::Invalid("contact limit reached"));
                }
                let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
                let contact = Contact {
                    id: id.clone(),
                    name,
                    address,
                    payment_id,
                    notes,
                };
                if let Some(existing) = entries.iter_mut().find(|e| e.id == id) {
                    *existing = contact;
                } else {
                    entries.push(contact);
                }
                save_attribute(
                    client,
                    "next.contacts.v1",
                    &serde_json::to_string(&entries).map_err(|_| OperationError::Storage)?,
                )
                .await?;
                json!(entries)
            }
            WalletOperation::DeleteContact { id } => {
                let mut entries = contacts(client).await?;
                entries.retain(|e| e.id != id);
                save_attribute(
                    client,
                    "next.contacts.v1",
                    &serde_json::to_string(&entries).map_err(|_| OperationError::Storage)?,
                )
                .await?;
                json!(entries)
            }
            WalletOperation::LabelAddress { index, label } => {
                text(&label, 100, false)?;
                let _: Value = client
                    .transport
                    .call(
                        "label_address",
                        &json!({"index":{"major":0,"minor":index},"label":label}),
                    )
                    .await?;
                client.empty_call("store").await?;
                json!({})
            }
            WalletOperation::AddressBalances => {
                #[derive(Deserialize)]
                struct PerAddress {
                    address_index: u32,
                    balance: u64,
                    unlocked_balance: u64,
                    num_unspent_outputs: u64,
                }
                #[derive(Deserialize)]
                struct Balances {
                    #[serde(default)]
                    per_subaddress: Vec<PerAddress>,
                }
                let balances: Balances = client
                    .transport
                    .call("get_balance", &json!({"account_index":0}))
                    .await?;
                json!(balances.per_subaddress.into_iter().map(|b| json!({"index":b.address_index,"balance":b.balance.to_string(),"unlocked":b.unlocked_balance.to_string(),"outputs":b.num_unspent_outputs.to_string()})).collect::<Vec<_>>())
            }
            WalletOperation::MakeRequest {
                address,
                amount,
                payment_id,
                description,
            } => {
                client.validate_address(&address).await?;
                validate_payment_id(&payment_id)?;
                text(&description, 500, false)?;
                let amount = parse_amount(&amount, false)?;
                #[derive(Deserialize)]
                struct Uri {
                    uri: String,
                }
                let result:Uri=client.transport.call("make_uri", &json!({"address":address,"amount":amount,"payment_id":payment_id,"tx_description":description,"recipient_name":""})).await?;
                json!({"uri":result.uri})
            }
            WalletOperation::ParseRequest { uri } => {
                if !uri.starts_with("ryo:") || uri.len() > 4096 {
                    return Err(OperationError::Invalid("invalid Ryo payment request"));
                }
                #[derive(Deserialize)]
                struct Uri {
                    address: String,
                    payment_id: String,
                    amount: u64,
                    tx_description: String,
                }
                #[derive(Deserialize)]
                struct Parsed {
                    uri: Uri,
                    #[serde(default)]
                    unknown_parameters: Vec<String>,
                }
                let parsed: Parsed = client
                    .transport
                    .call("parse_uri", &json!({"uri":uri}))
                    .await?;
                if !parsed.unknown_parameters.is_empty() {
                    return Err(OperationError::Invalid(
                        "payment request contains unsupported parameters",
                    ));
                }
                json!({"address":parsed.uri.address,"payment_id":parsed.uri.payment_id,"amount":AtomicAmount::from_atomic(parsed.uri.amount).to_ryo(),"description":parsed.uri.tx_description})
            }
            WalletOperation::PrepareSend {
                address,
                amount,
                sweep,
                payment_id,
                priority,
                ring_size,
            } => {
                self.invalidate();
                require_synced(client, node).await?;
                if read_journal(directory)?
                    .iter()
                    .any(|e| e.state == "unknown")
                {
                    return Err(OperationError::Invalid(
                        "check the previous uncertain send in transaction history before preparing another",
                    ));
                }
                client.validate_address(&address).await?;
                validate_payment_id(&payment_id)?;
                if priority > 4 || ![25, 100].contains(&ring_size) {
                    return Err(OperationError::Invalid("invalid priority or ring size"));
                }
                let amount = if sweep {
                    0
                } else {
                    parse_amount(&amount, true)?
                };
                let balance = client.balance().await?;
                if balance.multisig_import_needed
                    || balance.unlocked.atomic() == 0
                    || amount > balance.unlocked.atomic()
                {
                    return Err(OperationError::Invalid("not enough unlocked balance"));
                }
                let mut params = json!({"account_index":0,"subaddr_indices":[],"priority":priority,"ring_size":ring_size,"unlock_time":0,"payment_id":payment_id,
                    "get_tx_keys":false,"do_not_relay":true,"get_tx_hex":false,"get_tx_metadata":true});
                let method = if sweep {
                    params["address"] = json!(address);
                    params["below_amount"] = json!(0);
                    "sweep_all"
                } else {
                    params["destinations"] = json!([{"address":address,"amount":amount}]);
                    "transfer_split"
                };
                let raw: RawPrepared = client.transport.call(method, &params).await?;
                let draft = prepared(raw, generation, &address, &payment_id, amount, sweep)?;
                let summary = draft.summary.clone();
                self.draft = Some(draft);
                summary
            }
            WalletOperation::CancelSend { token } => {
                if self.draft.as_ref().is_some_and(|d| d.token == token) {
                    self.invalidate();
                }
                json!({})
            }
            WalletOperation::ConfirmSend { token } => {
                require_synced(client, node).await?;
                let draft = self.draft.take().ok_or(OperationError::Invalid(
                    "send preview has expired or was already used",
                ))?;
                if draft.token != token
                    || draft.generation != generation
                    || Instant::now() >= draft.expires
                {
                    return Err(OperationError::Invalid(
                        "send preview has expired or belongs to a different wallet",
                    ));
                }
                let mut journal = read_journal(directory)?;
                let start = journal.len();
                journal.extend(draft.hashes.iter().map(|h| JournalEntry {
                    txid: h.clone(),
                    state: "not_sent".into(),
                }));
                // Persist intent before every network call. A lost reply must never cause a blind retry.
                for (i, metadata) in draft.metadata.iter().enumerate() {
                    journal[start + i].state = "unknown".into();
                    write_journal(directory, &journal)?;
                    #[derive(Deserialize)]
                    struct Relayed {
                        tx_hash: String,
                    }
                    #[derive(Serialize)]
                    struct Relay<'a> {
                        hex: &'a str,
                    }
                    match client
                        .transport
                        .call::<_, Relayed>(
                            "relay_tx",
                            &Relay {
                                hex: metadata.as_str(),
                            },
                        )
                        .await
                    {
                        Ok(result) if result.tx_hash == draft.hashes[i] => {
                            journal[start + i].state = "relayed".into()
                        }
                        _ => {
                            write_journal(directory, &journal)?;
                            break;
                        }
                    }
                    write_journal(directory, &journal)?;
                }
                let wallet_saved = client.empty_call("store").await.is_ok();
                json!({"transactions":journal[start..], "wallet_saved":wallet_saved})
            }
            WalletOperation::SendJournal => {
                let mut journal = read_journal(directory)?;
                for entry in journal.iter_mut().filter(|e| e.state == "unknown") {
                    #[derive(Deserialize)]
                    struct Found {
                        transfer: RawTransfer,
                    }
                    if let Ok(found) = client
                        .transport
                        .call::<_, Found>(
                            "get_transfer_by_txid",
                            &json!({"txid":entry.txid,"account_index":0}),
                        )
                        .await
                        && found.transfer.txid == entry.txid
                    {
                        entry.state = match found.transfer.kind.as_str() {
                            "out" => "confirmed",
                            "pending" => "relayed",
                            "failed" => "failed",
                            _ => "unknown",
                        }
                        .into();
                    }
                }
                write_journal(directory, &journal)?;
                json!(journal)
            }
            WalletOperation::ChangePassword {
                old_password,
                new_password,
            } => {
                if new_password.len() < 12 || new_password.len() > 4096 {
                    return Err(OperationError::Invalid(
                        "new password must contain 12 to 4096 bytes",
                    ));
                }
                self.invalidate();
                change_password(client, &old_password, &new_password).await?;
                json!({})
            }
            WalletOperation::Authenticate { password } => {
                authenticate(client, &password).await?;
                json!({})
            }
            WalletOperation::Secrets { password } => {
                authenticate(client, &password).await?;
                let phrase = client.query_mnemonic().await?.into_secret();
                let view_key = query_key(client, "view_key").await?;
                let spend_key = query_key(client, "spend_key").await?;
                return Ok(WalletOperationOutput::Secrets(SecretMaterial {
                    phrase,
                    view_key,
                    spend_key,
                }));
            }
            WalletOperation::Rescan { spent_only } => {
                self.invalidate();
                if spent_only {
                    client.empty_call("rescan_spent").await?;
                } else {
                    let _: Value = client
                        .transport
                        .call("rescan_blockchain", &json!({"full_rescan":true}))
                        .await?;
                }
                client.empty_call("store").await?;
                json!({})
            }
            WalletOperation::ExportKeyImages { filename, password } => {
                authenticate(client, &password).await?;
                let _: Value = client
                    .transport
                    .call("export_key_images", &json!({"filename":filename}))
                    .await?;
                json!({})
            }
            WalletOperation::ImportKeyImages { filename } => {
                self.invalidate();
                #[derive(Deserialize)]
                struct Imported {
                    height: u64,
                    spent: u64,
                    unspent: u64,
                }
                let result: Imported = client
                    .transport
                    .call("import_key_images", &json!({"filename":filename}))
                    .await?;
                client.empty_call("store").await?;
                json!({"height":result.height.to_string(),"spent":result.spent.to_string(),"unspent":result.unspent.to_string()})
            }
        };
        Ok(WalletOperationOutput::Json(value))
    }
}

#[derive(Deserialize)]
pub(crate) struct RawPrepared {
    tx_hash_list: Vec<String>,
    amount_list: Vec<u64>,
    fee_list: Vec<u64>,
    tx_metadata_list: Vec<Zeroizing<String>>,
    #[serde(default)]
    multisig_txset: String,
}
fn prepared(
    raw: RawPrepared,
    generation: &str,
    address: &str,
    payment_id: &str,
    requested: u64,
    sweep: bool,
) -> Result<PreparedSend, OperationError> {
    let count = raw.tx_hash_list.len();
    if count == 0
        || count > 64
        || raw.amount_list.len() != count
        || raw.fee_list.len() != count
        || raw.tx_metadata_list.len() != count
        || !raw.multisig_txset.is_empty()
        || raw
            .tx_hash_list
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != count
        || raw.tx_hash_list.iter().any(|h| !is_hash(h))
        || raw.tx_metadata_list.iter().any(|m| {
            m.is_empty() || m.len() > 1024 * 1024 || !m.bytes().all(|b| b.is_ascii_hexdigit())
        })
    {
        return Err(RpcError::InvalidResponse.into());
    }
    let amount = sum(&raw.amount_list)?;
    let fee = sum(&raw.fee_list)?;
    let total = amount.checked_add(fee).ok_or(OperationError::Invalid(
        "transaction total exceeds supported range",
    ))?;
    if amount == 0 || (!sweep && amount != requested) {
        return Err(RpcError::InvalidResponse.into());
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let summary = json!({"token":token,"address":address,"payment_id":payment_id,"amount":amount.to_string(),"fee":fee.to_string(),"total":total.to_string(),
        "transactions":raw.tx_hash_list,"expires_in_seconds":300});
    Ok(PreparedSend {
        token,
        generation: generation.into(),
        expires: Instant::now() + Duration::from_secs(300),
        hashes: raw.tx_hash_list,
        metadata: raw.tx_metadata_list,
        summary,
    })
}
fn sum(values: &[u64]) -> Result<u64, OperationError> {
    values.iter().try_fold(0_u64, |sum, v| {
        sum.checked_add(*v).ok_or(OperationError::Invalid(
            "transaction total exceeds supported range",
        ))
    })
}
fn parse_amount(value: &str, positive: bool) -> Result<u64, OperationError> {
    let amount = AtomicAmount::parse_ryo(if value.is_empty() && !positive {
        "0"
    } else {
        value
    })
    .map_err(|_| {
        OperationError::Invalid("enter an exact RYO amount with at most nine decimal places")
    })?
    .atomic();
    if positive && amount == 0 {
        return Err(OperationError::Invalid("amount must be greater than zero"));
    }
    Ok(amount)
}
fn text(value: &str, max: usize, multiline: bool) -> Result<(), OperationError> {
    if value.len() > max
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
    {
        return Err(OperationError::Invalid(
            "text is too long or contains invalid characters",
        ));
    }
    Ok(())
}
fn validate_payment_id(id: &str) -> Result<(), OperationError> {
    if !id.is_empty()
        && (![16, 64].contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(OperationError::Invalid(
            "payment ID must contain 16 or 64 hexadecimal characters",
        ));
    }
    Ok(())
}
fn clean_payment_id(id: &str) -> &str {
    if id.bytes().all(|b| b == b'0') {
        ""
    } else {
        id
    }
}
fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
async fn attribute(client: &WalletRpcClient, key: &str) -> Result<Option<String>, OperationError> {
    #[derive(Deserialize)]
    struct Attribute {
        value: String,
    }
    match client
        .transport
        .call::<_, Attribute>("get_attribute", &json!({"key":key}))
        .await
    {
        Ok(value) => Ok(if value.value.is_empty() {
            None
        } else {
            Some(value.value)
        }),
        Err(error) => Err(error.into()),
    }
}
async fn save_attribute(
    client: &WalletRpcClient,
    key: &str,
    value: &str,
) -> Result<(), OperationError> {
    let _: Value = client
        .transport
        .call("set_attribute", &json!({"key":key,"value":value}))
        .await?;
    client.empty_call("store").await?;
    Ok(())
}
async fn contacts(client: &WalletRpcClient) -> Result<Vec<Contact>, OperationError> {
    attribute(client, "next.contacts.v1")
        .await?
        .map(|s| serde_json::from_str(&s).map_err(|_| OperationError::Storage))
        .unwrap_or(Ok(Vec::new()))
}
#[derive(Serialize)]
struct PasswordChange<'a> {
    old_password: &'a str,
    new_password: &'a str,
}
async fn change_password(
    client: &WalletRpcClient,
    old: &str,
    new: &str,
) -> Result<(), OperationError> {
    let _: Value = client
        .transport
        .call(
            "change_wallet_password",
            &PasswordChange {
                old_password: old,
                new_password: new,
            },
        )
        .await
        .map_err(|e| {
            if matches!(e, RpcError::Remote(-22)) {
                OperationError::Invalid("wallet password was rejected")
            } else {
                e.into()
            }
        })?;
    Ok(())
}
async fn authenticate(client: &WalletRpcClient, password: &str) -> Result<(), OperationError> {
    if password.is_empty() || password.len() > 4096 {
        return Err(OperationError::Invalid("enter the wallet password"));
    }
    // Upstream exposes no verify_password RPC. Rewriting to the same password verifies it.
    change_password(client, password, password).await
}
async fn query_key(
    client: &WalletRpcClient,
    key_type: &str,
) -> Result<Zeroizing<String>, OperationError> {
    #[derive(Deserialize)]
    struct Key {
        key: Zeroizing<String>,
    }
    let key: Key = client
        .transport
        .call("query_key", &json!({"key_type":key_type}))
        .await?;
    if !is_hash(&key.key) {
        return Err(RpcError::InvalidResponse.into());
    }
    Ok(key.key)
}
async fn require_synced(client: &WalletRpcClient, node: &NodeConfig) -> Result<(), OperationError> {
    let health = DaemonRpcClient::configured(node)?.health().await?;
    if health.network != node.network
        || health.offline
        || !health.ready
        || health.height == 0
        || health.target_height > health.height
        || client.height().await? < health.height
    {
        return Err(OperationError::Invalid(
            "wait for both the node and wallet to finish syncing before sending",
        ));
    }
    Ok(())
}
fn journal_path(directory: &Path) -> PathBuf {
    directory.join("send-journal-v1.json")
}
fn read_journal(directory: &Path) -> Result<Vec<JournalEntry>, OperationError> {
    let path = journal_path(directory);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 2 * 1024 * 1024 => {
            let entries: Vec<JournalEntry> =
                serde_json::from_slice(&fs::read(path).map_err(|_| OperationError::Storage)?)
                    .map_err(|_| OperationError::Storage)?;
            if entries.iter().any(|e| {
                !is_hash(&e.txid)
                    || !["not_sent", "unknown", "relayed", "confirmed", "failed"]
                        .contains(&e.state.as_str())
            }) {
                return Err(OperationError::Storage);
            }
            Ok(entries)
        }
        _ => Err(OperationError::Storage),
    }
}
fn write_journal(directory: &Path, entries: &[JournalEntry]) -> Result<(), OperationError> {
    let path = journal_path(directory);
    let temp = directory.join(format!(
        "send-journal.{}.new",
        uuid::Uuid::new_v4().simple()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temp).map_err(|_| OperationError::Storage)?;
        file.write_all(&serde_json::to_vec(entries).map_err(|_| OperationError::Storage)?)
            .map_err(|_| OperationError::Storage)?;
        file.sync_all().map_err(|_| OperationError::Storage)?;
        fs::rename(&temp, path).map_err(|_| OperationError::Storage)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn mock_rpc(
        lose_second_relay: bool,
    ) -> (
        WalletRpcClient,
        NodeConfig,
        Arc<Mutex<Vec<Value>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let SocketAddr::V4(address) = listener.local_addr().unwrap() else {
            panic!("IPv4 required")
        };
        let calls = Arc::new(Mutex::new(Vec::new()));
        let capture = calls.clone();
        let handle = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let end = loop {
                    let mut chunk = [0_u8; 4096];
                    let count = stream.read(&mut chunk).await.unwrap();
                    if count == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length: usize = headers
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap();
                while bytes.len() < end + length {
                    let mut chunk = [0_u8; 4096];
                    let count = stream.read(&mut chunk).await.unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&chunk[..count]);
                }
                let request: Value = serde_json::from_slice(&bytes[end..end + length]).unwrap();
                capture.lock().unwrap().push(request.clone());
                let result = match request["method"].as_str().unwrap() {
                    "get_info" => {
                        json!({"height":100,"target_height":100,"mainnet":true,"testnet":false,"stagenet":false,"is_ready":true,"offline":false,"untrusted":false})
                    }
                    "get_height" => json!({"height":100}),
                    "get_balance" => {
                        json!({"balance":1000,"unlocked_balance":1000,"multisig_import_needed":false})
                    }
                    "parse_uri" => json!({"uri":{"address":"RYoLtest"}}),
                    "transfer_split" | "sweep_all" => {
                        assert_eq!(request["params"]["do_not_relay"], true);
                        assert_eq!(request["params"]["get_tx_metadata"], true);
                        assert_eq!(request["params"]["get_tx_keys"], false);
                        json!({"tx_hash_list":["a".repeat(64),"b".repeat(64)],"amount_list":[10,5],"fee_list":[2,3],"tx_metadata_list":["aa","bb"],"multisig_txset":""})
                    }
                    "relay_tx" => {
                        let n = capture
                            .lock()
                            .unwrap()
                            .iter()
                            .filter(|r| r["method"] == "relay_tx")
                            .count();
                        assert_eq!(request["params"]["hex"], if n == 1 { "aa" } else { "bb" });
                        if n == 2 && lose_second_relay {
                            continue;
                        }
                        json!({"tx_hash":if n==1 {"a".repeat(64)} else {"b".repeat(64)}})
                    }
                    "store" => json!({}),
                    _ => panic!("unexpected test method"),
                };
                let response =
                    json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).as_bytes()).await.unwrap();
            }
        });
        let client = WalletRpcClient::new(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, address.port()),
            crate::rpc::RpcCredentials::new("test".into(), "test".into()).unwrap(),
        )
        .unwrap();
        let mut node = NodeConfig::managed_local(crate::domain::Network::Mainnet);
        node.port = address.port();
        (client, node, calls, handle)
    }
    async fn prepare_test(
        state: &mut OperationsState,
        client: &WalletRpcClient,
        node: &NodeConfig,
        path: &Path,
    ) -> Value {
        match state
            .execute(
                client,
                node,
                path,
                "7",
                WalletOperation::PrepareSend {
                    address: "RYoLtest".into(),
                    amount: "0.000000015".into(),
                    sweep: false,
                    payment_id: String::new(),
                    priority: 0,
                    ring_size: 25,
                },
            )
            .await
            .unwrap()
        {
            WalletOperationOutput::Json(value) => value,
            _ => panic!("unexpected secrets"),
        }
    }
    #[tokio::test]
    async fn split_send_requires_confirmation_and_lost_reply_is_never_relayed_again() {
        let (client, node, calls, handle) = mock_rpc(true).await;
        let temp = tempfile::tempdir().unwrap();
        let mut state = OperationsState::default();
        let preview = prepare_test(&mut state, &client, &node, temp.path()).await;
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "relay_tx")
        );
        let token = preview["token"].as_str().unwrap().to_owned();
        let output = state
            .execute(
                &client,
                &node,
                temp.path(),
                "7",
                WalletOperation::ConfirmSend {
                    token: token.clone(),
                },
            )
            .await
            .unwrap();
        let WalletOperationOutput::Json(output) = output else {
            panic!("unexpected secrets")
        };
        assert_eq!(output["transactions"][0]["state"], "relayed");
        assert_eq!(output["transactions"][1]["state"], "unknown");
        assert!(
            state
                .execute(
                    &client,
                    &node,
                    temp.path(),
                    "7",
                    WalletOperation::ConfirmSend { token }
                )
                .await
                .is_err()
        );
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r["method"] == "relay_tx")
                .count(),
            2
        );
        assert!(
            state
                .execute(
                    &client,
                    &node,
                    temp.path(),
                    "7",
                    WalletOperation::PrepareSend {
                        address: "RYoLtest".into(),
                        amount: "1".into(),
                        sweep: false,
                        payment_id: String::new(),
                        priority: 0,
                        ring_size: 25
                    }
                )
                .await
                .is_err()
        );
        assert_eq!(read_journal(temp.path()).unwrap()[1].state, "unknown");
        handle.abort();
    }
    #[tokio::test]
    async fn cancelled_expired_and_changed_session_drafts_cannot_broadcast() {
        let (client, node, calls, handle) = mock_rpc(false).await;
        let temp = tempfile::tempdir().unwrap();
        let mut state = OperationsState::default();
        let preview = prepare_test(&mut state, &client, &node, temp.path()).await;
        let token = preview["token"].as_str().unwrap().to_owned();
        state
            .execute(
                &client,
                &node,
                temp.path(),
                "7",
                WalletOperation::CancelSend {
                    token: token.clone(),
                },
            )
            .await
            .unwrap();
        assert!(
            state
                .execute(
                    &client,
                    &node,
                    temp.path(),
                    "7",
                    WalletOperation::ConfirmSend { token }
                )
                .await
                .is_err()
        );
        let preview = prepare_test(&mut state, &client, &node, temp.path()).await;
        let token = preview["token"].as_str().unwrap().to_owned();
        state.draft.as_mut().unwrap().expires = Instant::now();
        assert!(
            state
                .execute(
                    &client,
                    &node,
                    temp.path(),
                    "7",
                    WalletOperation::ConfirmSend { token }
                )
                .await
                .is_err()
        );
        let preview = prepare_test(&mut state, &client, &node, temp.path()).await;
        let token = preview["token"].as_str().unwrap().to_owned();
        assert!(
            state
                .execute(
                    &client,
                    &node,
                    temp.path(),
                    "8",
                    WalletOperation::ConfirmSend { token }
                )
                .await
                .is_err()
        );
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "relay_tx")
        );
        handle.abort();
    }
    fn raw() -> RawPrepared {
        RawPrepared {
            tx_hash_list: vec!["a".repeat(64), "b".repeat(64)],
            amount_list: vec![9007199254740993, 1],
            fee_list: vec![100, 101],
            tx_metadata_list: vec![Zeroizing::new("aa".into()), Zeroizing::new("bb".into())],
            multisig_txset: String::new(),
        }
    }
    #[test]
    fn preview_is_exact_and_metadata_is_never_serialized() {
        let d = prepared(raw(), "7", "address", "", 9007199254740994, false).unwrap();
        assert_eq!(d.summary["amount"], "9007199254740994");
        assert_eq!(d.summary["fee"], "201");
        assert_eq!(d.summary["total"], "9007199254741195");
        assert!(d.summary.get("metadata").is_none());
        assert!(
            !serde_json::to_string(&d.summary)
                .unwrap()
                .contains("tx_metadata")
        );
    }
    #[test]
    fn mismatched_previews_and_overflow_are_rejected() {
        let mut r = raw();
        r.fee_list.pop();
        assert!(prepared(r, "1", "a", "", 1, false).is_err());
        assert!(prepared(raw(), "1", "a", "", 1, false).is_err());
        assert!(sum(&[u64::MAX, 1]).is_err());
        assert!(parse_amount("1.0000000001", true).is_err());
    }
    #[test]
    fn uncertain_send_journal_survives_restart_without_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let entry = JournalEntry {
            txid: "a".repeat(64),
            state: "unknown".into(),
        };
        write_journal(temp.path(), &[entry]).unwrap();
        let stored = read_journal(temp.path()).unwrap();
        assert_eq!(stored[0].state, "unknown");
        assert!(
            !fs::read_to_string(journal_path(temp.path()))
                .unwrap()
                .contains("metadata")
        );
    }
}
