---
type: ADR
title: Task-card layout is one pure task_layout module — card content, height, inner width, and first-visible live once, shared by reflow_tasks and draw_tasks, retiring the two-place content string
description: The Tasks-screen card layout is split across two modules with a duplicated source of truth. task_card_height lives in model.rs and formats the card content string ("#{number}  {name}") to measure its wrapped height; task_card_content in screens/tasks.rs formats the IDENTICAL string to render it. The height cache (model) and the render (view) each derive from their own copy — change the content format and the height is silently wrong. tasks.rs also re-declares the box-drawing chars render already owns and reimplements the CARD_CHROME formula. The Detail side already has the pure detail_geometry + draw-only screens/detail.rs discipline; the Tasks side does not. Extract a pure src/tui/task_layout.rs owning card_content (single source), card_height, inner_w/CARD_CHROME, and first_visible (+ binary/linear); model.rs::reflow_tasks and screens/tasks.rs::draw_tasks both derive from it, and tasks.rs becomes a draw-only adapter over it. Box chars come from text_measure (ADR 0049). No behavior change — the existing Tasks buffer/layout specs are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [tui, tasks, layout, refactor, locality, depth, ratatui]
timestamp: 2026-06-30T00:00:00Z
---

# 0051. Task-card layout is one pure task_layout module

## Context

The Tasks-screen card layout ([ADR 0026](/adr/0026-task-list-as-cards.md),
[ADR 0031](/adr/0031-tasks-card-layout-cache.md)) is **split across two modules with a duplicated
source of truth**:

- `task_card_height(task, card_inner_w) -> u16` lives in **`model.rs:730`**. To compute a card's
  height it formats the content string inline: `format!("#{}  {}", task.task_number, task.name)`,
  wraps it, and counts rows. `reflow_tasks` calls it to build the `card_heights` / `card_offsets`
  cache.
- `task_card_content(task) -> String` lives in **`screens/tasks.rs:310`** and formats the
  **identical** string — `format!("#{}  {}", task.task_number, task.name)` — which
  `render_single_card` then wraps and draws.

So the height (model) and the render (view) each derive from their **own copy** of the content
format. Change it — add a status badge, a prefix, a separator — and the measured height no longer
matches the rendered content: the card is clipped or padded. The formula has two homes kept in
agreement only by convention.

The duplication continues in the chrome: `screens/tasks.rs` re-declares the box-drawing chars
(`BOX_TL`/`TR`/`BL`/`BR`/`H`/`V`, `tasks.rs:11-16`) that `render` already owns
(`render.rs:957-965`), and reimplements the card-chrome width as `CARD_CHROME = 2 + 2*PANEL_HPAD`.
Meanwhile the layout math itself (`tasks_card_inner_w`, `resolve_heights`, `first_visible_card` +
`first_visible_binary`/`first_visible_linear`) sits in `tasks.rs` interleaved with the ratatui
drawing code.

The **Detail side already has the right shape**: pure `detail_geometry`
([ADR 0045](/adr/0045-detail-viewport-geometry-module.md)) for the math + draw-only
`screens/detail.rs` for the ratatui pass. The Tasks side has no such separation — its pure layout
math is scattered between the model cache-builder and the view renderer.

This lands after [ADR 0049](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md),
which gives the box chars and `PANEL_HPAD` a single home in `text_measure`.

## Decision

Extract a **pure** `src/tui/task_layout.rs` that owns the Tasks-card layout math, and make
`screens/tasks.rs` a draw-only adapter over it — mirroring the `detail_geometry` +
`screens/detail.rs` split.

1. **One home for the content string.** `card_content(task: &TaskRow) -> String` lives once in
   `task_layout`; both the height measurement and the render derive the wrapped lines from it. The
   two `format!("#{}  {}", …)` copies are deleted.

2. **The pure layout interface.** `task_layout` owns:
   - `card_content(task) -> String` — the single content source.
   - `card_height(task, card_inner_w) -> u16` — wraps `card_content` and counts rows (relocated
     from `model.rs`).
   - `inner_w(terminal_width) -> usize` + `CARD_CHROME` — the width derivation (relocated from
     `tasks.rs`).
   - `first_visible(offsets, rendered_width, card_inner_w, inline_heights, selected, visible_h)
     -> usize` + the `binary`/`linear` implementations — the scroll-into-view search (relocated
     from `tasks.rs`), preserving the [ADR 0031](/adr/0031-tasks-card-layout-cache.md) prefix-sum
     semantics exactly.

3. **Both sides derive from it.** `model.rs::reflow_tasks` calls `task_layout::card_height`;
   `screens/tasks.rs::draw_tasks` calls `task_layout::inner_w` / `first_visible` / `card_content`.
   `tasks.rs` keeps only the ratatui drawing (`render_single_card`, `due_line`, the `Frame`
   widgets) — a draw-only adapter.

4. **Box chars from `text_measure`.** `tasks.rs` imports the box-drawing chars and `PANEL_HPAD`
   from `text_measure`
   ([ADR 0049](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md)) instead of
   re-declaring them.

### Guard / fitness function

- **Behavior preserved — invisible to the user.** The Tasks screen renders identically: same card
  heights, same first-visible card on scroll, same buffer. All existing Tasks buffer/layout specs
  (including the [ADR 0031](/adr/0031-tasks-card-layout-cache.md) prefix-sum/binary-search cases)
  stay green.
- **One content string, one height formula.** Grep finds exactly one `format!("#{}  {}"` (in
  `task_layout::card_content`) and one `card_height`. The cache and the render provably derive from
  the same source — a content-format change is a one-place edit that updates both height and draw.
- **The interface is the test surface.** `task_layout` unit tests assert `card_height` (single-line
  and wrapped titles), `inner_w`, and `first_visible` (selected fits / scrolls, binary vs linear
  parity) from primitives — no `Frame`, no `Model`.
- **One home for the box chars.** Grep finds `BOX_TL` declared once (in `text_measure`);
  `tasks.rs` imports it.
- **The deletion test passes.** Deleting `task_layout` would scatter the content string, height,
  width, and first-visible math back across `model` and `tasks` — it concentrates complexity, not
  merely moves it.
- Full suite green; `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, `comment_policy`
  clean; complexity within budget.

## Alternatives considered

- **Relocate `task_card_height` into `tasks.rs` and dedupe the string, no new module.** Rejected
  in the design grilling: it fixes the duplication but leaves the pure layout math interleaved with
  ratatui drawing in one file, so the Tasks side stays asymmetric with the Detail side and the math
  is not testable without the draw code around it. A pure `task_layout` module restores the
  `detail_geometry` symmetry.
- **Move the whole card rendering (`render_single_card`, `due_line`) into `task_layout` too.**
  Rejected: that pulls the ratatui `Frame` into the module and breaks the pure/impure split — the
  constitution's "pure, testable TUI core" non-negotiable. `task_layout` stays pure; drawing stays
  in `tasks.rs`.
- **Leave the split (status quo).** Rejected: the two-place content string is a live drift risk
  (height derived from a copy of what is rendered), and the box-char duplication means a visual
  change touches two files.

## Consequences

**Positive:** the card content string has one home, so the height cache and the render cannot
disagree; the pure Tasks layout math is single-homed and directly testable, symmetric with
`detail_geometry`; `tasks.rs` becomes a focused draw-only adapter; the duplicated box chars
collapse to one home in `text_measure`.

**Accepted trade-offs:** another small TUI submodule (`task_layout`) beside `detail_geometry` and
`hit_test` — a deliberate seam; `model.rs` and `screens/tasks.rs` both gain a dependency on it
(correct: both consume the layout, neither owns it).

## Related

- ADR: [/adr/0031-tasks-card-layout-cache.md](/adr/0031-tasks-card-layout-cache.md) (the prefix-sum / binary-search first-visible semantics `task_layout` preserves)
- ADR: [/adr/0026-task-list-as-cards.md](/adr/0026-task-list-as-cards.md) (the card layout being single-homed)
- ADR: [/adr/0045-detail-viewport-geometry-module.md](/adr/0045-detail-viewport-geometry-module.md) (the pure-geometry + draw-only split this mirrors on the Tasks side)
- ADR: [/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md) (the `text_measure` home for the box chars and `PANEL_HPAD`)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md) (the `src/tui/` module tree `task_layout` joins)
