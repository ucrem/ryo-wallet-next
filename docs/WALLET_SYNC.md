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
- Busy balances/history use a compact `Syncing` indicator beside their headings. Its tooltip and accessible label explain automatic updates and identify retained snapshots. It disappears after successful refresh; the transaction list stays visible without an extra notice row.
- Failed receive-address, contact and wallet-name reads also retry. The send journal is checked only through its existing explicit flow; financial actions are never automatically repeated.
- A node probe has five seconds to respond. On a failed probe, the last heights remain visible for at most 30 seconds, bound to the same endpoint and process generation. The label is `Node response delayed`; reachability and readiness stay false. The tooltip identifies the values as last reported. Expired context, network mismatch or changed ownership cannot reuse these heights.
- Wallet events update the footer; periodic snapshots recover from missed events or listener registration failures.

## Saving an interrupted scan

The wallet cache and the daemon's blockchain database are separate. Ryo stores
the encrypted wallet cache when `stop_wallet` is served. Previously a long
refresh could block that request until the app's bounded shutdown forcibly
terminated the process. The latest processed blocks were then lost from the
cache, even though the daemon's chain remained on disk.

Each wallet RPC session now owns a byte-preserving daemon transport. Manual
lock, inactivity lock and normal application shutdown first close that
session's daemon connections and listener. The reviewed runtime can finish its
current processed results, leave the blocking refresh, serve `stop_wallet`,
store the actual cache and exit. Reopening uses the same wallet file, including
its processed blocks and recovered funds; no renderer height is used to skip
blocks. The independently supervised local daemon remains running on a wallet
lock. A full rescan or a new seed restoration still deliberately starts a new
scan.

The transport forwards to the original configured node and never logs request
or response bodies. Only same-host connections are accepted, with at most 16
live connections. Local endpoints use loopback. Public remote endpoints use a
routed local IPv4 address and reject foreign source addresses before reading
or forwarding any data: upstream 0.6.1.0 automatically trusts loopback daemon
addresses and lacks a force-untrusted flag, so routing a public node through
loopback would change its trust. A remote session fails startup if no usable
non-loopback IPv4 route is available. Wallet RPC itself remains authenticated
and loopback-bound. Node health still probes the original endpoint directly.

The bounded process termination fallback remains for an unresponsive runtime.
A forced OS termination, crash or disk write failure cannot promise that the
latest in-memory scan was saved. This change does not add crash-safe periodic
checkpointing or replace Ryo's cache format.

### Persistence evidence (2026-10-04)

- A genuine isolated two-node fixture restored a freshly funded wallet and held
  a real mempool response after processing 80 validated blocks. Wallet height
  reads timed out, reproducing the blocked RPC condition.
- Lock completed through the graceful path in under ten seconds and changed
  the wallet cache file. A new process reopened the cache with an unavailable
  daemon and returned saved height **81** and the original funded balance.
- A second fresh process retained that height and the original transaction
  history without requesting a blockchain rescan. The daemon was reachable for
  the history assertion because Ryo's history RPC also refreshes its mempool.
- The existing funded send, receive, split-send, lost-reply and recovery suite
  passed with the transport enabled. These remain genesis-era testnet checks,
  not current-fork acceptance.
- Unit tests cover unchanged request/response bytes, cancellation of in-flight
  reads, refusal of reconnects after lock, rejection of foreign sources and
  preservation of the public remote endpoint's non-loopback trust boundary.
- A separate opt-in read-only mainnet test used a freshly generated disposable
  wallet against `wallet-node.ryo-currency.com:12211`. It observed processed
  height **1,238** during a busy scan, gracefully saved height **2,720** and
  reopened that height with an unavailable daemon. The extra blocks were
  processed between the initial observation and the graceful checkpoint.
  No existing wallet, funds or signing operation was used. Public node
  availability is not a required CI gate. Repeat with
  `cargo test -p ryo-wallet-service --test public_scan_resume --locked -- --ignored --nocapture`
  after setting `RYO_TEST_WALLET_RPC` to the reviewed executable.
- Local Windows validation passed 61 service unit tests, 10 RPC transport tests,
  66 frontend tests, TypeScript, ESLint, service Clippy with warnings denied,
  formatting and version consistency. Both genuine testnet fixtures passed in
  38.45 seconds with fresh non-sensitive receipts. The optimized native desktop
  executable compiled successfully. The already-open old desktop process was
  preserved during compilation, then closed at the user's explicit request.
  The new executable was launched through commands and process metadata confirms
  a responding native desktop window. No computer-use automation or user wallet
  authentication was performed.
- In CI for `f8c216f`, busy-scan persistence passed on Linux and macOS, but
  funded operations and the Linux local-node lifecycle encountered intermittent
  RPC authentication failures. Sequential fixtures in `db984b5` did not resolve
  the issue: macOS still rejected authenticated operations and Linux failed a
  genuine lost-reply assertion. The checks remain strict.

### Connection-bound RPC authentication

The reviewed [HTTP handler](https://github.com/ryo-currency/ryo-currency/blob/0.6.1.0/contrib/epee/include/net/http_protocol_handler.h)
owns one Digest authentication session per TCP connection. The previous client
helper could leave the nonempty HTTP 401 body unread while sending its Digest
answer. Depending on buffering, that answer used another connection with a
different server nonce, producing an authentication failure with valid credentials.

The transport now fully drains the bounded challenge body before answering on
the same short-lived HTTP client. It uses the already-reviewed `digest_auth`
dependency directly. There is one retry only after an unauthenticated request
is rejected; authenticated operations are never automatically repeated. Each
logical call still uses a fresh client and a fresh nonce exchange.

A socket-based regression sends a nonempty keep-alive challenge and requires
the answer on that exact connection, then repeats the exchange with new nonces.
It fails when challenge draining is removed and passes with the correction.
All 11 RPC transport checks, 61 service unit checks and strict service Clippy
pass on Windows. Both real native testnet fixtures passed with this correction
in 79.89 seconds and generated fresh non-sensitive receipts. Cross-platform CI
results for this correction are recorded in the PR separately from earlier
revisions.

## Evidence and remaining limits

- Windows unit checks cover fragmented stdout, processed versus downloaded blocks, invalid hashes, overflow, oversized output, process EOF, read deadlines, abandoned-read handling and preservation of mutation commands.
- UI regressions verify busy-to-ready recovery of balances, history and name, explicit stale-data wording, delayed-node context and refusal to report a synced wallet while RPC is busy.
- The genuine isolated two-node funded testnet suite passes with unchanged manifest-verified upstream runtimes. After each full scan, the processed height must equal the authenticated RPC height. It also retains its existing funded recovery, send, receipt and restart assertions. The short chain remains a genesis-era fixture; it does not verify current-fork compatibility.
- Local Windows checks passed: 58 service unit tests, nine host unit tests, 48 frontend tests, TypeScript, ESLint, formatting, version consistency, service Clippy with warnings denied and optimized desktop compilation. The real two-node suite passed again in 49.41 seconds at height 195. A repeated run completed its assertions but could not create an already-existing receipt; rerunning with a fresh receipt path passed.
- The corrected native executable was relaunched from the workspace release directory. Its node resumed the original `G:\` chain above height 627,000. Native inspection observed a delayed node reply retaining the previous heights, then returning to `Node syncing`. Wallet recovery inspection awaits manual authentication.
- A read-only diagnostic of the user's local mainnet session observed the wallet RPC timeout while the daemon responded and advanced its chain height. No transfer, password entry or wallet alteration was performed for that diagnostic.

Completion of the user's full mainnet recovery and agreement of the final balance/history with the original wallet still require the scan to finish. Native cross-platform behavior and stable-release gates remain open. See [Testnet validation](TESTNET_VALIDATION.md).
