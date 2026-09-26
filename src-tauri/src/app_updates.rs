use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
#[cfg(target_os = "linux")]
use tauri::utils::{config::BundleType, platform::bundle_type};
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
        install_in_app: bool,
    },
}

#[derive(Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxPackage {
    Deb,
    Rpm,
}

#[cfg(target_os = "linux")]
impl LinuxPackage {
    fn installed() -> Option<Self> {
        Self::from_bundle_type(bundle_type())
    }

    fn from_bundle_type(bundle: Option<BundleType>) -> Option<Self> {
        match bundle {
            Some(BundleType::Deb) => Some(Self::Deb),
            Some(BundleType::Rpm) => Some(Self::Rpm),
            _ => None,
        }
    }

    fn feed(self) -> &'static str {
        match self {
            Self::Deb => {
                "https://raw.githubusercontent.com/ucrem/ryo-wallet-next/update-channel/latest-deb.json"
            }
            Self::Rpm => {
                "https://raw.githubusercontent.com/ucrem/ryo-wallet-next/update-channel/latest-rpm.json"
            }
        }
    }

    fn installer(self) -> &'static str {
        match self {
            Self::Deb => "/usr/bin/apt-get",
            Self::Rpm => "/usr/bin/dnf",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Deb => "deb",
            Self::Rpm => "rpm",
        }
    }

    fn matches_format(self, bytes: &[u8]) -> bool {
        match self {
            Self::Deb => bytes.starts_with(b"!<arch>\n"),
            Self::Rpm => bytes.starts_with(&[0xed, 0xab, 0xee, 0xdb]),
        }
    }
}

fn build_updater(
    app: &AppHandle,
    timeout: Duration,
) -> Result<tauri_plugin_updater::Updater, &'static str> {
    let builder = app.updater_builder().timeout(timeout);
    #[cfg(target_os = "linux")]
    let builder = {
        let package =
            LinuxPackage::installed().ok_or("updates require an installed DEB or RPM package")?;
        let feed = package
            .feed()
            .parse()
            .map_err(|_| "update feed is invalid")?;
        let previous_feed =
            "https://raw.githubusercontent.com/ucrem/ryo-wallet-next/update-channel/latest.json"
                .parse()
                .map_err(|_| "update feed is invalid")?;
        builder
            .endpoints(vec![feed, previous_feed])
            .map_err(|_| "update feed is invalid")?
    };
    builder.build().map_err(|_| "update service is unavailable")
}

#[tauri::command]
pub async fn app_update_check(app: AppHandle) -> Result<UpdateCheck, &'static str> {
    if cfg!(debug_assertions) {
        return Ok(UpdateCheck::Development);
    }
    let updater = build_updater(&app, Duration::from_secs(15))?;
    match updater
        .check()
        .await
        .map_err(|_| "could not check for updates")?
    {
        Some(update) => Ok(UpdateCheck::Available {
            version: update.version,
            notes: update.body,
            install_in_app: true,
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
    if cfg!(debug_assertions) {
        return Err("installation is unavailable in development");
    }
    #[cfg(target_os = "linux")]
    let package =
        LinuxPackage::installed().ok_or("updates require an installed DEB or RPM package")?;
    if install.0.swap(true, Ordering::AcqRel) {
        return Err("an update is already being installed");
    }
    let _guard = InstallGuard(&install.0);
    let updater = build_updater(&app, Duration::from_secs(180))?;
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

    #[cfg(target_os = "linux")]
    if !package.matches_format(&bytes) {
        return Err("the signed update is not a valid package for this installation");
    }

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

    #[cfg(target_os = "linux")]
    install_linux_package(package, &bytes).await?;
    #[cfg(not(target_os = "linux"))]
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

#[cfg(target_os = "linux")]
async fn install_linux_package(package: LinuxPackage, bytes: &[u8]) -> Result<(), &'static str> {
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};

    if !Path::new("/usr/bin/pkexec").is_file() || !Path::new(package.installer()).is_file() {
        return Err("system package installer or authorization service is unavailable");
    }
    let directory = tempfile::Builder::new()
        .prefix("ryo-wallet-update-")
        .tempdir()
        .map_err(|_| "update package could not be staged")?;
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .map_err(|_| "update package could not be secured")?;
    let path = directory
        .path()
        .join(format!("ryo-wallet-next-update.{}", package.extension()));
    fs::write(&path, bytes).map_err(|_| "update package could not be staged")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|_| "update package could not be secured")?;
    // The operating system owns the privilege prompt. No administrator
    // password is collected by the wallet or sent through the WebView.
    let status = tokio::process::Command::new("/usr/bin/pkexec")
        .arg(package.installer())
        .arg("install")
        .arg("-y")
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|_| "system authorization could not be started")?;
    if !status.success() {
        return Err("system package installation was cancelled or failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn linux_package_feeds_and_installers_are_explicit() {
        assert_eq!(
            LinuxPackage::from_bundle_type(Some(BundleType::Deb)),
            Some(LinuxPackage::Deb)
        );
        assert_eq!(
            LinuxPackage::from_bundle_type(Some(BundleType::Rpm)),
            Some(LinuxPackage::Rpm)
        );
        assert_eq!(
            LinuxPackage::from_bundle_type(Some(BundleType::AppImage)),
            None
        );
        assert!(LinuxPackage::Deb.feed().ends_with("latest-deb.json"));
        assert!(LinuxPackage::Rpm.feed().ends_with("latest-rpm.json"));
        assert_eq!(LinuxPackage::Deb.installer(), "/usr/bin/apt-get");
        assert_eq!(LinuxPackage::Rpm.installer(), "/usr/bin/dnf");
        assert!(LinuxPackage::Deb.matches_format(b"!<arch>\npackage"));
        assert!(LinuxPackage::Rpm.matches_format(&[0xed, 0xab, 0xee, 0xdb]));
        assert!(!LinuxPackage::Deb.matches_format(b"not a deb"));
    }
}
