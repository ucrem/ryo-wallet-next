# Wallet operations in alpha.6

Updated 2026-10-03. Inventory: [Atom wallet screens at 6c8d0aa](https://github.com/ryo-currency/ryo-wallet/tree/6c8d0aa68245271fe0e781084b38583abf758869/src/pages/wallet), [reviewed RPC definitions](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server_commands_defs.h).

| Operation | Local alpha.6 implementation |
| --- | --- |
| Wallet | Name, address, balances, recent activity, copy, hide balances and lock |
| Receive | Primary/subaddresses, used status, labels, balances, QR/identicon SVG export, payment request |
| Send | Address/contact/request, exact amount, all unlocked coins, payment ID, priority, ring size, optional contact save |
| Review | Actual fee/amount, full destination, immutable Rust draft, confirmation and five-minute expiry |
| Address Book | Create/edit/delete/search, name, notes, payment ID, copy and send shortcut |
| TX History | Type/hash filters, pagination, details, confirmations, notes, copy and explorer |
| Private keys | Password reauthentication, phrase/view/spend keys, explicit copying, display hidden after one minute |
| Password | Current password plus new password/confirmation; inputs cleared at handoff |
| Rescan | Spent outputs or full genesis scan with explicit confirmation |
| Key images | Native encrypted-file export/import via private staged runtime files |
| Delete | Password, backup confirmation and DELETE; recoverable encrypted-file archive |

This covers the open-wallet screens and Wallet actions in Atom. [Settings](SETTINGS.md), including appearance, are also implemented in the local alpha.6 build. Mining/pool pages, view-only/key-based/legacy wallet creation and legacy Atom contact migration are outside this scope. Multiple-account and multisig wallets remain unsupported.

## Send behavior

The reviewed node and wallet must be synced. `transfer_split` and `sweep_all` prepare with no relay and request metadata, never transaction keys. All amounts remain exact `u64` in RPC and strings in IPC, with checked sums. Account zero is explicit. Confirmation consumes its token before relay. Split outcomes are reported per hash. An ambiguous reply remains unknown; the app never retries or rebuilds it automatically. The private hash-only journal is persisted before each relay and reconciled with read-only `get_transfer_by_txid`.

Names/contacts and transaction notes persist in the wallet; seed restoration alone does not restore them. Signed drafts are memory-only. Removed encrypted wallets remain under `mainnet/removed-wallets`. Key image exports and copied private keys contain sensitive material and require explicit actions.

Each wallet tab and session has a separate page lifetime. Switching tabs closes Wallet actions and its menu and discards the previous form; returning does not reopen it. Balance visibility remains unchanged across tabs.

Receive, Send and Address Book use a single-row wallet summary without repeating the primary address. Receive's left list selects the address by label, index and used status; the selected full address appears once beside its QR. Labels and payment requests remain on the same screen. Send uses four-column form rows; contact creation uses paired fields with full-width address/notes. Long lists scroll inside their panels, keeping the form visible.

## Evidence and release gates

- Windows checks passed: TypeScript, lint, production frontend build, strict clippy, 35 frontend tests (including QR decode round trip, six navigation regressions and receive address selection), 45 Rust unit tests and ten RPC fixtures.
- A headless Chromium layout fixture using the actual dashboard components and styles verified Receive, Send and Address Book at a 1400 × 768 content viewport. Each had main `scrollHeight == clientHeight == 680`, with all forms above the footer; Receive contained the selected full address once. The fixture uses synthetic data, never a real wallet. Screenshots and measurements are under `target/wallet-compact-preview`.
- The opt-in `wallet_operations` test passed against the reviewed Windows RPC with a disposable empty wallet, including persistence, password authentication/change, key response handling, key image export and exact payment URI round trip.
- Native Windows UI verification passed after the user unlocked the rebuilt alpha.6: Wallet, Receive, Send, Address Book and TX History render in the compact top menu; Receive displays the real primary address and QR, the empty contact/history states load, and the six Wallet actions are present. Send review is disabled while synchronization is incomplete. The single-row footer showed node height 516,485 and wallet height 516,465 against 1,196,628; the original `G:\` data folder and existing chain were retained. No transfer or destructive action was performed in the user's wallet.
- Fixtures cover no relay during preparation, immutable relay, cancel/expiry/session rejection and split/lost-response behavior. They do not validate cryptographic or real network behavior.
- **Not verified:** funded isolated test-chain prepare/sign/relay and sweep, actual chain partial outcomes, rescan/import with mature outputs, clean installed builds on all supported platforms and production ACL/process/signing gates. No public promotion is claimed. Builds remain development previews.

The funded transfer gate requires a deterministic isolated test-chain fixture. Do not use real funded wallets or public mainnet sends for automated checks.
