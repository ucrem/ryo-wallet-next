use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::{AppPaths, PathError, WalletId};

const MAX_WALLET_BYTES: u64 = 512 * 1024 * 1024;
const MAX_KEYS_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("imported wallet path must be absolute")]
    RelativeSource,
    #[error("select the wallet file, not its .keys companion")]
    SelectedKeyFile,
    #[error("wallet file or companion is missing, not a regular file, or is a symlink")]
    UnsafeSource,
    #[error("wallet .keys companion is missing")]
    MissingKeys,
    #[error("wallet file exceeds the import size limit")]
    WalletTooLarge,
    #[error("wallet .keys companion exceeds the import size limit")]
    KeysTooLarge,
    #[error("could not create private imported-wallet storage")]
    Destination(#[source] PathError),
    #[error("could not copy imported wallet data")]
    Io(#[source] std::io::Error),
}

/// The app-owned paths created by a completed import. Their names deliberately
/// do not reveal the original filename or location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedWallet {
    pub wallet_path: PathBuf,
    pub keys_path: PathBuf,
}

/// Copies an existing wallet pair to private storage. No source is renamed,
/// deleted, or opened by a wallet engine here.
pub fn copy_wallet_pair(
    app_paths: &AppPaths,
    wallet_id: &WalletId,
    source_wallet: &Path,
) -> Result<ImportedWallet, ImportError> {
    if !source_wallet.is_absolute() {
        return Err(ImportError::RelativeSource);
    }
    if source_wallet
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with(".keys"))
    {
        return Err(ImportError::SelectedKeyFile);
    }
    let source_keys = companion_keys_path(source_wallet);
    validate_source(source_wallet, MAX_WALLET_BYTES, ImportError::WalletTooLarge)?;
    if !source_keys.exists() {
        return Err(ImportError::MissingKeys);
    }
    validate_source(&source_keys, MAX_KEYS_BYTES, ImportError::KeysTooLarge)?;

    app_paths
        .ensure_private_dirs()
        .map_err(ImportError::Destination)?;
    let destination_dir = app_paths.wallet_dir(wallet_id);
    fs::create_dir(&destination_dir).map_err(ImportError::Io)?;
    set_owner_only(&destination_dir).map_err(ImportError::Io)?;

    let wallet_path = destination_dir.join("wallet");
    let keys_path = destination_dir.join("wallet.keys");
    let copy_result = (|| {
        copy_file_bounded(source_wallet, &wallet_path, MAX_WALLET_BYTES, false)?;
        copy_file_bounded(&source_keys, &keys_path, MAX_KEYS_BYTES, true)?;
        Ok(ImportedWallet {
            wallet_path,
            keys_path,
        })
    })();
    if copy_result.is_err() {
        let _ = fs::remove_dir_all(&destination_dir);
    }
    copy_result
}

fn companion_keys_path(wallet: &Path) -> PathBuf {
    let mut value: OsString = wallet.as_os_str().to_owned();
    value.push(".keys");
    PathBuf::from(value)
}

fn validate_source(
    path: &Path,
    maximum_size: u64,
    oversized: ImportError,
) -> Result<(), ImportError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ImportError::UnsafeSource)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ImportError::UnsafeSource);
    }
    if metadata.len() > maximum_size {
        return Err(oversized);
    }
    Ok(())
}

fn copy_file_bounded(
    source: &Path,
    destination: &Path,
    maximum_size: u64,
    is_keys: bool,
) -> Result<(), ImportError> {
    let source_file = File::open(source).map_err(ImportError::Io)?;
    let metadata = source_file.metadata().map_err(ImportError::Io)?;
    if !metadata.is_file() {
        return Err(ImportError::UnsafeSource);
    }
    let destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(ImportError::Io)?;
    let mut reader = BufReader::new(source_file);
    let mut writer = BufWriter::new(destination_file);
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(ImportError::Io)?;
        if read == 0 {
            break;
        }
        copied = copied.saturating_add(read as u64);
        if copied > maximum_size {
            return Err(if is_keys {
                ImportError::KeysTooLarge
            } else {
                ImportError::WalletTooLarge
            });
        }
        writer.write_all(&buffer[..read]).map_err(ImportError::Io)?;
    }
    writer.flush().map_err(ImportError::Io)
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Network;

    fn paths(temp: &tempfile::TempDir) -> AppPaths {
        AppPaths::new(temp.path().join("app-data"), Network::Mainnet).unwrap()
    }

    fn id() -> WalletId {
        WalletId::parse("0123456789abcdef0123456789abcdef").unwrap()
    }

    #[test]
    fn import_copies_the_wallet_pair_without_modifying_originals() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("personal-wallet");
        fs::write(&source, b"encrypted wallet").unwrap();
        fs::write(companion_keys_path(&source), b"encrypted keys").unwrap();

        let imported = copy_wallet_pair(&paths(&temp), &id(), &source).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"encrypted wallet");
        assert_eq!(
            fs::read(companion_keys_path(&source)).unwrap(),
            b"encrypted keys"
        );
        assert_eq!(fs::read(imported.wallet_path).unwrap(), b"encrypted wallet");
        assert_eq!(fs::read(imported.keys_path).unwrap(), b"encrypted keys");
    }

    #[test]
    fn import_requires_the_wallet_and_keys_pair() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("wallet");
        fs::write(&source, b"wallet").unwrap();
        assert!(matches!(
            copy_wallet_pair(&paths(&temp), &id(), &source),
            Err(ImportError::MissingKeys)
        ));
        assert!(matches!(
            copy_wallet_pair(&paths(&temp), &id(), &companion_keys_path(&source)),
            Err(ImportError::SelectedKeyFile)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_a_symlinked_wallet_source() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        fs::write(&target, b"wallet").unwrap();
        let source = temp.path().join("wallet");
        symlink(&target, &source).unwrap();
        fs::write(companion_keys_path(&source), b"keys").unwrap();
        assert!(matches!(
            copy_wallet_pair(&paths(&temp), &id(), &source),
            Err(ImportError::UnsafeSource)
        ));
    }
}
