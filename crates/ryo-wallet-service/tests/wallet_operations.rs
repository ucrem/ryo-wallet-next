//! Opt-in compatibility checks against a verified binary, using an empty disposable wallet.
use ryo_wallet_service::application::{
    WalletOperation, WalletOperationOutput, WalletService, WalletServiceError,
};
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::process::{BinaryDigest, BinaryKind, VerifiedBinary, upstream_path};
use ryo_wallet_service::storage::{AppPaths, WalletId, load_wallet_name};
use serde_json::Value;
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::time::Duration;
use zeroize::Zeroizing;

fn binary() -> VerifiedBinary {
    let path =
        PathBuf::from(std::env::var("RYO_TEST_WALLET_RPC").expect("set reviewed executable path"));
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
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(
            manifest["platforms"][target]["binarySha256"]
                .as_str()
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
async fn start(service: &WalletService, paths: AppPaths) {
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    service
        .start_wallet_rpc(
            binary(),
            paths,
            NodeConfig::managed_local(Network::Mainnet),
            port,
        )
        .await
        .unwrap();
}
async fn op(service: &WalletService, generation: &str, operation: WalletOperation) -> Value {
    match service
        .operation(generation.into(), operation)
        .await
        .unwrap()
    {
        WalletOperationOutput::Json(value) => value,
        WalletOperationOutput::Secrets(_) => {
            panic!("secret material must never enter generic JSON")
        }
    }
}

#[tokio::test]
#[ignore = "requires reviewed RYO_TEST_WALLET_RPC binary; no funds or real wallet data"]
async fn wallet_operations_persist_and_password_reauthentication_rejects_wrong_password() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::new(
        std::fs::canonicalize(temp.path()).unwrap(),
        Network::Mainnet,
    )
    .unwrap();
    let service = WalletService::new();
    start(&service, paths.clone()).await;
    let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
    let password = "disposable-operations-password";
    let created = service
        .create_wallet(
            id.clone(),
            Zeroizing::new(password.into()),
            "English".into(),
        )
        .await
        .unwrap();
    let generation = created.status().session_generation.clone();
    drop(created);
    let primary = service.overview().await.unwrap().primary_address;
    assert_eq!(
        op(&service, &generation, WalletOperation::Info).await["name"],
        ""
    );
    assert_eq!(
        op(&service, &generation, WalletOperation::Contacts)
            .await
            .as_array()
            .unwrap()
            .len(),
        0
    );
    op(
        &service,
        &generation,
        WalletOperation::SetName {
            name: "Disposable test".into(),
        },
    )
    .await;
    assert_eq!(
        load_wallet_name(&paths.wallet_dir(&id)).unwrap().as_deref(),
        Some("Disposable test")
    );
    let contacts = op(
        &service,
        &generation,
        WalletOperation::SaveContact {
            id: None,
            name: "Test contact".into(),
            address: primary.clone(),
            payment_id: String::new(),
            notes: "Private test note".into(),
        },
    )
    .await;
    let contact_id = contacts[0]["id"].as_str().unwrap().to_owned();
    let new_address = service
        .create_receive_address(generation.clone())
        .await
        .unwrap();
    op(
        &service,
        &generation,
        WalletOperation::LabelAddress {
            index: new_address.address_index,
            label: "Invoice".into(),
        },
    )
    .await;
    let request = op(
        &service,
        &generation,
        WalletOperation::MakeRequest {
            address: primary,
            amount: "9007199.254740993".into(),
            payment_id: String::new(),
            description: "Round trip".into(),
        },
    )
    .await;
    let parsed = op(
        &service,
        &generation,
        WalletOperation::ParseRequest {
            uri: request["uri"].as_str().unwrap().into(),
        },
    )
    .await;
    assert_eq!(parsed["amount"], "9007199.254740993");
    assert!(
        service
            .operation(
                generation.clone(),
                WalletOperation::Secrets {
                    password: Zeroizing::new("wrong-password".into())
                }
            )
            .await
            .is_err()
    );
    let secrets = service
        .operation(
            generation.clone(),
            WalletOperation::Secrets {
                password: Zeroizing::new(password.into()),
            },
        )
        .await
        .unwrap();
    assert!(matches!(secrets, WalletOperationOutput::Secrets(_)));
    drop(secrets);
    let export = temp.path().join("test-key-images.ryoki");
    op(
        &service,
        &generation,
        WalletOperation::ExportKeyImages {
            filename: upstream_path(&export)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            password: Zeroizing::new(password.into()),
        },
    )
    .await;
    assert!(export.is_file());
    op(
        &service,
        &generation,
        WalletOperation::ChangePassword {
            old_password: Zeroizing::new(password.into()),
            new_password: Zeroizing::new("disposable-changed-password".into()),
        },
    )
    .await;
    service.lock(Duration::from_secs(5)).await.unwrap();
    assert!(matches!(
        service
            .operation(generation.clone(), WalletOperation::History)
            .await,
        Err(WalletServiceError::Lifecycle(_))
    ));
    assert_eq!(
        load_wallet_name(&paths.wallet_dir(&id)).unwrap().as_deref(),
        Some("Disposable test")
    );
    // An older installation has the encrypted name but no locked-picker cache.
    std::fs::remove_file(paths.wallet_dir(&id).join("wallet-name-v1.json")).unwrap();
    assert_eq!(load_wallet_name(&paths.wallet_dir(&id)).unwrap(), None);
    start(&service, paths.clone()).await;
    assert!(
        service
            .open_imported_wallet(id.clone(), Zeroizing::new(password.into()))
            .await
            .is_err()
    );
    let status = service
        .open_imported_wallet(
            id.clone(),
            Zeroizing::new("disposable-changed-password".into()),
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .operation(generation, WalletOperation::Contacts)
            .await,
        Err(WalletServiceError::StaleSession)
    ));
    let generation = status.session_generation;
    assert_eq!(
        op(&service, &generation, WalletOperation::Info).await["name"],
        "Disposable test"
    );
    assert_eq!(
        load_wallet_name(&paths.wallet_dir(&id)).unwrap().as_deref(),
        Some("Disposable test")
    );
    assert_eq!(
        op(&service, &generation, WalletOperation::Contacts).await[0]["name"],
        "Test contact"
    );
    assert_eq!(
        service.receive_addresses(generation.clone()).await.unwrap()[1].label,
        "Invoice"
    );
    op(
        &service,
        &generation,
        WalletOperation::SaveContact {
            id: Some(contact_id.clone()),
            name: "Edited".into(),
            address: service.overview().await.unwrap().primary_address,
            payment_id: String::new(),
            notes: String::new(),
        },
    )
    .await;
    assert_eq!(
        op(&service, &generation, WalletOperation::Contacts).await[0]["name"],
        "Edited"
    );
    op(
        &service,
        &generation,
        WalletOperation::DeleteContact { id: contact_id },
    )
    .await;
    assert!(
        op(&service, &generation, WalletOperation::Contacts)
            .await
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        service
            .operation(
                generation,
                WalletOperation::PrepareSend {
                    address: "invalid".into(),
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
    service.lock(Duration::from_secs(5)).await.unwrap();
}
