---
type: Issue
title: "Memoize the Tasks-screen card layout — cache heights + prefix-sum offsets, binary-search first-visible, widen offsets to u32"
description: draw_tasks re-wraps every task's title on every redraw (per event) to recompute card heights; total_rows and the cumulative-offset accumulator are u16 (saturate past ~16k rows); first_visible_card is a linear O(T) walk. Embed a memoized card-layout cache in Screen::Tasks (mirroring the Detail line cache), rebuild only on task-list/width change, binary-search first-visible, widen offsets to u32. Behavior-preserving.
status: open
labels: [tui, performance, scalability, tasks, cache]
blocked_by:
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Memoize the Tasks-screen card layout

Implements [ADR 0031](/adr/0031-tasks-card-layout-cache.md). Surfaced by an
algorithmic-complexity review of the render path. Behavior-preserving scalability
hardening — no observable render change, so no BDR.

### Problem

`draw_tasks` (`src/tui/screens/tasks.rs`) runs on every redraw (event-driven TUI), and
each call `build_card_heights(tasks, …)` re-wraps every task title — O(total task text)
per event — even though `render_cards` is already windowed. `total_rows` and the
`first_visible_card` cumulative accumulator are `u16` (saturate past ~16k rows);
`first_visible_card` is an O(T) linear walk.

### Decision

Embed a memoized cache (`card_heights: Vec<u16>`, `card_offsets: Vec<u32>` prefix sum
of length `n+1`, `rendered_width: usize`) in `Screen::Tasks`, mirroring the Detail line
cache. Rebuild via `Model::reflow_tasks` only on width or list change; `draw_tasks`
reads the cache; `first_visible_card` becomes a binary search (O(log T)); offsets are
`u32` (no saturation). See ADR 0031 for the full design + rejected alternatives.

### Scope (atomic — single slice)

This is an **atomic** change: adding required fields to the `Screen::Tasks` enum variant
forces every construction site to change in the same compilation unit, so it cannot be
split across green sub-slices. Accepted cap-exception (same class as the ADR 0029 S2b
cutover).

Included:

- `src/tui/model.rs` — add `card_heights`/`card_offsets`/`rendered_width` to
  `Screen::Tasks`; add `Model::reflow_tasks(card_inner_w)` (mirrors `reflow_detail`);
  invalidate in `handle_loaded_mine_tasks` (set `rendered_width = usize::MAX`, clear
  cache vecs); set `rendered_width: usize::MAX` + empty caches at every `Screen::Tasks`
  construction (model.rs:299, 976, 1199).
- `src/tui/screens/tasks.rs` — `draw_tasks` reads `card_heights`/`card_offsets` instead
  of recomputing (with a defensive inline-compute fallback on width mismatch);
  `first_visible_card` → binary search over `card_offsets`; `total_rows` from
  `card_offsets[n]` (`u32`).
- `src/tui/mod.rs` — call `model.reflow_tasks(card_inner_w)` pre-draw, where
  `reflow_detail` is called, using the same `card_inner_w` formula `draw_tasks` uses.
- `src/tui/view.rs` — destructure the new `Screen::Tasks` cache fields and pass them to
  `draw_tasks`.
- `tests/unit/{app,model,tui_render}.rs` — update every `Screen::Tasks` construction
  with the new fields; add the ADR 0031 fitness tests (cache reuse no-op, list/width
  rebuild, binary-search first-visible, no u16 saturation).

Excluded: the SWR snapshot cache ([ADR 0017](/adr/0017-task-list-first-paint-cache-swr-entry.md),
a different layer); the Detail cache; any change to rendered output.

### Acceptance

- The Tasks card layout (heights + prefix-sum offsets) is memoized in `Screen::Tasks`
  and rebuilt by `reflow_tasks` only when the task list or `card_inner_w` changes; an
  unchanged width + unchanged list reflow is a no-op (does not rebuild).
- A list swap (`handle_loaded_mine_tasks`) and a width change both invalidate/rebuild;
  changing only `selected` (navigation) does NOT rebuild.
- `first_visible_card` is a binary search over the prefix-sum offsets (O(log T)) and
  produces the same first-visible index as the previous linear walk (selection near the
  end scrolls correctly).
- `card_offsets`/`total_rows` are a non-saturating width type (`u32`): a cumulative
  height beyond `u16::MAX` does not clamp.
- Behavior-preserving: existing `draw_tasks`/card-render tests (geometry, click targets,
  selection highlight, scrollbar) stay green; the rendered buffer is unchanged.
- Full suite green; `clippy -D warnings`, `fmt`, comment-policy clean; per-function
  complexity within budget (cyclomatic ≤ 10 / ≤ 8 new); tests mutation-resistant
  (buffer-derived assertions).

### Plan

Single atomic slice (production cutover + all construction-site updates + fitness
tests). No vertical sub-cut is demoable (the field addition must compile across all
sites at once).
