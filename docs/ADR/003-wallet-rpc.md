# 003 — Reuse wallet-rpc instead of implementing cryptography

Date: 2026-09-22. Status: Accepted for architecture.

## Context

Ryo already implements wallet storage, seeds, synchronization and signing.

## Decision

Wrap a pinned verified wallet-rpc process in Rust; use existing Ryo methods and retain private wire DTOs. No JavaScript wallet SDK or new crypto.

## Alternatives

Direct C++ FFI/libwallet would remove a process/RPC boundary but introduce ABI/build coupling. Reimplementation is too risky and unnecessary. Revisit only on evidence that RPC prevents required features.

## Consequences and verification

Use transfer_split(no-relay) then relay_tx with opaque metadata. Signing precedes confirmation; relay is gated. Missing validate_address uses parse_uri with an open wallet. Evidence: [src/wallet/wallet_rpc_server.cpp — `on_transfer_split / on_relay_tx / on_parse_uri`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp). Runtime tests are a prerequisite for shipping.
