# Local wallet creation test

This development flow is available on Linux only. It accepts the reviewed
Ryo 0.6.1.0 `ryo-wallet-rpc` executable at the exact SHA-256
`5ef7395ce822a02905e68a63abf6f3a9d75654c8a9773c23777902b5522169e3`.
The executable is not bundled, downloaded by the app, or accepted from `PATH`.
Keep this test wallet empty; do not transfer funds to it.

From this branch's checkout, with the verified executable already on disk:

```sh
RYO_WALLET_RPC_BIN=/absolute/path/to/ryo-wallet-rpc pnpm tauri dev --config src-tauri/tauri.local-test.conf.json
```

The test configuration gives the app a separate identity and settings directory,
so it can run alongside an installed Ryo Wallet Next. Choose a new empty data
folder, choose a local node, then create a wallet with a password of at least
12 bytes. Write the recovery phrase on paper and answer the three word prompts.
Lock the wallet, return to Start, choose Open existing wallet, select the new
wallet, and enter its password. Try a wrong password first; the files must remain
available for a correct retry. If you quit before confirming the backup, open
the wallet again to resume the backup step.

The local node option does not start `ryod` yet. Creation and backup work with
an empty wallet; balance or sync may be unavailable until a node is running.
This test does not validate packaged binaries, Windows ACLs, macOS support, or
transactions.
