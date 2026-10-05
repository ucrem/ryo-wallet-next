# Diagnostic report

In the native desktop app, open **About → Diagnostics → Export diagnostic report** and choose a new `.json` file outside application storage. It works with the wallet locked and before setup. Cancellation writes nothing. Existing files are never overwritten; choose another filename for another snapshot.

Schema **2** contains:

- Application version, debug/release profile, target architecture, source revision and whether tracked source was modified when compiled. Git metadata is optional when building outside a checkout; build identity is not a publisher signature or an attestation.
- OS/architecture, snapshot time and application uptime. On Windows, native major/minor/build and the numeric update build revision identify the installed OS patch level. Windows 11 also uses kernel major version 10; do not infer the marketing name from that number alone.
- Available WebView engine version (WebView2 on Windows), CPU parallelism available to the app, and Windows physical RAM total/available at collection time. The engine query identifies the default available runtime, not a per-window engine instance or GPU/driver diagnostics.
- On Windows, the selected local data volume's filesystem category, persistent ACL support, read-only flag, and total/free bytes available to the current user. Quotas may affect these values. Network shares and mapped network drives are skipped. No volume label, serial, drive letter or path is recorded; no write or actual wallet-permission test is performed.
- Reviewed Ryo runtime manifest version and whether the wallet RPC and daemon binaries pass the existing hash verification at export time. A false result means missing/unverified/unavailable, not a claim that a process has crashed. No runtime is launched.
- An explicit preference subset: theme, inactivity lock duration, minimize-to-tray, and the saved startup preference. The latter is not a fresh check of OS startup registration; other preferences are omitted.
- Selected network and node mode, wallet lifecycle, cached wallet scan height, node heights/reachability/readiness, and elapsed time for the node status probe. This timing includes actor scheduling and cached-health behavior; it is not an isolated network latency measurement.

Heights and byte counts are decimal strings to retain full integer precision. Observation categories are fixed labels such as `node_unreachable`, `node_status_timeout`, `wallet_rpc_busy` and `environment_timeout`.

The report excludes seeds, keys, passwords, RPC credentials, signed transaction metadata, wallet IDs/names/addresses, node endpoints, data paths, transaction history and raw logs/error text. It also excludes usernames, computer names, device/volume identifiers and process command lines. The app never uploads the file. Timestamps, heights, preferences and system/resource facts still describe activity and the environment; review the report before sharing it yourself.

Collection is read-only. Wallet lifecycle and node probes have deadlines; cached scan progress is read without queueing behind an upstream refresh. Environment collection runs off the UI thread with a four-second deadline in parallel with those probes. An OS call already running in a worker cannot be forcibly cancelled; an expired environment probe is omitted and labeled. Missing data remains null/unavailable and is not interpreted as zero or synchronized. Windows OS/RAM/volume fields remain null on other platforms in this implementation. Resource availability can change immediately after collection. The report is a snapshot, not a trace or a proof that a wallet's balance/history is current.

Tests cover endpoint/session/malformed-height exclusion, populated-report preference/path redaction, bounded API-version text, filesystem category filtering, native Windows OS/RAM/disk probes, remote/invalid storage refusal, cancelled export, app-storage refusal, existing-file preservation, locked unconfigured collection without lifecycle changes, native-only dispatch and duplicate-click suppression. Native dialog interaction and installed cross-platform export remain manual acceptance checks.

## Native API references

- [Windows version information](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-rtlgetversion)
- [Physical memory snapshot](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)
- [Disk space available to the caller](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw)
- [Filesystem capabilities](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw)
