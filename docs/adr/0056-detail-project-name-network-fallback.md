---
type: ADR
title: The detail load resolves an unknown project name over the network and writes it back to cache
description: On a project-name cache miss the detail load no longer settles for the "(unknown)" fallback; it issues one GET /api/v1/projects/{id}, renders the resolved name, and writes it back into the per-instance ProjectNamesCache — extending ADR 0014's SWR to the single-project detail case and amending the "no network on detail load" property noted by ADR 0022.
status: Accepted
supersedes:
superseded_by:
tags: [tui, detail, controller, cache, swr, client, network]
timestamp: 2026-07-03T00:00:00Z
---

# 0056. Detail project-name network fallback with cache write-back

## Context

The operator opened a task detail whose project *is* known to the server
(`https://collab.base.digital/projects/722` resolves to a real name) and the
`Detalhes` panel still showed `Projeto (desconhecido)`.

Tracing the detail load path:

- `controller::load_task_core` calls `enrich_task_with_project_name`, which calls
  `project_name_from_cache`.
- `project_name_from_cache` reads **only** the per-instance `ProjectNamesCache`
  (`read_fresh`) and, on a miss, returns `t("(unknown)")`. **No network call is
  made.**

The `ProjectNamesCache` is populated as a side effect of the **browse/mine list**
load ([ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md)), which fetches
the whole project directory (`GET /api/v1/projects`). A detail opened **before**
any browse of that instance — or for a project the list never surfaced, or after
the entry went stale — therefore has no cached name, and the row falls back to
`(unknown)` even though the name is one request away.

This "no network on detail load" property was an emergent consequence of reusing
the browse cache for detail; [ADR 0022](/adr/0022-detail-title-as-meta-row.md) and
[BDR 0016](/bdr/0016-detail-title-row-project-name.md) recorded it as the accepted
miss behavior. In practice it reads as a defect: the field exists in the API and
the panel shows "unknown".

Force: **legibility of a read view** — the `Projeto` value is a primary detail
field; when the server can name the project, the panel should show that name, not
a placeholder. The single-project fetch is cheap and one-shot; the cost ADR 0014
guarded against was the **whole-directory** fetch on every list refresh, not a
single `projects/{id}` lookup on a cache miss.

## Decision

On a project-name cache miss, the detail load resolves the name **over the
network** and writes it back, extending ADR 0014's stale-while-revalidate to the
single-project detail case.

### 1. New client method (`src/client.rs`)

`fetch_project_name(project_id) -> Result<Option<String>>` issues
`GET /api/v1/projects/{project_id}` with the instance token and returns
`Some(name)` on a 200 with a non-empty `name`, `None` otherwise (non-200, missing
or empty name). A small interface over the single-project endpoint, mirroring the
existing `fetch_task` / `fetch_open_tasks` shape (the `client/http` seam stays the
only outbound-network boundary).

### 2. Detail load resolves then falls back (`src/controller.rs`)

`enrich_task_with_project_name` becomes `async` and takes the `client`
`load_task_core` already builds. Resolution order for `project_id`:

- **fresh cache hit that contains the id** → use the cached name, **no network
  call** (the warm-cache path is unchanged).
- **miss / stale / id absent from the fresh map** → `fetch_project_name(id)`:
  - `Some(name)` → render it and **write it back** to `ProjectNamesCache`, merged
    into the instance's current map so no sibling name is clobbered.
  - `None` (fetch failed or nameless) → the existing `t("(unknown)")` fallback.

The connection discipline is preserved: the cache read and the merged write are
each a synchronous `Store` open/drop around the `await`; no `Connection` is held
across the network call (same pattern as `resolve_project_names`).

### 3. Fitness function

Asserted against the mocked server on the detail load path:

- **cache miss + server names the project** → the `Projeto` row shows the resolved
  name (not `(unknown)`), exactly **one** `projects/{id}` request is issued, and a
  subsequent detail load for the same id serves from cache with **zero** requests.
- **warm cache hit** → **zero** `projects/{id}` requests (the ADR 0014 warm-path
  guarantee is not weakened for detail).
- **cache miss + server has no name (non-200 / empty)** → the row shows the
  `(unknown)` fallback, never blank.

## Alternatives considered

- **Keep cache-only; populate detail names by pre-warming the browse cache.**
  Rejected: it couples "can I see a project's name in detail" to "did I browse this
  instance's list first", which is exactly the surprising coupling the operator
  hit. Detail is a legitimate entry point (task URL, `ac get`, deep link).
- **Fetch the whole directory (`list_projects`) on the detail miss.** Rejected:
  re-introduces the expensive whole-directory fetch ADR 0014 removed, to resolve a
  single name. The `projects/{id}` endpoint is the targeted call.
- **Fetch but do not write back.** Rejected: the operator asked for the name to
  stick; without write-back every detail open re-fetches, and the browse list
  stays blind to the name just resolved. Write-back makes the single fetch pay
  back across both surfaces.
- **Give detail its own single-project cache table.** Rejected: a second cache for
  the same `{instance, project_id → name}` fact is a duplicate home; merging into
  the existing `ProjectNamesCache` keeps one home for project names.

## Consequences

**Positive:** the `Projeto` row shows the real name whenever the server knows it,
regardless of whether the browse list was visited first; the resolved name is
written back once and then served from cache to both the detail and browse
surfaces; the warm-cache path and the ADR 0014 whole-directory guarantee are
unchanged; the fix reuses the existing cache table and network boundary.

**Accepted trade-offs:** a detail open on a cold/stale cache now issues one
`projects/{id}` request (previously zero) — bounded, one-shot, and only on a miss.
The write-back merges the single name into the instance map and advances that
map's freshness timestamp; because project names are slowly-changing reference
data (ADR 0014's own framing), refreshing the whole-instance TTL off a
single-project fetch is acceptable — the worst case is sibling names staying
cached slightly longer, the same SWR bargain ADR 0014 already accepts.

## Related

- ADR: [/adr/0014-browse-list-project-name-cache-swr.md](/adr/0014-browse-list-project-name-cache-swr.md) (SWR extended to the single-project detail case)
- ADR: [/adr/0022-detail-title-as-meta-row.md](/adr/0022-detail-title-as-meta-row.md) (its "no network on detail load" miss note is amended here)
- ADR: [/adr/0052-cache-owns-freshness-read-fresh.md](/adr/0052-cache-owns-freshness-read-fresh.md) (`read_fresh` freshness ownership reused)
- BDR: [/bdr/0030-detail-project-name-resolved-on-miss.md](/bdr/0030-detail-project-name-resolved-on-miss.md) (the observable behavior)
- BDR: [/bdr/0016-detail-title-row-project-name.md](/bdr/0016-detail-title-row-project-name.md) (Scenario 3 miss behavior refined by BDR 0030)
- Architecture: [/architecture.md](/architecture.md)
</content>
</invoke>
