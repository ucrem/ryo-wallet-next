# Import an existing wallet

The next-release Open wallet screen offers both saved-wallet selection and **Import a wallet file**. Importing opens an app-owned copy; the selected original files are never opened by wallet RPC, renamed or overwritten.

## Desktop workflow

1. Close the wallet in Atom so its files are no longer being written, and keep a separate backup of the wallet and its `.keys` companion.
2. Select the wallet's network in Settings. For an existing production wallet, use **Mainnet** and a mainnet node. Each network has separate wallet and chain storage.
3. Choose **Open an existing wallet**, complete the saved data-location/node steps and continue to **Open wallet**.
4. In **Import a wallet file**, click **Choose wallet file**. In the native dialog, select the main wallet file, not `.keys`, `.address.txt` or Atom metadata. Its matching `.keys` file must be beside it. For example, select `mining` when the folder contains `mining` and `mining.keys`. This step checks the pair's metadata without copying files or starting wallet RPC.
5. The selected filename appears, followed by its password and backup acknowledgement fields. Enter the existing password, acknowledge the original-file backup, then click **Import and open wallet**. Existing short passwords are accepted; new wallets still require at least 12 bytes. Empty-password imports are not supported. **Change file** starts a new selection and clears the previous password and acknowledgement.
6. A fresh opaque wallet directory receives both files. The reviewed wallet RPC opens that copy, validates supported account/multisig scope and starts the normal wallet sync monitor. The existing backup acknowledgement makes the dashboard available without retrieving or displaying a recovery phrase.

Cancelling the dialog imports nothing and leaves no import password form. Missing `.keys`, selecting `.keys` itself, linked/non-regular or oversized files are reported before requesting a password. Only the basename and an opaque selection token reach the renderer; the full source path stays in Rust. Import checks the token and the selected data folder/network again, and revalidates the file pair during copying. Incorrect passwords and unsupported scope are reported without raw paths or RPC diagnostics. On a failed open, the key-bearing process is stopped before removing only this attempt's new copy. If shutdown or removal cannot be confirmed, the copy is retained and the error explains the required recovery.

## What to compare with Atom

After the node and wallet have synchronized, compare the primary address, subaddresses, total/unlocked balances, and the received/sent transaction hashes, amounts, fees and confirmations. Preserve the wallet cache when comparing old outgoing transaction details; seed restoration or a full rescan can lose locally stored outgoing details. Do not request a full rescan merely to perform this import check.

Only account zero and its subaddresses are supported. Multiple-account and multisig wallets are rejected. Atom-specific metadata/contact-file migration, unprotected and watch-only workflows are not covered. Importing an existing wallet verifies display/reading of historical activity; it does not validate signing or relaying new transactions and cannot replace current-fork testnet acceptance.

## Verification

- The five frontend import regressions cover file selection before password entry, selected-filename display, an empty saved-wallet list, backup acknowledgement, existing short passwords, cancellation, invalid file pairs, incorrect-password retry, clearing fields on file changes and no recovery-phrase request during import. All 44 frontend tests, 55 service unit tests, TypeScript and ESLint passed after this correction.
- The selection boundary regression rejects missing/stale tokens and changed data folders or networks. Storage validation checks metadata without creating destination files. Formatting, version consistency and the optimized native desktop build passed after this correction. Windows blocked the previous import revision's workspace Clippy build script with application-control error 4551; the subsequent service-only check also failed to load `ts_rs`. These local Clippy checks are not recorded as passes. CI remains the independent Clippy gate.
- `wallet_import::tests::native_import_preserves_originals_cache_and_existing_short_password` passed on Windows with the prepared SHA-256-verified Ryo 0.6.1.0 RPC. It creates disposable empty source data, checks cached name/subaddresses, rejects `.keys` selection and a wrong password, checks failed-copy cleanup, imports with a short password, locks/reopens the copy and checks both originals byte for byte. It uses no real wallet, funds or daemon.
- Native Windows inspection of the corrected optimized executable confirmed file selection without a password and, after selection of a disposable metadata-only pair, display of its basename followed by password/backup fields. Changing files and cancelling returns to selection without an import password form. Both panels fit the desktop without scrolling. No import was submitted through the UI and no user wallet was opened. Authentication remains the user's action.
- Historical funded Atom data and the user's manual comparison are not yet verified. Unix native dialog behavior and Windows ACL enforcement remain separate release gates.

Repeat the Windows backend integration after preparing the reviewed runtime:

```powershell
cargo test -p ryo-wallet-next --lib wallet_import --locked -- --ignored --nocapture
```
