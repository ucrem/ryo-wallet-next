# Local node with a locked wallet

Alpha.6 separates the owned daemon from wallet RPC. Start the node from the
status bar after selecting Local mode, or open a wallet to start it automatically.
Locking the wallet closes wallet RPC but leaves the daemon running. The node
and chain progress remain visible. Lock the wallet before manually starting or
stopping the node, so an open wallet keeps its configured daemon endpoint.
Close the application to stop both processes. No background service is installed.
Setup's Continue button reuses unchanged saved node settings while the node
is syncing. Editing and saving a different configuration still requires idle
wallet and local-node processes.

## Reproducible disposable test

Prepare both runtimes with `node scripts/prepare-dev-runtime.mjs`. Set
`RYO_TEST_DAEMON` and `RYO_TEST_WALLET_RPC` to the absolute reviewed executables
in `src-tauri/.dev-runtime`, then run:

```text
cargo test -p ryo-wallet-service --test locked_node --locked -- --ignored --nocapture
```

The test verifies binary digests, starts a mainnet daemon in disposable storage,
creates a disposable wallet, locks it, and checks that chain height advances
while wallet RPC is absent. It reopens the wallet without starting a second
daemon, stops both processes and restarts the same blockchain directory. It
captures the height before shutdown and asserts that a fresh `NodeService`
reloads at least that height, without reusing any in-memory status cache. It
never prints passwords or recovery phrases and never sends transactions.
Mainnet peer connectivity is required; the observation has a 60-second bound.
The optional `RYO_TEST_DATA_ROOT` preserves diagnostics in a chosen test directory.

Windows x64: passed on 2026-10-03 with upstream Ryo 0.6.1.0, Rust 1.98.1,
including canonical Windows paths as returned by the native folder picker.
The fresh-owner restart check resumed at height 101 after a pre-stop height of
91. The native layout preview also reloaded its saved height of 2,295 after
closing and reopening, then continued syncing in the same data folder.
The extracted alpha.6 NSIS desktop app also starts the node from the user's
persisted Windows data folder. The user confirmed synchronization; the native
status bar displayed a locked wallet with an active node, and a process check
confirmed one app-owned daemon and no app-owned wallet RPC process.
The chain advanced from height 1 in the native status bar to height 2,211 in
the owned daemon's RPC response while the wallet process remained absent.
This is process and initial-sync evidence, not a completed full-chain sync.
The rebuilt top-menu desktop app subsequently reopened the same original
`G:\` data folder: its daemon loaded last block 56,641 and reported RPC height
56,642, with no app-owned wallet RPC process. This confirms saved-chain reuse
across an actual desktop close and reopen, as well as the disposable test.
Linux/macOS clean installation, capability denial, crash containment and
production signing remain separate release gates.
