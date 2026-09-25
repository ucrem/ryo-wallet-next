use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::UpdaterExt;

use crate::{ActiveWalletState, SyncMonitorState, WalletService, stop_wallet_sync_monitor};

#[derive(Default)]
pub struct InstallState(AtomicBool);

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateCheck {
    Development,
    Current,
    Available {
        version: String,
        notes: Option<String>,
        automatic_install: bool,
    },
}

#[derive(Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn automatic_install_supported() -> bool {
    install_allowed(
        cfg!(debug_assertions),
        cfg!(target_os = "linux"),
        std::env::var_os("APPIMAGE").is_some(),
    )
}

fn install_allowed(development: bool, linux: bool, running_appimage: bool) -> bool {
    // The Linux updater replaces the running AppImage. DEB/RPM packages remain
    // owned by the distribution's package manager.
    !development && (!linux || running_appimage)
}

#[tauri::command]
pub async fn app_update_check(app: AppHandle) -> Result<UpdateCheck, &'static str> {
    if cfg!(debug_assertions) {
        return Ok(UpdateCheck::Development);
    }
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| "update service is unavailable")?;
    match updater
        .check()
        .await
        .map_err(|_| "could not check for updates")?
    {
        Some(update) => Ok(UpdateCheck::Available {
            version: update.version,
            notes: update.body,
            automatic_install: automatic_install_supported(),
        }),
        None => Ok(UpdateCheck::Current),
    }
}

struct InstallGuard<'a>(&'a AtomicBool);

impl Drop for InstallGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub async fn app_update_install(
    expected_version: String,
    app: AppHandle,
    install: State<'_, InstallState>,
    service: State<'_, WalletService>,
    active: State<'_, ActiveWalletState>,
    monitor: State<'_, SyncMonitorState>,
) -> Result<(), &'static str> {
    if !automatic_install_supported() {
        return Err("automatic installation is unavailable for this package");
    }
    if install.0.swap(true, Ordering::AcqRel) {
        return Err("an update is already being installed");
    }
    let _guard = InstallGuard(&install.0);
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| "update service is unavailable")?;
    let update = updater
        .check()
        .await
        .map_err(|_| "could not verify the available update")?
        .ok_or("the update is no longer available")?;
    if update.version != expected_version {
        return Err("the available update changed; check again");
    }

    let mut downloaded = 0_u64;
    let progress_app = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ =
                    progress_app.emit("app-update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|_| "update download or signature verification failed")?;

    if !service
        .is_idle()
        .await
        .map_err(|_| "wallet state is unavailable")?
    {
        service
            .lock(Duration::from_secs(5))
            .await
            .map_err(|_| "lock the wallet before installing the update")?;
        stop_wallet_sync_monitor(monitor.inner());
        *active.0.lock().map_err(|_| "wallet state is unavailable")? = None;
    }

    update
        .install(bytes)
        .map_err(|_| "the signed update could not be installed")?;
    #[cfg(not(target_os = "windows"))]
    {
        app.restart()
    }
    #[cfg(target_os = "windows")]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_installation_matches_the_package_format() {
        assert!(!install_allowed(true, true, true));
        assert!(!install_allowed(false, true, false));
        assert!(install_allowed(false, true, true));
        assert!(install_allowed(false, false, false));
    }
}
