//! Private application paths and opaque identifiers.
//!
//! Import/copy and configuration persistence will use these paths in a later
//! phase. This module intentionally accepts no renderer-supplied path segment.

mod import;
mod paths;
mod settings;
mod wallet_id;

pub use import::{ImportError, ImportedWallet, copy_wallet_pair};
pub use paths::{AppPaths, PathError};
pub use settings::{
    AppSettings, SettingsError, Theme, load_settings, load_settings_if_present, save_settings,
};
pub use wallet_id::{WalletId, WalletIdError};
