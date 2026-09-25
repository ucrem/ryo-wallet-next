# MVP specification

A usable MVP must meet every included acceptance criterion on Linux, Windows and macOS. Source-based backend constraints are in [UPSTREAM_ANALYSIS](UPSTREAM_ANALYSIS.md); this document specifies intended product behavior, not completed implementation.

## Included flows and acceptance criteria

| Feature | Flow and release acceptance |
| --- | --- |
| First run | Explain independent status; choose data location through native UI and Local/Remote node mode; show remote transport/privacy implication; no wallet or node started without a chosen configuration |
| Create | Name + nonempty password/confirmation → upstream create (long address default) → one-time recovery phrase → verify selected words → wallet overview. No overwrite; duplicate names handled; phrase absent from logs/caches |
| Restore | Seed + password + optional scan height → upstream validation → open and scan. Handle valid upstream legacy/new formats; explain embedded restore height and offer explicit full rescan. Wrong seed never creates a seemingly successful empty wallet |
| Open/import | Native select → preserve originals → app-owned copy of wallet and companions → password → open. Wrong password, missing .keys, corruption and unsupported type are distinct where evidence allows; don't invent a diagnosis |
| Password protection | Nonempty passwords on creation/restoration; existing empty-password wallets must be protected before normal use. Change password through upstream RPC; confirm old password fails and new succeeds after reopening |
| Scope checks | One open wallet; account 0. Inspect accounts/type on import. Reject unsupported multiple-account, watch-only or multisig use with a clear explanation; never hide funds behind an incomplete total |
| Sync | Show wallet scan and daemon progress separately, last update and disconnected/stale state. No misleading 100% based on one height; send disabled until conservative readiness checks pass |
| Balance | Exact total/unlocked/locked with nine-place precision; never negative/rounded into a different atomic amount. Clarify unconfirmed/locked state |
| Receive | Primary address and user-created account-0 subaddresses returned by backend; list and copy any address, with a locally generated QR of the exact selection. QR round-trip matches address; no network request. Show wallet/network and don't truncate copied data |
| Send | One recipient + decimal amount + optional payment ID; validate through Rust/upstream. Support current/legacy long, Kurz and integrated destinations accepted by backend; reject conflicting integrated/separate IDs |
| Confirm | Display full destination, explicit/embedded ID context, amount, actual fee, total, transaction count and network. Editing invalidates draft. No transaction relay before explicit confirmation |
| Submit | One-shot draft submission; disable duplicate actions. Track each split transaction; show txids, pending/confirmed/rejected/unknown/partial distinctly. Never automatically recreate a transfer after uncertainty |
| Activity | Incoming, outgoing, pool, pending, failed states; stable ordering and detail with fee/time/height where available. Empty, loading, stale and failed-to-load differ. Reorgs update status |
| Nodes | Local starts owned ryod; Remote reaches an explicit host/port. Show network, availability, readiness, transport and freshness. Node change invalidates draft and requires wallet reopening; no hybrid/autofallback |
| Lock/close | Manual lock + idle timeout (proposed default 5 minutes, configurable). Immediately hide sensitive UI; report locking until key-bearing process exits. Reopen requires password. OS suspend/resume returns to locked/reconciliation state |
| Settings | Theme/system preference, node mode/endpoint, idle lock, change password, explicit full rescan, app/backend versions, private diagnostic status. Signed desktop updates are described in [packaging](PACKAGING.md) |
| Accessibility | Keyboard-only completion of onboarding/send/lock, visible focus, proper modal focus return, readable light/dark states, screen-reader labels and errors; usable at small desktop window sizes |

## Payment flow detail

Enter → validate → prepare/sign locally without relay → review → confirm → relay stored metadata → reconcile/history. The preparatory signature never leaves Rust except back to the local wallet process. A cancelled or expired draft must be impossible to submit via IPC. An ambiguous send shows “Submission outcome unknown; check this transaction before sending again,” with recorded hashes; it must not show “Failed, retry” as though no broadcast happened.

MVP handles a single recipient but multiple underlying transactions because upstream may split. A maximum number of prepared transactions and body-size limit must be explicit, tested product limits. If a legitimate payment exceeds them, explain the limit without auto-splitting it into independent user payments. No maximum-balance sweep shortcut in MVP: subtracting an estimated fee is not an exact sweep implementation.

## State acceptance examples

- A late balance response from wallet A never appears after wallet B is opened.
- Port collision does not connect to an unrelated wallet process.
- Refresh blocking RPC shows scanning/unresponsive status without an endless spinner or false lock success.
- Disk full during save preserves existing files and reports recovery required.
- Wrong-network daemon does not become a valid node because it uses the default port.
- Cancel/lock/node switch during preparation prevents later submission of the returned draft.
- After a crash immediately following network broadcast, restart does not resend or report definite failure.
- No secret-bearing frontend state is persisted; a password/seed canary is absent from captured test logs and production assets.

## Explicitly post-MVP

Mobile; hardware wallets; multisig/watch-only/key-image workflows; account management; subaddress labeling and advanced management; integrated receive addresses; address book and notes; mining/pool features; hybrid bootstrap; app-managed TLS proxy/Tor; deep links; price feeds; exchanges; sweep all; multi-recipient sends; CSV export; tray/background wallet-unlocked mode; automatic legacy-directory discovery; broad localization beyond a localization-ready initial interface.

Receiving at existing addresses still works through upstream synchronization within the supported account scope. Advanced-wallet users are told which workflows remain in Atom; this MVP does not claim full feature parity. Mainnet is the product default network; separate developer builds/configurations expose testnet/stagenet without shared wallet directories. Live public test-network availability is not assumed.
