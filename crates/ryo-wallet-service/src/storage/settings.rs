use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{NodeConfig, NodeConfigError};

use super::{AppPaths, PathError};

const MIN_IDLE_LOCK_SECONDS: u16 = 60;
const MAX_IDLE_LOCK_SECONDS: u16 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub node: NodeConfig,
    pub idle_lock_seconds: u16,
    pub theme: Theme,
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings could not be stored in a private application directory")]
    Paths(#[source] PathError),
    #[error("settings file is missing or malformed")]
    InvalidFile,
    #[error("settings contain an invalid node configuration")]
    InvalidNode(#[source] NodeConfigError),
    #[error("idle-lock duration must be between 60 seconds and one hour")]
    InvalidIdleLock,
    #[error("settings could not be written atomically")]
    Io(#[source] std::io::Error),
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), SettingsError> {
        self.node.validate().map_err(SettingsError::InvalidNode)?;
        if !(MIN_IDLE_LOCK_SECONDS..=MAX_IDLE_LOCK_SECONDS).contains(&self.idle_lock_seconds) {
            return Err(SettingsError::InvalidIdleLock);
        }
        Ok(())
    }
}

pub fn load_settings(paths: &AppPaths) -> Result<AppSettings, SettingsError> {
    load_settings_if_present(paths)?.ok_or(SettingsError::InvalidFile)
}

/// A missing settings file means onboarding has not been completed. Existing
/// malformed settings are errors, so they cannot be silently overwritten.
pub fn load_settings_if_present(paths: &AppPaths) -> Result<Option<AppSettings>, SettingsError> {
    let bytes = match fs::read(settings_path(paths)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(SettingsError::InvalidFile),
    };
    let settings: AppSettings =
        serde_json::from_slice(&bytes).map_err(|_| SettingsError::InvalidFile)?;
    settings.validate()?;
    Ok(Some(settings))
}

/// Writes only non-secret configuration. Passwords, seeds, RPC credentials,
/// signed metadata, and wallet paths have no representation in this type.
pub fn save_settings(paths: &AppPaths, settings: &AppSettings) -> Result<(), SettingsError> {
    settings.validate()?;
    paths.ensure_private_dirs().map_err(SettingsError::Paths)?;
    let final_path = settings_path(paths);
    let temporary_path = paths.network_root().join("settings.json.new");
    let bytes = serde_json::to_vec(settings).map_err(|_| SettingsError::InvalidFile)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(SettingsError::Io)?;
    let write_result = (|| {
        let mut writer = BufWriter::new(file);
        writer.write_all(&bytes).map_err(SettingsError::Io)?;
        writer.flush().map_err(SettingsError::Io)?;
        writer.get_ref().sync_all().map_err(SettingsError::Io)?;
        fs::rename(&temporary_path, final_path).map_err(SettingsError::Io)
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(temporary_path);
    }
    write_result
}

fn settings_path(paths: &AppPaths) -> PathBuf {
    paths.network_root().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Network, NodeConfig};

    fn paths(temp: &tempfile::TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("app-data"), Network::Mainnet).unwrap()
    }

    fn settings() -> AppSettings {
        AppSettings {
            node: NodeConfig::managed_local(Network::Mainnet),
            idle_lock_seconds: 300,
            theme: Theme::System,
        }
    }

    #[test]
    fn settings_round_trip_without_wallet_secrets() {
        let temp = tempfile::tempdir().unwrap();
        let app_paths = paths(&temp);
        save_settings(&app_paths, &settings()).unwrap();
        assert_eq!(load_settings(&app_paths).unwrap(), settings());
        let serialized = fs::read_to_string(settings_path(&app_paths)).unwrap();
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("seed"));
    }

    #[test]
    fn invalid_idle_lock_and_invalid_json_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let app_paths = paths(&temp);
        let mut invalid = settings();
        invalid.idle_lock_seconds = 59;
        assert!(matches!(
            save_settings(&app_paths, &invalid),
            Err(SettingsError::InvalidIdleLock)
        ));

        app_paths.ensure_private_dirs().unwrap();
        fs::write(settings_path(&app_paths), b"not json").unwrap();
        assert!(matches!(
            load_settings(&app_paths),
            Err(SettingsError::InvalidFile)
        ));
        assert!(matches!(
            load_settings_if_present(&app_paths),
            Err(SettingsError::InvalidFile)
        ));
    }

    #[test]
    fn missing_settings_are_unconfigured() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(load_settings_if_present(&paths(&temp)).unwrap(), None);
    }
}
