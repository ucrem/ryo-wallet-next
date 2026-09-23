# Architecture decisions

2026-09-22. “Accepted” denotes the chosen design for this proposal, not a claim of implementation or upstream approval.

- [React + Vite instead of Next.js](001-react-vite.md) — Accepted for architecture
- [Rust owns the backend boundary](002-rust-boundary.md) — Accepted for architecture
- [Reuse wallet-rpc instead of implementing cryptography](003-wallet-rpc.md) — Accepted for architecture
- [Use Tauri 2 for the desktop shell](004-tauri2.md) — Accepted for architecture
- [Package verified sidecars; supervise them from Rust](005-sidecars.md) — Proposed; runtime/platform and license gates open
- [Gate relay with immutable Rust transaction drafts](006-transaction-drafts.md) — Accepted design; real-backend tests required
- [Explicit Local or Remote; defer hybrid mode](007-node-modes.md) — Accepted design with documented transport limitation
- [React local state with read-only TanStack Query caches](008-state.md) — Accepted for architecture
