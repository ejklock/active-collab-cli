---
type: ADR
title: render.rs splits into a layered stack — a pure text_measure core, a wrap module over it, and two output adapters (cli_render, detail_render) — retiring the 2234-line god-module
description: src/render.rs (2234 lines) does three unrelated jobs behind one module — non-TTY CLI string rendering (render_task_to_str/render_meta_to_str/render_comments_to_str/html_to_text, the get/current/mine parity path), TUI detail content layout (build_detail_content + header/body/comment/asset builders), and the greedy word-wrap engine (greedy_wrap + WrapCell/WrapLine + wrap_text/wrap_rich) — on top of a set of display-width primitives (display_width, slice_by_display_cols, box_inner_content, fit_to_display_width, PANEL_HPAD/BODY_LEFT_CHROME_COLS, the box-drawing chars) that other modules reach across the seam to borrow (model.rs, screens/tasks.rs). Carve render.rs into a layered stack: a tiny pure text_measure module owns the width/slice/box primitives and chrome constants (no richtext dependency); a wrap module owns the greedy engine over text_measure; and two thin output adapters, cli_render (plain strings) and detail_render (TUI DetailContent), sit on top. box_inner_content becomes text_measure's public interface, retiring the box_inner_content_pub pass-through. No behavior change — the split is structural; the existing CLI parity and TUI buffer suites are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [render, tui, cli, refactor, locality, depth, module-structure, ratatui]
timestamp: 2026-06-30T00:00:00Z
---

# 0049. render.rs splits into text_measure ← wrap ← {cli_render, detail_render}

## Context

`src/render.rs` has grown to **2234 lines** and is the largest module in the crate. Behind its
single `render` interface it carries three unrelated jobs plus a shared foundation:

- **Non-TTY CLI string rendering** — `render_task_to_str`, `render_meta_to_str`,
  `render_comments_to_str`, `html_to_text`: the plain-text output for `ac get`/`current`/`mine`
  ([ADR 0011](/adr/0011-agent-json-output-contract.md) sibling; the Python-parity path). These
  consume `richtext` and produce `String`s for stdout. No terminal geometry, no styling.
- **TUI detail content layout** — `build_detail_content` and its builders (`build_header_lines`,
  `build_body_lines`, `build_comment_lines`, `build_comment_card`, `splice_asset_section`): the
  `DetailContent { lines, line_styles, affordances, comment_spans }` the Detail screen renders
  ([ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md)). These consume `richtext`,
  `asset_panel`, and the wrap engine, and emit styled lines + the affordance registry.
- **The greedy word-wrap engine** — `greedy_wrap` + the `WrapCell`/`WrapLine` traits +
  `wrap_text`/`wrap_rich` ([ADR 0048](/adr/0048-unify-plain-and-rich-wrap-engines.md)). It
  depends on the width primitives and on `richtext` (for the rich adapter).
- **Display-width primitives** — `display_width`, `slice_by_display_cols`, `box_inner_content`
  (+ its pass-through `box_inner_content_pub`, `render.rs:130`), `fit_to_display_width`,
  `panel_content_width`, the chrome constants `PANEL_HPAD`/`BODY_LEFT_CHROME_COLS`, and the
  box-drawing chars. These are pure, `richtext`-free, and — the tell — **reached across the seam
  by other modules**: `model.rs` borrows `BODY_LEFT_CHROME_COLS`, `box_inner_content_pub`,
  `display_width`, and `slice_by_display_cols` for text selection; `screens/tasks.rs` borrows
  `display_width`, `wrap_text`, and `PANEL_HPAD` and re-declares its own copy of the box chars.

Because the four concerns live in one module, a bug in "render a task" has two unrelated homes
to search (CLI path vs TUI path), the god-module keeps growing with every detail feature, and the
shared primitives are exposed as ad-hoc `pub` items (the `box_inner_content_pub` wrapper exists
solely so `model.rs` can borrow a private function) rather than as a named, deep interface.

## Decision

Carve `render.rs` into a **layered stack of four modules**, each deep with a small interface,
dependencies pointing one direction (up the stack).

1. **`src/render/text_measure.rs` — the pure width/geometry core.** Owns the display-width
   primitives and the layout constants, with **no `richtext` dependency**:
   - `display_width(&str) -> usize`, `slice_by_display_cols(&str, start, end) -> String`,
     `fit_to_display_width`, `truncate_to_display_width`.
   - `box_inner_content(&str) -> Option<&str>` — now a first-class **public** function; the
     `box_inner_content_pub` pass-through is deleted and its callers point here.
   - the chrome constants (`PANEL_HPAD`, `BODY_LEFT_CHROME_COLS`, `panel_content_width`) and the
     box-drawing chars (`BOX_TL`/`TR`/`BL`/`BR`/`H`/`V`) — single-homed, so `screens/tasks.rs`
     stops re-declaring them.

2. **`src/render/wrap.rs` — the greedy word-wrap engine.** Owns `greedy_wrap` + the
   `WrapCell`/`WrapLine` traits + the private placement/hard-split helpers, and the two public
   adapters `wrap_text` / `wrap_rich` ([ADR 0048](/adr/0048-unify-plain-and-rich-wrap-engines.md)
   is preserved verbatim — this only relocates it). Depends on `text_measure` for measurement and
   on `richtext` for the rich adapter's cell type.

3. **`src/render/cli_render.rs` — the non-TTY output adapter.** Owns `render_task_to_str`,
   `render_meta_to_str`, `render_comments_to_str`, `html_to_text`. Depends on `richtext` and
   `text_measure`; produces `String`s for the CLI. The byte-for-byte parity contract is unchanged.

4. **`src/render/detail_render.rs` — the TUI detail-content adapter.** Owns `build_detail_content`
   and its builders. Depends on `wrap`, `text_measure`, `richtext`, and `asset_panel`; emits
   `DetailContent`. The `DetailContent` type and the affordance emission
   ([ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md)) are unchanged.

`render.rs` becomes `render/mod.rs` — a thin module root that re-exports the public surface the
rest of the crate already imports (`build_detail_content`, `wrap_text`, `display_width`, …), so
call sites outside the split are untouched in this slice; a follow-up may point them at the leaf
modules directly.

### Guard / fitness function

- **No behavior change.** CLI output (`ac get`/`current`/`mine` parity) and the TUI detail buffer
  are byte-for-byte / cell-for-cell identical; every existing CLI-parity and buffer-derived detail
  spec stays green. The split is pure relocation.
- **The layering is acyclic and one-directional.** `text_measure` depends on nothing in `render`;
  `wrap` depends only on `text_measure` (+ `richtext`); `cli_render` and `detail_render` depend on
  the two below them, never on each other. `text_measure` stays `richtext`-free (grep finds no
  `richtext` import in it) — the property [ADR 0050](/adr/0050-detail-geometry-absorbs-selection-column-math.md)
  and [ADR 0051](/adr/0051-extract-task-layout-module.md) rely on to borrow width math without
  dragging `richtext`.
- **The primitives are a named interface, not ad-hoc `pub`.** `box_inner_content_pub` is gone;
  `box_inner_content` is `text_measure`'s public function. `screens/tasks.rs` imports the box
  chars and `PANEL_HPAD` from `text_measure` instead of re-declaring them — grep finds one home
  for `BOX_TL`.
- **The deletion test passes.** Deleting `text_measure` would scatter the width/box math back
  across `render`, `model`, and `tasks`; deleting `wrap` would force the greedy loop back inline.
  Each module concentrates complexity rather than merely moving it.
- Full suite green; `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, and the
  `comment_policy` gate clean; complexity within budget (cyclomatic ≤ 10, ≤ 8 for new functions).

## Alternatives considered

- **Split by job only (cli_render / detail_render), leave the primitives + wrap in a shared
  `render` root.** Rejected: it leaves the width primitives as ad-hoc `pub` items that `model` and
  `tasks` still reach for by name, and keeps `box_inner_content_pub` alive. The point of the split
  is to give the shared foundation a *named, deep interface* (`text_measure`) other modules depend
  on deliberately — not to leave it as a grab-bag.
- **Fold `wrap` into `text_measure` (one "text geometry" module).** Rejected: `wrap_rich` pulls in
  `richtext`, so folding it in would force `text_measure` to depend on `richtext`, and every
  consumer that only needs to *measure* a width (`detail_geometry` selection, `task_layout` card
  height) would drag `richtext` transitively. Keeping `wrap` a separate layer keeps the core
  `richtext`-free (chosen in the design grilling for this reason).
- **Leave render.rs as one module (status quo).** Rejected: the god-module is the reason a
  "render a task" bug has two homes and the primitives leak as ad-hoc `pub`; it grows with every
  detail feature. This continues the [ADR 0016](/adr/0016-refactor-render-decompose-relocate.md)
  render de-duplication discipline.

## Consequences

**Positive:** one home per job — a CLI-output bug and a detail-layout bug no longer share a file;
the shared width/box math is a named, pure, `richtext`-free module (`text_measure`) other modules
depend on deliberately instead of borrowing private items; `box_inner_content_pub` and the
duplicated box chars are gone; the 2234-line module is broken into four scannable, independently
testable modules; the layering makes the dependency direction explicit and enforceable.

**Accepted trade-offs:** four modules where there was one, and a `render/mod.rs` re-export shim so
this slice does not have to rewrite every external call site at once (a deliberate, temporary
seam; a later tidy can point imports at the leaf modules). The split is large and touches many
files, so it is executed as an ordered set of slices (extract `text_measure` first, then `wrap`,
then the two adapters), each behavior-preserving and independently reviewable.

## Related

- ADR: [/adr/0048-unify-plain-and-rich-wrap-engines.md](/adr/0048-unify-plain-and-rich-wrap-engines.md) (the wrap engine this relocates into `wrap`, unchanged)
- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md) (the prior render.rs decomposition discipline this continues)
- ADR: [/adr/0043-detail-hit-targets-emitted-structurally.md](/adr/0043-detail-hit-targets-emitted-structurally.md) (the DetailContent + affordance registry `detail_render` owns, unchanged)
- ADR: [/adr/0050-detail-geometry-absorbs-selection-column-math.md](/adr/0050-detail-geometry-absorbs-selection-column-math.md) (consumes `text_measure` to deepen `detail_geometry`)
- ADR: [/adr/0051-extract-task-layout-module.md](/adr/0051-extract-task-layout-module.md) (consumes `text_measure` for card layout)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md) (the module tree these leaves join)
