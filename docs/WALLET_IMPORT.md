# Import an existing wallet

The next-release Open wallet screen offers both saved-wallet selection and **Import a wallet file**. Importing opens an app-owned copy; the selected original files are never opened by wallet RPC, renamed or overwritten.

## Desktop workflow

1. Close the wallet in Atom so its files are no longer being written, and keep a separate backup of the wallet and its `.keys` companion.
2. Select the wallet's network in Settings. For an existing production wallet, use **Mainnet** and a mainnet node. Each network has separate wallet and chain storage.
3. Choose **Open an existing wallet**, complete the saved data-location/node steps and continue to **Open wallet**.
4. In **Import a wallet file**, enter its existing password and acknowledge the original-file backup. Existing short passwords are accepted; new wallets still require at least 12 bytes. Empty-password imports are not supported.
5. Click **Choose file and import**. In the native dialog, select the main wallet file, not `.keys`, `.address.txt` or Atom metadata. Its matching `.keys` file must be beside it. For example, select `mining` when the folder contains `mining` and `mining.keys`.
6. A fresh opaque wallet directory receives both files. The reviewed wallet RPC opens that copy, validates supported account/multisig scope and starts the normal wallet sync monitor. The existing backup acknowledgement makes the dashboard available without retrieving or displaying a recovery phrase.

Cancelling the dialog imports nothing and clears the submitted password. Missing `.keys`, selecting `.keys` itself, linked/non-regular or oversized files, incorrect passwords and unsupported scope are reported without exposing raw paths or RPC diagnostics. On a failed open, the key-bearing process is stopped before removing only this attempt's new copy. If shutdown or removal cannot be confirmed, the copy is retained and the error explains the required recovery.

## What to compare with Atom

After the node and wallet have synchronized, compare the primary address, subaddresses, total/unlocked balances, and the received/sent transaction hashes, amounts, fees and confirmations. Preserve the wallet cache when comparing old outgoing transaction details; seed restoration or a full rescan can lose locally stored outgoing details. Do not request a full rescan merely to perform this import check.

Only account zero and its subaddresses are supported. Multiple-account and multisig wallets are rejected. Atom-specific metadata/contact-file migration, unprotected and watch-only workflows are not covered. Importing an existing wallet verifies display/reading of historical activity; it does not validate signing or relaying new transactions and cannot replace current-fork testnet acceptance.

## Verification

- The frontend regressions cover import with an empty saved-wallet list, backup acknowledgement, existing short passwords, cancellation, password errors, clearing password inputs and no recovery-phrase request during import.
- Local Windows checks passed: 42 frontend tests, 54 service unit tests, TypeScript, ESLint, formatting, version consistency and an optimized native desktop build. Windows blocked workspace Clippy's build script with application-control error 4551; the subsequent service-only check also failed to load `ts_rs`. These local Clippy checks are not recorded as passes. CI remains the independent Clippy gate.
- `wallet_import::tests::native_import_preserves_originals_cache_and_existing_short_password` passed on Windows with the prepared SHA-256-verified Ryo 0.6.1.0 RPC. It creates disposable empty source data, checks cached name/subaddresses, rejects `.keys` selection and a wrong password, checks failed-copy cleanup, imports with a short password, locks/reopens the copy and checks both originals byte for byte. It uses no real wallet, funds or daemon.
- Native Windows inspection confirmed the new optimized executable is running, Mainnet is selected, and the saved-wallet and import panels, existing-password input, backup acknowledgement and enabled native-selection button fit the desktop without scrolling. The user's original wallet has not been imported by this inspection. Authentication and file selection are left to the user.
- Historical funded Atom data and the user's manual comparison are not yet verified. Unix native dialog behavior and Windows ACL enforcement remain separate release gates.

Repeat the Windows backend integration after preparing the reviewed runtime:

```powershell
cargo test -p ryo-wallet-next --lib wallet_import --locked -- --ignored --nocapture
```
