# Wallet recovery and synchronization

Updated 2026-10-04 for the next release on the alpha.6 baseline.

## Runtime behavior

The reviewed Ryo 0.6.1.0 wallet RPC refreshes the wallet on the same single thread that serves RPC requests. A long seed recovery or rescan can therefore delay `get_height`, balances, attributes and transaction history while the daemon continues downloading blocks. See the [upstream refresh loop](https://github.com/ryo-currency/ryo-currency/blob/166cf188bf351e3eecff904450eda2785f9d0357/src/wallet/wallet_rpc_server.cpp#L113).

The app now reads processed-block heights from its own verified wallet RPC process's private stdout. Console verbosity is 3; the configured file verbosity remains unchanged. Only validated numeric `On new block` heights survive the boundary. The block index is incremented by one to match RPC chain length; downloaded-block ranges are ignored. Raw output, hashes and private material are never stored or forwarded to the renderer. Buffers are bounded and zeroized. Process exit, lock and a full rescan clear this progress.

## App behavior

- Wallet progress remains visible during a blocked refresh. `Wallet scanning` means a read timed out and a processed height is available; `Wallet busy` means no processed height is available yet. A successful height RPC is required before reporting `Wallet synced`.
- Wallet height and node health are collected concurrently. Node status has its own service and queue.
- Background wallet reads have a four-second execution deadline and a six-second queue-plus-execution deadline. Expired queued reads are discarded. Mutations, authentication, signing, confirmation, relay, send-outcome reconciliation and lifecycle commands are not cancelled by these read deadlines.
- Balances and transaction history retry automatically. When the RPC is busy, the dashboard reports scanning and clearly identifies retained values as the last available snapshot. This replaces the previous instruction to lock and reopen. Other read errors remain visible and retry automatically.
- Failed receive-address, contact and wallet-name reads also retry. The send journal is checked only through its existing explicit flow; financial actions are never automatically repeated.
- A node probe has five seconds to respond. On a failed probe, the last heights remain visible for at most 30 seconds, bound to the same endpoint and process generation. The label is `Node response delayed`; reachability and readiness stay false. The tooltip identifies the values as last reported. Expired context, network mismatch or changed ownership cannot reuse these heights.
- Wallet events update the footer; periodic snapshots recover from missed events or listener registration failures.

## Evidence and remaining limits

- Windows unit checks cover fragmented stdout, processed versus downloaded blocks, invalid hashes, overflow, oversized output, process EOF, read deadlines, abandoned-read handling and preservation of mutation commands.
- UI regressions verify busy-to-ready recovery of balances, history and name, explicit stale-data wording, delayed-node context and refusal to report a synced wallet while RPC is busy.
- The genuine isolated two-node funded testnet suite passes with unchanged manifest-verified upstream runtimes. After each full scan, the processed height must equal the authenticated RPC height. It also retains its existing funded recovery, send, receipt and restart assertions. The short chain remains a genesis-era fixture; it does not verify current-fork compatibility.
- Local Windows checks passed: 58 service unit tests, nine host unit tests, 48 frontend tests, TypeScript, ESLint, formatting, version consistency, service Clippy with warnings denied and optimized desktop compilation. The real two-node suite passed again in 49.41 seconds at height 195. A repeated run completed its assertions but could not create an already-existing receipt; rerunning with a fresh receipt path passed.
- The corrected native executable was relaunched from the workspace release directory. Its node resumed the original `G:\` chain above height 627,000. Native inspection observed a delayed node reply retaining the previous heights, then returning to `Node syncing`. Wallet recovery inspection awaits manual authentication.
- A read-only diagnostic of the user's local mainnet session observed the wallet RPC timeout while the daemon responded and advanced its chain height. No transfer, password entry or wallet alteration was performed for that diagnostic.

Completion of the user's full mainnet recovery and agreement of the final balance/history with the original wallet still require the scan to finish. Native cross-platform behavior and stable-release gates remain open. See [Testnet validation](TESTNET_VALIDATION.md).
