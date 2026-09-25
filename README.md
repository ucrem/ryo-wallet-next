# Ryo Wallet Next

Independent desktop wallet proposal by **ucrem**. Not an official Ryo Currency project.

Original code in this repository is licensed under [MIT](LICENSE). This permits the Ryo team to adopt or modify it while retaining the copyright and license notice. Only the Ryo team can designate a wallet as official; its name and branding need separate agreement. Upstream Ryo code and future bundled binaries remain subject to their own licenses and notices.

This repository contains architecture research and an early Rust/Tauri/React foundation. Linux developers can test wallet creation, recovery-phrase backup, lock and reopen with a verified Ryo 0.6.1.0 wallet RPC binary. Public packages do not bundle that runtime, and there is no transaction flow: **do not use this preview for funds**. Upstream repositories were not modified.

Start with the [implementation status](docs/IMPLEMENTATION_STATUS.md) and [architectural report](docs/REPORT.md). Supporting documents:

- [Project definition](docs/PROJECT.md)
- [Source-based upstream analysis](docs/UPSTREAM_ANALYSIS.md)
- [Architecture and contracts](docs/ARCHITECTURE.md)
- [Security](docs/SECURITY.md)
- [MVP acceptance criteria](docs/MVP.md)
- [Roadmap and testing](docs/ROADMAP.md)
- [Architecture decisions](docs/ADR/README.md)
- [Visual asset provenance](docs/ASSETS.md)
- [Desktop packaging](docs/PACKAGING.md)
- [Local wallet creation test](docs/LOCAL_WALLET_TEST.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)

Research baseline: 22 September 2026. Source conclusions are pinned to commits; runtime compatibility remains to be demonstrated. The local directory name `ryo-currency` is incidental: this was an empty Git repository without remotes when research began. The project repository is `github.com/ucrem/ryo-wallet-next`.

## Development checks

Use Rust 1.98.1, Node 24.21.0 and pnpm 12.6.0 (see the pin files). Then run `cargo test -p ryo-wallet-service --locked`, `pnpm install --frozen-lockfile`, `pnpm check:version`, `pnpm typecheck`, `pnpm lint`, and `pnpm build`. The Tauri desktop build additionally needs the [platform prerequisites](https://tauri.app/start/prerequisites/). On Fedora, the host packages include `gtk3-devel`, `webkit2gtk4.1-devel`, `librsvg2-devel` and `dbus-devel`. Run `pnpm tauri dev` to preview the desktop screens and exercise the verified Linux wallet runtime; the runtime is prepared automatically on first use. About shows the executable version, bundled changelog and update controls. Update installation is available in published installed builds; the development launcher shows that the updater is inactive. The desktop window starts at 1400 × 800 and has a 1400 × 800 minimum. Tauri starts Vite itself; keep its command running while using the app. On Linux with an NVIDIA driver, the launcher applies Tauri's documented WebKitGTK DMABUF workaround before creating the window.
