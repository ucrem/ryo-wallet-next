//! Opt-in read-only mainnet scan of a newly generated, disposable wallet.
//! No user wallets, signing, sends or funds. Public availability is not a CI gate.
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::time::Duration;

use ryo_wallet_service::application::WalletService;
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::process::{BinaryDigest, BinaryKind, VerifiedBinary};
use ryo_wallet_service::storage::{AppPaths, WalletId};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const PASSWORD: &str = "disposable-public-scan-cache-test";

fn port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn binary() -> VerifiedBinary {
    let target = if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../src-tauri/runtime-manifest.json")).unwrap();
    VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &PathBuf::from(std::env::var("RYO_TEST_WALLET_RPC").expect("set reviewed wallet RPC path")),
        BinaryDigest::parse_hex(
            manifest["platforms"][target]["binarySha256"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "contacts official mainnet RPC with a freshly generated disposable wallet; read-only"]
async fn genuine_public_scan_resumes_from_the_saved_cache() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let remote = NodeConfig::remote(
        Network::Mainnet,
        "wallet-node.ryo-currency.com".into(),
        12211,
    )?;
    let source = WalletService::new();
    source
        .start_wallet_rpc(
            binary(),
            AppPaths::new(temporary.path().join("source"), Network::Mainnet)?,
            remote.clone(),
            port(),
        )
        .await?;
    let created = source
        .create_wallet(
            WalletId::parse(uuid::Uuid::new_v4().simple().to_string())?,
            Zeroizing::new(PASSWORD.into()),
            "English".into(),
        )
        .await?;
    let seed = created.into_recovery_phrase().into_secret();
    source.lock(Duration::from_secs(5)).await?;

    let paths = AppPaths::new(temporary.path().join("scan"), Network::Mainnet)?;
    let id = WalletId::parse(uuid::Uuid::new_v4().simple().to_string())?;
    let wallet = WalletService::new();
    wallet
        .start_wallet_rpc(binary(), paths.clone(), remote, port())
        .await?;
    wallet
        .restore_wallet(id.clone(), Zeroizing::new(PASSWORD.into()), seed, 1)
        .await?;
    let until = tokio::time::Instant::now() + Duration::from_secs(90);
    while wallet.scan_height().unwrap_or(0) < 1_000 {
        if tokio::time::Instant::now() >= until {
            return Err("public node did not provide genuine scan progress".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let processed = wallet.scan_height().ok_or("missing processed height")?;
    assert!(
        wallet.height().await.is_err(),
        "must observe a busy long scan"
    );
    let started = tokio::time::Instant::now();
    wallet.lock(Duration::from_secs(5)).await?;
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "graceful checkpoint exceeded budget"
    );
    wallet
        .start_wallet_rpc(
            binary(),
            paths,
            NodeConfig::remote(Network::Mainnet, "127.0.0.1".into(), port())?,
            port(),
        )
        .await?;
    wallet
        .open_imported_wallet(id, Zeroizing::new(PASSWORD.into()))
        .await?;
    let persisted = wallet.height().await?;
    assert!(persisted >= processed, "reopen lost processed blocks");
    wallet.lock(Duration::from_secs(5)).await?;
    println!(
        "Read-only public mainnet scan: observed {processed}; persisted {persisted}; reopened without a reachable daemon."
    );
    Ok(())
}
