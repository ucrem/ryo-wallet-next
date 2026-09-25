# Changelog

## [0.1.0-alpha.3] - 2026-09-25

### Added

- Initial app-owned wallet creation flow.
- Password-protected wallet creation using the reviewed Ryo wallet RPC runtime.
- Recovery phrase backup and three-word verification flow.
- Reopening of wallets previously created by the application.
- Wallet lock flow.
- Read-only wallet screen with primary address and balance information.
- Wallet RPC integration with the configured Ryo node.
- Persistent wallet status bar with node connectivity and synchronization progress.
- Wallet and network block-height monitoring.
- Read-only remote daemon health checks for the configured node.
- New receive addresses for an open wallet after recovery-phrase backup, with a selector and copy action.

### Changed

- Wallet data is refreshed after creation and while a wallet remains open.
- Wallet balance is displayed in RYO instead of raw atomic units.
- Temporary node or synchronization failures are presented as recoverable wallet states.
- Wallet synchronization now updates silently in the persistent status bar instead of showing transient connection messages in the wallet view.
- The status bar shows wallet and network heights, connection state, and synchronization progress while a wallet is open.
- Wallet actions use compact icon buttons with accessible labels and tooltips; the open-wallet form has clearer action spacing.
- Navigation remains available while the wallet is open.

### Known limitations

- Transaction creation, signing, and submission are not implemented.
- Restore-from-recovery-phrase UI is not yet complete.
- Public installers do not bundle the reviewed Ryo wallet runtime, so wallet operations are available only in the Linux development build.
- After recovery from a phrase, additional receive addresses must be recreated in the same order to appear again.
- Preview builds are not intended for use with funds.

## [0.1.0-alpha.2] - 2026-09-25

### Added

- Dedicated About screen.
- Runtime application version display.
- Bundled in-app changelog.
- Direct link to the matching GitHub release.
- Automatic version and changelog consistency validation.

### Changed

- The sidebar now exposes application version information.

## [0.1.0-alpha.1] - 2026-09-24

### Added

- Initial Ryo Wallet Next desktop foundation.
- Native Tauri application for Linux, macOS, and Windows.
- Wallet setup flow for create, restore, and open actions.
- Configurable wallet data location.
- Local and remote node configuration.
- Native Rust wallet-service foundation.
- Linux DEB and RPM installers.
- macOS Intel and Apple Silicon DMG installers.
- Windows NSIS installer.
- Automated staging builds and GitHub pre-release publishing.

### Known limitations

- Wallet creation, restoration, and opening are not yet implemented.
- No production Ryo runtime is bundled.
- Transaction signing and submission are not available.
- Preview builds are not intended for use with funds.
