---
type: Issue
title: "S8 — task-list first-paint-from-cache SWR on browse/mine entry"
description: Add a TaskListCache snapshot of the built open-tasks list; on entering browse/mine paint it instantly and always revalidate in the background. Unblock mine (today it fetches before the TUI opens). Refresh is unchanged.
status: closed
labels: [tui, performance, cache, swr, entry]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## S8 — task-list first-paint SWR (entry)

Make entering `browse`/`mine` paint the last-known list instantly, then revalidate.
Implements [ADR 0017](/adr/0017-task-list-first-paint-cache-swr-entry.md); pins
[BDR 0011](/bdr/0011-task-list-first-paint-swr-entry.md). Completes the entry half
of S8 that [ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md) deferred.

### Scope

Included: a `TaskListCache` (`(scope, instances_key)` → built list + `fetched_at`)
in `src/store/cache.rs` + migration in `src/store/mod.rs`, mirroring
`UserMapCache`; entry seeding for browse (`init_browse`) and mine (`dispatch_mine` /
mine constructor) with a new `revalidating` model flag and a subtle view indicator;
a mine load `Cmd`/`Msg` so the TUI opens before the fetch; snapshot write-back on
revalidation. Excluded: any change to refresh (`handle_refresh` stays as ADR 0014
left it); caching open tasks for refresh; the project-name cache (already shipped in
R2).

### Acceptance

- Warm browse/mine entry paints the cached list immediately (`loading=false,
  revalidating=true`) and **always** dispatches a revalidation (BDR 0011 S1, S4).
- Revalidation replaces the list, clears `revalidating`, stamps `last_loaded`, and
  rewrites the snapshot (S2).
- Cold entry shows `loading=true` + fetch, then writes the snapshot (S3).
- `mine` opens the TUI without a pre-TUI blocking fetch when a snapshot exists (S4).
- Snapshot is isolated per `instances_key`; sets do not cross-read (S5).
- Refresh is byte-identical to pre-0017 — it never seeds from the snapshot (S6).
- A within-max-age snapshot is painted; over-max-age is treated as a miss (S7).
- Full suite green; clippy/fmt/comment-policy clean; complexity within budget;
  `architecture.md` store diagram updated with the new table.

### Plan (slices, persisted plan `s8-task-list-swr`)

- **S8a** — `TaskListCache` table + migration + read/write/max-age/isolation (store
  layer); update the `architecture.md` store diagram. Pure + store unit tests.
- **S8b** — browse entry SWR: `init_browse` seeds from the snapshot, `revalidating`
  flag + view indicator, `handle_loaded_tasks` writes the snapshot. Pure model +
  integration tests.
- **S8c** — mine entry SWR: seed the mine model from the snapshot, add the mine load
  `Cmd`/`Msg` so the TUI opens before the fetch, revalidate + write-back. Tests.
