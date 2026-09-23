# Architectural report — Ryo Wallet Next

Recommend an independent desktop SPA under `ucrem/ryo-wallet`, with **two Cargo members**, a Rust-owned RPC/process boundary, and a prepare/review/relay transaction flow. The follow-up implementation has begun with a read-only RPC and UI foundation; see [implementation status](IMPLEMENTATION_STATUS.md). Upstream code was inspected at core `185dd1fa33ba88c88bb22df9069ad368c0f9a27e` and Atom `6c8d0aa68245271fe0e781084b38583abf758869`; it was not modified, built or executed.

## Structure and exact stack

```text
ryo-wallet/
  Cargo.toml, Cargo.lock
  crates/ryo-wallet-service/src/
    domain/, application/, rpc/, process/, storage/
  src-tauri/src/commands/, adapters/      # thin native shell
  src-tauri/capabilities/, permissions/, binaries/
  src/app/, api/, features/, components/ui/, styles/
  docs/, tests/
```

Frontend baseline observed 2026-09-22: React/DOM 19.3.0, TypeScript 6.0.3, Vite 8.3.0, plugin-react 6.1.1, Tailwind/Vite integration 4.3.3, shadcn CLI 4.21.0, pnpm 12.5.1, Tauri API 2.11.1/CLI 2.11.5. React local state/context plus TanStack Query 5.103.2 for read-only models; no Next.js, Redux or initial Zustand. Tauri Rust 2.11.6. Exact registry sources and native dependency candidates are in [ARCHITECTURE](ARCHITECTURE.md#exact-frontend-baseline). Compatibility is checked at metadata level; lockfile resolution and builds remain implementation gates.

One reusable `ryo-wallet-service` library owns domain amounts/errors, application actor/events, wallet/daemon RPC adapters, binary/process supervision, private paths/config/imports and submission journal. The second Cargo member is the Tauri app. This avoids four RPC/types crates before independent consumers exist.

## Communication and secrets

React calls named typed Tauri commands. Commands validate and dispatch to the Rust actor; generated TS types describe only stable domain DTOs. Rust handles authenticated loopback wallet JSON-RPC, daemon health and owned child lifecycle. Wallet-rpc handles scanning and signing, with either local ryod or a selected remote daemon. No general RPC, shell, HTTP or filesystem API reaches React.

Passwords/seeds pass transiently from input through IPC/Rust to local wallet-rpc, never settings/logs/query caches. Only mnemonic backup deliberately returns a secret to the frontend. Private keys stay upstream; prepared transaction metadata stays in Rust. Lock terminates the key-bearing process. Upstream `query_key` supports mnemonic separately from spend/view keys; `stop_wallet` needs an open wallet. [src/wallet/wallet_rpc_server.cpp — `on_query_key`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp#L1273); [src/wallet/wallet_rpc_server.cpp — `on_stop_wallet`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp#L1374).

## MVP and transaction decision

Include onboarding, create/seed backup, restore, import/open a protected wallet, account-0 balances, sync, primary receive address/QR, one-recipient send with payment-ID support, actual-fee confirmation, split-aware submission, history, node health/choice, lock and basic settings. Explicitly defer account/subaddress management, watch-only/multisig, mining, sweeps, mobile, hybrid mode and automatic updates. [Acceptance criteria](MVP.md).

Prepare with `transfer_split(do_not_relay=true, get_tx_metadata=true)`, review the fee/total, then relay metadata by opaque draft ID. This already signs during preparation; it gates **broadcast**, not signing. Split submissions may partly succeed; store hashes for reconciliation and never blindly resend. [src/wallet/wallet_rpc_server.cpp — `fill_response / on_transfer_split / on_relay_tx`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp#L693); [src/wallet/wallet2.cpp — `commit_tx`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.cpp#L4481).

## Major risks and licensing

- `validate_address` is implemented but not registered. Use open-wallet `parse_uri` validation; no guessed address regex or crypto port. [src/wallet/wallet_rpc_server.h — `RPC map`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.h#L98); [src/wallet/wallet_rpc_server.cpp — `on_validate_address`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp#L2975); [src/wallet/wallet2.cpp — `parse_uri`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.cpp#L9203).
- Wallet RPC has no registered `set_daemon` or `get_version`; node switching requires restart/reopen and compatibility must use verified binaries/probes. [src/wallet/wallet_rpc_server.h — `RPC map`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.h#L98).
- Single-thread RPC refresh can stall health/lock commands; design bounded waits and recovery, not infinite spinners. [src/wallet/wallet_rpc_server.cpp — `run`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp#L115).
- CLI daemon initialization uses default `ssl=false`; do not claim direct HTTPS security. Remote HTTP requires explicit privacy disclosure; prefer local mode. [src/wallet/wallet2.cpp — `make_basic`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.cpp#L256); [src/wallet/wallet2.h — `init`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.h#L616).
- Nine-decimal atomic amounts must never pass through JS floating arithmetic. [src/cryptonote_config.h — `CRYPTONOTE_DISPLAY_DECIMAL_POINT / MK_COINS`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/cryptonote_config.h#L74).
- Upstream licenses contain historical restrictions and expired public-domain clauses alongside inherited BSD obligations. Do not assume uniform unrestricted reuse; original implementation plus per-binary dependency/notices review is the recommendation. No source/assets copied. [LICENSE](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/LICENSE); [ORIGINAL-LICENSE](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/ORIGINAL-LICENSE); [LICENSE](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/LICENSE); [ORIGINAL-LICENSE](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/ORIGINAL-LICENSE).

## Developer input still needed

Supported core release/artifact provenance; validation method registration; verified remote TLS and tunneled-node trust; stable prepared-metadata/recovery semantics; supported seed/legacy wallet migrations; deterministic funded test-chain recipe; macOS/ARM target support; current licensing/branding clarification. Full questions and source evidence are in [UPSTREAM_ANALYSIS](UPSTREAM_ANALYSIS.md#questions-for-ryo-developers-and-release-gates).

## First ten implementation tasks

1. Freeze/probe supported upstream binaries and establish disposable test-chain fixtures.
2. Scaffold two Cargo members and SPA; pin dependencies/toolchains and basic CI.
3. Implement domain amounts/networks/errors/session IDs and generated TS DTOs.
4. Implement bounded Digest-authenticated typed RPC clients.
5. Implement binary verification, credentials and cross-platform process supervision.
6. Implement private imports/config and create/restore/open/backup/password/lock lifecycle.
7. Implement node orchestration, trust/network checks, sync and events.
8. Build onboarding, overview, receive, activity and settings with strict IPC capabilities.
9. Implement immutable drafts, no-relay preparation and partial/unknown submission recovery.
10. Build confirmation/results and pass real integration/platform/security acceptance gates.

See [ROADMAP](ROADMAP.md) for dependencies, tests and CI; [SECURITY](SECURITY.md) for trust boundaries and distribution gates. Research is sufficient to start foundational implementation with explicit runtime gates, not to call the wallet production-ready.
