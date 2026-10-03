# Changelog

## [0.1.0-alpha.6] - 2026-10-03

### Added

- Settings with General and Preferences tabs in the top menu, available while the wallet is locked.
- Local, remote and local-with-remote-bootstrap node selection; advanced peer/bandwidth limits, log levels, private ports and isolated network selection.
- Persistent light/dark/system themes, minimize-to-tray with Open/Lock/Exit controls, optional startup launch and native session-aware inactivity locking.
- Configurable missing Payment ID and weak-password warnings in the send review and create/restore forms.
- Verified, bundled Ryo 0.6.1.0 local node on Windows x64, Linux x64 and macOS Intel.
- Independent local-node start and stop controls available with the wallet locked.
- Separate blockchain and wallet scan progress in the persistent status bar.
- Local-node storage isolated from wallet files and other Ryo applications.
- Wallet, Receive, Send, Address Book and TX History sections in the compact top menu.
- Receive address labels, per-address balances, QR codes, SVG image exports and exact-amount payment requests.
- Wallet contacts with names, notes, payment IDs, editing, deletion and recipient selection.
- Recent activity and filterable transaction history with details, confirmations, notes and explicit explorer links.
- Send and sweep-all preparation with priority/ring-size controls, actual-fee review, expiring immutable drafts and one-shot confirmation.
- Persistent send outcome journal with per-transaction partial/unknown results and read-only reconciliation.
- Wallet rename, password change, password-protected private key display, spent/full rescan, and native key image import/export.
- Recoverable wallet removal after password authentication and explicit backup/removal confirmation.

### Changed

- The desktop uses a top menu with contextual setup navigation; the original sidebar remains available in the layout comparison preview.
- Locking the wallet terminates its key-bearing process while the local node continues synchronizing.
- Opening a wallet in local mode starts or reuses the app-owned node without requiring a separate daemon installation.
- App exit and update installation stop the app-owned node; node and data-location changes require stopped processes.
- Desktop package checks verify both the wallet RPC and daemon hashes.
- Windows sidecars receive the system directory required by native DNS libraries and run without console windows.
- Canonical Windows data-folder paths are converted for compatibility with the bundled Ryo runtime.
- Selected data-folder paths and the setup summary display familiar Windows drive and network paths without the extended-path prefix.
- Node status distinguishes a stopped node or peer discovery from blockchain synchronization; starting the node reloads the selected folder's saved chain.
- Setup can continue with the saved node configuration while the local node is syncing or a wallet is open; editing active node settings still requires stopped processes.
- Compact desktop navigation and content spanning the available width, with setup steps shown only in setup screens.
- One-line status bar with separate inline node and wallet heights, percentages and progress indicators.
- A reachable node still downloading the chain no longer labels wallet synchronization as "Node connecting".
- Changing wallet tabs closes Wallet actions and its menu, discards the previous form and keeps the balance visibility preference.
- Receive, Send and Address Book use a compact wallet summary and denser forms that fit the minimum desktop window; long address/contact lists scroll independently.
- Receive shows the selected address once, with a compact label/status selector and its QR, balance, label and payment request together.
- Home wallet choices stack at their previous single-card width, alongside a decorative Ryo symbol fading from left to right.

### Known limitations

- The local node runs while the app is open; continuing after app exit is not supported.
- Tray/autostart and bootstrap handover still require clean-install checks on all supported platforms.
- macOS Apple Silicon packaging remains paused pending reviewed native Ryo binaries.
- Transaction preparation and submission require a synchronized node and wallet. Real signing/relay with funded test-chain outputs and failure recovery remain unverified; this release stays a development preview.
- Clean-install and full-chain synchronization validation on all platforms remain outstanding.
- Preview builds are not intended for use with funds.

## [0.1.0-alpha.5] - 2026-10-02

### Added

- Bundled and verified Ryo wallet RPC runtime in supported desktop packages.
- Recovery-phrase restoration form with an optional scan start height and backup verification.

### Changed

- Hardened Linux DEB and RPM runtime verification.
- Fixed signed macOS updater artifact generation.
- Removed AppImage from future Linux builds in favor of DEB and RPM packages.
- Improved installed-build update handling and release artifact verification.

### Known limitations

- macOS Apple Silicon packaging is paused until a reviewed native ARM64 Ryo wallet RPC runtime is available.
- Transaction creation, signing, and submission are not yet implemented.
- Recovery-phrase restoration needs packaged validation and does not yet offer a full genesis rescan.
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
