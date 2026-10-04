use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use ryo_wallet_service::application::{WalletService, WalletServiceError};
use ryo_wallet_service::rpc::RpcError;
use ryo_wallet_service::storage::{
    AppPaths, ImportError, WalletId, copy_wallet_pair, validate_wallet_pair,
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

use super::{
    ActiveWalletState, DataRootState, NodeService, RestoredWalletResponse, SetupState,
    SyncMonitorState, ready_wallet_service_under_setup, require_app_running, require_stopped,
    save_backup_acknowledgement, selected_paths, start_wallet_sync_monitor_for_current_node,
};

#[derive(Default)]
pub(super) struct ImportSelectionState(Mutex<Option<PendingImport>>);

#[derive(Clone)]
struct PendingImport {
    selection_id: String,
    source: PathBuf,
    paths: AppPaths,
}

#[derive(serde::Serialize)]
pub struct ImportSelection {
    selection_id: String,
    file_name: String,
}

/// File selection precedes authentication. Only a basename and opaque token leave Rust.
#[tauri::command]
pub async fn wallet_select_import(
    app: tauri::AppHandle,
) -> Result<Option<ImportSelection>, &'static str> {
    let setup = app.state::<SetupState>();
    let _guard = setup.0.lock().await;
    require_app_running(&app)?;
    let state = app.state::<DataRootState>();
    let service = app.state::<WalletService>();
    require_stopped(&service).await?;
    let paths = selected_paths(&state)?;
    let selection = app.state::<ImportSelectionState>();
    *selection
        .0
        .lock()
        .map_err(|_| "wallet selection is unavailable")? = None;
    let picker = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Select the wallet file, not its .keys companion")
            .blocking_pick_file()
    })
    .await
    .map_err(|_| "wallet file picker is unavailable")?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let source = selected
        .into_path()
        .map_err(|_| "selected wallet file path is invalid")?;
    let validation_source = source.clone();
    tauri::async_runtime::spawn_blocking(move || validate_wallet_pair(&validation_source))
        .await
        .map_err(|_| "wallet file validation is unavailable")?
        .map_err(import_error)?;
    require_app_running(&app)?;
    let response = ImportSelection {
        selection_id: uuid::Uuid::new_v4().simple().to_string(),
        file_name: source
            .file_name()
            .ok_or("selected wallet file name is invalid")?
            .to_string_lossy()
            .into_owned(),
    };
    *selection
        .0
        .lock()
        .map_err(|_| "wallet selection is unavailable")? = Some(PendingImport {
        selection_id: response.selection_id.clone(),
        source,
        paths,
    });
    Ok(Some(response))
}

fn selected_import(
    pending: Option<&PendingImport>,
    selection_id: &str,
    paths: &AppPaths,
) -> Result<PendingImport, &'static str> {
    let pending = pending
        .filter(|pending| pending.selection_id == selection_id)
        .ok_or("select the wallet file again")?;
    if pending.paths.root() != paths.root() || pending.paths.network() != paths.network() {
        return Err("data folder or network changed; select the wallet file again");
    }
    Ok(pending.clone())
}

/// The renderer supplies the selection token, password and acknowledgement, never a path.
#[tauri::command]
pub async fn wallet_import(
    selection_id: String,
    password: String,
    backup_confirmed: bool,
    app: tauri::AppHandle,
) -> Result<RestoredWalletResponse, &'static str> {
    let password = Zeroizing::new(password);
    if password.is_empty() || password.len() > 1024 {
        return Err("enter the existing wallet password (at most 1024 bytes)");
    }
    if !backup_confirmed {
        return Err("confirm that you have a backup of the original wallet files");
    }
    let setup = app.state::<SetupState>();
    let _guard = setup.0.lock().await;
    require_app_running(&app)?;
    let state = app.state::<DataRootState>();
    let service = app.state::<WalletService>();
    require_stopped(&service).await?;
    let paths = selected_paths(&state)?;
    let selection = app.state::<ImportSelectionState>();
    let pending = selected_import(
        selection
            .0
            .lock()
            .map_err(|_| "wallet selection is unavailable")?
            .as_ref(),
        &selection_id,
        &paths,
    )?;
    let nodes = app.state::<NodeService>();
    ready_wallet_service_under_setup(&service, &state, &app, &nodes).await?;
    let imported = import_wallet_copy(&paths, &service, &pending.source, password).await?;
    *selection
        .0
        .lock()
        .map_err(|_| "wallet selection is unavailable")? = None;
    let id =
        WalletId::parse(imported.wallet_id.clone()).map_err(|_| "wallet identifier is invalid")?;
    *app.state::<ActiveWalletState>()
        .0
        .lock()
        .map_err(|_| "wallet state is unavailable")? = Some(id);
    start_wallet_sync_monitor_for_current_node(
        app.clone(),
        service.inner(),
        nodes.inner(),
        &state,
        app.state::<SyncMonitorState>().inner(),
    );
    Ok(imported)
}

async fn import_wallet_copy(
    paths: &AppPaths,
    service: &WalletService,
    source: &Path,
    password: Zeroizing<String>,
) -> Result<RestoredWalletResponse, &'static str> {
    let id = WalletId::parse(uuid::Uuid::new_v4().simple().to_string())
        .map_err(|_| "wallet identifier could not be created")?;
    let copy_paths = paths.clone();
    let copy_id = id.clone();
    let source = source.to_path_buf();
    let copied = tauri::async_runtime::spawn_blocking(move || {
        copy_wallet_pair(&copy_paths, &copy_id, &source)
    })
    .await
    .map_err(|_| "wallet copy could not be completed")?;
    if let Err(error) = copied {
        // No wallet was opened. Stop the newly started key-bearing runtime so retry works.
        service
            .lock(Duration::from_secs(5))
            .await
            .map_err(|_| "wallet runtime could not be stopped; close the app before retrying")?;
        return Err(import_error(error));
    }
    let result = async {
        let status = service
            .open_imported_wallet(id.clone(), password)
            .await
            .map_err(|error| match error {
                WalletServiceError::Rpc(RpcError::IncorrectPassword) => "incorrect wallet password",
                WalletServiceError::UnsupportedWalletScope => {
                    "multiple-account and multisig wallets are not supported"
                }
                _ => {
                    "wallet copy could not be opened; check the file, password and selected network"
                }
            })?;
        save_backup_acknowledgement(paths, &id)?;
        Ok(RestoredWalletResponse {
            wallet_id: id.to_string(),
            status,
        })
    }
    .await;
    if result.is_err() {
        // Stop RPC before removing only this attempt's fresh, app-owned copy.
        service.lock(Duration::from_secs(5)).await.map_err(
            |_| "wallet runtime could not be stopped; the imported copy was kept; close the app",
        )?;
        let directory = paths.wallet_dir(&id);
        if directory.parent() != Some(paths.wallets_root().as_path())
            || fs::remove_dir_all(&directory).is_err()
        {
            return Err(
                "import failed; its private copy could not be removed; originals were preserved",
            );
        }
    }
    result
}

fn import_error(error: ImportError) -> &'static str {
    match error {
        ImportError::SelectedKeyFile => "select the wallet file, not its .keys companion",
        ImportError::MissingKeys => {
            "the matching .keys file must be beside the selected wallet file"
        }
        ImportError::RelativeSource | ImportError::UnsafeSource => {
            "select a regular wallet file; linked files are not supported"
        }
        ImportError::WalletTooLarge => "wallet file exceeds the 512 MiB import limit",
        ImportError::KeysTooLarge => "wallet .keys file exceeds the 64 MiB import limit",
        ImportError::Destination(_) | ImportError::Io(_) => {
            "wallet files could not be copied into the selected data folder"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryo_wallet_service::application::{LifecycleState, WalletOperation};
    use ryo_wallet_service::domain::{Network, NodeConfig};
    use std::net::{Ipv4Addr, TcpListener};

    #[test]
    fn selection_requires_the_native_token_and_unchanged_storage_and_network() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().join("copies"), Network::Mainnet).unwrap();
        let pending = PendingImport {
            selection_id: "native-selection".into(),
            source: temp.path().join("original-wallet"),
            paths: paths.clone(),
        };
        assert!(selected_import(None, "native-selection", &paths).is_err());
        assert!(selected_import(Some(&pending), "renderer-path", &paths).is_err());
        let changed_root = AppPaths::new(temp.path().join("other"), Network::Mainnet).unwrap();
        let changed_network = AppPaths::new(paths.root().to_path_buf(), Network::Testnet).unwrap();
        assert!(selected_import(Some(&pending), "native-selection", &changed_root).is_err());
        assert!(selected_import(Some(&pending), "native-selection", &changed_network).is_err());
        assert_eq!(
            selected_import(Some(&pending), "native-selection", &paths)
                .unwrap()
                .source,
            pending.source
        );
    }

    async fn start(service: &WalletService, paths: AppPaths) {
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        service
            .start_wallet_rpc(
                crate::wallet_runtime::reviewed_wallet_rpc().expect("prepare the reviewed runtime"),
                paths,
                NodeConfig::managed_local(Network::Mainnet),
                port,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires prepared, hash-verified wallet RPC; disposable empty wallets only"]
    async fn native_import_preserves_originals_cache_and_existing_short_password() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let originals = AppPaths::new(root.join("originals"), Network::Mainnet).unwrap();
        let copies = AppPaths::new(root.join("copies"), Network::Mainnet).unwrap();
        let source_id = WalletId::parse("11111111111111111111111111111111").unwrap();
        let original_service = WalletService::new();
        start(&original_service, originals.clone()).await;
        let created = original_service
            .create_wallet(
                source_id.clone(),
                Zeroizing::new("short".into()),
                "English".into(),
            )
            .await
            .unwrap();
        let generation = created.status().session_generation.clone();
        drop(created);
        original_service
            .operation(
                generation.clone(),
                WalletOperation::SetName {
                    name: "Disposable import check".into(),
                },
            )
            .await
            .unwrap();
        original_service
            .create_receive_address(generation.clone())
            .await
            .unwrap();
        let expected_addresses = original_service
            .receive_addresses(generation)
            .await
            .unwrap();
        original_service.lock(Duration::from_secs(5)).await.unwrap();
        let source = originals.wallet_dir(&source_id).join("wallet");
        let keys = originals.wallet_dir(&source_id).join("wallet.keys");
        let original_cache = fs::read(&source).unwrap();
        let original_keys = fs::read(&keys).unwrap();

        let service = WalletService::new();
        start(&service, copies.clone()).await;
        assert_eq!(
            import_wallet_copy(&copies, &service, &keys, Zeroizing::new("short".into()))
                .await
                .err(),
            Some("select the wallet file, not its .keys companion")
        );
        assert!(service.is_idle().await.unwrap());
        assert_eq!(fs::read_dir(copies.wallets_root()).unwrap().count(), 0);

        start(&service, copies.clone()).await;
        assert_eq!(
            import_wallet_copy(&copies, &service, &source, Zeroizing::new("wrong".into()))
                .await
                .err(),
            Some("incorrect wallet password")
        );
        assert!(service.is_idle().await.unwrap());
        assert_eq!(fs::read_dir(copies.wallets_root()).unwrap().count(), 0);

        start(&service, copies.clone()).await;
        let imported =
            import_wallet_copy(&copies, &service, &source, Zeroizing::new("short".into()))
                .await
                .unwrap();
        let imported_id = WalletId::parse(imported.wallet_id).unwrap();
        assert_eq!(imported.status.state, LifecycleState::Open);
        assert!(crate::backup_complete(&copies, &imported_id));
        let actual = service
            .receive_addresses(imported.status.session_generation.clone())
            .await
            .unwrap();
        assert_eq!(actual, expected_addresses);
        match service
            .operation(imported.status.session_generation, WalletOperation::Info)
            .await
            .unwrap()
        {
            ryo_wallet_service::application::WalletOperationOutput::Json(info) => {
                assert_eq!(info["name"], "Disposable import check");
            }
            _ => panic!("expected public wallet metadata"),
        }
        service.lock(Duration::from_secs(5)).await.unwrap();
        start(&service, copies.clone()).await;
        service
            .open_imported_wallet(imported_id, Zeroizing::new("short".into()))
            .await
            .unwrap();
        service.lock(Duration::from_secs(5)).await.unwrap();
        assert_eq!(fs::read(source).unwrap(), original_cache);
        assert_eq!(fs::read(keys).unwrap(), original_keys);
    }
}
