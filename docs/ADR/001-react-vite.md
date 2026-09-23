# 001 — React + Vite instead of Next.js

Date: 2026-09-22. Status: Accepted for architecture.

## Context

The product is a desktop SPA with local assets and native Rust services.

## Decision

Use React, TypeScript and Vite with Tailwind and shadcn/ui. No SSR, server actions, RSC or API routes.

## Alternatives

Next.js adds server-oriented concepts without a required server. A plain SPA keeps the IPC boundary visible. Hand-written DOM would reduce dependencies but lose the requested React component foundation.

## Consequences and verification

Use local component navigation, typed invoke wrappers and production static assets. Check bundle/CSP output. Exact proposed pins and remaining compatibility gates are in [ARCHITECTURE](../ARCHITECTURE.md).
