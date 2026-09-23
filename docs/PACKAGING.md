# Desktop packaging

The current bundles are **development previews**, not wallet releases. The UI
does not yet create, restore, open, sign, or submit wallets. No `ryo-wallet-rpc`
or `ryod` executable is bundled. Do not use these artifacts with funds.

| Platform | CI runner | Preview bundle |
| --- | --- | --- |
| Linux x64 | Ubuntu 24.04 | `.deb` and `.rpm` |
| macOS Intel | macOS 15 Intel | `.dmg` |
| macOS Apple Silicon | macOS 15 arm64 | `.dmg` |
| Windows x64 | Windows 2025 | NSIS `-setup.exe` |

The [desktop installer workflow](../.github/workflows/desktop-bundles.yml)
builds each target on its native operating system **only after a change is
merged into `staging`**. The resulting five artifacts are retained for 30 days;
no GitHub Release is created. A pull request from `staging` to `main` waits for
the successful build of that exact staging commit, downloads the existing
installers, verifies all five formats, reports SHA-256 hashes, and uploads those
same files to the PR run. It never rebuilds the installers for promotion. If
the staging commit changes, another staging build and review are required.
See the [development branch flow](../CONTRIBUTING.md).

macOS previews use an ad-hoc signature; Windows and Linux previews are
unsigned. They are not a substitute for platform installation and runtime
tests.

Install the pinned Rust, Node, and pnpm versions plus the
[Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/), then
run `pnpm install --frozen-lockfile`. Build on the target operating system:

- Linux: `pnpm exec tauri build --ci --bundles deb,rpm`
- macOS: `APPLE_SIGNING_IDENTITY=- pnpm exec tauri build --ci --bundles dmg`
- Windows: `pnpm exec tauri build --ci --bundles nsis --no-sign`

Bundles are written under `target/release/bundle/` because this project uses
a Cargo workspace. The committed `.icns`, `.ico`, and PNG sizes are generated
from `src-tauri/icons/icon.png`; the [asset provenance](ASSETS.md) applies to
all of them.

Before a user-facing release, each platform still needs verified Ryo runtime
binaries, dependency and license notices, signing (and Apple notarization),
installation tests, and wallet-flow tests on clean systems. See the
[MVP criteria](MVP.md) and [implementation status](IMPLEMENTATION_STATUS.md).
