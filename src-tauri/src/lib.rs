use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use ryo_wallet_service::application::{
    LifecycleState, LifecycleStatus, WalletOverview, WalletService,
};
use ryo_wallet_service::domain::{Network, NodeConfig};
use ryo_wallet_service::storage::{
    AppPaths, AppSettings, Theme, load_settings_if_present, save_settings,
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

const DATA_ROOT_SELECTION_FILE: &str = "wallet-data-root.json";

/// This is deliberately separate from wallet settings: it is only the
/// user-approved base directory, kept in Tauri's private app config area.
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedDataRoot {
    root: PathBuf,
}

struct DataRootState(Mutex<Option<AppPaths>>);

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
    match service.status().await {
        Ok(status) if status.state == LifecycleState::Stopped => Ok(()),
        _ => Err("lock and stop the wallet before changing setup"),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(WalletService::new());
            app.manage(DataRootState::load(&app.handle())?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            wallet_overview,
            data_root_configuration,
            choose_data_root,
            node_configuration,
            save_node_selection
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Ryo Wallet Next");
}
