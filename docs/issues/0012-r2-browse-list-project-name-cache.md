---
type: Issue
title: "R2 — cache the per-instance project directory so list refresh stops re-fetching it"
description: Add a ProjectNamesCache (TTL, SWR); list refresh always fetches open tasks but serves project names from cache unless stale.
status: open
labels: [tui, performance, cache, controller]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## R2 — browse-list project-name cache (SWR)

Remove the dominant refresh latency: the uncached full-directory fetch
(`list_projects`) paid on every `r`. Implements
[ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md); pins
[BDR 0008](/bdr/0008-browse-list-refresh-cached-directory.md). Realizes the
browse-list half of backlog S8.

### Scope

Included: a `ProjectNamesCache` (per instance, TTL) in `src/store/cache.rs`
mirroring `UserMapCache`; `controller::tasks_by_project` reads names from cache and
calls `list_projects` only on miss/stale; `fetch_open_tasks` stays always-fetch.
Excluded: caching open tasks (refresh must keep them fresh); a first-paint-from-
cache SWR on *entry* (remaining S8 scope, deferred); per-project refresh scoping.

### Acceptance

- A warm refresh does **not** call `list_projects`; a cold/stale entry calls it
  once and writes the cache (BDR 0008 S1–S2).
- `fetch_open_tasks` is called on every refresh (S3).
- A missing cached name degrades to the numeric `project_id`, no panic (S4).
- Cache is per-instance; colliding pids do not cross-leak (S5).
- `browse_list_load` timing on a warm refresh drops to the open-tasks fetch alone.
- Integration tests against the mocked server assert the directory endpoint is hit
  iff the cache is cold/stale; pure tests cover fallback + isolation.

### Plan

Per ADR 0014: add the cache table + migration; refactor `tasks_by_project` to the
SWR read; keep `build_groups` unchanged. The fitness check is the
not-hit assertion on the mocked `list_projects` for a warm refresh.
