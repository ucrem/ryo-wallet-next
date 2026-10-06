# Autonomous release audit — 2026-10-06

## Scope and source

- Audited application revision: **`e5a7f55df8515b0873565286fcd7b2c8ec93bbc7`**, PR #37, unreleased changes on the alpha.6 baseline.
- Requested outcome: execute available release checks, preserve evidence, create bug issues, and **apply no application fixes**.
- Paid publisher signing/notarization is excluded by the owner's instruction. Cryptographic updater authentication remains in scope.
- Local environment: Windows x64 build **26200.9550**, Node **24.19.0**, Rust **1.98.1**, PowerShell **7.6.5**, fallback pnpm **11.19.0**. The reviewed CI uses pinned pnpm **12.6.0**.
- All wallet/RPC tests used disposable wallets and fresh storage. No user wallet password, seed, private logs or transaction was accessed. The user's desktop process remained running; its original executable was restored with the same SHA-256.
- Native installer testing used a separate product, executable basename, bundle identifier, registry entry and app-data profile. The application source was unchanged. No computer-use, screenshot or UI-input automation was used.

## Findings and issue count

**9 issues opened:** 5 application behavior defects, 1 verified packaging omission, and 3 dependency findings. The dependency findings have narrower evidence than an application exploit; that distinction is explicit in their issues.

| Issue | Proposed priority | Finding | Evidence / certainty |
| --- | --- | --- | --- |
| [#38](https://github.com/ucrem/ryo-wallet-next/issues/38) | P2 | Receive still says `Copied` after selecting another address | Actual React component reproduction; clipboard retains first address while second address is displayed |
| [#39](https://github.com/ucrem/ryo-wallet-next/issues/39) | P2 | Deleted contact remains the default recipient on returning to Send | Actual React/query-cache reproduction after successful mocked deletion; no transaction sent |
| [#40](https://github.com/ucrem/ryo-wallet-next/issues/40) | P3 | Contact selector describes the old contact after manually editing the recipient | Actual React input/change events; selector and visible address diverge |
| [#41](https://github.com/ucrem/ryo-wallet-next/issues/41) | P1 | Malformed non-wallet preferences/data-location configuration causes startup panic | Installed native Windows process exits **101** with both malformed files; restoring valid configuration returns exit **0** |
| [#42](https://github.com/ucrem/ryo-wallet-next/issues/42) | P1 | Installer omits third-party notices/inventory for bundled runtimes | Complete installed payload enumerated; NSIS license input is the project's MIT license; applicable runtime notices absent |
| [#43](https://github.com/ucrem/ryo-wallet-next/issues/43) | P2 | Linux selects `glib 0.18.5`, matching a known unsound iterator advisory | OSV/RustSec match and selected Linux dependency tree confirmed; application reachability/crash **not demonstrated** |
| [#44](https://github.com/ucrem/ryo-wallet-next/issues/44) | P2 | Development graph selects advisory-affected `braces 3.0.3` | Full pnpm audit reports high severity; bounded local nesting probes **did not reproduce** stack exhaustion |
| [#45](https://github.com/ucrem/ryo-wallet-next/issues/45) | P2 | Build/test graph selects `source-map-js 1.2.1` with an offset DoS advisory | Full pnpm audit match; isolated large-offset probe exceeds a **1500 ms** deadline, while the zero-offset control completes in about **67 ms** |
| [#46](https://github.com/ucrem/ryo-wallet-next/issues/46) | P2 | Interrupted settings save leaves a fixed stage that blocks subsequent saves | Unchanged storage API, disposable injected stage: repeated `AlreadyExists`, committed settings preserved, retry succeeds only after external fixture cleanup |

P1/P2/P3 are proposed project priorities. They are separate from advisory-provider severity. Neither recipient finding proves an automatic transfer; transaction review remains required.

## Checks executed

| Area | Result | Evidence / limit |
| --- | --- | --- |
| Existing frontend suite | **PASS: 72 tests / 18 files** | Local unchanged source; extra audit probes are separate |
| TypeScript, complete ESLint, version consistency | **PASS** | Baseline remains `0.1.0-alpha.6`; local logs retained |
| Rust workspace tests | **PASS: 105 passed, 13 opt-in/helper tests ignored** | Host 25, service 69, transport 11; applicable opt-in checks below run separately |
| Current-source CI | **PASS: all 7 jobs** | [Run 37443371047](https://github.com/ucrem/ryo-wallet-next/actions/runs/37443371047), source revision checked on all receipt artifacts |
| Linux/macOS host tests and native diagnostic collectors | **PASS** | Exact CI job logs: Linux **27 passed / 1 ignored**, macOS **26 passed / 1 ignored**; native version/RAM/disk collector assertions executed |
| Three-platform funded and scan-resume CI receipts | **PASS on the initial fork** | Each platform: 15 funded checks at chain height **195**, 5 scan-resume checks at processed height **81**; artifact ZIP digests verified |
| Local real funded transactions and busy-scan cache persistence | **PASS: 2 opt-in tests, 92.00 s** | Genuine signatures, fees, subaddress receipt, recovery, sweep, partial/unknown outcomes and restart; initial fork only |
| Real empty-wallet operations and password reauthentication | **PASS: 1 opt-in test, 17.38 s** | Disposable contacts/name/address labels, payment requests, password/key-image behavior and session checks |
| Native import boundary | **PASS: 1 opt-in test, 30.02 s** | Source wallet bytes unchanged; copied cache/short-password opening, wrong-password cleanup and reopening |
| Local node while wallet locked, node reuse and persisted-chain restart | **PASS: 1 opt-in test, 52.57 s** | Actual reviewed runtimes copied into the audit package; disposable mainnet data, independent wallet/node ownership |
| Windows existing-owner Modify-only storage / authenticated RPC startup | **PASS: 1 opt-in test, 6.54 s** | Actual reviewed empty RPC listener; no wallet opened; ordinary unelevated owner |
| Production frontend and isolated optimized NSIS build | **PASS** | Native compilation **54.08 s**, embedded frontend; distinct test product/identifier/basename |
| Clean current-user install, runtime hashes, normal native close, same-version repair and uninstall | **PASS** | 7 installer checks pass; repaired preferences hash unchanged; test registration and executable removed |
| Recovery from malformed configuration | **FAIL: 2 cases, one defect** | Both native cases exit 101; tracked together in #41 |
| Recovery from a leftover interrupted settings stage | **FAIL: one defect** | Actual public storage API and original lockfile; repeated saves remain blocked by `settings.json.new`, #46. The analogous host data-location writer is source-inspected, not separately invoked |
| Targeted React audit probes | **3 PASS / 3 FAIL** | Failures correspond to #38–40. Controls cover clipboard retry, expired confirmation and preferences saving while General is locked |
| Update-manifest/runtime preparation fixtures | **PASS: 3 Python + 2 Node tests** | Correct preparation/validation behavior; separate from installed upgrade acceptance |
| Published updater artifacts: Windows EXE, macOS archive, DEB, RPM | **PASS: all 4** | Downloaded public alpha.6 artifacts match checksums/feed signatures. Pinned Minisign verifier accepts signatures and rejects changed bytes; authenticated version is alpha.6 and differs from a forged 99.0.0 label |
| Production npm dependency audit | **PASS: zero reported vulnerabilities** | Provider/database snapshot; not an application security audit |
| Full npm dependency audit | **FAIL: 2 high-severity findings** | Both in development dependencies, #44–45; no dependency changes made |
| Rust registry package review | **Known findings** | OSV checked **592 locked registry entries** across targets; one unsoundness advisory plus six unmaintained-package notices |
| Published testnet availability | **EXTERNALLY BLOCKED** | Fresh verified testnet daemon remained at genesis height 1; 0/4 upstream-listed peers reachable on port 13310. Same no-sync result in all three native CI receipts |

### Financial fixture boundary

The genuine funded fixture uses unchanged reviewed Ryo runtimes and consensus, but a short genesis-era testnet chain. **Every funded/cache receipt records `current_fork_gate_passed=false`.** It does not exercise the maintained current testnet fork (v10 at height 283000) or close current-protocol acceptance.

The public testnet probe is a bounded observation, not a permanent assertion that the network will remain unavailable. Its reports record runtime identity, timestamps and peer observations. No new guessed endpoint was used to claim acceptance.

### Updater authentication boundary

The cryptographic probe uses `minisign-verify 0.2.5`, the same pinned verification library as updater 2.12.0, and reads the trusted version only after signature verification. It tests **published alpha.6 artifacts**, not a newly promoted candidate installer or the in-app upgrade/authorization flow. No downloaded production installer was executed. The isolated NSIS package has updater-artifact generation disabled and is only a test fixture.

## Gates that remain unverified

These are acceptance limits, not additional confirmed bugs and not counted in the nine issues.

| Gate | Status / reason |
| --- | --- |
| Funded current-fork send/receive/sweep/recovery | Waiting for the Ryo developers' maintained testnet restoration or a separately reviewed current-fork fixture |
| Complete native wallet UI workflows on clean installs | Actual services and React components exercised; native dialogs, rendered UI and end-to-end GUI authentication were not automated under the owner's standing computer-use constraint |
| Clean installed Linux/macOS desktop behavior | Native CI host tests/financial fixtures pass; no clean interactive Linux/macOS installation was available in this Windows audit |
| Tray menu actions, OS-login startup, hidden-window idle lock, suspend/resume | Preferences/idle rules and ordinary native tray setup are covered; OS session transitions, system sleep and tray interactions not exercised on the user's desktop |
| Full hybrid bootstrap handover after complete-chain synchronization | Requires an end-to-end synchronized local-chain/bootstrap scenario; short funded fixture does not establish this behavior |
| Native capability-denial and CSP behavior | Source configuration and service/path/session negative tests inspected; native WebView denial behavior not exercised |
| Installed in-app update and authorization lifecycle | Artifact cryptography passes; candidate version is not published and no signed installed upgrade was executed |
| Complete installed crash recovery and Unix descendant process ownership | Windows abrupt-owner-exit Job Object test passes in the existing suite; Unix/full installed acceptance remains open, including the documented spawn-to-assignment limit |
| Complete dependency/notices inventory | Verified omission tracked in #42; this audit does not supply the missing inventory |

## Deduplication and negative observations

- `RUSTSEC-2024-0429` and `GHSA-wrw7-89jp-8q8g` identify **one** glib finding.
- Six other Rust packages are flagged as **unmaintained**: `proc-macro-error` and five `unic-*` packages. These are maintenance risks, not six newly demonstrated exploits, and no extra bug issues were opened for them.
- The braces audit registry suggests a patched range `>=3.0.4`, while the [primary advisory](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm) currently lists no patched version. This discrepancy is retained for triage. Nested compile/expand controls completed on Node 24.19.0. The initial over-length input was correctly rejected and is not a DoS reproduction.
- An initial native harness sampled process/window state before window creation. Its timing error was corrected in the **audit harness only** and its first-attempt log preserved. This was not reported as an application defect.
- A native window briefly existed in the malformed-configuration cases, but subsequent exit code 101 and captured setup panic establish failure. The final classification uses those observations; window existence alone is not a successful startup or rendered-UI check.
- The GitHub app connector could not create issues (HTTP 403). The nine authorized issues were created with the existing Git authentication already used for this PR; no credential was printed or saved.
- The interrupted-write reproduction uses a disposable injected stage rather than killing a user process. Its standalone Cargo probe reuses the original lockfile, and the application implementation is unchanged. The committed settings are preserved even while repeated writes are blocked.

## Saved evidence and reproductions

Permanent, sanitized evidence is in this directory:

- [Structured results and issue index](results.json).
- `evidence/`: native installer observations, financial/cache receipts, exact-source three-platform receipt summaries, dependency findings and updater cryptographic/checksum results.
- `reproductions/`: copies of the additional probe sources as `.txt`; these are audit fixtures and deliberately are not added to normal CI as known-failing application tests.

Full local command logs, downloaded artifacts and runnable audit harnesses remain in:

```text
target/release-audit-2026-10-06-e5a7f55/
```

To repeat the React reproductions from the repository root, copy the preserved `ui-probes.test.tsx.txt` and `vitest.audit.config.mts.txt` back to that target directory using their names without `.txt`, then run:

```text
pnpm exec vitest run --config target/release-audit-2026-10-06-e5a7f55/vitest.audit.config.mts
```

At the audited revision the expected audit outcome is **three failing bug assertions and three passing controls**. The normal frontend suite remains 72/72 passing. These probe failures are preserved for review; none was corrected.

The native installer harness is scoped to its distinct audit identity, checks registry/install paths, and must never be redirected to a real wallet/app-data profile. All newly opened issues contain human-readable reproduction steps, proposed priority, expected/actual behavior, the immutable source revision and evidence limits.

## Outcome

The available autonomous checks are recorded, with nine open issues and explicit remaining acceptance limits. **No application implementation, dependency version, release version or published artifact was changed, and no bug was fixed.** This report does not declare stable-release readiness.
