---
type: Issue
title: "Comment-card keyboard focus — j/k focuses a comment card, highlight + scroll-into-view, actions stay on click"
description: Slice 1 of comment-card navigation. Add focused_comment + a comment_spans line-range cache to Screen::Detail (mirroring the Tasks card-layout cache, ADR 0031); j/k (and Up/Down) move the focus across comment cards via Msg::FocusNextComment/FocusPrevComment, clamped at both ends; moving focus derives a new scroll offset so the focused card is fully visible; the focused card renders with a highlight style; PageUp/PageDown still scroll lines and leave focus unchanged; edit/delete stay on the existing Ctrl/Cmd+click affordances. Empty thread = no focus.
status: open
labels: [tui, comments, navigation, focus, keyboard, slice]
blocked_by:
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Comment-card keyboard focus — move, highlight, scroll into view

Slice 1 of comment-card navigation. Implements
[BDR 0025](/bdr/0025-comment-card-navigation-and-contextual-footer.md) Scenarios 1–6
under [ADR 0037](/adr/0037-comment-card-keyboard-focus.md), mirroring the Tasks
card-layout cache ([ADR 0031](/adr/0031-tasks-card-layout-cache.md)) and keeping the
edit/delete click path ([ADR 0036](/adr/0036-permission-aware-comment-targeting.md))
unchanged.

### Problem

The detail thread is keyboard-navigable only by raw line scroll; there is no notion of
"the comment I'm on" and no visible cursor over the comment cards. ADR 0036 deferred a
keyboard path; this adds it as a focus cursor (navigation), leaving actions on the proven
click affordances.

### Decision (from ADRs)

- **Focus state (ADR 0037 §1):** `Screen::Detail` gains `focused_comment: Option<usize>`
  — an index into the thread's comments, `None` when there are none. It is separate from
  the line-scroll `offset`.
- **Line-range cache (ADR 0037 §2, mirror ADR 0031):** `build_detail_content` also
  exports `comment_spans: Vec<(start_line, line_count)>` (the global line range of each
  comment card), cached on `Screen::Detail` and rebuilt by `reflow_detail` on the same
  `rendered_width` invalidation as the line cache.
- **Key mapping (ADR 0037 §3):** `map_browse_key_event` (Detail) maps `j`/`k` (and reuses
  `Up`/`Down`) to `Msg::FocusNextComment`/`FocusPrevComment`; `PageUp`/`PageDown` and the
  wheel stay raw line scroll. Compose mode is unaffected (compose key map owns every key).
- **Move → highlight + scroll into view (ADR 0037 §4):** a focus move clamps the index,
  then sets `offset` so the focused card is fully visible (reusing the Tasks
  `first_visible_card` discipline); the focused card renders with a focus style.

### Scope

Included:

- `src/tui/model.rs` — `focused_comment` + `comment_spans` fields on `Screen::Detail`;
  `Msg::FocusNextComment`/`FocusPrevComment` handling in `update()` (clamp + derive
  `offset`); `reflow_detail` builds/invalidates `comment_spans`; the scroll-into-view
  helper (mirrors `first_visible_card`).
- `src/render.rs` — `build_detail_content` exports `comment_spans`; the focused card
  renders with the focus highlight style (driven by `focused_comment`).
- `src/tui/events.rs` — `map_browse_key_event` gains `j`/`k` → focus msgs.
- `src/theme.rs` — the focus-highlight style (if a new style is needed).
- Tests: `tests/unit/model.rs`, `tests/unit/tui_render.rs`.

Excluded: the contextual footer + status line (issue 0036); any `e`/`x` key action on the
focused comment (kept on click, ADR 0037 Alternatives).

### Acceptance

- AC1 — `update()`: `FocusNextComment`/`FocusPrevComment` move `focused_comment` by one
  and emit no Cmd; at the last/first the move is a no-op (no wraparound); a zero-comment
  thread keeps `focused_comment = None`.
- AC2 — `update()`: with a `comment_spans` fixture, a focus move sets `offset` so the
  focused card is fully visible — below the viewport → its last line visible, above → its
  first line, already visible → `offset` unchanged.
- AC3 — `update()`: `PageUp`/`PageDown` (and wheel) change `offset` and leave
  `focused_comment` unchanged (the two move models do not collide).
- AC4 — render (`TestBackend`): the focused card carries the focus style and the others do
  not; moving focus moves the highlight.
- AC5 — `reflow_detail`: `comment_spans` is rebuilt on width/data change and reused (no
  rebuild) at unchanged width — same invalidation as the line cache.
- AC6 — regression: a Ctrl/Cmd+click on the focused own card's `[editar]`/`[excluir]`
  still emits `ComposeOpen(Edit)` / `DeleteCommentRequest`; no key acts on the focused
  comment.
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  functions over explanatory comments) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior and survive the mutation floor on changed lines
  (`verify_by: command`).

### Plan

1. Add `focused_comment` + `comment_spans` to `Screen::Detail`; update every construction
   site (born with empty `comment_spans`, `focused_comment = None`).
2. `build_detail_content` exports `comment_spans`; `reflow_detail` builds/invalidates it.
3. `update()`: `FocusNextComment`/`FocusPrevComment` — clamp + the scroll-into-view helper
   (mirror `first_visible_card`).
4. `map_browse_key_event`: `j`/`k` (+ `Up`/`Down`) → focus msgs.
5. Render the focus highlight on the focused card (`theme.rs` style).
6. Tests: focus transitions, clamp, scroll-into-view (fixture), page-scroll-keeps-focus,
   highlight render, cache reuse, click-still-acts regression.

Observable end-to-end: open a task with several comments, press `j`/`k`, and watch the
focus highlight move card to card and scroll the focused card into view.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
