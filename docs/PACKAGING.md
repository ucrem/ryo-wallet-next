# Desktop packaging

The bundles are **development previews**, not production wallets. The already
published alpha.4 installers omitted `ryo-wallet-rpc` and cannot use wallet
flows. Builds from this source stage the reviewed upstream 0.6.1.0 wallet RPC
binary inside each supported package. They can use the existing create, backup,
open, lock, balance, receive-address and remote-node flows; `ryod` is not bundled,
so local-node operation is still unavailable. Recovery from a phrase and
transactions are not available in the UI. Do not use these artifacts with funds.

| Platform | CI runner | Preview bundle |
| --- | --- | --- |
| Linux x64 | Ubuntu 22.04 | `.deb`, `.rpm` and `.AppImage` |
| macOS Intel | macOS 15 Intel | `.dmg` |
| Windows x64 | Windows 2025 | NSIS `-setup.exe` |

Apple Silicon packaging is paused: the official Ryo 0.6.1.0 release has no
native arm64 macOS wallet RPC binary. An Intel executable under Rosetta is not
treated as a verified native runtime. The reviewed archive and executable hashes
for supported targets live in [one runtime manifest](../src-tauri/runtime-manifest.json).
`prepare-package-runtime.mjs` verifies the official archive and executable on
each build, then stages only the wallet RPC sidecar for Tauri `externalBin`.
Rust resolves that sidecar next to the installed application executable and
verifies its SHA-256 again before `WalletService` may launch it. `pnpm tauri dev`
keeps a separate `.dev-runtime` preparation path.

The [desktop installer workflow](../.github/workflows/desktop-bundles.yml)
builds each target on its native operating system **only after a change is
merged into `staging`**. Installers and signed updater files are retained for 30 days;
no GitHub Release is created. A pull request from `staging` to `main` runs a
[single promotion check](../.github/workflows/promote.yml). It confirms that
the proposed merge has the same source files as the staging build, downloads
the existing installers and updater signatures, verifies the expected files, and reports SHA-256 hashes.
It neither rebuilds nor uploads installers. After the merge, the release
workflow checks the final source again before publishing the same staging
artifacts. If the staging commit changes, another staging build and review are
required.
See the [development branch flow](../CONTRIBUTING.md).

These previews lack platform code signing. macOS ad-hoc signing modifies the
upstream Mach-O executable, so packaging currently uses `--no-sign` to retain
the reviewed digest. The separate Tauri updater signature authenticates
update downloads, but does not replace platform signing or installation tests.
Production macOS signing needs a reviewed post-signing digest strategy.

The AppImage pipeline removes ten Ubuntu-era Wayland/X11 protocol libraries
from the generated AppDir, restores the exact reviewed wallet RPC after
linuxdeploy patches its ELF RUNPATH, repacks with the AppImage output plugin,
and signs the final image. GTK/WebKitGTK and the Ubuntu 22.04 glibc baseline
remain. DEB, RPM and the final AppImage are extracted in CI and their sidecars
are checked for regular-file status, executable mode, SHA-256 and unresolved
Linux libraries. macOS and Windows workflows inspect their native bundles.

Installed builds check the public `update-channel/latest.json` feed at startup,
and About has a manual check button. The updater verifies a signature bound to
the announced app version before installation. Windows NSIS, macOS app bundles
and Linux AppImage can install updates in the app. DEB and RPM users receive an
availability notice and link to the matching release, then install the new
package with their package manager. The updater signing private key and its
passphrase are kept outside Git and configured as repository Actions secrets;
the public key is embedded in the app. Back up the private key securely: losing
it prevents future updates for installed builds. The initial alpha.4 install
must be manual because older builds did not contain the updater.

Install the pinned Rust, Node, and pnpm versions plus the
[Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/), then
run `pnpm install --frozen-lockfile`. These release builds require
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; CI reads
them from repository secrets. Build on the target operating system:

- Linux: `pnpm exec tauri build --ci --bundles deb,rpm,appimage`, then
  `bash scripts/repack-appimage.sh` and `bash scripts/verify-linux-bundles.sh`
- macOS Intel: `pnpm exec tauri build --ci --bundles app,dmg --no-sign`
- Windows: `pnpm exec tauri build --ci --bundles nsis`

Bundles are written under `target/release/bundle/` because this project uses
a Cargo workspace. The committed `.icns`, `.ico`, and PNG sizes are generated
from `src-tauri/icons/icon.png`; the [asset provenance](ASSETS.md) applies to
all of them. Tauri uses the Ryo icon for the Linux package launchers, the macOS
app inside each DMG, and the Windows app executable. The NSIS setup and
uninstaller executables explicitly use the same `.ico` file.

Before a user-facing release, each platform still needs dependency and license
notices, platform signing (and Apple notarization), installation tests, and
wallet-flow tests on clean systems. See the
[MVP criteria](MVP.md) and [implementation status](IMPLEMENTATION_STATUS.md).
