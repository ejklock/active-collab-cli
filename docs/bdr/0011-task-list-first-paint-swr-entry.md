---
type: BDR
title: "Task-list first-paint SWR on entry: paint the cached list instantly, always revalidate"
description: Observable behavior of entering browse/mine — a warm snapshot paints immediately while a revalidation runs; a cold entry shows the loading placeholder and fetches; refresh is unchanged. Includes the Test Design matrix.
status: Accepted
supersedes:
superseded_by:
tags: [tui, performance, cache, swr, entry, behavior]
timestamp: 2026-06-26T00:00:00Z
---

# 0011. Task-list first-paint SWR on entry

Realizes [ADR 0017](/adr/0017-task-list-first-paint-cache-swr-entry.md) — the
entry half of backlog S8. The human/refresh behaviors of
[BDR 0005](/bdr/0005-loader-single-flight-refresh.md) and
[BDR 0008](/bdr/0008-browse-list-refresh-cached-directory.md) are unchanged.

## Definitions

- **Snapshot** — the `TaskListCache` row for `(scope, instances_key)`: the built
  list (`Vec<ProjectGroup>` for browse, `Vec<MineTableRow>` for mine) plus
  `fetched_at`.
- **Warm entry** — a snapshot exists for the entered scope + instance set, within
  max-age. **Cold entry** — no snapshot (or older than max-age).
- **`revalidating`** — model flag: cached content is shown while a fresh fetch is in
  flight (distinct from `loading` = no content yet).

## Scenarios

### S1 — Warm browse entry paints the cached list immediately
**Given** a warm `browse` snapshot for the target instances,
**When** the operator enters `browse`,
**Then** the first painted frame shows the cached project groups with
`loading: false` and `revalidating: true`, **and** a `Cmd` to fetch open tasks is
dispatched (the revalidation always runs).

### S2 — Revalidation swaps in fresh data and rewrites the snapshot
**Given** a warm entry is showing the cached list (`revalidating: true`),
**When** the fresh open-tasks fetch completes,
**Then** the list is replaced with the fetched result, `revalidating` is cleared,
`last_loaded` is stamped, **and** the snapshot is written back for next time.

### S3 — Cold entry shows the loading placeholder and fetches
**Given** no snapshot for the entered scope + instances (or it exceeds max-age),
**When** the operator enters `browse` or `mine`,
**Then** the screen shows `loading: true` (the placeholder), a fetch `Cmd` is
dispatched, and on completion the list paints and the snapshot is written.

### S4 — Warm mine entry opens the TUI without blocking
**Given** a warm `mine` snapshot,
**When** `ac` (mine) runs in a TTY,
**Then** the TUI opens **immediately** seeded from the snapshot
(`loading: false, revalidating: true`) without a pre-TUI blocking fetch, and the
revalidation runs inside the async loop.

### S5 — Snapshot is isolated per instance set
**Given** a snapshot written for instance set A,
**When** the operator enters with a different instance set B (e.g. a different
`--instance`),
**Then** B does **not** read A's snapshot — a cold entry for B is shown.

### S6 — Refresh never seeds from the snapshot
**Given** a warm snapshot and the operator on a listing,
**When** they press `r` (refresh),
**Then** the refresh behaves exactly as before ADR 0017: it sets the in-flight
state and re-fetches; it does **not** paint from the snapshot (entry-only SWR).

### S7 — A stale-but-within-max-age snapshot is still painted on entry
**Given** a snapshot older than the refresh-freshness window but within max-age,
**When** the operator enters,
**Then** it is still painted immediately (stale-while-revalidate by design) and the
revalidation corrects it; an over-max-age snapshot is treated as a cold entry (S3).

## Test Design

| Scenario | Level | Technique | Instrument / assertion |
|---|---|---|---|
| S1 | unit (pure model) | example | `init_browse` with a seeded snapshot yields `groups` non-empty, `loading=false`, `revalidating=true`, and a load `Cmd` in the returned cmds |
| S2 | unit (pure model) | example | `handle_loaded_tasks` (warm) replaces groups, clears `revalidating`, stamps `last_loaded`; shell writes the snapshot (cache write asserted at the store seam) |
| S3 | unit (pure model) | example | cold `init_browse` / mine ctor → `loading=true`, empty list, load `Cmd` dispatched |
| S4 | unit + integration | example | mine ctor seeded from snapshot has `loading=false, revalidating=true`; `dispatch_mine` does not call the blocking pre-fetch when a snapshot exists (the TUI-open precedes the fetch) |
| S5 | unit (store) | example | `TaskListCache` read keyed by `instances_key` — set A not returned for key B (isolation, mirrors `ProjectNamesCache` per-instance test) |
| S6 | unit (pure model) | example | `handle_refresh` with a warm snapshot present is byte-identical to the pre-0017 model (in-flight set, snapshot ignored) |
| S7 | unit (store) | boundary | within-max-age row returned; over-max-age row treated as miss (TTL boundary) |

Pure model scenarios exercise the TEA `update`/constructors without terminal or
network (the TUI core stays pure); store scenarios exercise `TaskListCache`
read/write/max-age/isolation; the mine non-blocking behavior (S4) is asserted at the
`dispatch_mine` seam.

## References

- ADR: [/adr/0017-task-list-first-paint-cache-swr-entry.md](/adr/0017-task-list-first-paint-cache-swr-entry.md)
- BDR: [/bdr/0005-loader-single-flight-refresh.md](/bdr/0005-loader-single-flight-refresh.md)
- BDR: [/bdr/0008-browse-list-refresh-cached-directory.md](/bdr/0008-browse-list-refresh-cached-directory.md)
- Issue: [/issues/0016-s8-task-list-first-paint-swr.md](/issues/0016-s8-task-list-first-paint-swr.md)
