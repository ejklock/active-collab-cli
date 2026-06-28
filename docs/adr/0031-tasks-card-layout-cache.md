---
type: ADR
title: Memoize the Tasks-screen card layout (prefix-sum offsets + binary-search first-visible) so per-event cost scales with the viewport, not the task count
description: draw_tasks recomputes every card's height by re-wrapping every task's title on every redraw (per event), making the Tasks screen O(total task text) per keystroke; total_rows and the cumulative-offset accumulator are u16 and saturate past ~16k rows; first_visible_card is a linear O(T) walk. Embed a memoized card-layout cache (heights + prefix-sum offsets) in Screen::Tasks, rebuilt only on task-list or width change (mirroring the Detail line cache), widen offsets to u32, and make first_visible_card a binary search. Behavior-preserving scalability hardening; no observable render change.
status: Accepted
supersedes:
superseded_by:
tags: [tui, performance, scalability, ratatui, tasks, cache]
timestamp: 2026-06-28T00:00:00Z
---

# 0031. Memoize the Tasks-screen card layout (prefix-sum + binary search)

## Context

The Tasks screen renders the project/mine task list as stacked bordered cards.
`draw_tasks` (`src/tui/screens/tasks.rs`) runs on **every redraw** — and this is an
event-driven TUI (`EventStream + tokio::select!`, [ADR 0008](/adr/0008-async-event-loop-with-eventstream-and-select.md)),
so a redraw fires on **every keystroke, mouse event, resize, and load**. On each one,
`draw_tasks` calls `build_card_heights(tasks, card_inner_w)`, which **re-wraps every
task's title** (`wrap_text` per task) to recompute each card's row count — even though
the rendering itself is already **windowed** (`render_cards` draws and records click
targets only for the visible cards via `.skip(first_visible)` + `break`).

Three cost facts follow, all of which are **dormant at small N and bite at large N**:

1. **Per-event O(total task text).** Computing all card heights re-wraps the full
   list's text on every event. With thousands of tasks in one project view, every
   arrow-key scroll re-wraps thousands of titles. The visible work is O(viewport); the
   hidden work is O(T).
2. **`u16` saturation ceiling.** `total_rows: u16 = card_heights.iter().sum()` and the
   cumulative offset accumulator in `first_visible_card` are `u16`. Past ~16k rows they
   `saturating_add`-clamp at 65535 — a silent **correctness** ceiling, not just perf.
3. **Linear `first_visible_card`.** It builds a cumulative-offset `Vec` over all heights
   and walks it linearly — O(T) per redraw.

The project already has the right pattern for exactly this: the **Detail** screen
embeds a memoized cache (`lines`, `line_styles`, `rendered_width`) in its `Screen::Detail`
variant, rebuilt by `reflow_detail` only when the width changes (`*rendered_width ==
inner_width` early-returns) and invalidated on data change by resetting `rendered_width
= usize::MAX` (`handle_loaded_detail`/`handle_user_map_resolved`). The Tasks screen is
the one list view **without** that memoization.

Force: **scalability of the read path** — per-event cost should scale with what is
shown (the viewport), not with the whole list. This is a code-level performance force
with a present, named instrument (re-wrap-per-event), so the fix is a local deepening
mirroring an existing in-repo pattern, not an architecture change.

> Honest scope note (evidence gate): at the current typical scale (tens of tasks) this
> is **negligible** — the change is a deliberate **future-proofing** for large lists,
> accepted as such. It is behavior-preserving; the rendered output does not change.

## Decision

Embed a **memoized card-layout cache** in `Screen::Tasks`, mirroring the Detail line
cache, and derive scrolling from a **prefix-sum offset array** via **binary search**.

### 1. Cache the card layout in the variant

`Screen::Tasks` gains three fields, parallel to Detail's cache:

- `card_heights: Vec<u16>` — per-card row count (the current `build_card_heights`
  output).
- `card_offsets: Vec<u32>` — the **prefix sum** of `card_heights`, length `tasks.len()
  + 1`, where `card_offsets[i]` is the cumulative y-start of card `i` and
  `card_offsets[n]` is the total row count. **`u32`** removes the `u16` saturation
  ceiling (fact 2).
- `rendered_width: usize` — the `card_inner_w` the cache was built at; `usize::MAX`
  means "not yet built".

### 2. Rebuild only on width or list change (`reflow_tasks`)

A new `Model::reflow_tasks(&mut self, card_inner_w: usize)` mirrors `reflow_detail`:
top screen must be `Tasks` and not `loading`; if `rendered_width == card_inner_w` it
**early-returns** (cache hit); otherwise it rebuilds `card_heights`, recomputes
`card_offsets` as the prefix sum, and sets `rendered_width = card_inner_w`. It is called
**pre-draw** in the shell (`src/tui/mod.rs`), exactly where `reflow_detail` is called,
using the same `card_inner_w` formula `draw_tasks` uses.

Invalidation follows the Detail precedent: every `Screen::Tasks` **construction** is born
with `rendered_width: usize::MAX` and empty caches (forcing a first build), and the
in-place list swap in `handle_loaded_mine_tasks` resets `rendered_width = usize::MAX`
(+ clears the cache vectors) just as `handle_loaded_detail` does for the line cache. The
cache depends only on `tasks` + width, **not** on `selected` — so arrow-key navigation
(which mutates only `selected`) reuses the cache, which is the whole point.

### 3. `draw_tasks` reads the cache; `first_visible_card` is a binary search

`draw_tasks` receives `card_heights` + `card_offsets` (passed from `view.rs`, which
already destructures `Screen::Tasks`) instead of recomputing; `total_rows` is
`card_offsets[n]` (`u32`). `first_visible_card(offsets, selected, visible_h)` becomes a
**binary search** (`partition_point`) for the smallest `first` with `offsets[first] >=
sel_end - visible_h`, clamped to `<= selected` — **O(log T)** instead of O(T) (fact 3).

**Defensive floor:** if `draw_tasks` is ever handed a width that does not match the
cache (`rendered_width != card_inner_w`), it falls back to computing heights inline for
that one frame (the old path), so a width-derivation mismatch can never render wrong —
it only misses the cache benefit until the next `reflow_tasks`.

### Guard / fitness function

The decision is backed by instruments, not vibes:

- **Behavior-preserving:** the existing `draw_tasks`/card-render tests (geometry, click
  targets, selection highlight, scrollbar) stay green unchanged — the rendered buffer is
  identical.
- **Cache reuse:** a test asserts `reflow_tasks` at an unchanged width + unchanged list
  is a no-op (does not rebuild), and that a list swap / width change rebuilds.
- **O(log T) first-visible:** a unit test pins `first_visible_card` against a prefix-sum
  fixture (selection near the end scrolls correctly) and the binary-search boundary.
- **No saturation:** a test with a cumulative height beyond `u16::MAX` asserts the
  `u32` offsets do not clamp.
- Full suite green; `clippy -D warnings`, `fmt`, comment-policy clean; complexity within
  budget; tests mutation-resistant (buffer-derived assertions per the standing project
  pattern).

## Alternatives considered

- **Cache in `Model` as an `Option<TasksLayout>` instead of the variant.** Rejected:
  the cache would no longer be co-located with the data it derives from, so a list-swap
  site that forgot to invalidate would render a stale layout — the exact bug the
  variant-embedded Detail cache structurally prevents (a fresh list is *born* with
  `rendered_width: usize::MAX`). It also does not reduce churn: the test suite builds
  `Model` with full struct literals even more often than it builds `Screen::Tasks`.
- **No cache; only fix the cheap scaling defects** (widen `u16 → u32`, binary-search
  `first_visible_card`). Rejected as the primary fix: it removes the saturation ceiling
  and the O(T) walk but leaves the **dominant** cost — re-wrapping all task text on
  every event — untouched. (Those two fixes are folded *into* this change, not done
  alone.)
- **Leave it (YAGNI).** A legitimate position at current scale; the cost is dormant.
  Rejected here because the change was explicitly requested as future-proofing and the
  in-repo Detail pattern makes the correct shape low-risk. Recorded so the trade-off is
  visible.

## Consequences

**Positive:** per-event Tasks-screen cost drops from O(total task text) to O(viewport)
on the common path (cache hit); `first_visible_card` is O(log T); the `u16` saturation
ceiling is gone (`u32` offsets). The Tasks screen now uses the same memoize-and-reflow
discipline as Detail, so the two list/read views are consistent. Rendered output is
unchanged.

**Accepted trade-offs:** adding required fields to the `Screen::Tasks` enum variant
forces **every construction site to change in the same compilation unit** (~3 in `src/`,
~14 across `tests/unit/{app,model,tui_render}.rs`) — an irreducible, mechanical, **atomic**
diff (the field cannot be added across green sub-slices; the cap-exception is the same
class as the [ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md) S2b
cutover). The cache adds three fields and one prefix-sum vector to the variant's memory
footprint (bounded by the task count, already held). The pre-draw `reflow_tasks` width
must match `draw_tasks`'s `card_inner_w`; the defensive inline fallback makes a mismatch
a missed-cache, never a render bug.

## Related

- ADR: [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md) (why a redraw fires per event)
- ADR: [/adr/0026-task-list-as-cards.md](/adr/0026-task-list-as-cards.md) (the card list this memoizes)
- ADR: [/adr/0017-task-list-first-paint-cache-swr-entry.md](/adr/0017-task-list-first-paint-cache-swr-entry.md) (the other Tasks-list cache — SWR snapshot, a different layer)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (the atomic-cutover cap-exception precedent)
- Issue: [/issues/0030-tasks-card-layout-cache.md](/issues/0030-tasks-card-layout-cache.md)
- Architecture: [/architecture.md](/architecture.md)
