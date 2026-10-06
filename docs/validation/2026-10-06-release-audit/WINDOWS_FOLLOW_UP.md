# Installed Windows follow-up — 2026-10-06

## Outcome

**73 additional assertions pass:** 46 installed wallet/security/preferences assertions, 10 native lifecycle assertions, and 17 installed updater assertions. **No additional application defect was confirmed.** The nine issues in the main audit remain open and unchanged; no application fix was applied.

The application source is still **`e5a7f55df8515b0873565286fcd7b2c8ec93bbc7`**. The previous report-only revision was `4114abc7c5299a55dcc1fe56275d70df96e1fdbd`. The source/runtime/dependency files are identical at both revisions.

These are recorded assertions within three suites, not 73 independent user scenarios. Failed assertions in the initial probe attempts were investigated before classification. The corrected receipts include their evidence and the probe corrections; those attempts are not newly confirmed application bugs.

## Isolation and method

- Current-user NSIS installation with a different product, executable basename, bundle identifier, registry entry, app-data profile and disposable data root. The actual application implementation, permissions and CSP were unchanged.
- Commands ran through the **installed application's embedded WebView2 and actual Tauri IPC**, using a loopback debug endpoint scoped to that test process. Microsoft documents this process-local debugging mechanism in [WebView2 debugging](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code). The production process received no debugging environment change.
- No computer-use, screenshots, mouse/keyboard injection, file-dialog automation or user-wallet authentication. Native window close and a second native instance were used for the tray/lifecycle checks. The frontend's initial DOM mount and asset origin were read; complete visual GUI acceptance is not claimed.
- All wallet operations used generated disposable passwords/seeds and reviewed runtimes with exclusive loopback peers. Every financial receipt remains **`current_fork_gate_passed=false`**. Real signatures, blocks, relay and balances were used on the short, genesis-era chain.
- Updater fixtures used an **ephemeral test signing key**, a loopback feed and configuration-only test versions `0.1.0-alpha.6 → 0.1.0-alpha.7`. The repository/release version is unchanged. No production signing key, publisher certificate, GitHub feed, release or tag was changed. The temporary private key and its private generation log were removed after testing.
- The test installation was uninstalled. The user's original desktop process **24644** remained running, and its executable retained SHA-256 **`F531EE8549B2581A4A274D46EF6BD57D916E3E5AC7394F1F6099A3A6F39C7DF1`**.

## Executed checks

| Suite | Result | Actual coverage |
| --- | --- | --- |
| Installed wallet, security and preferences | **46/46 PASS** | Embedded frontend mount; actual packaged runtimes; remote health; create/backup gate; overview; rename/discovery; subaddress/label; payment request roundtrip; contact create/read/edit/delete; stale-session rejection; private-key password rejection; password change/wrong-password rejection/reopen; real funded send/cancel/one-shot relay/incoming and outgoing history/note; seed restoration; cache reopen; removal confirmation/password/archive; native preference validation; autostart register/remove; tray close/second instance; actual hidden-window idle lock; official update check and absent-candidate guard |
| Native lifecycle | **10/10 PASS** | Saved state restart; real bootstrap/local height distinction; complete short-chain hybrid handover; local readiness after bootstrap loss; idle lock after process pause; node continuity while locked; actual installed owner crash with two managed children; descendant cleanup; startup recovery; chain/wallet cache reuse; native node stop |
| Installed updater | **17/17 PASS** | Installed baseline/version; open wallet and local node before update; candidate discovery; expected-version mismatch; altered-byte rejection; authenticated-version binding; failed-install guard recovery; actual signed installer invocation; installed payload/version; automatic new-version restart; no orphan runtimes; preserved keys/discovery/preferences; current check after upgrade; real saved-wallet reopen |

### Real transaction and scan observations

The installed IPC fixture mined **144 validated blocks**, prepared and cancelled a real signed transfer without relay, then prepared and confirmed a new transfer of **1.000000001 RYO**, with a real nonzero fee and ring size 25. After 12 more validated blocks, outgoing and exact incoming history were observed. Repeated confirmation of the same token was rejected.

History was checked after the reviewed runtime's background refresh completed. A 2500 ms initial history observation was too early; bounded repeated reads observed the sender and recipient transfers. A separate native query observed **64 incoming mining rows** in the restored wallet. That timing correction changes the probe, not the application.

### Native capabilities and CSP

The installed WebView rejected clipboard reading, direct dialog opening, path opening and direct window closing through denied plugin commands. An out-of-scope URL was rejected. An injected inline script did not execute; an external loopback fetch was blocked by `connect-src`, with **zero requests** reaching the canary server. These are selected negative cases, not an exhaustive exploit/security review.

### Tray, startup and inactivity

Saving autostart through native IPC created the exact isolated installed executable's current-user startup entry. Disabling it removed the entry. This verifies OS registration, **not an actual Windows logout/login**.

With tray mode enabled, a normal native close kept the app alive. Without renderer activity, the native idle monitor locked the real open wallet after the configured **60 seconds** and cleared the active-wallet state. Launching another isolated instance exited that instance and retained one owner. Menu clicks and the separate minimize gesture were not automated.

Only the isolated native app process was paused for **65 seconds** and resumed. The native monitor then locked the wallet; its independently managed node remained ready. **The Windows session and PC were not put to sleep.** This verifies elapsed-time handling after a process pause, not real OS sleep/network/device restoration.

### Hybrid handover and installed crash

Before synchronization, actual daemon/IPC health reported **local height 1**, **RPC/bootstrap height 157** and `untrusted=true`. A loopback P2P bridge with the same validated chain brought the managed local daemon to **157**, with matching tip hash, `untrusted=false` and `ready=true`. The local node remained ready after the bootstrap daemon was stopped.

The isolated installed owner was then forcibly terminated while its wallet RPC and local daemon were active. **Both owned runtime children disappeared; zero survived.** On restart, the app recovered and reused **wallet height 157 / node height 157**, without a new import. This does not claim current-fork public-chain handover or eliminate the documented spawn-to-assignment process-ownership limit.

### Actual in-app updater

The installed updater rejected a changed installer byte and a forged feed version `99.0.0` paired with the valid alpha.7 signature. Both failures left the original open wallet/node running and released the install guard.

The valid signed candidate was downloaded and invoked by the real `app_update_install` command. The old owner disconnected, the NSIS registry changed to alpha.7, and the updated native process restarted. It began without an open wallet or orphan managed runtimes. Encrypted test key bytes, wallet discovery and preferences survived; the saved wallet reopened with its disposable password.

The payload comparison accounts for Tauri's standard NSIS marker: the copied build output contains `__TAURI_BUNDLE_TYPE_VAR_UNK`, while the installer payload contains `__TAURI_BUNDLE_TYPE_VAR_NSS`. **Every payload byte matches after that expected three-byte patch.** The pinned implementation is in `tauri-utils 2.9.3`, `src/platform.rs`, and the build log records the NSIS patch. The initial probe's unbundled-versus-bundled digest expectation was corrected; the installation itself had succeeded.

Installed candidate payload SHA-256: `1e2ecad68ff76e20c1729a308edb79903009e0336087dd3214bf50e17e9c4825`.

This closes the **isolated installed Windows updater-fixture check**. It does not authenticate or publish a future production candidate, verify a paid publisher signature, or establish Linux/macOS installation behavior. Windows current-user installation required no elevation prompt.

## Remaining platform acceptance

| Area | Windows | Linux/macOS |
| --- | --- | --- |
| Installed native command workflows | Passed above, including real disposable transactions | Existing native CI service/host fixtures pass; installed desktops not exercised |
| Complete GUI workflows and native file dialogs | Unverified under the standing CLI-only constraint | Unverified |
| Tray | Close-to-tray, hidden idle lock and second-instance ownership pass; menu clicks/minimize gesture unverified | Unverified installed behavior |
| Startup | Current-user registration/removal passes; actual login launch unverified | Unverified installed behavior |
| Suspend/resume | Process-pause/elapsed-time check passes; real system sleep unverified | Unverified installed behavior |
| Hybrid handover | Actual complete genesis-chain handover passes; current public fork remains pending | Not exercised in installed desktops |
| Installed update | Real signed isolated fixture passes; future production candidate remains subject to publication validation | Unverified installed behavior |
| Installed crash/cache/children | Isolated owner crash and two-child cleanup/reopen pass | Existing CI foundation checks; full installed acceptance unverified |
| Current-fork funded acceptance | Waiting for maintained Ryo testnet | Same external dependency |

No Windows logout, reboot or system sleep was performed on the user's working desktop. Those remaining transitions and complete GUI interaction are explicit limits, not counted as bugs or passes.

## Evidence and reproducibility

- [46 installed IPC assertions](evidence/windows-installed-report.json).
- [10 native lifecycle assertions](evidence/windows-lifecycle-report.json).
- [17 real installed updater assertions](evidence/windows-updater-report.json).
- [Post-refresh restored history](evidence/windows-restored-history-after-refresh.json).
- [Installed payload verification](evidence/windows-update-payload-verification.json).
- Probe sources are preserved as `.txt` in `reproductions/`; they are separate from normal CI. They intentionally target the distinct audit identity and must not be pointed at the production app/profile.

Full command logs, first-attempt observations, signed test packages/public key, disposable chain data and runnable probes remain in `target/release-audit-2026-10-06-e5a7f55/`. The ephemeral private signing key has been removed. Preparing another updater fixture requires generating a new test key.

Probe-only corrections retained in the local logs include child-output handle inheritance, exact IPC command spelling, case-sensitive error matching, refresh deadlines, preserving a bootstrap RPC port across fixture restart, and the standard NSIS bundle marker. None was reported as an application defect or changed in application code.
