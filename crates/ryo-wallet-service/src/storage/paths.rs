use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::Network;

use super::WalletId;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("application data root must be absolute")]
    RelativeRoot,
    #[error("could not create or secure a private application directory")]
    Io(#[source] std::io::Error),
}

/// Network-isolated, app-owned directories. These paths never point to an
/// imported original wallet file.
#[derive(Debug, Clone)]
pub struct AppPaths {
    root: PathBuf,
    network: Network,
}

impl AppPaths {
    pub fn new(root: PathBuf, network: Network) -> Result<Self, PathError> {
        if !root.is_absolute() {
            return Err(PathError::RelativeRoot);
        }
        Ok(Self { root, network })
    }

    pub fn ensure_private_dirs(&self) -> Result<(), PathError> {
        for path in [
            self.root().to_path_buf(),
            self.network_root(),
            self.wallets_root(),
            self.runtime_root(),
        ] {
            fs::create_dir_all(&path).map_err(PathError::Io)?;
            set_owner_only(&path).map_err(PathError::Io)?;
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn network_root(&self) -> PathBuf {
        self.root.join(network_segment(self.network))
    }

    pub fn wallets_root(&self) -> PathBuf {
        self.network_root().join("wallets")
    }

    pub fn runtime_root(&self) -> PathBuf {
        self.network_root().join("runtime")
    }

    pub fn wallet_dir(&self, wallet_id: &WalletId) -> PathBuf {
        self.wallets_root().join(wallet_id.as_str())
    }

    /// Allocates a fresh, app-owned directory before create or restore. It
    /// never overwrites a prior wallet directory.
    pub fn create_wallet_dir(&self, wallet_id: &WalletId) -> Result<PathBuf, PathError> {
        self.ensure_private_dirs()?;
        let directory = self.wallet_dir(wallet_id);
        fs::create_dir(&directory).map_err(PathError::Io)?;
        set_owner_only(&directory).map_err(PathError::Io)?;
        Ok(directory)
    }
}

fn network_segment(network: Network) -> &'static str {
    match network {
        Network::Mainnet => "mainnet",
        Network::Testnet => "testnet",
        Network::Stagenet => "stagenet",
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> std::io::Result<()> {
    // Windows ACL enforcement is an explicit platform adapter task. Do not
    // claim Unix modes provide equivalent protection there.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::WalletId;

    #[test]
    fn paths_are_network_isolated_and_cannot_use_traversal_ids() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_path_buf(), Network::Testnet).unwrap();
        paths.ensure_private_dirs().unwrap();
        assert!(paths.wallets_root().is_dir());
        assert!(paths.runtime_root().is_dir());
        assert!(
            !paths
                .network_root()
                .starts_with(temp.path().join("mainnet"))
        );

        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        assert_eq!(
            paths.wallet_dir(&id),
            temp.path()
                .join("testnet")
                .join("wallets")
                .join("0123456789abcdef0123456789abcdef")
        );
    }

    #[cfg(unix)]
    #[test]
    fn created_directories_are_not_group_or_world_accessible() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_path_buf(), Network::Mainnet).unwrap();
        paths.ensure_private_dirs().unwrap();
        for path in [
            paths.root().to_path_buf(),
            paths.network_root(),
            paths.wallets_root(),
            paths.runtime_root(),
        ] {
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        }
    }

    #[test]
    fn relative_data_roots_are_rejected() {
        assert!(matches!(
            AppPaths::new(PathBuf::from("wallet-data"), Network::Mainnet),
            Err(PathError::RelativeRoot)
        ));
    }

    #[test]
    fn wallet_directory_is_fresh_and_never_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(temp.path().to_path_buf(), Network::Mainnet).unwrap();
        let id = WalletId::parse("0123456789abcdef0123456789abcdef").unwrap();
        let directory = paths.create_wallet_dir(&id).unwrap();
        assert!(directory.is_dir());
        assert!(paths.create_wallet_dir(&id).is_err());
    }
}
