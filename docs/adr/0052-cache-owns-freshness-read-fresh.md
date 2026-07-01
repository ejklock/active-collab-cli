---
type: ADR
title: The cache layer owns freshness — ProjectNamesCache gains a read_fresh(max_age) that does the TTL check inside, mirroring TaskListCache::read, so the caller stops doing age arithmetic
description: The project-names SWR freshness check is split across two homes. ProjectNamesCache::read returns the raw fetched_at epoch and the caller (controller::fresh_project_names_cache_read) applies the TTL comparison itself — now_epoch_secs() - cached.fetched_at <= PROJECT_NAMES_TTL_SECS — so the "is this entry fresh?" policy lives in the controller, not the cache. The sibling TaskListCache::read(scope, key, max_age_secs) already does the age check INSIDE and returns None when stale, hiding fetched_at entirely. Align ProjectNamesCache with that established deep shape — add read_fresh(instance, max_age_secs) -> Option<CachedProjectNames> that owns the now - fetched_at <= max_age comparison; the controller helper collapses to open-store + read_fresh with no arithmetic. Behavior-preserving; the existing controller/cache specs are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [cache, swr, controller, refactor, locality, depth, freshness]
timestamp: 2026-07-01T00:00:00Z
---

# 0052. The cache layer owns freshness (read_fresh)

## Context

The browse-list project-name SWR cache ([ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md))
serves project names from `ProjectNamesCache` when fresh and re-fetches the directory only on a miss
or stale entry. The **freshness check itself is split across two homes**:

- `ProjectNamesCache::read(instance) -> Option<CachedProjectNames>` (`store/cache.rs:144`) returns
  the raw `fetched_at` epoch — it does **not** know what "fresh" means.
- `controller::fresh_project_names_cache_read` (`controller.rs:85`) opens the store, calls `read`,
  then applies the TTL comparison **itself**:
  `now_epoch_secs() - cached.fetched_at <= PROJECT_NAMES_TTL_SECS`. The policy — the age arithmetic
  and the `PROJECT_NAMES_TTL_SECS` window — lives in the controller.

So a caller must know both *how to read* the cache and *how to judge* its freshness. The
`fetched_at` field leaks across the seam purely so the controller can subtract it.

The **sibling cache already has the right shape**: `TaskListCache::read(scope, instances_key,
max_age_secs) -> Option<String>` (`store/cache.rs:198`) takes the max age, does the
`now_epoch_secs() - fetched_at <= max_age_secs` check **inside**, and returns `None` when stale —
`fetched_at` never escapes. The task-list SWR entry ([ADR 0017](/adr/0017-task-list-first-paint-cache-swr-entry.md))
reads a fresh snapshot without any age arithmetic at the call site. The project-names cache is the
odd one out.

## Decision

Align `ProjectNamesCache` with the deep `TaskListCache::read` shape: the cache owns the freshness
judgement; the caller passes a max age and gets back only a fresh entry.

1. **`ProjectNamesCache::read_fresh(instance, max_age_secs) -> Result<Option<CachedProjectNames>>`.**
   It reads the row and returns `Some` only when `now_epoch_secs() - fetched_at <= max_age_secs`,
   `None` when absent or stale — the same comparison `TaskListCache::read` already performs. The raw
   `read` stays as the primitive `read_fresh` builds on (and the test seam).

2. **The controller helper collapses.** `fresh_project_names_cache_read` becomes: open the store,
   call `ProjectNamesCache::read_fresh(instance, PROJECT_NAMES_TTL_SECS)`, map to `names`. The
   `now_epoch_secs()` call and the `age <= …` comparison leave `controller.rs`. The
   `PROJECT_NAMES_TTL_SECS` policy constant stays in the controller (the project-names window is a
   controller-level policy, passed in — exactly as callers pass `max_age_secs` to
   `TaskListCache::read`).

### Guard / fitness function

- **Behavior preserved — invisible to the user.** Fresh entries still resolve names; stale/absent
  entries still trigger the one `list_projects` re-fetch (ADR 0014). All existing controller and
  cache specs stay green.
- **Freshness has one home.** Grep finds the `now_epoch_secs() - fetched_at <= …` comparison for
  project names in exactly one place — `ProjectNamesCache::read_fresh` — not in the controller. The
  cache decides "fresh"; the caller only names the window.
- **The interface is the test surface.** `read_fresh` unit tests assert fresh-returns-`Some` and
  stale-returns-`None` directly against the cache (using `write_with_fetched_at` to control age
  without sleeping), mirroring the existing `TaskListCache` tests.
- **The deletion test passes.** Deleting `read_fresh` would push the age arithmetic back into the
  controller (and any future caller) — it concentrates the freshness policy, not merely moves it.
- Full suite green; `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, `comment_policy`
  clean; complexity within budget.

## Alternatives considered

- **Move `PROJECT_NAMES_TTL_SECS` into `cache.rs` too (a fixed window baked into the cache).**
  Rejected: `TaskListCache::read` takes `max_age_secs` as a parameter so the *policy* (how long is
  fresh) stays with the caller and the *mechanism* (the comparison) stays with the cache. Matching
  that split keeps the two caches symmetric and leaves the window a controller-level knob.
- **A generic `Cache<T>::get_or_fetch(key, refresh)` that owns the network fetch too** (the shape
  floated in the architecture review). Rejected: it would couple the cache layer to the async,
  type-specific HTTP fetchers (`list_projects` vs `fetch_user_map`) and drag `ActiveCollabClient`
  into `store::cache`. The fetch stays in the controller; only the freshness *check* moves. Depth
  without the wrong coupling.
- **Leave the split (status quo).** Rejected: `fetched_at` leaks across the seam and every caller
  that reads project names must re-derive the TTL comparison — a live inconsistency risk against the
  `TaskListCache` precedent.

## Consequences

**Positive:** the project-names freshness policy has one home (the cache), symmetric with
`TaskListCache`; `fetched_at` no longer leaks to the controller; `fresh_project_names_cache_read`
shrinks to a store-open plus one call; a future consumer of the project-names cache inherits the
freshness check for free instead of copying the arithmetic.

**Accepted trade-offs:** `ProjectNamesCache` gains one method (`read_fresh`) beside `read` — a
deliberate, small interface addition that increases depth (more behavior hidden) rather than surface.

## Related

- ADR: [/adr/0014-browse-list-project-name-cache-swr.md](/adr/0014-browse-list-project-name-cache-swr.md) (the project-names SWR cache whose freshness check this single-homes)
- ADR: [/adr/0017-task-list-first-paint-cache-swr-entry.md](/adr/0017-task-list-first-paint-cache-swr-entry.md) (the task-list SWR entry served by `TaskListCache::read`, the deep shape this mirrors)
