# Issue closure and remaining notice verification — 2026-10-06

## Issue decisions

The owner explicitly authorized closing verified fixes. Issues **#38, #39,
#40, #41, #43, #44, #45 and #46** were closed as `completed`. Each issue retains
its original failing reproduction and now records the implementation, tests,
reviewed commit and the fact that PR #37 is still unmerged/unreleased.

The reviewed fix head `fac5383bfb9ad6b928eba520b9008b008c820a13` passed all
seven jobs in [CI run 37513356575](https://github.com/ucrem/ryo-wallet-next/actions/runs/37513356575).
The [previous fix report](../2026-10-06-bug-fixes/README.md) and its receipts
remain unchanged historical evidence for the application behavior fixes.

**#42 remains open.** The original packaging omission is corrected, and the
available materials have been expanded, but its requirement for coverage of
actual linked runtime dependencies is not yet established.

## Additional materials and verification

Material revision: `ebdd7b299b81162de55357cad6cecefec6d290ab`.

- 43 Cargo packages whose archives omit notices now have reviewed source
  supplements. Most use the exact commit in the published `.cargo_vcs_info.json`.
  `libappindicator-sys` also has a matching source package name/version at that
  commit. Selectors' published `lib.rs` matches the recorded source commit
  byte-for-byte and declares MPL 2.0; its header, full SPDX standard license and
  unmodified source-access URL are included.
- The existing `AUTHORS` files for both `r-efi` versions contain licensing terms
  and are now retained by the collector. The original collector only looked for
  conventional license/notice filenames.
- Eight additional source-component notices from the exact reviewed Ryo source
  commit are included for epee, LMDB, fmt, MiniUPnPc, RapidJSON, Unbound and sodium
  materials. This conservative source collection is not asserted to be a
  verified executable bill of materials.
- The inventory still covers 592 Cargo and 117 production npm packages; 1162
  notice files are represented. Packages without a collected notice dropped
  from 49 to **4**.
- Packaging verifies the supplemental package/provenance relationships and
  hashes, and the Ryo component notice hashes/source commit. Five integrity
  regressions and two runtime-manifest tests pass; full ESLint passes.
- A clean isolated Windows NSIS installation passes **41/41 checks**, including
  all **37 legal payload file hashes**, both original runtime hashes, installation
  and uninstall. No user wallet or GUI-input automation is used.

The locked graph and runtime executable bytes are unchanged by the additional
notice work. No signature, wallet, transaction or network selection behavior
was modified. The separate material commit CI runs automatically; its status
does not replace the completed fix-head receipt above.

The material commit's macOS genesis job initially failed while downloading
`yoke` from the crates.io registry (timeouts/broken pipe), before the service
tests could execute. Its log is retained and the affected job is eligible for
retry once the run finishes. It is not classified as a new application bug.

## Four package notice gaps

| Package | Verified scope / remaining requirement |
| --- | --- |
| `rustls-platform-verifier-android 0.1.1` | Absent from the filtered resolve graph for Windows MSVC x64, Linux x64 and macOS Intel; a conservative all-target inventory entry, not a shipped desktop component |
| `winapi-i686-pc-windows-gnu 0.4.0` | Absent from all three supported desktop resolve graphs |
| `winapi-x86_64-pc-windows-gnu 0.4.0` | Absent from all three supported desktop resolve graphs |
| `react-remove-scroll-bar 2.3.8` | Production npm dependency declares MIT, but its published archive omits the notice. Published `gitHead` cannot be resolved in the declared public repository, matching version tags are absent, and the public source currently declares 2.3.7. Do not describe that source's notice as publication-matched for 2.3.8; obtain the corresponding notice or publisher confirmation |

Target selection is checked using locked Cargo metadata with `--filter-platform`
for all three supported targets. The npm checks record exact package/version,
published Git SHA, tag/commit lookup outcomes and public source version.
This is a material-provenance gap, not a claim that the npm package is malicious
or that its MIT declaration is invalid.

## Upstream Ryo evidence still needed

The three official 0.6.1.0 archive contents were enumerated: no SBOM, dependency
version inventory, link map or full build receipt is supplied. The reviewed
source links externally resolved libraries such as Boost, OpenSSL and ZeroMQ,
as well as vendored components. Build conditions vary by target and options.

The two Windows PE import tables contain only system DLLs; the extra
`libwinpthread-1.dll` in the original archive is not imported by these two
executables. Version strings include OpenSSL 1.1.1d and LMDB 0.9.70. These
observations are saved, but neither import tables nor version strings establish
the exact statically linked dependency composition or all applicable notices.
No new application vulnerability or missing-DLL defect is claimed from them.

To complete #42, obtain a per-platform bill of materials for the official
`ryod` and `ryo-wallet-rpc` artifacts, or the corresponding build records/link
maps with dependency revisions and their attribution/license notices. The
verified artifact hashes are already recorded in
[runtime-inventory.json](../../../src-tauri/resources/legal/runtime-inventory.json).
No message was sent to Ryo maintainers as part of this check.

## Current testnet check

A fresh disposable unchanged-runtime testnet daemon was observed for 30 seconds.
It exited normally and no public synchronization was observed. The detailed
TCP/chain observations are retained in the receipt. This bounded observation
does not prove permanent global unavailability, and does not satisfy the
current-fork funded transaction gate.

## Evidence

The `evidence/` directory records issue state decisions, packaging checks,
remaining package selection/source lookups, runtime import observations and the
fresh testnet availability receipt. Reproduction scripts are included as text;
full local build logs and upstream metadata remain in
`target/issue-closure-2026-10-06/`.
