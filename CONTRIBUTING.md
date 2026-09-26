# Contributing

Start each independent change from the latest `staging` on its own branch and
open a pull request into `staging`. Codex work uses the `codex/<topic>` branch
prefix. Do not push feature work directly to `staging` or `main`.
Use the normal project checkout for local `pnpm tauri dev` testing. If a
temporary Git worktree is used, remove its checkout and generated directory
after its changes reach `main`.
Both branches are protected on GitHub: PRs and passing checks are required,
including the staging-installer reuse check for `main`.

After review, merge the change into `staging`. The staging push builds Linux
DEB/RPM, a macOS Intel DMG, and a Windows NSIS setup EXE, with signed update packages. Apple Silicon packaging is paused until a reviewed native Ryo wallet RPC exists. Download
and verify those artifacts before opening a pull request from `staging` to
`main`. That PR runs one promotion check: it verifies that the proposed merge
has exactly the same files as the **staging commit** and that its successful
installer build contains the expected installers and updater signatures. It reports SHA-256 hashes without
building or uploading new installers. Merge to `main` only after the staging
artifacts and promotion check are accepted. The release workflow then verifies
the final main commit has the same files as that staging commit and publishes
the existing installers. If `staging` moves, review its new build and the
updated promotion PR. A merge commit on `main` alone does not require a sync
back into `staging`. Do not push directly to `main`.

Keep the project clearly independent from Ryo Currency. Run the relevant
frontend, Rust, and desktop checks for the files you change. Packaging
artifacts remain development previews until the
[release gates](docs/PACKAGING.md) are met.
