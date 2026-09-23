# 002 — Rust owns the backend boundary

Date: 2026-09-22. Status: Accepted for architecture.

## Context

Wallet/process/filesystem authority must be separate from presentation.

## Decision

Tauri commands call one reusable Rust service crate. Renderer receives domain DTOs, not raw JSON-RPC or arbitrary filesystem/process APIs.

## Alternatives

JavaScript RPC clients would duplicate validation and make privileged transport reachable from the renderer. A multi-crate split is premature; modular boundaries inside one library are sufficient initially.

## Consequences and verification

Generate domain types, validate every command, serialize lifecycle operations and test authorization. Rust is trusted code, not automatically a sandbox or an independent confirmation display.
