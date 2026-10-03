//! Opt-in test against reviewed upstream binaries and a disposable mainnet chain.
use ryo_wallet_service::application::{NodeService, NodeState, WalletService};
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::process::{BinaryDigest, BinaryKind, VerifiedBinary};
use ryo_wallet_service::storage::{AppPaths, WalletId};
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::time::Duration;
use zeroize::Zeroizing;

fn reviewed_binary(kind: BinaryKind) -> VerifiedBinary {
    let (variable, key) = match kind {
        BinaryKind::Daemon => ("RYO_TEST_DAEMON", "daemonSha256"),
        BinaryKind::WalletRpc => ("RYO_TEST_WALLET_RPC", "binarySha256"),
    };
    let path =
        PathBuf::from(std::env::var(variable).expect("set the reviewed test executable path"));
    let target = if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../src-tauri/runtime-manifest.json")).unwrap();
    let digest =
        BinaryDigest::parse_hex(manifest["platforms"][target][key].as_str().unwrap()).unwrap();
    VerifiedBinary::verify(kind, &path, digest).unwrap()
}

#[tokio::test]
#[ignore = "starts reviewed Ryo binaries and downloads blocks to disposable storage"]
async fn local_node_keeps_syncing_after_wallet_lock_and_can_restart() {
    let temporary = tempfile::tempdir().unwrap();
    let test_root = std::env::var_os("RYO_TEST_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| temporary.path().to_owned());
    std::fs::create_dir_all(&test_root).unwrap();
    // Native folder selection uses canonical paths (verbatim prefixes on Windows).
    let paths = AppPaths::new(std::fs::canonicalize(test_root).unwrap(), Network::Mainnet).unwrap();
    let node = NodeConfig::managed_local(Network::Mainnet);
    let nodes = NodeService::new();
    let wallets = WalletService::new();
    let endpoint = nodes
        .start(
            reviewed_binary(BinaryKind::Daemon),
            paths.clone(),
            node.clone(),
        )
        .await
        .unwrap();
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    wallets
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            paths.clone(),
            endpoint.clone(),
            port,
        )
        .await
        .unwrap();
    let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    let password = "disposable-node-lock-test-password";
    let created = wallets
        .create_wallet(
            id.clone(),
            Zeroizing::new(password.to_owned()),
            "English".to_owned(),
        )
        .await
        .unwrap();
    drop(created);
    let before = nodes.status(&node).await.unwrap();
    wallets.lock(Duration::from_secs(5)).await.unwrap();
    assert!(wallets.is_idle().await.unwrap());
    assert!(!nodes.is_idle().await.unwrap());
    let locked = nodes.status(&node).await.unwrap();
    assert_eq!(locked.state, NodeState::Running);
    assert_eq!(locked.generation, before.generation);
    assert!(locked.reachable);
    // Assert actual chain progress while the key-bearing process is absent.
    let initial: u64 = locked.height.as_ref().unwrap().parse().unwrap();
    let mut advanced = false;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let status = nodes.status(&node).await.unwrap();
        if status
            .height
            .as_ref()
            .and_then(|height| height.parse::<u64>().ok())
            .is_some_and(|height| height > initial)
        {
            advanced = true;
            break;
        }
    }
    let current = nodes
        .start(
            reviewed_binary(BinaryKind::Daemon),
            paths.clone(),
            node.clone(),
        )
        .await
        .unwrap();
    assert_eq!(current, endpoint); // Idempotent: unlocking never starts a second node.
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    wallets
        .start_wallet_rpc(
            reviewed_binary(BinaryKind::WalletRpc),
            paths.clone(),
            current,
            port,
        )
        .await
        .unwrap();
    wallets
        .open_imported_wallet(id, Zeroizing::new(password.to_owned()))
        .await
        .unwrap();
    wallets.lock(Duration::from_secs(5)).await.unwrap();
    let saved_height: u64 = nodes
        .status(&node)
        .await
        .unwrap()
        .height
        .unwrap()
        .parse()
        .unwrap();
    nodes.stop().await.unwrap();
    assert_eq!(nodes.status(&node).await.unwrap().state, NodeState::Stopped);
    assert!(nodes.is_idle().await.unwrap());
    // A fresh owner has no cached height, as after closing and reopening the app.
    let restarted = NodeService::new();
    restarted
        .start(
            reviewed_binary(BinaryKind::Daemon),
            paths.clone(),
            node.clone(),
        )
        .await
        .unwrap();
    let resumed = restarted.status(&node).await.unwrap();
    assert!(resumed.reachable);
    let resumed_height: u64 = resumed.height.unwrap().parse().unwrap();
    restarted.stop().await.unwrap();
    assert!(
        resumed_height >= saved_height,
        "chain height regressed across restart: {saved_height} -> {resumed_height}"
    );
    assert!(
        advanced,
        "mainnet chain did not advance during the 60-second locked-wallet observation"
    );
    println!(
        "Verified wallet process exit, live chain progress while locked, unlock reuse, node stop and persisted-chain restart."
    );
    println!("Persisted chain resumed at height {resumed_height}, previously {saved_height}.");
}
