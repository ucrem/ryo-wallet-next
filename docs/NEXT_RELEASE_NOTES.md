# Next release notes — draft

These are unreleased changes on the 0.1.0-alpha.6 baseline. Assign a version and move accepted items into `CHANGELOG.md` when preparing a reviewed release; existing published changelog entries are historical.

## Wallet and recovery

- Native wallet-file import with selection before password entry, preserved originals and app-owned copies.
- Saved-wallet reopening from Home, name recovery during unlock, and compact named wallet selection.
- Bounded busy RPC reads, live scan progress and scan-cache persistence across lock/reopen.
- Consistent history freshness feedback, incoming/outgoing icons and red outgoing icons/amounts.

## Hardening and support

- Protected Windows permissions for app-owned storage and handle-based owner/ACL validation of generated RPC credentials.
- Fix wallet runtime startup on existing user-owned Windows data folders with Modify-only grants: protect the DACL without requesting an unnecessary ownership change. Keep other-owner/reparse restrictions and surface distinct storage/process/readiness errors.
- Windows sidecar Job Object ownership and kernel cleanup on abrupt owner exit, with an explicitly documented spawn-to-assignment limit.
- Native, local-only diagnostic JSON export from About, excluding wallet secrets and identifying data.
- Expanded diagnostic support facts: Windows build/patch, WebView runtime, memory, local data-volume capacity/capabilities, verified Ryo runtimes, build identity, uptime, selected preferences and node probe timing.
- Equivalent core diagnostics on macOS/Linux: product/distribution and kernel versions, physical memory and selected local-volume filesystem/read-only/capacity/free-space facts. Schema 3 labels differing memory/space semantics and excludes arbitrary vendor, kernel-flavor and mount/device text; native collectors are tested in Linux/macOS host CI.
- Fresh default diagnostic filenames and specific save-error feedback for existing files, private storage, unavailable folders, permissions and full disks.
- Reject release executables configured to load the UI from Vite, document the standalone Tauri build command, expose frontend mode in diagnostics and compile embedded frontend contexts in Linux/macOS host CI.
- Native service permission/process tests in the three-platform integration CI.
- Updated implementation, security, CI and acceptance documentation.

## Validation and pending gates

Real transactions on the unchanged-runtime short genesis-fork fixture cover fees, subaddress receipt, recovery, sweep and partial/unknown outcome reconciliation. They do not establish current-fork compatibility. Funded current-fork tests await restoration of Ryo testnet.

Clean installed wallet workflows, tray/autostart/suspend, capability-denial behavior, full hybrid handover, installed updates, publisher signing/notarization and the complete dependency/notices inventory remain required gates. Preview installers have updater authentication where configured, but Windows Authenticode publisher signing is not configured. No stable-readiness claim follows from these changes.
