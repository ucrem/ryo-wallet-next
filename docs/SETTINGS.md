# Settings in local alpha.6

Settings is available in the top menu with an open or locked wallet. General and Preferences save separately. Preferences are kept in the app configuration directory, independently of the chosen chain folder; legacy theme/idle values are migrated on first use.

## General

- Local: verify/download the chain with the owned Ryo daemon.
- Remote: use the explicitly entered host/port. The existing plain HTTP privacy notice applies.
- Mainnet Remote and Local + Remote show `wallet-node.ryo-currency.com:12211`, sourced from the [official Ryo Atom defaults](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/src-electron/main-process/modules/backend.js#L54-L57). **Use suggestion** fills the draft host and RPC port; Save is still required. Opening a form preserves custom endpoints. The suggestion comes from upstream wallet configuration, not blockchain data or a P2P seed list. No public RPC suggestion is supplied for Testnet or Stagenet.
- Local + Remote: the owned daemon receives Ryo's `--bootstrap-daemon-address`; Ryo supplies remote data until its local chain catches up. Local chain progress uses `height_without_bootstrap`, while wallet progress uses the RPC height. No public server is silently selected.
- A native folder picker selects storage; changing folders does not move existing wallet/chain files.
- Advanced options expose daemon/wallet log levels 0–4, incoming/outgoing peer limits and upload/download kB/s limits. −1 omits the limit flag and uses upstream defaults.
- Node P2P/RPC/ZMQ and wallet RPC ports use 0 for an automatic private port. Fixed ports must be distinct, available and at least 1024. RPC listeners remain on loopback. The peer option can enable incoming P2P connections independently of RPC.
- Mainnet, Testnet and Stagenet have separate wallet and chain directories. Network changes preserve the previous network's files.

General saves require an idle wallet process and stopped owned daemon; Rust repeats those checks under the setup guard. Existing settings without the new options remain readable.

## Preferences

- Light, Dark or System applies to navigation, cards, forms and the status bar after Save.
- Minimize to tray hides the window on minimize/close while the application continues running. The tray menu offers Open, Lock and Exit; Exit performs normal process shutdown.
- Startup launch uses the OS autostart registration and always starts with a locked wallet. A single-instance handler reopens the existing window.
- Inactivity can be disabled or configured between one minute and one hour. Rust owns the timer, including while the window is hidden; keyboard/pointer activity is recorded for the current session. Expiry calls generation-checked wallet locking, terminates its key-bearing process and clears frontend wallet caches. The daemon continues.
- A missing Payment ID warning in the immutable send review requires an acknowledgement that the recipient does not need a separate ID. It does not submit automatically.
- The weak-password warning requires acknowledgement in creation/restoration and resets when a password input changes. Disabling the advisory leaves the backend's 12-byte minimum in place.

Preferences and node settings contain no wallet passwords, phrases or signed transaction metadata. Startup/tray changes are rolled back when saving preferences fails.

## Evidence and remaining checks

- On 2026-10-04, a read-only `get_info` request to the suggested mainnet RPC returned `status: OK`, `mainnet: true`, `testnet: false`, `stagenet: false`, `is_ready: true` and height 1,197,078. This records availability at the time of the check; the application continues to verify node health and network when connecting.
- Remote-suggestion follow-up: all 58 frontend tests, TypeScript, ESLint, version consistency and the optimized Windows desktop build pass. DOM checks cover explicit draft filling and submission in both forms, preservation of custom endpoints, disabled settings and absence of mainnet suggestions on Testnet/Stagenet, including first-time setup without saved node settings. Windows application control blocked the rebuilt executable at launch, so native verification of this change remains pending. The previous executable was restored and reopened through commands. No native UI automation or wallet authentication was used for this follow-up.
- Rust preference defaults/bounds and idle expiry tests, legacy/hybrid node validation, structured daemon arguments and backend persistence tests pass.
- DOM tests cover preferences saving while General is locked by sync, hydration of hybrid settings and the missing-ID acknowledgement gate. The frontend suite has 39 passing tests; TypeScript/lint and desktop Rust Clippy pass.
- Actual components with production CSS fit 1400 × 768 without page scrolling for both collapsed General and Preferences, in dark and light themes. Advanced options can scroll when expanded.
- The final unsigned Windows alpha.6 installer was rebuilt and both packaged runtimes passed the reviewed digest checks. Native inspection covered General, Preferences and expanded local advanced controls; the packaged executable was relaunched at the existing desktop location with G:\ retained, and left on Preferences. No user settings were changed for inspection. Package digests and test counts are recorded in [implementation status](IMPLEMENTATION_STATUS.md).
- The reviewed Windows disposable locked-node test still passes after these changes, including restart from saved chain height 111.
- Native Windows tray lifecycle, OS login startup, inactivity after hide/suspend and bootstrap handover with a complete local chain still need end-to-end checks. Linux/macOS platform checks remain outstanding.

Sources: [Atom General](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/src/components/settings_general.vue), [Atom Preferences](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/src/components/settings.vue), [Tauri autostart](https://tauri.app/plugin/autostart/), [Tauri tray](https://tauri.app/learn/system-tray/).
