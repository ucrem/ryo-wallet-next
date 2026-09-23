# 006 — Gate relay with immutable Rust transaction drafts

Date: 2026-09-22. Status: Accepted design; real-backend tests required.

## Context

A send must show actual fees and never broadcast before explicit confirmation.

## Decision

Prepare/sign using transfer_split(do_not_relay=true, get_tx_metadata=true); Rust stores metadata, returns only summary/token, and relays the same prepared transactions on one-shot confirmation.

## Alternatives

A fee estimate followed by rebuilding can change cost/outputs after approval. Blind transfer broadcasts immediately. Keeping signed metadata in React unnecessarily exposes spend-related data.

## Consequences and verification

Handle split/partial/unknown outcomes and never regenerate automatically. Cancel is local token invalidation. Evidence: [src/wallet/wallet_rpc_server.cpp — `fill_response / on_relay_tx`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet_rpc_server.cpp); [src/wallet/wallet2.cpp — `commit_tx`](https://github.com/ryo-currency/ryo-currency/blob/185dd1fa33ba88c88bb22df9069ad368c0f9a27e/src/wallet/wallet2.cpp#L4481).
