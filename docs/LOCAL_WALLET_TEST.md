# Local wallet creation test

This development flow is available on Linux x64 only. `pnpm tauri dev` prepares
the reviewed Ryo 0.6.1.0 wallet RPC binary on first run and verifies its exact
SHA-256. The app never accepts an executable from `PATH`. Keep this test wallet
empty; do not transfer funds to it.

From the project directory:

```sh
pnpm tauri dev
```

The development app automatically uses a separate identity and settings directory,
so it can run alongside an installed Ryo Wallet Next. Choose a new empty data
folder, choose a local node, then create a wallet with a password of at least
12 bytes. Write the recovery phrase on paper and answer the three word prompts.
Lock the wallet, return to Start, choose Open existing wallet, select the new
wallet, and enter its password. Try a wrong password first; the files must remain
available for a correct retry. If you quit before confirming the backup, open
the wallet again to resume the backup step.

After confirming the backup, use the plus icon beside the address to create a
second receive address. Select it and copy it with the adjacent icon. Lock and
reopen the wallet to confirm the address remains in the list.

The local node option does not start `ryod` yet. Creation and backup work with
an empty wallet; balance or sync may be unavailable until a node is running.
This test does not validate packaged binaries, Windows ACLs, macOS support, or
transactions.
