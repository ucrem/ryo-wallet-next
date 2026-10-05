# Diagnostic report

In the native desktop app, open **About → Diagnostics → Export diagnostic report** and choose a new `.json` file outside application storage. It works with the wallet locked and before setup. Cancellation writes nothing. Existing files are never overwritten; choose another filename for another snapshot.

The schema-versioned report contains the app version, OS/architecture, snapshot time, selected network and node mode, wallet lifecycle, cached wallet scan height, and node heights/reachability/readiness. Heights are decimal strings to retain full integer precision. Observation categories are fixed labels such as `node_unreachable`, `node_status_timeout` and `wallet_rpc_busy`.

The report excludes seeds, keys, passwords, RPC credentials, signed transaction metadata, wallet IDs/names/addresses, node endpoints, data paths, transaction history and raw logs/error text. The app never uploads the file. The timestamp and heights still describe activity; review the report before sharing it yourself.

Collection is read-only. Wallet lifecycle and node probes have deadlines; cached scan progress is read without queueing behind an upstream refresh. Missing data remains null/unavailable and is not interpreted as zero or synchronized. The report is a snapshot, not a trace or a proof that a wallet's balance/history is current.

Tests cover endpoint/session/malformed-height exclusion, cancelled export, app-storage refusal, existing-file preservation, locked unconfigured collection without lifecycle changes, native-only dispatch and duplicate-click suppression. Native dialog interaction and installed cross-platform export remain manual acceptance checks.
