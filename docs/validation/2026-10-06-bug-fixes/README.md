# Release audit fixes and review — 2026-10-06

## Scope

The owner authorized implementing and reviewing the bugs raised by the
[original audit](../2026-10-06-release-audit/README.md). That audit remains a
historical failing baseline; its receipts have not been replaced with passing
results. Changes continue in [draft PR #37](https://github.com/ucrem/ryo-wallet-next/pull/37).
There is no version bump, merge or release publication in this work.

Application fix revision: `3debc4f58d83c43f0df4495fa72fe2ce0126be63`.
The subsequent review removes an obsolete test-only preferences parser and
tests the real recovery loader instead; production behavior is unchanged.
Windows checks use isolated storage, product name, executable basename,
identifier, registry and loopback WebView2 debugger. No user wallet, password,
seed, private log or transaction is accessed. No computer-use, screenshot or
GUI-input automation is used.

## Fixes

| Issue | Change | Verification |
| --- | --- | --- |
| [#38](https://github.com/ucrem/ryo-wallet-next/issues/38) | Copy acknowledgement belongs to the exact value being copied; selection changes remount that state | Actual React regression, clipboard failure/retry control, late-response regression |
| [#39](https://github.com/ucrem/ryo-wallet-next/issues/39) | Consume Send contact navigation intent once, validate against current contacts, remove matching intent after successful deletion | Delete → Send regression and deleted-contact intent regression; no transfer sent |
| [#40](https://github.com/ucrem/ryo-wallet-next/issues/40) | Controlled contact selection clears when the address or payment ID is edited or a payment request replaces the recipient | Actual React input/change events, payment-ID edit and reselect regression |
| [#41](https://github.com/ucrem/ryo-wallet-next/issues/41) | Bounded validated non-wallet configuration loading, preserved invalid-file backups, defaults/unconfigured root and visible guidance instead of setup failure | Installed Windows: malformed and semantically invalid preferences/root, backup byte equality, IPC available after each restart, unrelated data preserved. Unit tests include oversized data and Unix links |
| [#42](https://github.com/ucrem/ryo-wallet-next/issues/42) | Ship upstream Ryo notices, runtime hashes/provenance, dependency notice text and lockfile-bound inventory | Six installed files checked byte-for-byte; build rejects stale runtime/lockfile/notice/backport data. Completeness limits below remain |
| [#43](https://github.com/ucrem/ryo-wallet-next/issues/43) | Local source backport of the official iterator-pointer correction into the GTK-required GLib 0.18.5 | Original archive digest verified; 121 upstream files compared, only `src/variant_iter.rs` changed. Linux optimized test exercises all five affected methods against the selected local dependency |
| [#44](https://github.com/ucrem/ryo-wallet-next/issues/44) | Remove unused development shadcn CLI and its advisory-affected dependency chain | `braces` absent from selected pnpm graph; complete npm audit reports zero vulnerabilities |
| [#45](https://github.com/ucrem/ryo-wallet-next/issues/45) | Pin corrected `source-map-js` 1.2.2 | Normal indexed map succeeds; the former billion-line-offset probe is explicitly rejected in 63 ms, within a 1500 ms child deadline; npm audit clean |
| [#46](https://github.com/ucrem/ryo-wallet-next/issues/46) | Unique same-directory temporary files, file sync and atomic replacement for node settings/data-location writes | Service/host regressions plus installed IPC repeated saves/network changes with legacy `.new` stages preserved |

## Review

Primary-agent source review covered the complete first-party diff, recipient
state, asynchronous clipboard/deletion outcomes, recovery bounds/backup failure
behavior, path ownership boundaries, atomic replacements and dependency/package
provenance. No independent-agent review was performed.

The review corrected these points before completion:

- React Query ignores `setQueryData(..., undefined)`; removal uses the exact
  query key so the stale navigation intent is actually deleted.
- Cached contact objects must be checked against current contact data before
  initializing a recipient. Matching includes payment ID as well as address.
- Removed an obsolete parser retained only in tests. Persistence checks now
  load through the same recovery implementation as application startup.
- Lockfiles and pinned notice/vendor bytes use Git attributes that prevent
  host newline conversion from breaking packaged integrity checks. Upstream
  notice/source whitespace is deliberately retained.
- Notice verification requires both lockfiles and both original Ryo notices,
  and verifies the reviewed GLib patch. Local patched GLib provenance is included
  even though Cargo metadata describes it as a path dependency.
- Windows pnpm metadata collection uses a fixed literal launcher command;
  collected package data cannot become shell arguments.

No additional unresolved defect was found in the reviewed changes. This does
not certify unrelated GUI/operating-system or current-fork acceptance gates.

## Validation

| Check | Result |
| --- | --- |
| Complete frontend suite | 82 passed / 19 files |
| TypeScript, full ESLint, application version | Pass; remains `0.1.0-alpha.6` |
| Windows workspace tests | 110 passed; 13 existing opt-in/helper tests ignored; the Linux iterator test is inapplicable on Windows |
| Rust formatting and workspace Clippy with `-D warnings` | Pass |
| Packaging integrity regressions | 4 passed, plus 2 existing runtime-manifest checks |
| npm audit | Zero critical/high/moderate/low/info findings |
| Optimized standalone/native NSIS build | Pass, embedded frontend; separate test product |
| Installed Windows targeted checks | 30/30 passed: notice/runtime hashes, embedded frontend startup, interrupted-stage saves, malformed/invalid configuration recovery, exact preservation and normal uninstall |
| Linux optimized GLib regression | Pass against `vendor/glib`; normal host, import and locked-node checks also pass |
| Source-fix CI | All 7 jobs pass on `3debc4f`: [run 37511603241](https://github.com/ucrem/ryo-wallet-next/actions/runs/37511603241); Linux host 32 passed / 1 ignored; macOS host 31 passed / 1 ignored |
| Three-platform native funded/scan-cache fixtures | 15 funded and 5 scan-resume assertions per platform, receipt ZIP hashes/source/runtime verified; real genesis-chain transactions at height 195, processed cache height 81; no current-fork acceptance claim |

The first Windows harness attempt incorrectly required a numeric `ExitCode`
from a process retrieved with PowerShell `Get-Process`; normal exit had been
observed but that property was null. The prior receipt is retained. The corrected
harness requires observed exit/no remaining test process, then fresh real IPC
startup. This harness error is not classified as an application bug.

Evidence and reproducible focused probes are saved alongside this report.
Full build/test logs remain under `target/bug-fixes-2026-10-06/`.

## Limits and follow-up

- GLib retains upstream version 0.18.5 for GTK compatibility. Version-only
  scanners may continue matching RUSTSEC-2024-0429; mitigation is the verified
  source backport, not a clean registry-version claim. Remove it when the
  selected GTK graph accepts an upstream corrected version. See
  [backport provenance](../../../vendor/README.md),
  [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) and
  [official fix](https://github.com/gtk-rs/gtk-rs-core/pull/1343).
- The installed notice payload covers 592 Cargo packages and 117 production
  npm packages, with 1098 available notice files. 49 published packages have
  no notice files, recorded explicitly. Internal static-library composition of
  the official Ryo binaries and full distribution license clearance still need
  upstream/manual review; shipping the new payload does not establish that.
- Current-fork funded acceptance awaits Ryo testnet restoration. Genesis-chain
  fixtures verify real signatures/relay but do not certify the active fork.
- GUI inputs/native file-dialog navigation, real OS login/sleep and complete
  Linux/macOS installed acceptance remain separate from these fixes. Previously
  recorded lifecycle/updater results remain in the original audit.
- Paid publisher signing/notarization remains excluded by the owner's request.

At this report's `fac5383` snapshot, issues were left open for review/merge.
The owner's subsequent explicit closure request and the expanded notice
verification are recorded in the [issue closure follow-up](../2026-10-06-issue-closure/README.md).
There is no stable-readiness claim.
