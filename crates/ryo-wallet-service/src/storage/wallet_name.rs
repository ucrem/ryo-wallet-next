//! A display-only name for the locked wallet picker. The encrypted wallet's
//! `next.name` attribute remains authoritative; no wallet secrets belong here.
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

const NAME_FILE: &str = "wallet-name-v1.json";
const MAX_FILE_BYTES: u64 = 512;

#[derive(Debug, thiserror::Error)]
#[error("wallet display name could not be saved or read")]
pub struct WalletNameError;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DisplayName {
    name: String,
}

fn valid_name(name: &str) -> bool {
    name.len() <= 100 && !name.chars().any(char::is_control)
}

pub fn load_wallet_name(directory: &Path) -> Result<Option<String>, WalletNameError> {
    let path = directory.join(NAME_FILE);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= MAX_FILE_BYTES => {}
        _ => return Err(WalletNameError),
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| WalletNameError)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WalletNameError)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(WalletNameError);
    }
    let value: DisplayName = serde_json::from_slice(&bytes).map_err(|_| WalletNameError)?;
    if !valid_name(&value.name) {
        return Err(WalletNameError);
    }
    Ok((!value.name.trim().is_empty()).then_some(value.name))
}

pub fn save_wallet_name(directory: &Path, name: &str) -> Result<(), WalletNameError> {
    if !valid_name(name)
        || !fs::symlink_metadata(directory)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return Err(WalletNameError);
    }
    let expected = (!name.trim().is_empty()).then_some(name);
    if load_wallet_name(directory).is_ok_and(|cached| cached.as_deref() == expected) {
        return Ok(());
    }
    let path = directory.join(NAME_FILE);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
        _ => return Err(WalletNameError),
    }
    let temporary = directory.join(format!("wallet-name.{}.new", uuid::Uuid::new_v4().simple()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options.open(&temporary).map_err(|_| WalletNameError)?;
        let bytes =
            serde_json::to_vec(&DisplayName { name: name.into() }).map_err(|_| WalletNameError)?;
        file.write_all(&bytes).map_err(|_| WalletNameError)?;
        file.sync_all().map_err(|_| WalletNameError)?;
        fs::rename(&temporary, &path).map_err(|_| WalletNameError)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Network;
    use crate::storage::{AppPaths, WalletId};

    #[test]
    fn names_survive_fresh_reads_and_remain_isolated_by_wallet_and_network() {
        let temp = tempfile::tempdir().unwrap();
        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        let mainnet = AppPaths::new(temp.path().into(), Network::Mainnet).unwrap();
        let testnet = AppPaths::new(temp.path().into(), Network::Testnet).unwrap();
        let directory = mainnet.create_wallet_dir(&id).unwrap();
        let second_id = WalletId::parse("abcdef0123456789abcdef0123456789").unwrap();
        let second = mainnet.create_wallet_dir(&second_id).unwrap();
        let other = testnet.create_wallet_dir(&id).unwrap();
        assert_eq!(load_wallet_name(&directory).unwrap(), None);
        save_wallet_name(&directory, "Mining wallet").unwrap();
        assert_eq!(
            load_wallet_name(&directory).unwrap().as_deref(),
            Some("Mining wallet")
        );
        assert_eq!(load_wallet_name(&other).unwrap(), None);
        assert_eq!(load_wallet_name(&second).unwrap(), None);
        save_wallet_name(&directory, "Savings").unwrap();
        assert_eq!(
            load_wallet_name(&directory).unwrap().as_deref(),
            Some("Savings")
        );
        save_wallet_name(&directory, "").unwrap();
        assert_eq!(load_wallet_name(&directory).unwrap(), None);
    }

    #[test]
    fn malformed_or_oversized_names_are_rejected_without_changing_a_saved_name() {
        let temp = tempfile::tempdir().unwrap();
        save_wallet_name(temp.path(), "Valid name").unwrap();
        for name in ["x".repeat(101), "Invalid\nname".into()] {
            assert!(save_wallet_name(temp.path(), &name).is_err());
        }
        assert_eq!(
            load_wallet_name(temp.path()).unwrap().as_deref(),
            Some("Valid name")
        );
        fs::write(
            temp.path().join(NAME_FILE),
            br#"{"name":"Test","password":"unexpected"}"#,
        )
        .unwrap();
        assert!(load_wallet_name(temp.path()).is_err());
        fs::write(temp.path().join(NAME_FILE), vec![b'x'; 513]).unwrap();
        assert!(load_wallet_name(temp.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_name_files_are_rejected_and_new_files_are_owner_only() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        save_wallet_name(temp.path(), "Private").unwrap();
        let path = temp.path().join(NAME_FILE);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_file(&path).unwrap();
        let target = temp.path().join("outside");
        fs::write(&target, b"untouched").unwrap();
        symlink(&target, &path).unwrap();
        assert!(load_wallet_name(temp.path()).is_err());
        assert!(save_wallet_name(temp.path(), "Changed").is_err());
        assert_eq!(fs::read(target).unwrap(), b"untouched");
    }
}
