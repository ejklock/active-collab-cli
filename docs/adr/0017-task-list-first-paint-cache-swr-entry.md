---
type: ADR
title: First-paint-from-cache SWR on browse/mine entry (task-list snapshot cache)
description: Cache the built open-tasks list per instance-set so entering browse/mine paints the last-known list instantly, then always revalidates in the background and swaps. Unblocks mine (which today fetches before the TUI opens) and refines ADR 0014's "never cache open tasks" — scoped to the entry paint only; refresh stays always-fetch.
status: Accepted
supersedes:
superseded_by:
tags: [tui, performance, cache, swr, controller, store, entry]
timestamp: 2026-06-26T00:00:00Z
---

# 0017. First-paint-from-cache SWR on browse/mine entry

## Context

Entering a listing is slow on the first paint, and the two entry paths pay it
differently:

- **`browse`** opens the TUI immediately but with an empty `Screen::Projects {
  loading: true }`; `init_browse` (`src/tui/model.rs`) emits
  `Cmd::LoadTasksByProject` and the operator stares at a "Loading tasks…"
  placeholder until `controller::tasks_by_project` returns.
- **`mine`** (the default `ac` in a TTY) is worse: `mine_core` (`src/commands.rs`)
  runs `collect_mine_rows` **to completion before the TUI opens** — the terminal
  blocks on the network with no UI at all, then `run_mine` seeds a fully-loaded
  `Screen::Tasks`.

Force: **entry responsiveness** — the same [PRD 0001](/prd/0001-rust-tui-cli-parity.md)
NFR family as [ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md), but for
**entry**, not refresh. ADR 0014 fixed *refresh* latency by caching the project
**directory**, and explicitly **deferred** the entry case: *"A separate
first-paint-from-cache SWR for entry — not refresh — is the remaining S8 scope and
stays deferred."* It also rejected *"cache the open tasks too"* — but that
rejection was scoped to **refresh** (`r` must re-fetch the tasks the operator
wants fresh). Entry is different: the first paint may show a stale list **as long
as a revalidation always follows**.

## Decision

Add a **task-list snapshot cache** and make browse/mine **entry**
stale-while-revalidate (SWR), delivered as backlog item **S8**. This completes the
entry half ADR 0014 deferred.

### 1. New cache table (`src/store/cache.rs` + migration in `src/store/mod.rs`)

A `TaskListCache` storing the **built list** keyed by `(scope, instances_key)`:

- `scope` ∈ `{"browse", "mine"}` — the two list shapes.
- `instances_key` — the sorted, joined target instance names (the aggregation
  identity; open tasks are the authenticated user's, one user per instance token).
- `list_json` — the serialized built list (`Vec<ProjectGroup>` for browse,
  `Vec<MineTableRow>` for mine).
- `fetched_at` — epoch seconds, for the "last updated" footer and a generous
  **max-age** guard.

It mirrors the `UserMapCache`/`ProjectNamesCache` shape and connection discipline
(no `Connection` held across an `await`).

### 2. Entry seeds from the snapshot; the async loop always revalidates

- **`browse`**: `init_browse` reads the snapshot. **Hit** (present and within
  max-age) → seed `Screen::Projects { groups: snapshot, loading: false,
  revalidating: true }` and **still** emit the load `Cmd`. **Miss** → current
  behavior (`loading: true`, empty, fetch).
- **`mine`**: stop blocking before the TUI. `dispatch_mine` reads the snapshot and
  seeds the mine model from it (`loading: false, revalidating: true`); a new mine
  load `Cmd`/`Msg` revalidates **inside the async loop** (ADR 0008) so the TUI
  opens instantly. **Miss** → `loading: true`, fetch (the TUI still opens first).
- **On revalidation completion** (`handle_loaded_tasks` and the mine equivalent):
  replace the list, clear `revalidating`, stamp `last_loaded`, and **write the
  snapshot** for next time.

### 3. New model state: `revalidating` (distinct from `loading`)

`loading` means "no content yet, show a placeholder". `revalidating` means "showing
cached content while a fresh fetch is in flight". The view shows a subtle
indicator (e.g. in the last-updated footer) when `revalidating` is set; it never
blanks the already-painted list. The TUI core stays pure (the cache read happens in
the shell at construction; the model only carries the seeded data + flags).

### 4. Refresh is untouched (entry-only SWR)

`handle_refresh` keeps ADR 0014's semantics exactly: it sets `loading`/in-flight
and re-fetches; it **never** seeds from the snapshot. Caching open tasks is
introduced **solely** as an entry-paint accelerator — so this refines, and does not
contradict, ADR 0014's rejection of caching open tasks for refresh.

### 5. Fitness function

The path is instrumented (`timing::record("browse_list_load" | "mine_list_load",
…)`). Gate-checked targets:

- **Warm entry paints before any network completes** — a pure model test seeds the
  snapshot and asserts `init_browse` / the mine constructor yield a model with a
  **non-empty** list, `loading: false`, `revalidating: true`, **and** a load `Cmd`
  (revalidation always dispatched).
- **Cold entry falls back** to `loading: true` + fetch (no regression).
- **Revalidation rewrites the snapshot** (`handle_loaded_*` writes the cache).
- **Refresh never seeds from the snapshot** (a warm snapshot does not change the
  refresh model — still `loading`/in-flight).
- **Snapshot is per `instances_key`** — a different instance set does not cross-read.

## Alternatives considered

- **No task snapshot; just unblock mine with the browse-style placeholder.**
  Rejected as the chosen path: it honors ADR 0014 fully and never shows stale data,
  but it is not SWR — the operator still sees a placeholder, not real content, until
  the fetch returns. It does not deliver the S8 intent. (Its one real win — opening
  the mine TUI before the fetch — is folded into this decision anyway.)
- **SWR for browse only; leave mine blocking.** Rejected: `mine` is the **default**
  `ac` command, so its entry latency is the one the operator hits most. Fixing only
  browse leaves the common case slow.
- **Cache open tasks for refresh too.** Rejected — unchanged from ADR 0014: refresh
  is exactly when the operator wants the open tasks re-fetched.
- **One global snapshot across instance sets.** Rejected: the list is an
  aggregation over a specific target set; keying by `instances_key` avoids painting
  a snapshot from a different `--instance` scope ([obs 31] multi-instance identity).
- **TTL-gate the entry paint (don't paint a stale snapshot).** Rejected for the
  default: SWR's whole point is to paint stale-then-fresh. A generous **max-age**
  guard only avoids painting an absurdly old snapshot; within it, stale is shown by
  design because a revalidation always follows immediately.

## Consequences

**Positive:** entering `browse`/`mine` paints the last-known list instantly;
`mine` no longer blocks the terminal before the TUI opens; freshness is preserved
because entry **always** revalidates and refresh is unchanged; reuses the proven
`UserMapCache` cache pattern and the ADR 0008 async loop.

**Accepted trade-offs:** the first paint may show a briefly-stale list until the
revalidation swaps it in (bounded by max-age and an always-on revalidate); one more
cache table and a `store` migration; `mine` gains a load `Cmd`/`Msg` so its fetch
moves from pre-TUI into the async loop (a small restructure of `dispatch_mine`).

## Related

- ADR: [/adr/0014-browse-list-project-name-cache-swr.md](/adr/0014-browse-list-project-name-cache-swr.md) — the refresh half (project-name cache); this is the deferred entry half it named.
- ADR: [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md) — the loop the revalidation runs on.
- BDR: [/bdr/0005-loader-single-flight-refresh.md](/bdr/0005-loader-single-flight-refresh.md)
- BDR: [/bdr/0011-task-list-first-paint-swr-entry.md](/bdr/0011-task-list-first-paint-swr-entry.md)
- PRD: [/prd/0001-rust-tui-cli-parity.md](/prd/0001-rust-tui-cli-parity.md)
- Issue: [/issues/0016-s8-task-list-first-paint-swr.md](/issues/0016-s8-task-list-first-paint-swr.md)
- Architecture: [/architecture.md](/architecture.md)
