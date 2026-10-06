//! Recovery is confined to designated non-wallet configuration files.
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use serde::de::DeserializeOwned;

const MAX_CONFIG_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy)]
pub enum Kind {
    Preferences,
    DataLocation,
    NodeSettings,
}

#[derive(Default)]
pub struct RecoveryState(Mutex<Vec<(Kind, bool)>>);

impl RecoveryState {
    fn record(&self, kind: Kind, backed_up: bool) {
        if let Ok(mut notices) = self.0.lock() {
            notices.push((kind, backed_up));
        }
    }

    pub fn messages(&self) -> Vec<&'static str> {
        self.0.lock().map(|notices| notices.iter().map(|(kind, backed_up)| match (kind, backed_up) {
            (Kind::Preferences, true) => "Damaged preferences were backed up. Default preferences are active; review Settings.",
            (Kind::Preferences, false) => "Saved preferences could not be read and were left untouched. Default preferences are active; review Settings.",
            (Kind::DataLocation, true) => "The damaged data-location configuration was backed up. Choose your existing data folder in Setup to reopen your wallets.",
            (Kind::DataLocation, false) => "The saved data location could not be read and was left untouched. Choose your existing data folder in Setup to reopen your wallets.",
            (Kind::NodeSettings, true) => "Damaged node settings were backed up. Choose a node again in Setup before opening a wallet.",
            (Kind::NodeSettings, false) => "Saved node settings could not be read and were left untouched. Review the node configuration in Setup.",
        }).collect()).unwrap_or_else(|_| vec!["Startup configuration could not be read. Review Setup and Settings."])
    }
}

pub fn load<T: DeserializeOwned>(
    path: &Path,
    validate: impl FnOnce(&T) -> bool,
    kind: Kind,
    recovery: &RecoveryState,
) -> Option<T> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => {
            recovery.record(kind, false);
            return None;
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        recovery.record(kind, false);
        return None;
    }
    let mut bytes = Vec::new();
    let read = fs::File::open(path)
        .and_then(|file| file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes));
    if read.is_err() {
        recovery.record(kind, false);
        return None;
    }
    if bytes.len() as u64 <= MAX_CONFIG_BYTES
        && let Ok(value) = serde_json::from_slice(&bytes)
        && validate(&value)
    {
        return Some(value);
    }
    // Never discard an invalid configuration or touch wallet/key/cache files.
    let backup = path.with_extension(format!("invalid.{}.json", uuid::Uuid::new_v4().simple()));
    recovery.record(kind, fs::rename(path, backup).is_ok());
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryo_wallet_service::storage::Preferences;

    fn preferences(path: &Path, recovery: &RecoveryState) -> Option<Preferences> {
        load(
            path,
            |prefs: &Preferences| prefs.validate().is_ok(),
            Kind::Preferences,
            recovery,
        )
    }

    #[test]
    fn malformed_and_semantically_invalid_preferences_are_preserved_before_recovery() {
        for bytes in [b"{".as_slice(), b"{\"idle_lock_seconds\":59}".as_slice()] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("preferences.json");
            fs::write(&path, bytes).unwrap();
            let state = RecoveryState::default();
            assert!(preferences(&path, &state).is_none());
            assert!(!path.exists());
            let backup = fs::read_dir(dir.path())
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(fs::read(backup).unwrap(), bytes);
            assert_eq!(state.messages().len(), 1);
            assert!(state.messages()[0].contains("backed up"));
        }
    }

    #[test]
    fn valid_or_missing_configuration_does_not_produce_recovery_notices() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        let state = RecoveryState::default();
        assert!(preferences(&path, &state).is_none());
        fs::write(&path, b"{}").unwrap();
        assert_eq!(preferences(&path, &state), Some(Preferences::default()));
        assert_eq!(fs::read(path).unwrap(), b"{}");
        assert!(state.messages().is_empty());
    }

    #[test]
    fn oversized_configuration_is_preserved_and_directory_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        let bytes = vec![b' '; MAX_CONFIG_BYTES as usize + 1];
        fs::write(&path, &bytes).unwrap();
        let state = RecoveryState::default();
        assert!(preferences(&path, &state).is_none());
        assert_eq!(
            fs::read(
                fs::read_dir(dir.path())
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path()
            )
            .unwrap(),
            bytes
        );
        fs::create_dir(&path).unwrap();
        assert!(preferences(&path, &state).is_none());
        assert!(path.is_dir());
        assert!(state.messages().last().unwrap().contains("left untouched"));
    }

    #[cfg(unix)]
    #[test]
    fn linked_configuration_never_reads_or_moves_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("untouched");
        let path = dir.path().join("preferences.json");
        fs::write(&target, b"{}").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let state = RecoveryState::default();
        assert!(preferences(&path, &state).is_none());
        assert!(path.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read(target).unwrap(), b"{}");
    }
}
