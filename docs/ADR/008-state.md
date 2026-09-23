# 008 — React local state with read-only TanStack Query caches

Date: 2026-09-22. Status: Accepted for architecture.

## Context

The UI has asynchronous native snapshots plus short-lived forms and sensitive input.

## Decision

Use React state/context for UI/session/theme and TanStack Query only for read models. Direct invoke for password/seed operations; no persisted cache, Redux or Zustand initially.

## Alternatives

Context alone requires manual async cache mechanics. Zustand does not replace asynchronous freshness handling and would add another state owner. Query mutation variables can retain secrets, so secret calls bypass it.

## Consequences and verification

Clear read caches on lock and key them by session/account; events invalidate snapshots. Ignore old generations. Test secret cleanup and late responses.
