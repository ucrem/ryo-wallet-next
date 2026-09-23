# Contributing

Start each independent change from the latest `staging` on its own branch and
open a pull request into `staging`. Codex work uses the `codex/<topic>` branch
prefix. Do not push feature work directly to `staging` or `main`.

After review, merge the change into `staging`. The staging push builds Linux
DEB/RPM, macOS Intel/Apple Silicon DMGs, and a Windows NSIS setup EXE. Download
and verify those artifacts before opening a pull request from `staging` to
`main`. That PR downloads the installers built for the **exact staging commit**,
checks that all five exist, reports their SHA-256 hashes, and uploads the same
installer files for review. It does not compile a new release. Merge to `main`
only after the staging artifacts and PR checks are accepted. If `staging` moves,
review the new build and the updated promotion PR. Do not push directly to
`main`.

Keep the project clearly independent from Ryo Currency. Run the relevant
frontend, Rust, and desktop checks for the files you change. Packaging
artifacts remain development previews until the
[release gates](docs/PACKAGING.md) are met.
