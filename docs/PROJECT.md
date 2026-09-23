# Project definition

Status: architecture proposal, 2026-09-22. Owner: ucrem. Working name: Ryo Wallet Next. Project home: `github.com/ucrem/ryo-wallet-next`.

## Purpose and users

Build an independent, open-source desktop alternative to Ryo Wallet Atom for people who need to receive, restore, inspect and send RYO reliably. Support existing Atom users without requiring them to understand JSON-RPC or manage command lines. Experienced users can choose their own node. Any eventual proposal to the Ryo team is a separate decision; branding must not imply endorsement.

## Goals

- Linux, Windows and macOS desktop releases with the same wallet semantics.
- Reuse Ryo's wallet engine for keys, encrypted wallet storage, synchronization and signing.
- Keep a reusable Rust service independent of Tauri and React.
- Show accurate atomic amounts, synchronization freshness and transaction outcomes.
- Require review of the actual prepared transaction fees before transmission.
- Make failure recovery understandable, including uncertain submission and interrupted synchronization.
- Keep dependencies, permissions and runtime network access explicit.

## Non-goals

Mobile, a JavaScript wallet SDK, new cryptography, consensus changes, official-project claims, exchange integrations, token swaps, pricing feeds, mining, telemetry, automatic updates and upstream patches. Tauri 2 leaves a possible mobile UI route, but desktop child processes are not a mobile backend implementation.

## Product principles

A restrained financial application: useful density, readable labels and full addresses at confirmation; no decorative trading dashboards. Desktop navigation: Overview, Send, Receive, Activity, Settings. Wallet selection precedes that shell. Lock is always available. Node and sync state remain visible across screens.

Support light, dark and system themes, keyboard navigation, visible focus, semantic labels, screen-reader announcements, reduced motion, and layouts from the current 960×720 minimum window upward. Individual screens may scroll when content or display scaling requires it. Never communicate transaction state through color alone. Use locally bundled fonts/icons and shadcn/ui components. Loading, no wallet, no transactions, disconnected, stale, insufficient unlocked funds and partial submission are distinct states.

## Smallest usable product

One open software wallet at a time; create and back up a new wallet, restore supported Ryo seed formats, import/open an existing wallet copy, require password protection, display balances and primary receive address/QR, synchronize through local or explicitly selected remote nodes, review and submit a single-recipient payment, inspect activity, change password, close/lock, configure theme/node/idle lock and diagnostics privacy.

New wallets default to long addresses; existing Kurz wallets can be restored/opened. Account 0 is the MVP scope. Imports containing other accounts or unsupported watch-only/multisig types must be detected and explained, never silently presented as a complete zero balance. See [MVP](MVP.md) for boundaries and release criteria.

Original Next code is licensed under [MIT](../LICENSE), allowing the Ryo team to adopt and modify it while retaining the copyright and license notice. Official status and branding require a separate decision by the Ryo team. The Ryo symbol used by this app is documented in [asset provenance](ASSETS.md); upstream binary notices still need their own inventory. See [licensing findings](UPSTREAM_ANALYSIS.md#licensing).
