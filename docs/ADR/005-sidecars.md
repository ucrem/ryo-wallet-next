# 005 — Package verified sidecars; supervise them from Rust

Date: 2026-09-22. Status: Proposed; runtime/platform and license gates open.

## Context

wallet-rpc is mandatory; local mode additionally needs ryod, with predictable versions and lifecycle.

## Decision

Use Tauri externalBin packaging and target-specific manifests for both executables. Rust BinaryLocator resolves verified absolute paths; reusable supervisor uses native process APIs. Development may select external binaries only against an approved manifest.

## Alternatives

PATH lookup is nondeterministic; downloads at startup introduce update trust before it is designed. Frontend shell spawning violates the backend boundary. tauri-plugin-shell is an alternative Rust adapter but unnecessary for a Tauri-independent supervisor.

## Consequences and verification

Executable permissions, target compatibility, dynamic libraries, provenance and child cleanup must pass per-platform tests. No update mechanism or signing workflow now. See [sidecar documentation](https://tauri.app/develop/sidecar/) and [SECURITY](../SECURITY.md).
