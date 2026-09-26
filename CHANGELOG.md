# Changelog

## [0.1.0-alpha.5] - 2026-09-26

### Added

- Bundled and verified Ryo wallet RPC runtime in supported desktop packages.

### Changed

- Hardened Linux DEB and RPM runtime verification.
- Fixed signed macOS updater artifact generation.
- Removed AppImage from future Linux builds in favor of DEB and RPM packages.
- Improved installed-build update handling and release artifact verification.

### Known limitations

- macOS Apple Silicon packaging is paused until a reviewed native ARM64 Ryo wallet RPC runtime is available.
- Transaction creation, signing, and submission are not yet implemented.
- Restore-from-recovery-phrase UI is not yet complete.
- Preview builds are not intended for use with funds.


## [0.1.0-alpha.4] - 2026-09-25

### Added

- Automatic update checks on startup and a manual check button in About.
- Signed in-app updates for installed Windows and macOS builds and Linux AppImage builds.
- A Linux AppImage preview alongside the existing DEB and RPM installers.
- Update availability notices with a release link for DEB and RPM installations.

### Known limitations

- Existing alpha.3 installations must install alpha.4 manually once before in-app update checks become available.
- DEB and RPM installations require manual package updates; the in-app installer works only with AppImage on Linux.
- Public installers still do not bundle the Ryo wallet runtime. Do not use this preview with funds.

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
