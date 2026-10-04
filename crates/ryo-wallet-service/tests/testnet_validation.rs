//! Opt-in, genuine two-node testnet fixture using the unchanged reviewed runtimes.
//! This short chain exercises the genesis-era fork, NOT current-fork compatibility.
//! No public peers, mainnet wallets, faucet, modified core or mocked signing.
use std::ffi::OsString;
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use ryo_wallet_service::application::{WalletOperation, WalletOperationOutput, WalletService};
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::process::{BinaryDigest, BinaryKind, VerifiedBinary};
use ryo_wallet_service::storage::{AppPaths, WalletId};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use zeroize::Zeroizing;
#[path = "support/lossy_daemon.rs"]
mod lossy_daemon;
use lossy_daemon::LossyDaemon;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const PASSWORD: &str = "disposable-isolated-testnet-password";

fn reviewed_binary(kind: BinaryKind) -> VerifiedBinary {
    let (variable, key) = match kind {
        BinaryKind::Daemon => ("RYO_TEST_DAEMON", "daemonSha256"),
        BinaryKind::WalletRpc => ("RYO_TEST_WALLET_RPC", "binarySha256"),
    };
    let target = if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let manifest: Value =
        serde_json::from_str(include_str!("../../../src-tauri/runtime-manifest.json")).unwrap();
    VerifiedBinary::verify(
        kind,
        &PathBuf::from(std::env::var(variable).expect("set reviewed runtime paths")),
        BinaryDigest::parse_hex(manifest["platforms"][target][key].as_str().unwrap()).unwrap(),
    )
    .unwrap()
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct TestNode {
    child: Child,
    client: Client,
    port: u16,
}

impl TestNode {
    async fn start(directory: &Path, rpc: u16, p2p: u16, peer: u16) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        let config = directory.join("empty.conf");
        std::fs::write(&config, "")?;
        let args: Vec<OsString> = vec![
            "--testnet".into(),
            "--config-file".into(),
            config.into_os_string(),
            "--data-dir".into(),
            directory.as_os_str().into(),
            "--rpc-bind-ip".into(),
            "127.0.0.1".into(),
            "--rpc-bind-port".into(),
            rpc.to_string().into(),
            "--p2p-bind-ip".into(),
            "127.0.0.1".into(),
            "--p2p-bind-port".into(),
            p2p.to_string().into(),
            "--zmq-rpc-bind-ip".into(),
            "127.0.0.1".into(),
            "--zmq-rpc-bind-port".into(),
            free_port().to_string().into(),
            "--add-exclusive-node".into(),
            format!("127.0.0.1:{peer}").into(),
            "--allow-local-ip".into(),
            "--hide-my-port".into(),
            "--no-igd".into(),
            "--disable-dns-checkpoints".into(),
            "--check-updates".into(),
            "disabled".into(),
            "--log-level".into(),
            "0".into(),
            "--log-file".into(),
            directory.join("daemon.log").into_os_string(),
        ];
        let binary = reviewed_binary(BinaryKind::Daemon);
        let mut command = Command::new(binary.path());
        command
            .args(args)
            .current_dir(directory)
            .env_clear()
            .env("LANG", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            command.creation_flags(0x08000000);
            if let Some(system_root) = std::env::var_os("SystemRoot") {
                command.env("SystemRoot", system_root);
            }
        }
        let mut node = Self {
            child: command.spawn()?,
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()?,
            port: rpc,
        };
        for _ in 0..100 {
            if node.child.try_wait()?.is_some() {
                return Err("isolated daemon exited".into());
            }
            if let Ok(info) = node.rpc("get_info", json!({})).await {
                if info["testnet"] != true || info["mainnet"] != false || info["stagenet"] != false
                {
                    return Err("wrong daemon network".into());
                }
                return Ok(node);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err("isolated daemon readiness timed out".into())
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let body: Value = self
            .client
            .post(format!("http://127.0.0.1:{}/json_rpc", self.port))
            .json(&json!({"jsonrpc":"2.0", "id":"test", "method":method, "params":params}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(error) = body.get("error") {
            return Err(format!("daemon {method} error code {}", error["code"]).into());
        }
        body.get("result")
            .cloned()
            .ok_or_else(|| "missing daemon result".into())
    }

    async fn ready(&self) -> Result<()> {
        for _ in 0..120 {
            let info = self.rpc("get_info", json!({})).await?;
            if info["is_ready"] == true {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Err("isolated peers did not complete their handshake".into())
    }

    async fn match_tip(&self, source: &Self) -> Result<()> {
        let expected = source.rpc("get_info", json!({})).await?;
        for _ in 0..100 {
            let actual = self.rpc("get_info", json!({})).await?;
            if actual["height"] == expected["height"]
                && actual["top_block_hash"] == expected["top_block_hash"]
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err("isolated peer did not sync the actual chain tip".into())
    }

    async fn mine(&self, address: &str, count: u64) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        for _ in 0..count {
            let template = self
                .rpc(
                    "get_block_template",
                    json!({"wallet_address":address,"reserve_size":0}),
                )
                .await?;
            let height = template["height"]
                .as_u64()
                .ok_or("missing template height")?;
            // Fail closed if this recipe reaches a fork or needs actual nonce search.
            if template["difficulty"] != 1 || height >= 1000 {
                return Err("fixture left its reviewed difficulty-one profile".into());
            }
            let hex = template["blocktemplate_blob"]
                .as_str()
                .ok_or("missing block template")?;
            let mut blob = decode_hex(hex)?;
            let mut offset = 0;
            if read_varint(&blob, &mut offset)? != 1 {
                return Err("fixture requires genesis-era testnet fork".into());
            }
            read_varint(&blob, &mut offset)?;
            let timestamp_start = offset;
            read_varint(&blob, &mut offset)?;
            // Legitimate past timestamps at the consensus target keep real difficulty at one.
            // Every submitted block still passes the unchanged daemon's full validation.
            let timestamp = now - 7 * 86400 + height * 240;
            blob.splice(timestamp_start..offset, varint(timestamp));
            let submitted = self.rpc("submit_block", json!([encode_hex(&blob)])).await?;
            if submitted["status"] != "OK" {
                return Err("daemon rejected fixture block".into());
            }
            let info = self.rpc("get_info", json!({})).await?;
            if info["height"] != height + 1 {
                return Err("fixture block did not advance the real chain".into());
            }
        }
        Ok(())
    }

    async fn pool_count(&self) -> Result<u64> {
        let body: Value = self
            .client
            .post(format!(
                "http://127.0.0.1:{}/get_transaction_pool_stats",
                self.port
            ))
            .json(&json!({}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        body["pool_stats"]["txs_total"]
            .as_u64()
            .ok_or_else(|| "missing real pool count".into())
    }

    async fn transactions(&self, hashes: &[String]) -> Result<Vec<Value>> {
        let body: Value = self
            .client
            .post(format!("http://127.0.0.1:{}/get_transactions", self.port))
            .json(&json!({"txs_hashes":hashes,"decode_as_json":false,"prune":false}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        body["txs"]
            .as_array()
            .cloned()
            .ok_or_else(|| "missing daemon transactions".into())
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(mut stdin) = self.child.stdin.take() {
            stdin.write_all(b"exit\n").await?;
        }
        let status = match tokio::time::timeout(Duration::from_secs(15), self.child.wait()).await {
            Ok(status) => status?,
            Err(_) => {
                self.child.kill().await?;
                return Err("isolated daemon did not stop gracefully".into());
            }
        };
        if !status.success() {
            return Err("isolated daemon exited unsuccessfully".into());
        }
        Ok(())
    }
}

fn read_varint(bytes: &[u8], offset: &mut usize) -> Result<u64> {
    let mut result = 0_u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*offset).ok_or("truncated varint")?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err("varint overflow".into());
        }
        result |= u64::from(byte & 127) << shift;
        if byte < 128 {
            return Ok(result);
        }
    }
    Err("invalid varint".into())
}
fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while value >= 128 {
        bytes.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}
fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    if !hex.is_ascii() || !hex.len().is_multiple_of(2) {
        return Err("invalid block hex".into());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(Into::into))
        .collect()
}
fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn operation(wallet: &WalletService, op: WalletOperation) -> Result<Value> {
    let generation = wallet.status().await?.session_generation;
    match wallet.operation(generation, op).await? {
        WalletOperationOutput::Json(value) => Ok(value),
        WalletOperationOutput::Secrets(_) => Err("unexpected secret output".into()),
    }
}
async fn scan(wallet: &WalletService) -> Result<()> {
    operation(wallet, WalletOperation::Rescan { spent_only: false }).await?;
    // Observe genuine processed-block stdout from the unchanged reviewed runtime.
    let rpc_height = wallet.height().await?;
    let until = tokio::time::Instant::now() + Duration::from_secs(2);
    while wallet.scan_height() != Some(rpc_height) && tokio::time::Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let processed = wallet.scan_height().ok_or("wallet scan progress missing")?;
    if processed != rpc_height {
        return Err("processed scan height differs from RPC chain length".into());
    }
    Ok(())
}
async fn new_wallet(
    paths: AppPaths,
    port: u16,
) -> Result<(WalletService, WalletId, Zeroizing<String>)> {
    let wallet = WalletService::new();
    wallet
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            paths,
            NodeConfig::remote(Network::Testnet, "127.0.0.1".into(), port)?,
            free_port(),
        )
        .await?;
    let id = WalletId::parse(uuid::Uuid::new_v4().simple().to_string())?;
    let created = wallet
        .create_wallet(
            id.clone(),
            Zeroizing::new(PASSWORD.into()),
            "English".into(),
        )
        .await?;
    Ok((wallet, id, created.into_recovery_phrase().into_secret()))
}

#[tokio::test]
#[ignore = "requires verified runtimes; mines genuine genesis-era testnet blocks in two isolated nodes"]
async fn isolated_testnet_funded_wallet_operations() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let [rpc_a, rpc_b, p2p_a, p2p_b] = [free_port(), free_port(), free_port(), free_port()];
    let mut node_a = TestNode::start(&temporary.path().join("node-a"), rpc_a, p2p_a, p2p_b).await?;
    let mut node_b = TestNode::start(&temporary.path().join("node-b"), rpc_b, p2p_b, p2p_a).await?;
    node_a.ready().await?;
    node_b.ready().await?;
    println!(
        "Two isolated reviewed testnet nodes are connected; current-fork gate remains separate."
    );
    let paths_a = AppPaths::new(temporary.path().join("sender"), Network::Testnet)?;
    let paths_b = AppPaths::new(temporary.path().join("recipient"), Network::Testnet)?;
    let proxy = LossyDaemon::start(rpc_a).await?;
    let wallet_node_port = proxy.port;
    let (mut sender, sender_id, seed) = new_wallet(paths_a.clone(), wallet_node_port).await?;
    let (recipient, _, _) = new_wallet(paths_b, rpc_b).await?;
    let sender_address = sender.overview().await?.primary_address;
    let recipient_address = recipient.overview().await?.primary_address;
    let receive_address = recipient
        .create_receive_address(recipient.status().await?.session_generation)
        .await?;
    operation(
        &recipient,
        WalletOperation::LabelAddress {
            index: receive_address.address_index,
            label: "Testnet receive".into(),
        },
    )
    .await?;
    node_a.mine(&sender_address, 64).await?;
    node_a.mine(&recipient_address, 80).await?;
    node_b.match_tip(&node_a).await?;
    scan(&sender).await?;
    scan(&recipient).await?;
    let before = sender.overview().await?;
    if before.unlocked.atomic.parse::<u64>()? == 0 {
        return Err("fixture did not produce mature spendable outputs".into());
    }
    println!(
        "144 validated blocks; sender has mature outputs. Preparing an actual signed transfer."
    );
    let prepare = || WalletOperation::PrepareSend {
        address: receive_address.address.clone(),
        amount: "1.000000001".into(),
        sweep: false,
        payment_id: String::new(),
        priority: 1,
        ring_size: 25,
    };
    let draft = operation(&sender, prepare()).await?;
    assert_eq!(draft["amount"], "1000000001");
    assert!(
        draft["fee"]
            .as_str()
            .ok_or("missing actual fee")?
            .parse::<u64>()?
            > 0
    );
    assert!(
        node_a.pool_count().await? == 0,
        "preparation relayed before confirmation"
    );
    let token = draft["token"]
        .as_str()
        .ok_or("missing draft token")?
        .to_owned();
    operation(
        &sender,
        WalletOperation::CancelSend {
            token: token.clone(),
        },
    )
    .await?;
    assert!(
        operation(&sender, WalletOperation::ConfirmSend { token })
            .await
            .is_err()
    );
    let draft = operation(&sender, prepare()).await?;
    let token = draft["token"]
        .as_str()
        .ok_or("missing draft token")?
        .to_owned();
    let hashes: Vec<String> = serde_json::from_value(draft["transactions"].clone())?;
    let sent = operation(
        &sender,
        WalletOperation::ConfirmSend {
            token: token.clone(),
        },
    )
    .await?;
    assert!(
        sent["transactions"]
            .as_array()
            .ok_or("missing outcomes")?
            .iter()
            .all(|row| row["state"] == "relayed")
    );
    assert!(
        operation(&sender, WalletOperation::ConfirmSend { token })
            .await
            .is_err()
    );
    let pool = node_a.transactions(&hashes).await?;
    assert!(
        hashes.iter().all(|hash| pool
            .iter()
            .any(|row| row["tx_hash"] == *hash && row["in_pool"] == true)),
        "confirmed transactions absent from real pool"
    );
    node_a.mine(&recipient_address, 12).await?;
    node_b.match_tip(&node_a).await?;
    scan(&sender).await?;
    scan(&recipient).await?;
    let history = operation(&sender, WalletOperation::History).await?;
    assert!(
        hashes.iter().all(|hash| history
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["txid"] == *hash)),
        "real outgoing history missing"
    );
    let incoming = operation(&recipient, WalletOperation::History).await?;
    assert!(
        hashes.iter().all(|hash| incoming
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["txid"] == *hash
                && row["type"] == "in"
                && row["amount"] == "1000000001")),
        "recipient did not recover the exact transferred amount"
    );
    operation(
        &sender,
        WalletOperation::SetNote {
            txid: hashes[0].clone(),
            note: "Validated testnet send".into(),
        },
    )
    .await?;
    let uncertain = operation(&sender, prepare()).await?;
    let uncertain_hashes: Vec<String> = serde_json::from_value(uncertain["transactions"].clone())?;
    proxy.lose_next_send_reply();
    let outcome = operation(
        &sender,
        WalletOperation::ConfirmSend {
            token: uncertain["token"].as_str().unwrap().into(),
        },
    )
    .await?;
    assert_eq!(
        proxy
            .accepted_dropped
            .load(std::sync::atomic::Ordering::Acquire),
        1,
        "fault was not applied after real acceptance"
    );
    assert_eq!(outcome["transactions"][0]["state"], "unknown");
    let accepted = node_a.transactions(&uncertain_hashes).await?;
    assert!(uncertain_hashes.iter().all(|hash| {
        accepted
            .iter()
            .any(|row| row["tx_hash"] == *hash && row["in_pool"] == true)
    }));
    assert!(
        operation(&sender, prepare()).await.is_err(),
        "uncertain send allowed an automatic replacement"
    );
    sender.lock(Duration::from_secs(5)).await?;
    let journal: Value = serde_json::from_slice(&std::fs::read(
        paths_a.wallet_dir(&sender_id).join("send-journal-v1.json"),
    )?)?;
    assert!(
        journal
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["txid"] == uncertain_hashes[0] && row["state"] == "unknown")
    );
    sender = WalletService::new();
    sender
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            paths_a.clone(),
            NodeConfig::remote(Network::Testnet, "127.0.0.1".into(), wallet_node_port)?,
            free_port(),
        )
        .await?;
    sender
        .open_imported_wallet(sender_id.clone(), Zeroizing::new(PASSWORD.into()))
        .await?;
    node_a.mine(&recipient_address, 12).await?;
    node_b.match_tip(&node_a).await?;
    scan(&sender).await?;
    let reconciled = operation(&sender, WalletOperation::SendJournal).await?;
    assert!(
        uncertain_hashes.iter().all(|hash| reconciled
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["txid"] == *hash && row["state"] == "confirmed")),
        "actual accepted send did not reconcile after scan"
    );
    let address_before = sender.overview().await?.primary_address;
    let balance_before_restore = sender.overview().await?.total;
    sender.lock(Duration::from_secs(5)).await?;
    assert!(sender.is_idle().await?);
    let locked_height = node_a.rpc("get_info", json!({})).await?["height"]
        .as_u64()
        .unwrap();
    node_a.mine(&recipient_address, 2).await?;
    assert_eq!(
        node_a.rpc("get_info", json!({})).await?["height"],
        locked_height + 2
    );
    sender
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            paths_a.clone(),
            NodeConfig::remote(Network::Testnet, "127.0.0.1".into(), wallet_node_port)?,
            free_port(),
        )
        .await?;
    sender
        .open_imported_wallet(sender_id.clone(), Zeroizing::new(PASSWORD.into()))
        .await?;
    scan(&sender).await?;
    let restored_paths = AppPaths::new(temporary.path().join("restored"), Network::Testnet)?;
    let restored = WalletService::new();
    restored
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            restored_paths,
            NodeConfig::remote(Network::Testnet, "127.0.0.1".into(), rpc_a)?,
            free_port(),
        )
        .await?;
    restored
        .restore_wallet(
            WalletId::parse(uuid::Uuid::new_v4().simple().to_string())?,
            Zeroizing::new(PASSWORD.into()),
            seed.clone(),
            0,
        )
        .await?;
    scan(&restored).await?;
    let restored_overview = restored.overview().await?;
    assert_eq!(restored_overview.primary_address, address_before);
    assert_eq!(restored_overview.total, balance_before_restore);
    let images = temporary.path().join("encrypted-key-images");
    operation(
        &sender,
        WalletOperation::ExportKeyImages {
            filename: images.to_str().unwrap().into(),
            password: Zeroizing::new(PASSWORD.into()),
        },
    )
    .await?;
    assert!(std::fs::metadata(&images)?.len() > 0);
    operation(
        &restored,
        WalletOperation::ImportKeyImages {
            filename: images.to_str().unwrap().into(),
        },
    )
    .await?;
    operation(&sender, WalletOperation::Rescan { spent_only: true }).await?;
    let split = operation(
        &sender,
        WalletOperation::PrepareSend {
            address: receive_address.address.clone(),
            amount: String::new(),
            sweep: true,
            payment_id: String::new(),
            priority: 1,
            ring_size: 100,
        },
    )
    .await?;
    let split_hashes: Vec<String> = serde_json::from_value(split["transactions"].clone())?;
    println!(
        "Split sweep prepared {} real transactions.",
        split_hashes.len()
    );
    assert!(
        split_hashes.len() >= 3,
        "fixture did not force a real split send"
    );
    assert_eq!(node_a.pool_count().await?, 0);
    proxy.lose_send_reply_after(1);
    let split_token = split["token"].as_str().unwrap().to_owned();
    let split_result = operation(
        &sender,
        WalletOperation::ConfirmSend {
            token: split_token.clone(),
        },
    )
    .await?;
    let outcomes = split_result["transactions"].as_array().unwrap();
    assert_eq!(outcomes.len(), split_hashes.len());
    assert_eq!(outcomes[0]["state"], "relayed");
    assert_eq!(outcomes[1]["state"], "unknown");
    assert!(outcomes[2..].iter().all(|row| row["state"] == "not_sent"));
    assert_eq!(
        proxy
            .accepted_dropped
            .load(std::sync::atomic::Ordering::Acquire),
        2,
        "second lost reply must follow genuine acceptance; observations: {:?}",
        proxy
            .send_observations()
            .iter()
            .map(|row| (row.http, row.accepted, &row.rejection_flags))
            .collect::<Vec<_>>()
    );
    let accepted = node_a.transactions(&split_hashes).await?;
    assert!(split_hashes[..2].iter().all(|hash| {
        accepted
            .iter()
            .any(|row| row["tx_hash"] == *hash && row["in_pool"] == true)
    }));
    assert!(
        split_hashes[2..]
            .iter()
            .all(|hash| { !accepted.iter().any(|row| row["tx_hash"] == *hash) })
    );
    assert!(operation(&sender, prepare()).await.is_err());
    assert!(
        operation(&sender, WalletOperation::ConfirmSend { token: split_token })
            .await
            .is_err()
    );
    sender.lock(Duration::from_secs(5)).await?;
    sender = WalletService::new();
    sender
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            paths_a.clone(),
            NodeConfig::remote(Network::Testnet, "127.0.0.1".into(), wallet_node_port)?,
            free_port(),
        )
        .await?;
    sender
        .open_imported_wallet(sender_id, Zeroizing::new(PASSWORD.into()))
        .await?;
    node_a.mine(&recipient_address, 12).await?;
    node_b.match_tip(&node_a).await?;
    scan(&sender).await?;
    let reconciled = operation(&sender, WalletOperation::SendJournal).await?;
    for (index, hash) in split_hashes.iter().enumerate() {
        let row = reconciled
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["txid"] == *hash)
            .unwrap();
        let expected = match index {
            0 => "relayed",
            1 => "confirmed",
            _ => "not_sent",
        };
        assert_eq!(row["state"], expected);
    }
    assert_eq!(
        node_a.pool_count().await?,
        0,
        "reopen relayed an unsent split transaction"
    );
    let after_restart = node_a.transactions(&split_hashes).await?;
    assert!(split_hashes[..2].iter().all(|hash| {
        after_restart
            .iter()
            .any(|row| row["tx_hash"] == *hash && row["in_pool"] == false)
    }));
    assert!(
        split_hashes[2..]
            .iter()
            .all(|hash| { !after_restart.iter().any(|row| row["tx_hash"] == *hash) }),
        "restart sent a previously unsubmitted split transaction"
    );
    assert!(sender.overview().await?.unlocked.atomic.parse::<u64>()? > 0);
    let sweep_balance = sender.overview().await?.unlocked.atomic.parse::<u64>()?;
    let sweep = operation(
        &sender,
        WalletOperation::PrepareSend {
            address: receive_address.address,
            amount: String::new(),
            sweep: true,
            payment_id: String::new(),
            priority: 1,
            ring_size: 25,
        },
    )
    .await?;
    assert_eq!(
        sweep["total"].as_str().unwrap().parse::<u64>()?,
        sweep_balance
    );
    assert_eq!(
        node_a.pool_count().await?,
        0,
        "sweep relayed during preparation"
    );
    let sweep_hashes: Vec<String> = serde_json::from_value(sweep["transactions"].clone())?;
    println!("Sweep prepared {} real transactions.", sweep_hashes.len());
    let sweep_result = operation(
        &sender,
        WalletOperation::ConfirmSend {
            token: sweep["token"].as_str().unwrap().into(),
        },
    )
    .await?;
    assert!(
        sweep_result["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["state"] == "relayed")
    );
    node_a.mine(&recipient_address, 12).await?;
    node_b.match_tip(&node_a).await?;
    scan(&sender).await?;
    scan(&recipient).await?;
    assert_eq!(sender.overview().await?.total.atomic, "0");
    let incoming = operation(&recipient, WalletOperation::History).await?;
    assert!(sweep_hashes.iter().all(|hash| {
        incoming
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["txid"] == *hash && row["type"] == "in")
    }));
    println!(
        "Actual fee, no early relay, cancel, one-shot relay, subaddress receive, history, lost reply after real acceptance, fresh-owner journal reconciliation, real partial split send, funded seed recovery, key images and sweep passed."
    );
    restored.lock(Duration::from_secs(5)).await?;
    sender.lock(Duration::from_secs(5)).await?;
    recipient.lock(Duration::from_secs(5)).await?;
    node_a.stop().await?;
    let saved = node_b.rpc("get_info", json!({})).await?;
    let mut restarted =
        TestNode::start(&temporary.path().join("node-a"), rpc_a, p2p_a, p2p_b).await?;
    restarted.ready().await?;
    let resumed = restarted.rpc("get_info", json!({})).await?;
    assert_eq!(resumed["height"], saved["height"]);
    assert_eq!(resumed["top_block_hash"], saved["top_block_hash"]);
    restarted.stop().await?;
    node_b.stop().await?;
    check_secret_canaries(temporary.path(), &[PASSWORD, seed.as_str()])?;
    if let Some(path) = std::env::var_os("RYO_VALIDATION_REPORT") {
        use std::io::Write;
        let report = json!({"profile":"isolated-testnet-genesis-v1", "current_fork_gate_passed":false,
            "platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"runtime":"0.6.1.0",
            "finished_at_unix":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),"height":resumed["height"],
            "checks":["two real exclusive peers and chain propagation","mature mined outputs","actual fee and exact nine-decimal amount",
                "cancel and no relay before confirmation","one-shot relay and pool acceptance","subaddress receipt and confirmed history",
                "reply lost after daemon acceptance","persisted unknown journal and fresh-owner reconciliation",
                "wallet lock leaves nodes active","funded seed recovery","rescan and encrypted key images","real partial split send and no automatic unsent relay after restart","sweep and zero sender balance","persisted chain restart","no test password or recovery phrase in raw logs"]});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        serde_json::to_writer_pretty(&mut file, &report)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    Ok(())
}

fn check_secret_canaries(directory: &Path, secrets: &[&str]) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("unexpected symlink in isolated fixture".into());
        }
        if kind.is_dir() {
            check_secret_canaries(&entry.path(), secrets)?;
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "log")
        {
            let bytes = std::fs::read(entry.path())?;
            if secrets.iter().any(|secret| {
                !secret.is_empty()
                    && bytes
                        .windows(secret.len())
                        .any(|slice| slice == secret.as_bytes())
            }) {
                return Err("secret canary appeared in upstream log".into());
            }
        }
    }
    Ok(())
}
