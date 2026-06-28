---
type: ADR
title: Cache the per-instance project-name directory so a list refresh stops re-fetching it
description: A browse/mine list refresh today re-fetches the entire project directory of every instance (uncached `list_projects`) on top of the open-tasks call. Add a per-instance ProjectNamesCache (TTL, SWR) so refresh re-fetches only the open-tasks listing and serves project names from cache unless stale.
status: Accepted
supersedes:
superseded_by:
tags: [tui, performance, cache, controller, swr]
timestamp: 2026-06-26T00:00:00Z
---

# 0014. Browse-list project-name cache (SWR) — stop re-fetching the directory on refresh

## Context

The user reported that pressing `r` (refresh) on a listing "começou a demorar
muito" and suspected it was fetching children, not just the listing.

Tracing the refresh path:

- `handle_refresh` (`src/tui/model.rs`) on **Projects** or **Tasks** emits
  `Cmd::LoadTasksByProject`.
- The shell runs `controller::tasks_by_project` (`src/controller.rs`), which, **per
  instance, in parallel**, calls `client.fetch_open_tasks()` (one request: the
  user's open tasks) **and** `fetch_project_names()` → `client.list_projects()`
  (`GET /api/v1/projects` — the instance's **entire project directory**).
- `tasks_by_project` is **uncached**: every refresh hits the network for both
  calls, every instance.

The actual root cause is the **opposite** of the user's guess: refresh does not
fan out to per-task children — it re-fetches the **whole project directory** of
every instance each time, with no cache. On instances with many projects,
`list_projects` is the slow call, and it is paid on every refresh even though
project names rarely change. A second aggravator: refreshing the **Tasks** screen
(one project) re-runs the full multi-instance aggregation, including every
instance's directory fetch — not just the current project.

Force: **responsiveness of refresh** (a [PRD 0001](/prd/0001-rust-tui-cli-parity.md)
NFR). The listing data the operator wants fresh on refresh is the **open tasks**;
the **project-name directory** is reference data that changes slowly and does not
need a network round-trip on every `r`.

## Decision

Add a **per-instance `ProjectNamesCache`** and make the list load **stale-while-
revalidate (SWR)**, delivered as slice **R2**. This realizes the browse-list half
of the deferred S8 "Browse/mine list SWR cache" backlog item.

### 1. New cache table (`src/store/cache.rs`)

A `ProjectNamesCache` keyed by instance name, storing the `{project_id → name}`
map plus a written-at timestamp, with a TTL — mirroring the existing
`UserMapCache` shape and connection discipline (no `Connection` held across an
`await`).

### 2. `tasks_by_project` serves names from cache (`src/controller.rs`)

Per instance:

- **Always** `fetch_open_tasks()` — the open-tasks listing is the data refresh is
  for; it stays fresh.
- Project names come from `ProjectNamesCache`:
  - **fresh hit** → use cached names, **no `list_projects` call**.
  - **miss or stale** (or a future explicit force) → `list_projects`, then write
    the cache.

`build_groups` is unchanged — it already maps `pid → name`, falling back to the
numeric id when a name is absent, so a cold cache degrades gracefully (numeric
ids for one paint, names after the directory resolves).

### 3. Fitness function

The path is already instrumented: `timing::record("browse_list_load", …)` in
`tasks_by_project`. The fitness target: **a warm refresh issues zero
`list_projects` requests** (asserted against the mocked server — the directory
endpoint is not hit on a warm-cache refresh) and `browse_list_load` on a warm
refresh drops to the open-tasks fetch alone. This is the gate-checked counterpart
of "refresh is fast".

## Alternatives considered

- **Scope the Tasks-screen refresh to the current project only.** Rejected as the
  primary fix: `mine`/browse is multi-project, multi-instance by design — the list
  *is* an aggregation, and the open-tasks endpoint already returns the user's tasks
  across projects in one call. The cost is the directory fetch, not the tasks
  fetch, so caching the directory is the targeted fix. (Scoping is also orthogonal
  and could be layered later.)
- **Cache the open tasks too.** Rejected: open tasks are exactly what the operator
  wants re-fetched when they press `r`. Caching them would defeat refresh. (A
  separate first-paint-from-cache SWR for *entry* — not refresh — is the remaining
  S8 scope and stays deferred.)
- **Drop project names, show numeric ids.** Rejected: names are the usable label;
  the numeric id is only the cold-cache fallback.
- **One global project cache across instances.** Rejected: project ids collide
  across instances ([obs 31] latent multi-instance pid collision); the cache must
  be per-instance, like `UserMapCache`.

## Consequences

**Positive:** a warm refresh drops from "open tasks + full directory, every
instance" to "open tasks only", removing the dominant latency the user hit;
project names still appear (from cache), with numeric-id graceful degradation on a
cold cache; the design reuses the proven `UserMapCache` pattern.

**Accepted trade-offs:** a newly renamed/added project is not reflected until the
TTL expires (or a future force-refresh). This is acceptable for slowly-changing
reference data and matches how `UserMapCache` already treats the user directory.
One more cache table and a `store` migration.

## Related

- BDR: [/bdr/0005-loader-single-flight-refresh.md](/bdr/0005-loader-single-flight-refresh.md)
- BDR: [/bdr/0008-browse-list-refresh-cached-directory.md](/bdr/0008-browse-list-refresh-cached-directory.md)
- PRD: [/prd/0001-rust-tui-cli-parity.md](/prd/0001-rust-tui-cli-parity.md)
- Issue: [/issues/0012-r2-browse-list-project-name-cache.md](/issues/0012-r2-browse-list-project-name-cache.md)
- Architecture: [/architecture.md](/architecture.md)
