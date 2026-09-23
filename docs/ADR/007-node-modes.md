# 007 — Explicit Local or Remote; defer hybrid mode

Date: 2026-09-22. Status: Accepted design with documented transport limitation.

## Context

Atom hybrid mode uses a bootstrap daemon, adding implicit trust transitions.

## Decision

MVP offers managed local daemon or explicit remote HTTP endpoint, with transport/privacy disclosure. No silent failover, no direct HTTPS claim until supported by a tested compatibility profile.

## Alternatives

Hybrid speeds initial use but complicates readiness and node trust. A TLS relay could fix CLI transport limitations but adds a binary-endpoint proxy requiring its own testing; not part of the smallest MVP.

## Consequences and verification

Node switch restarts wallet-rpc and reopens with user password; never save it. A remote tunnel remains untrusted. Evidence: [src-electron/main-process/modules/daemon.js — `start`](https://github.com/ryo-currency/ryo-wallet/blob/6c8d0aa68245271fe0e781084b38583abf758869/src-electron/main-process/modules/daemon.js); [src/wallet/wallet2.cpp — `make_basic`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.cpp#L256); [src/wallet/wallet2.h — `init`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.h#L616).
