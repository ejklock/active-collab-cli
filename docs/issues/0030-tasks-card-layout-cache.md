---
type: Issue
title: "Memoize the Tasks-screen card layout — cache heights + prefix-sum offsets, binary-search first-visible, widen offsets to u32"
description: draw_tasks re-wraps every task's title on every redraw (per event) to recompute card heights; total_rows and the cumulative-offset accumulator are u16 (saturate past ~16k rows); first_visible_card is a linear O(T) walk. Embed a memoized card-layout cache in Screen::Tasks (mirroring the Detail line cache), rebuild only on task-list/width change, binary-search first-visible, widen offsets to u32. Behavior-preserving.
status: closed
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

### Resolution

Closed by commit `0477c48` (implementation) on top of `138ba6a` (docs trail). Pipeline:
Coder → quality-gate → Reviewer (opus), 2 rounds, approved.

- **Implementation.** `Screen::Tasks` gained `card_heights: Vec<u16>`, `card_offsets:
  Vec<u32>` (prefix sum, len `n+1`), and `rendered_width: usize`. `Model::reflow_tasks`
  mirrors `reflow_detail` (early-return on `rendered_width == card_inner_w`; rebuild +
  set width otherwise); invalidated in `handle_loaded_mine_tasks` and born empty /
  `usize::MAX` at every `Screen::Tasks` construction. `draw_tasks` reads the cache via
  `resolve_heights` (returns `Cow<[u16]>` — borrow on hit, inline-compute on width
  mismatch); `first_visible_card` dispatches `first_visible_binary` (`partition_point`,
  O(log T)) vs `first_visible_linear` (defensive floor). `total_rows` is `u32` (no
  saturation).
- **Single source of truth for `card_inner_w`.** Extracted `tasks_card_inner_w`,
  re-exported via `screens/mod.rs`, used by both the pre-draw `reflow_tasks` call in
  `tui/mod.rs` and `draw_tasks`, so the cache is actually hit at runtime (never silently
  floored).
- **Deviations.** Only two literal `Screen::Tasks` construction sites exist in
  `model.rs` (`handle_select`, `init_mine`); the third candidate (~line 299) was a match
  arm, not a literal. The `first_visible_card` fitness test reaches the production fn via
  the direct module path (`pub(crate)`); a `mod.rs` re-export was dropped because
  `pub(crate) use` tripped clippy `unused_import` under `-D warnings`.
- **Review round 2.** Round 1 caught a real test-effectiveness defect: the AC3/TE
  fitness test re-implemented both algorithms inline and compared the copies, so the
  production binary search was never executed (a mutant on the `partition_point`
  predicate / `.min(selected)` survived). Round 2 rewrote the test to call the production
  function against a structurally-distinct oracle, with explicit cases that kill both
  mutants. Approved.
- **Gates.** Dev container authoritative: 873 tests + 31 comment_policy green, clippy
  `-D warnings` clean, fmt clean, complexity within budget. (QG-image clippy exit 101 and
  a comment-syntax-check hit on a prose why-comment were confirmed false negatives/
  positives.)
