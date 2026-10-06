use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ryo_wallet_service::application::{LifecycleState, WalletService};
use ryo_wallet_service::domain::{Network, NodeConfig, NodeMode, NodeOptions};
use ryo_wallet_service::storage::{
    AppPaths, AppSettings, Preferences, Theme, idle_expired, load_settings_if_present,
    save_settings,
};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    ActiveWalletState, DataRootState, SetupState, ShutdownState, SyncMonitorState,
    persist_data_root, require_app_running, require_node_stopped, require_stopped, selected_paths,
    stop_wallet_sync_monitor,
};

pub struct PreferencesState(pub Mutex<Preferences>);
pub struct ActivityState(pub AtomicU64);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn preferences_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, &'static str> {
    app.path()
        .app_config_dir()
        .map(|path| path.join("preferences.json"))
        .map_err(|_| "preferences storage is unavailable")
}

fn write_preferences(path: &Path, prefs: &Preferences) -> Result<(), &'static str> {
    prefs.validate()?;
    let parent = path.parent().ok_or("preferences storage is unavailable")?;
    fs::create_dir_all(parent).map_err(|_| "preferences storage is unavailable")?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "preferences could not be saved")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "preferences could not be secured")?;
    }
    serde_json::to_writer(file.as_file_mut(), prefs)
        .map_err(|_| "preferences could not be saved")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "preferences could not be saved")?;
    file.persist(path)
        .map_err(|_| "preferences could not be saved")?;
    Ok(())
}

impl PreferencesState {
    pub fn load(app: &tauri::AppHandle) -> Result<Self, &'static str> {
        let recovery = app.state::<crate::configuration_recovery::RecoveryState>();
        let prefs = match crate::configuration_recovery::load(
            &preferences_path(app)?,
            |prefs: &Preferences| prefs.validate().is_ok(),
            crate::configuration_recovery::Kind::Preferences,
            &recovery,
        ) {
            Some(prefs) => prefs,
            None => {
                let mut prefs = Preferences::default();
                if let Ok(paths) = selected_paths(&app.state::<DataRootState>())
                    && let Some(old) = crate::configuration_recovery::load(
                        &paths.network_root().join("settings.json"),
                        |settings: &AppSettings| settings.validate().is_ok(),
                        crate::configuration_recovery::Kind::NodeSettings,
                        &recovery,
                    )
                {
                    if old.theme != Theme::System {
                        prefs.theme = old.theme;
                    }
                    prefs.idle_lock_seconds = old.idle_lock_seconds;
                }
                prefs
            }
        };
        Ok(Self(Mutex::new(prefs)))
    }
    pub fn snapshot(&self) -> Result<Preferences, &'static str> {
        self.0
            .lock()
            .map(|prefs| prefs.clone())
            .map_err(|_| "preferences are unavailable")
    }
}

#[tauri::command]
pub fn app_preferences(
    app: tauri::AppHandle,
    prefs: tauri::State<'_, PreferencesState>,
) -> Result<Preferences, &'static str> {
    let mut snapshot = prefs.snapshot()?;
    snapshot.launch_on_startup = app
        .autolaunch()
        .is_enabled()
        .map_err(|_| "startup registration could not be checked")?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn save_app_preferences(
    app: tauri::AppHandle,
    preferences: Preferences,
) -> Result<Preferences, &'static str> {
    preferences.validate()?;
    let setup = app.state::<SetupState>();
    let _guard = setup.0.lock().await;
    require_app_running(&app)?;
    let previous = app.state::<PreferencesState>().snapshot()?;
    let tray = app
        .tray_by_id("wallet-next")
        .ok_or("system tray is unavailable")?;
    let startup = app.autolaunch();
    let was_enabled = startup
        .is_enabled()
        .map_err(|_| "startup registration could not be checked")?;
    let change_startup = |enabled| {
        if enabled {
            startup.enable()
        } else {
            startup.disable()
        }
    };
    if preferences.launch_on_startup != was_enabled {
        change_startup(preferences.launch_on_startup)
            .map_err(|_| "startup registration could not be changed; preferences were not saved")?;
    }
    let result = tray
        .set_visible(preferences.minimize_to_tray)
        .map_err(|_| "system tray could not be updated")
        .and_then(|()| write_preferences(&preferences_path(&app)?, &preferences));
    if let Err(error) = result {
        let _ = change_startup(was_enabled);
        let _ = tray.set_visible(previous.minimize_to_tray);
        return Err(error);
    }
    *app.state::<PreferencesState>()
        .0
        .lock()
        .map_err(|_| "preferences are unavailable")? = preferences.clone();
    apply_window_theme(&app, preferences.theme);
    let _ = app.emit("preferences-changed", preferences.clone());
    Ok(preferences)
}

pub fn apply_window_theme(app: &tauri::AppHandle, theme: Theme) {
    if let Some(window) = app.get_webview_window("main") {
        let theme = match theme {
            Theme::Dark => Some(tauri::Theme::Dark),
            Theme::Light => Some(tauri::Theme::Light),
            Theme::System => None,
        };
        let _ = window.set_theme(theme);
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneralSettings {
    mode: NodeMode,
    network: Network,
    host: String,
    port: u16,
    advanced: NodeOptions,
}

impl GeneralSettings {
    fn node(&self) -> Result<NodeConfig, &'static str> {
        let mut node = match self.mode {
            NodeMode::Local => Ok(NodeConfig::managed_local(self.network)),
            NodeMode::Remote => {
                NodeConfig::remote(self.network, self.host.trim().to_owned(), self.port)
            }
            NodeMode::Hybrid => {
                NodeConfig::hybrid(self.network, self.host.trim().to_owned(), self.port)
            }
        }
        .map_err(|_| "remote host or port is invalid")?;
        node.advanced = Some(self.advanced.clone());
        node.validate()
            .map_err(|_| "advanced node options are invalid")?;
        Ok(node)
    }
}

#[tauri::command]
pub async fn save_general_settings(
    app: tauri::AppHandle,
    settings: GeneralSettings,
) -> Result<NodeConfig, &'static str> {
    let node = settings.node()?;
    let setup = app.state::<SetupState>();
    let _guard = setup.0.lock().await;
    require_app_running(&app)?;
    require_stopped(&app.state::<WalletService>()).await?;
    require_node_stopped(&app.state::<ryo_wallet_service::application::NodeService>()).await?;
    let old = selected_paths(&app.state::<DataRootState>())?;
    let paths = AppPaths::new(old.root().to_path_buf(), settings.network)
        .map_err(|_| "network storage is invalid")?;
    let previous =
        load_settings_if_present(&paths).map_err(|_| "saved node settings are invalid")?;
    let prefs = app.state::<PreferencesState>().snapshot()?;
    let saved = AppSettings {
        node: node.clone(),
        theme: prefs.theme,
        idle_lock_seconds: prefs.idle_lock_seconds.max(60),
    };
    save_settings(&paths, &saved).map_err(|_| "node settings could not be saved")?;
    if old.network() != settings.network {
        if let Err(error) = persist_data_root(&app, old.root(), settings.network) {
            if let Some(previous) = previous {
                let _ = save_settings(&paths, &previous);
            }
            return Err(error);
        }
        *app.state::<DataRootState>()
            .0
            .lock()
            .map_err(|_| "data location state is unavailable")? = Some(paths);
        *app.state::<ActiveWalletState>()
            .0
            .lock()
            .map_err(|_| "wallet state is unavailable")? = None;
    }
    Ok(node)
}

#[tauri::command]
pub async fn app_activity(
    app: tauri::AppHandle,
    session_generation: String,
) -> Result<(), &'static str> {
    let status = app
        .state::<WalletService>()
        .status()
        .await
        .map_err(|_| "wallet status is unavailable")?;
    if status.state == LifecycleState::Open && status.session_generation == session_generation {
        app.state::<ActivityState>()
            .0
            .store(now_ms(), Ordering::Release);
    }
    Ok(())
}

pub fn start_idle_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut generation = String::new();
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if app.state::<ShutdownState>().closing.load(Ordering::Acquire) {
                break;
            }
            if require_app_running(&app).is_err() {
                continue;
            }
            let service = app.state::<WalletService>();
            let Ok(status) = service.status().await else {
                continue;
            };
            if status.state != LifecycleState::Open {
                generation.clear();
                continue;
            }
            let activity = app.state::<ActivityState>();
            if generation != status.session_generation {
                generation = status.session_generation.clone();
                activity.0.store(now_ms(), Ordering::Release);
                continue;
            }
            let Ok(prefs) = app.state::<PreferencesState>().snapshot() else {
                continue;
            };
            if !idle_expired(
                activity.0.load(Ordering::Acquire),
                now_ms(),
                prefs.idle_lock_seconds,
            ) {
                continue;
            }
            if let Ok(locked) = service
                .lock_current(Duration::from_secs(5), generation.clone())
                .await
            {
                stop_wallet_sync_monitor(&app.state::<SyncMonitorState>());
                if let Ok(mut active) = app.state::<ActiveWalletState>().0.lock() {
                    *active = None;
                }
                let _ = app.emit("wallet-auto-locked", locked);
            }
        }
    });
}

pub fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let open = MenuItem::with_id(app, "open", "Open Ryo Wallet Next", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Lock wallet", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &lock, &exit])?;
    let mut builder = TrayIconBuilder::with_id("wallet-next")
        .tooltip("Ryo Wallet Next")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "exit" => app.exit(0),
            "lock" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let service = app.state::<WalletService>();
                    if let Ok(status) = service.status().await
                        && let Ok(locked) = service
                            .lock_current(Duration::from_secs(5), status.session_generation)
                            .await
                    {
                        stop_wallet_sync_monitor(&app.state::<SyncMonitorState>());
                        if let Ok(mut active) = app.state::<ActiveWalletState>().0.lock() {
                            *active = None;
                        }
                        let _ = app.emit("wallet-auto-locked", locked);
                    }
                });
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    let enabled = app
        .state::<PreferencesState>()
        .snapshot()
        .map(|prefs| prefs.minimize_to_tray)
        .unwrap_or(false);
    tray.set_visible(enabled)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_persist_replace_and_recover_malformed_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        let state = crate::configuration_recovery::RecoveryState::default();
        let read = || {
            crate::configuration_recovery::load(
                &path,
                |prefs: &Preferences| prefs.validate().is_ok(),
                crate::configuration_recovery::Kind::Preferences,
                &state,
            )
        };
        assert_eq!(read(), None);
        let mut prefs = Preferences::default();
        write_preferences(&path, &prefs).unwrap();
        prefs.theme = Theme::Light;
        prefs.idle_lock_seconds = 0;
        write_preferences(&path, &prefs).unwrap();
        assert_eq!(read(), Some(prefs));
        fs::write(&path, b"invalid").unwrap();
        assert_eq!(read(), None);
        assert!(state.messages()[0].contains("backed up"));
    }
    #[test]
    fn general_settings_rebuild_trust_and_validate_bootstrap_and_ports() {
        let mut settings = GeneralSettings {
            mode: NodeMode::Hybrid,
            network: Network::Mainnet,
            host: "bootstrap.example.org".into(),
            port: 12211,
            advanced: NodeOptions::default(),
        };
        let node = settings.node().unwrap();
        assert!(node.uses_local());
        assert_eq!(node.host, "127.0.0.1");
        assert_eq!(node.bootstrap.unwrap().host, settings.host);
        settings.host = "user@host/path".into();
        assert!(settings.node().is_err());
        settings.mode = NodeMode::Local;
        settings.advanced.rpc_port = 100;
        assert!(settings.node().is_err());
    }
}
