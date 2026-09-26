//! Opt-in compatibility test for the reviewed Ryo 0.6.1.0 Linux release.
//!
//! It starts only a disposable, empty wallet RPC listener and verifies the
//! generated login plus authenticated `get_languages`. It does not open a
//! wallet, contact a daemon, or require funds.

use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::path::PathBuf;
use std::time::Duration;

use ryo_wallet_service::application::{LifecycleState, WalletService, WalletServiceError};
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::process::{BinaryDigest, BinaryKind, VerifiedBinary, WalletRpcSession};
use ryo_wallet_service::rpc::RpcError;
use ryo_wallet_service::storage::{AppPaths, WalletId, copy_wallet_pair};
use zeroize::Zeroizing;

const RYO_0_6_1_0_WALLET_RPC_SHA256: &str =
    "5ef7395ce822a02905e68a63abf6f3a9d75654c8a9773c23777902b5522169e3";

fn free_loopback_port() -> u16 {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
#[ignore = "requires RYO_WALLET_RPC_BIN to point to the verified 0.6.1.0 Linux release"]
async fn verified_release_starts_with_generated_login_and_digest_readiness() {
    let path = PathBuf::from(
        std::env::var("RYO_WALLET_RPC_BIN")
            .expect("set RYO_WALLET_RPC_BIN to the reviewed ryo-wallet-rpc binary"),
    );
    let binary = VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(RYO_0_6_1_0_WALLET_RPC_SHA256).unwrap(),
    )
    .expect("the runtime test must use the reviewed release digest");
    let temporary = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temporary.path().join("wallet-data"), Network::Mainnet).unwrap();
    let session = WalletRpcSession::start_with_deadline(
        &binary,
        &paths,
        &NodeConfig::managed_local(Network::Mainnet),
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .expect("verified wallet RPC should become digest-authenticated and ready");

    assert!(!session.client().languages().await.unwrap().is_empty());
    let wallet_id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    paths.create_wallet_dir(&wallet_id).unwrap();
    session
        .client()
        .create_wallet(
            &wallet_id,
            Zeroizing::new("disposable-runtime-test-password".to_owned()),
            "English",
        )
        .await
        .unwrap();
    let phrase = session
        .client()
        .query_mnemonic()
        .await
        .unwrap()
        .into_secret();
    assert!(!phrase.is_empty());
    let primary_address = session.client().primary_address().await.unwrap();
    assert!(!primary_address.is_empty());
    let activity = session.client().activity().await.unwrap();
    assert!(activity.transactions.is_empty());
    assert!(!activity.truncated);
    assert!(activity.pool_unavailable);
    session.client().close_wallet().await.unwrap();
    session.stop(Duration::from_secs(5)).await.unwrap();

    let restored_id = WalletId::parse("fedcba9876543210fedcba9876543210").unwrap();
    paths.create_wallet_dir(&restored_id).unwrap();
    let restored_session = WalletRpcSession::start_with_deadline(
        &binary,
        &paths,
        &NodeConfig::managed_local(Network::Mainnet),
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    restored_session
        .client()
        .restore_wallet(
            &restored_id,
            Zeroizing::new("disposable-runtime-test-password".to_owned()),
            phrase,
            0,
        )
        .await
        .unwrap();
    assert_eq!(
        restored_session.client().primary_address().await.unwrap(),
        primary_address
    );
    restored_session.client().close_wallet().await.unwrap();
    restored_session.stop(Duration::from_secs(5)).await.unwrap();
}

#[tokio::test]
#[ignore = "requires RYO_WALLET_RPC_BIN to point to the verified 0.6.1.0 Linux release"]
async fn created_receive_address_is_saved_and_available_after_reopen() {
    let path = PathBuf::from(
        std::env::var("RYO_WALLET_RPC_BIN")
            .expect("set RYO_WALLET_RPC_BIN to the reviewed ryo-wallet-rpc binary"),
    );
    let binary = VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(RYO_0_6_1_0_WALLET_RPC_SHA256).unwrap(),
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temporary.path().join("wallet-data"), Network::Mainnet).unwrap();
    let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    paths.create_wallet_dir(&id).unwrap();

    let session = WalletRpcSession::start_with_deadline(
        &binary,
        &paths,
        &NodeConfig::managed_local(Network::Mainnet),
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    session
        .client()
        .create_wallet(
            &id,
            Zeroizing::new("disposable-address-test-password".to_owned()),
            "English",
        )
        .await
        .unwrap();
    let original = session.client().receive_addresses().await.unwrap();
    assert_eq!(original.len(), 1);
    assert_eq!(original[0].address_index, 0);
    let phrase = session
        .client()
        .query_mnemonic()
        .await
        .unwrap()
        .into_secret();
    let created = session.client().create_receive_address().await.unwrap();
    assert_eq!(created.address_index, 1);
    assert_ne!(created.address, original[0].address);
    session.client().close_wallet().await.unwrap();
    session.stop(Duration::from_secs(5)).await.unwrap();

    let reopened = WalletRpcSession::start_with_deadline(
        &binary,
        &paths,
        &NodeConfig::managed_local(Network::Mainnet),
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    reopened
        .client()
        .open_imported_wallet(
            &id,
            Zeroizing::new("disposable-address-test-password".to_owned()),
        )
        .await
        .unwrap();
    let addresses = reopened.client().receive_addresses().await.unwrap();
    assert_eq!(addresses.len(), 2);
    assert_eq!(addresses[0], original[0]);
    assert_eq!(addresses[1], created);
    reopened.client().close_wallet().await.unwrap();
    reopened.stop(Duration::from_secs(5)).await.unwrap();

    let restored_id = WalletId::parse("fedcba9876543210fedcba9876543210").unwrap();
    paths.create_wallet_dir(&restored_id).unwrap();
    let restored = WalletRpcSession::start_with_deadline(
        &binary,
        &paths,
        &NodeConfig::managed_local(Network::Mainnet),
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    restored
        .client()
        .restore_wallet(
            &restored_id,
            Zeroizing::new("disposable-address-test-password".to_owned()),
            phrase,
            0,
        )
        .await
        .unwrap();
    assert_eq!(
        restored.client().primary_address().await.unwrap(),
        original[0].address
    );
    assert_eq!(
        restored
            .client()
            .create_receive_address()
            .await
            .unwrap()
            .address,
        created.address
    );
    restored.client().close_wallet().await.unwrap();
    restored.stop(Duration::from_secs(5)).await.unwrap();
}

#[tokio::test]
#[ignore = "requires RYO_WALLET_RPC_BIN to point to the verified 0.6.1.0 Linux release"]
async fn verified_release_service_actor_creates_restores_and_locks() {
    let path = PathBuf::from(
        std::env::var("RYO_WALLET_RPC_BIN")
            .expect("set RYO_WALLET_RPC_BIN to the reviewed ryo-wallet-rpc binary"),
    );
    let binary = VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(RYO_0_6_1_0_WALLET_RPC_SHA256).unwrap(),
    )
    .expect("the runtime test must use the reviewed release digest");
    let temporary = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temporary.path().join("wallet-data"), Network::Mainnet).unwrap();
    let node = NodeConfig::managed_local(Network::Mainnet);
    let service = WalletService::new();
    assert_eq!(
        service
            .start_wallet_rpc(
                binary.clone(),
                paths.clone(),
                node.clone(),
                free_loopback_port(),
            )
            .await
            .unwrap()
            .state,
        LifecycleState::Locked
    );
    let created = service
        .create_wallet(
            WalletId::parse("11111111111111111111111111111111").unwrap(),
            Zeroizing::new("disposable-actor-test-password".to_owned()),
            "English".to_owned(),
        )
        .await
        .unwrap();
    assert_eq!(created.status().state, LifecycleState::Open);
    let overview = service.overview().await.unwrap();
    assert_eq!(
        overview.session_generation,
        created.status().session_generation
    );
    assert!(!overview.primary_address.is_empty());
    assert_eq!(overview.total.atomic, "0");
    assert_eq!(overview.unlocked.atomic, "0");
    assert_eq!(overview.locked.atomic, "0");
    assert!(matches!(
        service.create_receive_address("stale".to_owned()).await,
        Err(WalletServiceError::StaleSession)
    ));
    assert_eq!(
        service
            .receive_addresses(overview.session_generation.clone())
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        service
            .create_receive_address(overview.session_generation.clone())
            .await
            .unwrap()
            .address_index,
        1
    );
    let phrase = created.into_recovery_phrase().into_secret();
    assert_eq!(
        service
            .recovery_phrase()
            .await
            .unwrap()
            .into_secret()
            .as_str(),
        phrase.as_str()
    );
    assert_eq!(
        service.lock(Duration::from_secs(5)).await.unwrap().state,
        LifecycleState::Locked
    );
    assert!(service.is_idle().await.unwrap());
    assert_eq!(
        service
            .start_wallet_rpc(
                binary.clone(),
                paths.clone(),
                node.clone(),
                free_loopback_port(),
            )
            .await
            .unwrap()
            .state,
        LifecycleState::Locked
    );
    assert!(!service.is_idle().await.unwrap());
    assert_eq!(
        service
            .open_imported_wallet(
                WalletId::parse("11111111111111111111111111111111").unwrap(),
                Zeroizing::new("disposable-actor-test-password".to_owned()),
            )
            .await
            .unwrap()
            .state,
        LifecycleState::Open
    );
    assert!(matches!(
        service
            .receive_addresses(overview.session_generation.clone())
            .await,
        Err(WalletServiceError::StaleSession)
    ));
    service.lock(Duration::from_secs(5)).await.unwrap();

    let reopened_service = WalletService::new();
    assert_eq!(
        reopened_service
            .start_wallet_rpc(
                binary.clone(),
                paths.clone(),
                node.clone(),
                free_loopback_port(),
            )
            .await
            .unwrap()
            .state,
        LifecycleState::Locked
    );
    assert!(matches!(
        reopened_service
            .open_imported_wallet(
                WalletId::parse("11111111111111111111111111111111").unwrap(),
                Zeroizing::new("wrong-disposable-actor-test-password".to_owned()),
            )
            .await,
        Err(WalletServiceError::Rpc(RpcError::IncorrectPassword))
    ));
    assert_eq!(
        reopened_service.status().await.unwrap().state,
        LifecycleState::Locked
    );
    assert_eq!(
        reopened_service
            .open_imported_wallet(
                WalletId::parse("11111111111111111111111111111111").unwrap(),
                Zeroizing::new("disposable-actor-test-password".to_owned()),
            )
            .await
            .unwrap()
            .state,
        LifecycleState::Open
    );
    assert_eq!(
        reopened_service
            .lock(Duration::from_secs(5))
            .await
            .unwrap()
            .state,
        LifecycleState::Locked
    );

    let restored_service = WalletService::new();
    assert_eq!(
        restored_service
            .start_wallet_rpc(binary, paths, node, free_loopback_port())
            .await
            .unwrap()
            .state,
        LifecycleState::Locked
    );
    assert_eq!(
        restored_service
            .restore_wallet(
                WalletId::parse("22222222222222222222222222222222").unwrap(),
                Zeroizing::new("disposable-actor-test-password".to_owned()),
                phrase,
                0,
            )
            .await
            .unwrap()
            .state,
        LifecycleState::Open
    );
    assert_eq!(
        restored_service
            .lock(Duration::from_secs(5))
            .await
            .unwrap()
            .state,
        LifecycleState::Locked
    );
}

#[tokio::test]
#[ignore = "requires RYO_WALLET_RPC_BIN to point to the verified 0.6.1.0 Linux release"]
async fn unfinished_backup_can_be_resumed_after_process_exit() {
    let path = PathBuf::from(
        std::env::var("RYO_WALLET_RPC_BIN")
            .expect("set RYO_WALLET_RPC_BIN to the reviewed ryo-wallet-rpc binary"),
    );
    let binary = VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(RYO_0_6_1_0_WALLET_RPC_SHA256).unwrap(),
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(temporary.path().join("wallet-data"), Network::Mainnet).unwrap();
    let node = NodeConfig::managed_local(Network::Mainnet);
    let id = WalletId::parse("44444444444444444444444444444444").unwrap();
    let first = WalletService::new();
    first
        .start_wallet_rpc(
            binary.clone(),
            paths.clone(),
            node.clone(),
            free_loopback_port(),
        )
        .await
        .unwrap();
    let created = first
        .create_wallet(
            id.clone(),
            Zeroizing::new("disposable-unfinished-backup-password".to_owned()),
            "English".to_owned(),
        )
        .await
        .unwrap();
    let original = created.into_recovery_phrase().into_secret();
    drop(first);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let reopened = WalletService::new();
    reopened
        .start_wallet_rpc(binary, paths, node, free_loopback_port())
        .await
        .unwrap();
    reopened
        .open_imported_wallet(
            id,
            Zeroizing::new("disposable-unfinished-backup-password".to_owned()),
        )
        .await
        .unwrap();
    assert_eq!(
        reopened
            .recovery_phrase()
            .await
            .unwrap()
            .into_secret()
            .as_str(),
        original.as_str()
    );
    reopened.lock(Duration::from_secs(5)).await.unwrap();
}

#[tokio::test]
#[ignore = "requires RYO_WALLET_RPC_BIN to point to the verified 0.6.1.0 Linux release"]
async fn verified_release_opens_an_imported_copy_without_touching_the_source() {
    let path = PathBuf::from(
        std::env::var("RYO_WALLET_RPC_BIN")
            .expect("set RYO_WALLET_RPC_BIN to the reviewed ryo-wallet-rpc binary"),
    );
    let binary = VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(RYO_0_6_1_0_WALLET_RPC_SHA256).unwrap(),
    )
    .expect("the runtime test must use the reviewed release digest");
    let temporary = tempfile::tempdir().unwrap();
    let source_paths = AppPaths::new(temporary.path().join("source"), Network::Mainnet).unwrap();
    let destination_paths =
        AppPaths::new(temporary.path().join("destination"), Network::Mainnet).unwrap();
    let node = NodeConfig::managed_local(Network::Mainnet);
    let source_id = WalletId::parse("33333333333333333333333333333333").unwrap();
    let source_session = WalletRpcSession::start_with_deadline(
        &binary,
        &source_paths,
        &node,
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    source_paths.create_wallet_dir(&source_id).unwrap();
    source_session
        .client()
        .create_wallet(
            &source_id,
            Zeroizing::new("disposable-import-test-password".to_owned()),
            "English",
        )
        .await
        .unwrap();
    let primary_address = source_session.client().primary_address().await.unwrap();
    source_session.client().close_wallet().await.unwrap();
    source_session.stop(Duration::from_secs(5)).await.unwrap();

    let source_wallet = source_paths.wallet_dir(&source_id).join("wallet");
    let source_keys = source_paths.wallet_dir(&source_id).join("wallet.keys");
    let source_wallet_size = std::fs::metadata(&source_wallet).unwrap().len();
    let source_keys_size = std::fs::metadata(&source_keys).unwrap().len();
    let imported_id = WalletId::parse("44444444444444444444444444444444").unwrap();
    let imported = copy_wallet_pair(&destination_paths, &imported_id, &source_wallet).unwrap();
    assert_eq!(
        std::fs::metadata(&source_wallet).unwrap().len(),
        source_wallet_size
    );
    assert_eq!(
        std::fs::metadata(&source_keys).unwrap().len(),
        source_keys_size
    );
    assert!(imported.wallet_path.is_file());
    assert!(imported.keys_path.is_file());

    let imported_session = WalletRpcSession::start_with_deadline(
        &binary,
        &destination_paths,
        &node,
        free_loopback_port(),
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    imported_session
        .client()
        .open_imported_wallet(
            &imported_id,
            Zeroizing::new("disposable-import-test-password".to_owned()),
        )
        .await
        .unwrap();
    assert_eq!(
        imported_session.client().primary_address().await.unwrap(),
        primary_address
    );
    imported_session.client().close_wallet().await.unwrap();
    imported_session.stop(Duration::from_secs(5)).await.unwrap();
}
