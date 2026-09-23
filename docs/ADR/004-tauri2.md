# 004 — Use Tauri 2 for the desktop shell

Date: 2026-09-22. Status: Accepted for architecture.

## Context

Desktop targets need a Rust host with a restrained local WebView UI.

## Decision

Use Tauri 2, an explicitly restricted main-window capability and platform adapters for native operations.

## Alternatives

Electron follows Atom but increases the JavaScript/native runtime footprint. Native-only Rust UI would abandon the selected React/shadcn stack. Tauri still relies on OS WebView behavior and platform testing.

## Consequences and verification

Windows, Linux and macOS builds are required. Mobile remains post-MVP; the service interface can survive replacing the process adapter, but mobile cannot be promised by choosing Tauri alone. See [Tauri process model](https://tauri.app/concept/process-model/).
