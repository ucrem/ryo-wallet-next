//! Private application paths and opaque identifiers.
//!
//! Network-isolated wallet storage, imports and persisted configuration.
//! This module accepts no renderer-supplied wallet path segment.

mod import;
mod paths;
mod permissions;
mod preferences;
mod settings;
mod wallet_id;
mod wallet_name;

pub use import::{ImportError, ImportedWallet, copy_wallet_pair, validate_wallet_pair};
pub use paths::{AppPaths, PathError};
pub use permissions::{secure_directory, secure_file, verify_private_file};
pub use preferences::{Preferences, idle_expired};
pub use settings::{
    AppSettings, SettingsError, Theme, load_settings, load_settings_if_present, save_settings,
};
pub use wallet_id::{WalletId, WalletIdError};
pub use wallet_name::{WalletNameError, load_wallet_name, save_wallet_name};
