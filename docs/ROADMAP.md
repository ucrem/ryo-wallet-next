# Roadmap and validation

No dates promised. Each phase exits on evidence, not just code completion. No mainnet funds in normal automated tests.

## Phase 0 — Research (this delivery)

Completed source investigation, project definition, architecture/security/MVP and ADRs. Identified unregistered validation method, signed preparation semantics, RPC shutdown ordering, CLI TLS limitation and licensing inventory needs. Source and registry snapshots are documented. Pending before production implementation assumptions are fixed: actual binary/profile selection, runtime method fixtures, native platform availability and maintainer answers. No Ryo binaries were executed and no native build was validated in this stage.

## Phase 1 — Rust RPC foundation

Status: in progress; see [implementation status](IMPLEMENTATION_STATUS.md). Create two-member Cargo workspace and minimal frontend build baseline only. Pin exact toolchains/lockfiles and record the registry snapshot; resolve compatible shadcn primitives, ts-rs and Digest implementation. Define domain amount/network/errors and private wire schemas. Implement bounded authenticated transport and typed wallet/daemon clients. Prove generated-login access, required RPC method availability and parse_uri validation with a disposable real instance.

Exit: formatting/clippy/tests, frontend typecheck/build, authentication and serialization fixtures pass. No money-moving UI yet. If runtime differs from inspected source, update the compatibility profile and ADR before proceeding.

## Phase 2 — Wallet lifecycle

Implement verified binary locator, owned-process supervision, paths/import copy, config, create/restore/open, seed backup, password change, safe lock/shutdown and recovery. Prove private file permissions/ACLs on each platform. Confirm network isolation and unsupported-account/type checks.

Exit: create → backup → close → restore to a second wallet → same primary address; wrong passwords fail; files survive interruption; children exit on lock/app shutdown/crash recovery. No seed in logs, frontend caches or config.

## Phase 3 — Minimal UI

Local SPA with shadcn, navigation, onboarding, password/backup forms, balances, receive QR, activity, settings, node choice, health and conservative sync display. Generate domain DTO bindings, test invoke wrappers, add targeted state events and read-only query caches. Verify strict capabilities/CSP with built assets.

Exit: keyboard/screen-reader and light/dark smoke checks, stale-session rejection, QR round-trip, no frontend filesystem/network/process dependency. Packaged app launches on all target OSs.

## Phase 4 — Transactions

Implement immutable draft lifecycle, transfer_split no-relay, metadata storage, actual-fee review, one-shot relay, private hash journal, per-transaction partial/unknown status and reconciliation. Validate protocol behavior against real test funds/fixtures, including timeout after daemon accepts but before client receives result.

Exit: no relay before confirmation; no duplicate send on double click/retry; cancelled/expired/stale draft rejected; amount/fee exact; partial/unknown cases usable. No mainnet funds required.

## Phase 5 — Production hardening

Complete platform binary build provenance/license inventory, resource/performance profiling, minimum OS/WebView baseline, long-sync shutdown tests, recovery drills, supply-chain review and independent security review. Validate package contents/paths and binary identities per target. Only after architecture and release criteria hold, define signing/notarization/release publication infrastructure in a separate task. No updater in the MVP.

Exit: every MVP criterion and platform gate passes with documented artifacts, known limitations and operator recovery instructions.

## Practical test matrix

| Layer | Meaningful coverage |
| --- | --- |
| Rust unit/domain | 9-decimal parsing, u64 boundaries, checked sum/fee, invalid network/filename, state machine and secret redaction |
| RPC fixtures | large integers above JS safe range, all required request fields, missing/unknown fields, daemon status vs JSON-RPC errors, 401 Digest challenge/stale nonce, auth failure, malformed/oversized/truncated responses |
| Process | fake child with delayed readiness, port collision, blocked stdout, early exit, ignore termination, crash, inaccessible credential file, wrong hash/arch, stale PID, symlink import, disk-full save simulation |
| Frontend | domain DTO wrapper contract, old-session events, read-cache invalidation, error focus, QR decode, confirmation shows exact fees/address, edits invalidate draft, duplicate submit blocked, secret forms clear |
| Real wallet-rpc | isolated temporary wallet dir and ports, authenticated startup, get_languages, create, seed query/restore, address equality, wrong password, close/reopen, parse_uri validation, backup and empty-password migration |
| Real daemon / transfers | isolated testnet fixture with mature mined outputs and sufficient ring members; scan/balance, preparation has no pool entry, relay appears, split and partial outcomes, disconnect/reorg/rescan |
| E2E | packaged Tauri startup, open, receive, lock and no surviving wallet child; wallet workflows with mocked services plus selected real-backend smoke tests |

The upstream tree contains core and crypto tests, but its tests README leaves daemon/functional sections TODO; do not assume those form a ready end-to-end harness. See [upstream test README](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/tests/README.md). Build a deterministic isolated fixture or obtain a maintained recipe; never depend on a public faucet for every PR. A network enum named FAKECHAIN is insufficient evidence for a runnable private chain. Test-only secrets use disposable fixtures; redact them anyway.

Mock tests are not proof of signing/broadcast semantics. If an isolated chain fixture is unavailable, keep those integration tests explicitly blocked/opt-in and mark release unready, rather than substituting a mock pass. Heavy core builds may run scheduled/manual with a pinned fixture; PR checks run hermetic unit/domain/process/frontend tests and basic real wallet lifecycle when prebuilt verified artifacts exist.

Tauri WebDriver supports desktop Linux/Windows; macOS needs a manual/native smoke route because WKWebView lacks the supported WebDriver route. Browser tests do not prove Tauri ACL or native process behavior. [Tauri WebDriver documentation](https://tauri.app/develop/tests/webdriver/).

## CI design (not implemented yet)

GitHub Actions on PR/push: read-only token, pinned actions/toolchains, Cargo.lock and pnpm frozen lock install. Jobs:

1. Rust `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, unit/domain/process tests.
2. TypeScript typecheck, ESLint, Vitest and Vite production build; generated DTO drift check.
3. Linux/Windows/macOS matrix native `cargo check`/Tauri build validation with verified target binaries and required OS dependencies.
4. Runtime integration using disposable directories/network namespaces or isolated daemon ports; no real secrets/mainnet funds.
5. Inspect production permissions/CSP, license and dependency advisories; record binary manifests/build provenance.

No release signing credentials in these jobs. Later release workflows remain a separate reviewed scope. Cache keys include platform/toolchain/lockfiles; artifacts never contain real wallets, log secrets or credential files.

## First ten implementation tasks, in dependency order

1. Freeze supported upstream commits/binary provenance and capture disposable runtime probes for authentication, lifecycle, address validation and prepare/relay; document platform gaps and obtain test-chain recipe.
2. Scaffold the two-member workspace and React/Vite shell; resolve/pin exact compatible dependencies/toolchains and minimal CI checks.
3. Implement domain amounts/network/errors/session IDs plus fixtures and TS domain export contract.
4. Implement authenticated bounded RPC transport and typed wallet/daemon adapters; pass source-derived wire and real-instance tests.
5. Implement verified binary locator, private runtime credentials and cross-platform process supervisor, including shutdown with an empty RPC process.
6. Implement private paths/config/import-copy and wallet create/restore/open/backup/password/lock state machine with recovery.
7. Implement local/remote node orchestration, trust/network checks, conservative sync state and redacted event snapshots.
8. Build onboarding, overview, receive/QR, activity and settings against typed IPC; prove capabilities, stale-state and accessibility behavior.
9. Implement/test Rust draft preparation, exact-fee summary, cancellation, immutable one-shot relay and partial/unknown reconciliation journal.
10. Build confirmation/results UX and complete real test-chain/E2E/platform security acceptance gates; produce a reviewed MVP candidate, without automatic release publication.

## Post-MVP

Consider subaddress labeling/account management, contact/notes migration, watch-only and hardware wallets, tested secure remote transport, trusted updates and localization based on real demand. Revisit mobile only with an explicit alternative engine/process strategy and licensing review. No assumption that desktop sidecars can be transplanted to mobile.
