# Next release: testnet validation

Updated 2026-10-03. This milestone validates the existing wallet rather than declaring it stable after a successful transfer. Candidate version and publication remain undecided until the acceptance gates pass.

## Evidence so far

- The unchanged reviewed Windows daemon and wallet RPC (0.6.1.0, manifest SHA-256 checks) run in two fresh, isolated testnet nodes with exclusive loopback peers and disposable wallets.
- The funded fixture passes real block validation, mature mined outputs, actual signed preparation, actual fees and a nine-decimal amount, cancellation, no relay before confirmation, one-shot submission, daemon pool acceptance, subaddress receipt and confirmed history.
- A transparent fault proxy drops a reply only after the real daemon accepted the transaction. The service reports `unknown`, blocks a replacement, persists the journal, and reconciles it after a fresh wallet-service owner and a real chain scan. It never sends the same draft again.
- A real three-transaction split sweep with ring size 100 produces `relayed`, `unknown`, and `not_sent` after losing the second acceptance reply. A fresh service owner reconciles the accepted transaction and does not relay the unsubmitted remainder; the real daemon's pool and chain are checked for every hash.
- Funded seed restoration recovers the primary address and balance. Rescan, encrypted key image export/import, sweep to zero sender balance, continuing the chain with the wallet locked, peer chain propagation and persisted-chain restart pass.
- Recovery progress is also checked against the unchanged runtime: after each full scan, the processed-block height parsed from private stdout must match authenticated wallet RPC height. Busy-read deadlines and automatic UI recovery are covered separately; see [Wallet synchronization](WALLET_SYNC.md).
- A separate genuine busy-scan fixture verifies cache persistence at lock: it holds a real pool response, confirms wallet RPC reads are blocked, then checks saved height and funded balance in a fresh process with no reachable daemon. Another process restart retains history without a requested rescan. See the persistence evidence in [Wallet synchronization](WALLET_SYNC.md).
- Test passwords and the recovery phrase are checked against the disposable raw logs. The receipt contains only the profile, platform, time, height and passed checks. No phrase, password, credentials, addresses or signed metadata are uploaded.
- The initial funded suite passed all six checks in [CI run 37150606231](https://github.com/ucrem/ryo-wallet-next/actions/runs/37150606231), including native Windows, Linux and macOS Intel integrations. The expanded partial split-send case passed locally on Windows; its cross-platform result is recorded by the PR's current CI receipts.

### Critical profile limit

The fixture is `isolated-testnet-genesis-v1`. It submits genuinely valid past-timestamp blocks at difficulty one to **unchanged** upstream binaries; it does not disable validation, fake RPC signing or modify the consensus engine. It requires two connected peers because the upstream daemon otherwise reports `is_ready=false` at genesis.

However, the short chain has **not reached the current testnet fork**. In the reviewed source, testnet v10 starts at height 283,000; Bulletproofs, uniform IDs and increased required ring size activate at earlier forks. Passing this fixture **does not close current-fork compatibility or production readiness**. Its receipt explicitly says `current_fork_gate_passed=false`. See the [pinned fork schedule and difficulty implementation](https://github.com/ryo-currency/ryo-currency/blob/166cf188bf351e3eecff904450eda2785f9d0357/src/cryptonote_core/blockchain.cpp) and [feature configuration](https://github.com/ryo-currency/ryo-currency/blob/166cf188bf351e3eecff904450eda2785f9d0357/src/cryptonote_config.h).

## Public network probe

The Windows probe on 2026-10-03 observed no reachable TCP peer at `185.134.22.134:13310`, `81.19.208.43:13310`, `149.56.44.109:13310` (core seeds), or `45.77.68.151:13310` (Atom's explicit testnet peer). The daemon stayed at height 1 with no outgoing connections during both 45-second observations. These are observations from this host, not proof that all public testnet infrastructure is offline.

The explorer DNS resolves; HTTPS to its network API failed TLS negotiation. Testnet RPC attempts to `wallet-node.ryo-currency.com:13311` and `tnexp.ryo-currency.com:13311` were refused. Neither endpoint has been adopted as a functioning testnet node. Local detailed receipts are under `target/testnet-validation/probe-*`.

The same four TCP probes and 30-second daemon observations also found no peers and no chain progress from the Windows, Linux and macOS Intel GitHub runners in the linked CI run. This reproduces the availability problem beyond our host, without proving that no unlisted public peer exists. Port 13310 is P2P, not a wallet RPC endpoint.

Sources: [core seeds](https://github.com/ryo-currency/ryo-currency/blob/166cf188bf351e3eecff904450eda2785f9d0357/src/p2p/net_node.inl), [Atom testnet peer](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/src-electron/main-process/modules/daemon.js).

## Repeat the checks

Prepare the reviewed binaries with `node scripts/prepare-dev-runtime.mjs`. Every test verifies their identities again against `src-tauri/runtime-manifest.json` before starting a process.

Windows PowerShell:

```powershell
$env:RYO_TEST_DAEMON = (Resolve-Path src-tauri/.dev-runtime/ryod.exe).Path
$env:RYO_TEST_WALLET_RPC = (Resolve-Path src-tauri/.dev-runtime/ryo-wallet-rpc.exe).Path
# Optional: choose a NEW receipt path; an existing file is never overwritten.
$env:RYO_VALIDATION_REPORT = Join-Path (Resolve-Path target).Path 'testnet-genesis-report.json'
$env:RYO_SCAN_RESUME_REPORT = Join-Path (Resolve-Path target).Path 'wallet-scan-resume-report.json'
cargo test -p ryo-wallet-service --test testnet_validation --locked -- --ignored --nocapture
```

Linux / macOS Intel:

```sh
export RYO_TEST_DAEMON="$PWD/src-tauri/.dev-runtime/ryod"
export RYO_TEST_WALLET_RPC="$PWD/src-tauri/.dev-runtime/ryo-wallet-rpc"
export RYO_VALIDATION_REPORT="$PWD/target/testnet-genesis-report.json"
export RYO_SCAN_RESUME_REPORT="$PWD/target/wallet-scan-resume-report.json"
cargo test -p ryo-wallet-service --test testnet_validation --locked -- --ignored --nocapture
```

The test allocates fresh temporary storage, chooses private ports, keeps wallet login enabled, binds RPC/P2P/ZMQ to loopback, disables UPnP and connects exclusively to its second node. Test processes terminate on exit; temporary wallet/chain data is removed. The mainnet application and Atom data are never opened by the fixture. The RPC fault proxy exists only under `tests/support`, with bounded request sizes and timeouts.

Probe public availability without a wallet:

```sh
node scripts/probe-testnet.mjs 60
```

The probe contacts public testnet peers, creates a separate chain under `target/testnet-validation`, records only non-secret daemon health and stops its own process. No synchronization observed is a recorded outcome, not a wallet integration pass.

## Acceptance matrix

| Gate | Windows | Linux | macOS Intel | Stable requirement |
| --- | --- | --- | --- | --- |
| Funded isolated genesis-fork wallet suite | Passed locally and in initial CI | Initial CI passed; receipt required per revision | Initial CI passed; receipt required per revision | Foundation evidence only |
| Current-fork funded signing / relay / sweep | Blocked: usable current-fork testnet/fixture needed | Not verified | Not verified | Required |
| Real partial split-send failure | Passed locally in short fixture | Current CI receipt required | Current CI receipt required | Repeat on current fork |
| Funded recovery / real unknown reply / chain restart in short fixture | Passed locally and in initial CI | Initial CI passed; receipt required per revision | Initial CI passed; receipt required per revision | Repeat on current fork |
| Clean installed desktop, receive/send/recovery | Not verified | Not verified | Not verified | Required for each supported stable platform |
| Native tray, autostart, inactivity and OS suspend/resume | Not verified end to end | Not verified | Not verified | Required |
| Hybrid bootstrap handover to a complete local chain | Not verified | Not verified | Not verified | Required if hybrid is included |
| Native capability denials / CSP | Not verified end to end | Not verified | Not verified | Required |
| Private file permissions and crash process ownership | Owner ACL / Job Object gates open | Full gate open | Full gate open | Required |
| Installed update, OS signing/notarization, dependency/notices inventory | Gates open | Gates open | Gates open | Required |

CI adds this unchanged-runtime funded suite on all three supported native targets and retains **only** the non-sensitive acceptance and public-availability receipts. Each runner also probes public peers for 30 seconds to distinguish our host's connectivity from runner observations. That observation cannot waive a funded current-fork gate; a probe error is retained independently and does not prevent the isolated fixture from running. These checks do not prove GUI behavior, clean installation, OS signing, or current-fork readiness.

## Next dependent step

Obtain a maintained testnet peer and enough spendable test coins, or a reviewed reproducible fixture that exercises current consensus features. Verify daemon network, tip/fork and runtime identity before running a funded current-fork suite. A guessed RPC port, merely setting `--testnet`, or passing a mocked transaction is insufficient evidence.

Then close the remaining matrix rows, record outcomes and limitations, and choose beta / release candidate / stable based on that evidence. Follow the existing staging installer reuse and promotion process; this milestone does not publish a release automatically.
