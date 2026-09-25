use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ryo_wallet_service::application::{
    LifecycleStatus, WalletOverview, WalletService, WalletServiceError,
};
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::process::VerifiedBinary;
#[cfg(all(debug_assertions, target_os = "linux"))]
use ryo_wallet_service::process::{BinaryDigest, BinaryKind};
use ryo_wallet_service::rpc::{DaemonRpcClient, ReceiveAddress, RpcError};
use ryo_wallet_service::storage::{
    AppPaths, AppSettings, Theme, WalletId, load_settings_if_present, save_settings,
};
use serde::ser::SerializeStruct;
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

mod app_updates;

const DATA_ROOT_SELECTION_FILE: &str = "wallet-data-root.json";
const WALLET_SYNC_EVENT: &str = "wallet-sync-status";
const WALLET_SYNC_INTERVAL: Duration = Duration::from_secs(2);
#[cfg(all(debug_assertions, target_os = "linux"))]
const REVIEWED_LINUX_WALLET_RPC_SHA256: &str =
    "5ef7395ce822a02905e68a63abf6f3a9d75654c8a9773c23777902b5522169e3";

/// This development-only adapter accepts exactly the Linux 0.6.1.0 binary
/// exercised by the real RPC tests. Public packages need platform manifests.
#[cfg(all(debug_assertions, target_os = "linux"))]
fn reviewed_wallet_rpc() -> Option<VerifiedBinary> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".dev-runtime")
        .join("ryo-wallet-rpc");
    VerifiedBinary::verify(
        BinaryKind::WalletRpc,
        &path,
        BinaryDigest::parse_hex(REVIEWED_LINUX_WALLET_RPC_SHA256).ok()?,
    )
    .ok()
}

#[cfg(not(all(debug_assertions, target_os = "linux")))]
fn reviewed_wallet_rpc() -> Option<VerifiedBinary> {
    None
}

struct CreatedWalletResponse {
    wallet_id: String,
    status: LifecycleStatus,
    recovery_phrase: Zeroizing<String>,
}

struct SecretPhraseResponse(Zeroizing<String>);

impl serde::Serialize for SecretPhraseResponse {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("SecretPhraseResponse", 1)?;
        output.serialize_field("recovery_phrase", self.0.as_str())?;
        output.end()
    }
}

impl serde::Serialize for CreatedWalletResponse {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut output = serializer.serialize_struct("CreatedWalletResponse", 3)?;
        output.serialize_field("wallet_id", &self.wallet_id)?;
        output.serialize_field("status", &self.status)?;
        output.serialize_field("recovery_phrase", self.recovery_phrase.as_str())?;
        output.end()
    }
}

/// This is deliberately separate from wallet settings: it is only the
/// user-approved base directory, kept in Tauri's private app config area.
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedDataRoot {
    root: PathBuf,
}

struct DataRootState(Mutex<Option<AppPaths>>);
struct ActiveWalletState(Mutex<Option<WalletId>>);
struct SyncMonitorState(AtomicU64);

#[derive(serde::Serialize)]
struct WalletEntry {
    id: String,
    backup_complete: bool,
}

fn backup_ack_path(paths: &AppPaths, id: &WalletId) -> PathBuf {
    paths.wallet_dir(id).join("backup-acknowledged")
}

fn backup_complete(paths: &AppPaths, id: &WalletId) -> bool {
    let path = backup_ack_path(paths, id);
    fs::symlink_metadata(&path)
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        && fs::read(path).is_ok_and(|bytes| bytes == b"1")
}

#[derive(serde::Serialize)]
struct DataRootConfiguration {
    root: Option<PathBuf>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
enum NodeSelection {
    Local,
    Remote { host: String, port: u16 },
}

impl DataRootState {
    fn load(app: &tauri::AppHandle) -> Result<Self, &'static str> {
        let path = data_root_selection_path(app)?;
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self(Mutex::new(None)));
            }
            Err(_) => return Err("data location configuration is unavailable"),
        };
        let persisted: PersistedDataRoot =
            serde_json::from_slice(&bytes).map_err(|_| "data location configuration is invalid")?;
        let paths = AppPaths::new(persisted.root, Network::Mainnet)
            .map_err(|_| "data location configuration is invalid")?;
        Ok(Self(Mutex::new(Some(paths))))
    }
}

fn data_root_selection_path(app: &tauri::AppHandle) -> Result<PathBuf, &'static str> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(DATA_ROOT_SELECTION_FILE))
        .map_err(|_| "data location configuration is unavailable")
}

fn persist_data_root(app: &tauri::AppHandle, root: &PathBuf) -> Result<(), &'static str> {
    let final_path = data_root_selection_path(app)?;
    let parent = final_path
        .parent()
        .ok_or("data location configuration is unavailable")?;
    fs::create_dir_all(parent).map_err(|_| "data location configuration is unavailable")?;
    let temporary_path = parent.join("wallet-data-root.json.new");
    let bytes = serde_json::to_vec(&PersistedDataRoot { root: root.clone() })
        .map_err(|_| "data location configuration is unavailable")?;
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|_| "data location configuration is unavailable")?;
    let write_result = (|| {
        let mut writer = BufWriter::new(file);
        writer
            .write_all(&bytes)
            .map_err(|_| "data location configuration is unavailable")?;
        writer
            .flush()
            .map_err(|_| "data location configuration is unavailable")?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|_| "data location configuration is unavailable")?;
        fs::rename(&temporary_path, final_path)
            .map_err(|_| "data location configuration is unavailable")
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    write_result
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
struct WalletSyncStatusResponse {
    wallet_height: Option<String>,
    daemon_height: Option<String>,
    network_height: Option<String>,
    node_reachable: bool,
    node_ready: bool,
    node_offline: bool,
    node_untrusted: bool,
}

#[tauri::command]
async fn app_status(
    service: tauri::State<'_, WalletService>,
) -> Result<LifecycleStatus, &'static str> {
    service.status().await.map_err(|_| "service unavailable")
}

#[tauri::command]
async fn wallet_overview(
    service: tauri::State<'_, WalletService>,
) -> Result<WalletOverview, &'static str> {
    service
        .overview()
        .await
        .map_err(|_| "wallet overview unavailable")
}

#[tauri::command]
async fn wallet_receive_addresses(
    session_generation: String,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
) -> Result<Vec<ReceiveAddress>, &'static str> {
    require_completed_backup(&state, &active)?;
    service
        .receive_addresses(session_generation)
        .await
        .map_err(|error| match error {
            WalletServiceError::StaleSession => "wallet session changed; reopen the wallet page",
            _ => "receive addresses are unavailable",
        })
}

#[tauri::command]
async fn wallet_create_receive_address(
    session_generation: String,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
) -> Result<ReceiveAddress, &'static str> {
    require_completed_backup(&state, &active)?;
    service
        .create_receive_address(session_generation)
        .await
        .map_err(|error| match error {
            WalletServiceError::StaleSession => "wallet session changed; reopen the wallet page",
            _ => "address creation may have succeeded; check the address list before trying again",
        })
}

fn require_completed_backup(
    state: &DataRootState,
    active: &ActiveWalletState,
) -> Result<(), &'static str> {
    let id = active
        .0
        .lock()
        .map_err(|_| "wallet state is unavailable")?
        .clone()
        .ok_or("open a wallet first")?;
    let paths = selected_paths(state)?;
    if !backup_complete(&paths, &id) {
        return Err("confirm the recovery phrase backup before creating receive addresses");
    }
    Ok(())
}

#[tauri::command]
async fn wallet_sync_status(
    state: tauri::State<'_, DataRootState>,
    service: tauri::State<'_, WalletService>,
) -> Result<WalletSyncStatusResponse, &'static str> {
    let paths = selected_paths(&state)?;
    let node = load_settings_if_present(&paths)
        .map_err(|_| "node configuration is unavailable")?
        .ok_or("choose a node first")?
        .node;
    let daemon = DaemonRpcClient::configured(&node).map_err(|_| "node configuration is invalid")?;
    Ok(collect_wallet_sync_status(service.inner(), &daemon).await)
}

async fn collect_wallet_sync_status(
    service: &WalletService,
    daemon: &DaemonRpcClient,
) -> WalletSyncStatusResponse {
    let wallet_height = service.height().await.ok().map(|height| height.to_string());

    match daemon.health().await {
        Ok(health) => {
            let network_height = health.height.max(health.target_height);

            WalletSyncStatusResponse {
                wallet_height,
                daemon_height: Some(health.height.to_string()),
                network_height: Some(network_height.to_string()),
                node_reachable: true,
                node_ready: health.ready,
                node_offline: health.offline,
                node_untrusted: health.untrusted,
            }
        }
        Err(_) => WalletSyncStatusResponse {
            wallet_height,
            daemon_height: None,
            network_height: None,
            node_reachable: false,
            node_ready: false,
            node_offline: true,
            node_untrusted: false,
        },
    }
}

fn stop_wallet_sync_monitor(monitor: &SyncMonitorState) {
    monitor.0.fetch_add(1, Ordering::AcqRel);
}

fn start_wallet_sync_monitor(
    app: tauri::AppHandle,
    service: WalletService,
    node: NodeConfig,
    monitor: &SyncMonitorState,
) {
    let generation = monitor.0.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    tauri::async_runtime::spawn(async move {
        let Ok(daemon) = DaemonRpcClient::configured(&node) else {
            return;
        };
        let mut previous: Option<WalletSyncStatusResponse> = None;

        loop {
            if app.state::<SyncMonitorState>().0.load(Ordering::Acquire) != generation {
                break;
            }
            let snapshot = collect_wallet_sync_status(&service, &daemon).await;
            // A lock or a new wallet can invalidate this task while RPC is in flight.
            if app.state::<SyncMonitorState>().0.load(Ordering::Acquire) != generation {
                break;
            }
            if previous.as_ref() != Some(&snapshot) {
                let _ = app.emit(WALLET_SYNC_EVENT, snapshot.clone());
                previous = Some(snapshot);
            }
            tokio::time::sleep(WALLET_SYNC_INTERVAL).await;
        }
    });
}

fn start_wallet_sync_monitor_for_current_node(
    app: tauri::AppHandle,
    service: &WalletService,
    state: &DataRootState,
    monitor: &SyncMonitorState,
) {
    let Ok(paths) = selected_paths(state) else {
        return;
    };
    let Ok(Some(settings)) = load_settings_if_present(&paths) else {
        return;
    };
    start_wallet_sync_monitor(app, service.clone(), settings.node, monitor);
}

#[tauri::command]
fn wallet_runtime_ready() -> bool {
    reviewed_wallet_rpc().is_some()
}

fn wallet_file_pair_exists(paths: &AppPaths, id: &WalletId) -> bool {
    let directory = paths.wallet_dir(id);
    if !fs::symlink_metadata(&directory)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return false;
    }
    ["wallet", "wallet.keys"].iter().all(|name| {
        fs::symlink_metadata(directory.join(name))
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
    })
}

#[tauri::command]
fn wallet_list(state: tauri::State<'_, DataRootState>) -> Result<Vec<WalletEntry>, &'static str> {
    let paths = selected_paths(&state)?;
    let entries = fs::read_dir(paths.wallets_root()).map_err(|_| "wallet list is unavailable")?;
    let mut wallets = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| "wallet list is unavailable")?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if let Ok(id) = WalletId::parse(name) {
            if wallet_file_pair_exists(&paths, &id) {
                wallets.push(WalletEntry {
                    id: id.to_string(),
                    backup_complete: backup_complete(&paths, &id),
                });
            }
        }
    }
    wallets.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(wallets)
}

#[tauri::command]
fn wallet_active(
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
) -> Result<Option<WalletEntry>, &'static str> {
    let id = active
        .0
        .lock()
        .map_err(|_| "wallet state is unavailable")?
        .clone();
    let Some(id) = id else { return Ok(None) };
    let paths = selected_paths(&state)?;
    Ok(Some(WalletEntry {
        id: id.to_string(),
        backup_complete: backup_complete(&paths, &id),
    }))
}

fn free_loopback_port() -> Result<u16, &'static str> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|_| "local wallet RPC port is unavailable")?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|_| "local wallet RPC port is unavailable")
}

async fn ready_wallet_service(
    service: &WalletService,
    state: &DataRootState,
) -> Result<(), &'static str> {
    let paths = selected_paths(state)?;
    let binary = reviewed_wallet_rpc().ok_or(
        "verified Linux wallet runtime is unavailable; restart pnpm tauri dev to prepare it",
    )?;
    let node = load_settings_if_present(&paths)
        .map_err(|_| "node configuration is unavailable")?
        .ok_or("choose a node first")?
        .node;
    service
        .start_wallet_rpc(binary, paths, node, free_loopback_port()?)
        .await
        .map_err(
            |_| "wallet runtime could not be started; check the selected node and restart the app",
        )?;
    Ok(())
}

#[tauri::command]
async fn wallet_create(
    password: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
    monitor: tauri::State<'_, SyncMonitorState>,
) -> Result<CreatedWalletResponse, &'static str> {
    if !(12..=1024).contains(&password.len()) {
        return Err("wallet password must contain 12 to 1024 bytes");
    }
    let password = Zeroizing::new(password);
    ready_wallet_service(&service, &state).await?;
    let wallet_id = WalletId::parse(uuid::Uuid::new_v4().simple().to_string())
        .map_err(|_| "wallet identifier could not be created")?;
    let created = service
        .create_wallet(wallet_id.clone(), password, "English".to_owned())
        .await
        .map_err(
            |_| "wallet creation failed; keep the data folder and restart the app before retrying",
        )?;
    let status = created.status().clone();
    *active.0.lock().map_err(|_| "wallet state is unavailable")? = Some(wallet_id.clone());
    start_wallet_sync_monitor_for_current_node(app, service.inner(), &state, monitor.inner());
    Ok(CreatedWalletResponse {
        wallet_id: wallet_id.to_string(),
        status,
        recovery_phrase: created.into_recovery_phrase().into_secret(),
    })
}

#[tauri::command]
async fn wallet_open(
    wallet_id: String,
    password: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
    monitor: tauri::State<'_, SyncMonitorState>,
) -> Result<LifecycleStatus, &'static str> {
    let id = WalletId::parse(wallet_id).map_err(|_| "wallet identifier is invalid")?;
    let paths = selected_paths(&state)?;
    if !wallet_file_pair_exists(&paths, &id) {
        return Err("wallet files are unavailable");
    }
    if password.is_empty() {
        return Err("enter the wallet password");
    }
    let password = Zeroizing::new(password);
    ready_wallet_service(&service, &state).await?;
    let status = service
        .open_imported_wallet(id.clone(), password)
        .await
        .map_err(|error| match error {
            WalletServiceError::Rpc(RpcError::IncorrectPassword) => "incorrect password",
            _ => "wallet could not be opened; files were not changed",
        })?;
    *active.0.lock().map_err(|_| "wallet state is unavailable")? = Some(id);
    start_wallet_sync_monitor_for_current_node(app, service.inner(), &state, monitor.inner());
    Ok(status)
}

#[tauri::command]
async fn wallet_lock(
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
    monitor: tauri::State<'_, SyncMonitorState>,
) -> Result<LifecycleStatus, &'static str> {
    stop_wallet_sync_monitor(monitor.inner());
    let status = service
        .lock(Duration::from_secs(5))
        .await
        .map_err(|_| "wallet process could not be stopped; close the app before retrying")?;
    *active.0.lock().map_err(|_| "wallet state is unavailable")? = None;
    Ok(status)
}

fn require_active_wallet(active: &ActiveWalletState, id: &WalletId) -> Result<(), &'static str> {
    match active.0.lock() {
        Ok(guard) if guard.as_ref() == Some(id) => Ok(()),
        _ => Err("this wallet is not open"),
    }
}

#[tauri::command]
async fn wallet_backup_phrase(
    wallet_id: String,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
    service: tauri::State<'_, WalletService>,
) -> Result<SecretPhraseResponse, &'static str> {
    let id = WalletId::parse(wallet_id).map_err(|_| "wallet identifier is invalid")?;
    require_active_wallet(&active, &id)?;
    let paths = selected_paths(&state)?;
    if backup_complete(&paths, &id) {
        return Err("backup has already been acknowledged");
    }
    let phrase = service
        .recovery_phrase()
        .await
        .map_err(|_| "recovery phrase is unavailable")?
        .into_secret();
    Ok(SecretPhraseResponse(phrase))
}

#[tauri::command]
fn wallet_acknowledge_backup(
    wallet_id: String,
    state: tauri::State<'_, DataRootState>,
    active: tauri::State<'_, ActiveWalletState>,
) -> Result<(), &'static str> {
    let id = WalletId::parse(wallet_id).map_err(|_| "wallet identifier is invalid")?;
    require_active_wallet(&active, &id)?;
    let paths = selected_paths(&state)?;
    if !wallet_file_pair_exists(&paths, &id) {
        return Err("wallet files are unavailable");
    }
    let path = backup_ack_path(&paths, &id);
    if backup_complete(&paths, &id) {
        return Ok(());
    }
    let temporary = paths.wallet_dir(&id).join(format!(
        "backup-acknowledged.{}.new",
        uuid::Uuid::new_v4().simple()
    ));
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temporary)
        .map_err(|_| "backup acknowledgement could not be saved")?;
    let result = file
        .write_all(b"1")
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temporary, &path))
        .map_err(|_| "backup acknowledgement could not be saved");
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[tauri::command]
fn data_root_configuration(
    state: tauri::State<'_, DataRootState>,
) -> Result<DataRootConfiguration, &'static str> {
    state
        .0
        .lock()
        .map(|paths| DataRootConfiguration {
            root: paths.as_ref().map(|paths| paths.root().to_path_buf()),
        })
        .map_err(|_| "data location configuration is unavailable")
}

/// Opens a native folder picker in Rust. The WebView never supplies the
/// selected path, so it cannot redirect wallet storage through this command.
#[tauri::command]
async fn choose_data_root(
    app: tauri::AppHandle,
    state: tauri::State<'_, DataRootState>,
    service: tauri::State<'_, WalletService>,
) -> Result<bool, &'static str> {
    require_stopped(&service).await?;
    let picker = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Choose the folder for Ryo Wallet Next data")
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| "data location picker is unavailable")?;
    let Some(selected) = selected else {
        return Ok(false);
    };
    require_stopped(&service).await?;
    let root = selected
        .into_path()
        .map_err(|_| "selected data location is invalid")?;
    let root = fs::canonicalize(root).map_err(|_| "selected data location is unavailable")?;
    let paths = AppPaths::new(root.clone(), Network::Mainnet)
        .map_err(|_| "selected data location is invalid")?;
    paths
        .ensure_private_dirs()
        .map_err(|_| "selected data location cannot be secured")?;
    persist_data_root(&app, &root)?;
    let mut saved = state
        .0
        .lock()
        .map_err(|_| "data location configuration is unavailable")?;
    *saved = Some(paths);
    Ok(true)
}

#[tauri::command]
fn node_configuration(
    state: tauri::State<'_, DataRootState>,
) -> Result<Option<NodeConfig>, &'static str> {
    let paths = selected_paths(&state)?;
    load_settings_if_present(&paths)
        .map(|settings| settings.map(|settings| settings.node))
        .map_err(|_| "saved node configuration is unavailable or invalid")
}

#[tauri::command]
async fn save_node_selection(
    selection: NodeSelection,
    state: tauri::State<'_, DataRootState>,
    service: tauri::State<'_, WalletService>,
) -> Result<NodeConfig, &'static str> {
    require_stopped(&service).await?;
    let paths = selected_paths(&state)?;
    let node = match selection {
        NodeSelection::Local => NodeConfig::managed_local(Network::Mainnet),
        NodeSelection::Remote { host, port } => {
            NodeConfig::remote(Network::Mainnet, host.trim().to_owned(), port)
                .map_err(|_| "remote node host or port is invalid")?
        }
    };
    let previous = load_settings_if_present(&paths)
        .map_err(|_| "saved node configuration is unavailable or invalid")?;
    let settings = AppSettings {
        node: node.clone(),
        idle_lock_seconds: previous
            .as_ref()
            .map_or(300, |settings| settings.idle_lock_seconds),
        theme: previous.map_or(Theme::System, |settings| settings.theme),
    };
    save_settings(&paths, &settings).map_err(|_| "node configuration could not be saved")?;
    Ok(node)
}

fn selected_paths(state: &DataRootState) -> Result<AppPaths, &'static str> {
    state
        .0
        .lock()
        .map_err(|_| "data location configuration is unavailable")?
        .clone()
        .ok_or("choose a data location first")
}

async fn require_stopped(service: &WalletService) -> Result<(), &'static str> {
    match service.is_idle().await {
        Ok(true) => Ok(()),
        _ => Err("lock and stop the wallet before changing setup"),
    }
}

pub fn run() {
    let context = tauri::generate_context!();
    #[cfg(debug_assertions)]
    let context = {
        let mut context = context;
        context.config_mut().identifier = "io.github.ucrem.ryowalletnext.dev".to_owned();
        context.config_mut().product_name = Some("Ryo Wallet Next Dev".to_owned());
        context
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(WalletService::new());
            app.manage(DataRootState::load(&app.handle())?);
            app.manage(ActiveWalletState(Mutex::new(None)));
            app.manage(SyncMonitorState(AtomicU64::new(0)));
            app.manage(app_updates::InstallState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            app_updates::app_update_check,
            app_updates::app_update_install,
            wallet_overview,
            wallet_receive_addresses,
            wallet_create_receive_address,
            wallet_sync_status,
            wallet_runtime_ready,
            wallet_list,
            wallet_active,
            wallet_create,
            wallet_open,
            wallet_lock,
            wallet_backup_phrase,
            wallet_acknowledge_backup,
            data_root_configuration,
            choose_data_root,
            node_configuration,
            save_node_selection
        ])
        .run(context)
        .expect("failed to start Ryo Wallet Next");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_result_serializes_the_phrase_without_storing_it_in_settings() {
        use ryo_wallet_service::application::LifecycleState;

        let response = CreatedWalletResponse {
            wallet_id: "0123456789abcdef0123456789abcdef".to_owned(),
            status: LifecycleStatus {
                state: LifecycleState::Open,
                session_generation: "1".to_owned(),
            },
            recovery_phrase: Zeroizing::new("disposable words".to_owned()),
        };
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["wallet_id"], "0123456789abcdef0123456789abcdef");
        assert_eq!(value["status"]["state"], "open");
        assert_eq!(value["recovery_phrase"], "disposable words");
    }

    #[test]
    fn wallet_discovery_requires_an_app_owned_file_pair_and_backup_ack() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_path_buf(), Network::Mainnet).unwrap();
        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        let directory = paths.create_wallet_dir(&id).unwrap();
        fs::write(directory.join("wallet"), b"test").unwrap();
        assert!(!wallet_file_pair_exists(&paths, &id));
        fs::write(directory.join("wallet.keys"), b"test").unwrap();
        assert!(wallet_file_pair_exists(&paths, &id));
        assert!(!backup_complete(&paths, &id));
        fs::write(backup_ack_path(&paths, &id), b"1").unwrap();
        assert!(backup_complete(&paths, &id));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_wallet_files_are_not_listed() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_path_buf(), Network::Mainnet).unwrap();
        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        let directory = paths.create_wallet_dir(&id).unwrap();
        fs::write(directory.join("wallet"), b"test").unwrap();
        symlink(directory.join("wallet"), directory.join("wallet.keys")).unwrap();
        assert!(!wallet_file_pair_exists(&paths, &id));
    }
}
