# Navigation layout comparison

The top navigation was approved on 2026-10-03 and is now the standard alpha.6
layout. The original sidebar remains available in the optional comparison preview.

The desktop has Wallet, Setup and About in the top menu. Data location,
Node, Summary and Wallet appear in a second navigation row during setup.
The wallet screens and synchronization status bar are shared by both layouts.
The main top menu is 48 px high; setup navigation is 32 px high and is omitted
on the wallet dashboard. Content spans the available width with 20 px margins.
The 40 px footer presents node and wallet synchronization on one line, with
separate inline progress indicators. Secondary control guidance is available
on hover or through assistive text.

## Desktop preview

Run `pnpm build:desktop-layout-preview`, then open
`target/debug/ryo-wallet-next.exe` on Windows (the corresponding native
executable on other supported systems). No web server or installer is needed.
The preview window is named **Ryo Wallet Next — Navigation preview**.
The Windows executable shown locally was also copied to
`target/layout-preview/native/ryo-wallet-next.exe` for convenient reopening.

Use **Top menu** and **Sidebar** in the top-right comparison control to switch
layouts without leaving the current screen or remounting the wallet workspace.
The choice lasts only for that preview session. A standard `pnpm build` or
`pnpm tauri dev` uses the top menu and omits the comparison control.

The preview has a separate Tauri application identifier, so its data-folder
selection is independent of the running alpha.6 app. Use a disposable preview
folder when exercising setup. The locally prepared preview data starts without
a wallet or a running node. Its node can be started separately; the original
desktop app can continue syncing.

The default development identifier remains `io.github.ucrem.ryowalletnext.dev`.
An explicit preview identifier is preserved by the debug host, so the preview
does not inherit that development configuration either.

## Local verification

On 2026-10-03, TypeScript, ESLint, version consistency and the existing 20
frontend tests passed. The Windows native preview compiled and displayed the
home and About screens with the top navigation. The original alpha.6 desktop
app and its daemon remained running. The release installer and default sidebar
build were retained before promoting the top menu.

## Saved blockchain and reopening

The original app uses its existing selected folder (`G:\` on this Windows host).
The comparison preview was initialized in `target/layout-preview/data`. These
are separate blockchains. Switching applications can therefore show a different
height. Reopening the same application retains its selection and reloads
`<data-folder>/mainnet/chain/lmdb02` when Start node is pressed. Before startup,
the node is stopped and the UI has no live RPC height. Peer discovery may also
temporarily leave the target height unknown; neither state implies deletion.

The same preview was closed at height 2,295 and reopened on 2026-10-03. The
daemon log confirmed loading last block 2,294 (RPC height 2,295), and live RPC
subsequently reached height 4,795. The original sidebar installer was retained
in `target/layout-preview/original` before rebuilding the standard top-menu
installer. Both newly packaged runtime binaries passed the manifest hashes.
The updated standard desktop app also reopened the original `G:\` data folder
and loaded last block 56,641 (RPC height 56,642), retaining its existing selection.
