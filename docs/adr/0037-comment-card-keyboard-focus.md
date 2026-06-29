---
type: ADR
title: Comment cards gain a keyboard focus cursor (highlight + scroll-into-view) over the global scroll, with actions left on the click affordances
description: The detail thread renders comments as bordered cards in one globally-scrollable Vec<line>, with edit/delete only reachable by Ctrl/Cmd+click (ADR 0036 deferred a keyboard path). Add a focus cursor over the comment cards — j/k (and Up/Down) move a focused_comment index, the focused card is highlighted and scrolled into view — but keep edit/delete on the existing [editar]/[excluir] click targets of the focused card. This layers the deferred keyboard navigability without resurrecting the reverted per-section focus-scroll model (ADR 0010); focus is a selection cursor over the single global scroll, not independent panels.
status: Accepted
supersedes:
superseded_by:
tags: [tui, comments, navigation, focus, keyboard, render]
timestamp: 2026-06-28T00:00:00Z
---

# 0037. A keyboard focus cursor over the comment cards

## Context

The detail thread already renders each comment as a **bordered card**
(`build_comment_card` → `panel_box_rich`, `src/render.rs`) inside the **single,
globally-scrollable `Vec<line>`** ([ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md)).
Acting on a specific comment is, for now, **mouse-only**: scroll-aware
`[editar]`/`[excluir]` Ctrl/Cmd+click targets on each own comment
([ADR 0036](/adr/0036-permission-aware-comment-targeting.md)). That ADR explicitly
**deferred a keyboard path**: *"A per-comment selection cursor (arrow-key navigable,
`e`/`x` act on the selected comment). Rejected for v1 … A keyboard path can be layered
later."* This is that follow-up.

Two forces shape the layering:

1. **Team/UX force — keyboard reach.** A terminal user expects to move through a thread
   without a mouse. Today the only keyboard control on Detail is line scroll
   (`Up`/`Down` → `Msg::Up`/`Down`); there is no notion of "the comment I'm on".
2. **A reverted-model hazard.** The detail view *deliberately* dropped a focus model
   once: [ADR 0010](/adr/0010-detail-sectioned-panels-focus-scroll.md) (fixed sections,
   `Tab` focus, **independent per-section scroll**) was **Reverted (U6c)** in favour of
   one global scroll (ADR 0029). Any new "focus" must not drag that back.

The app also already has the exact mechanism to mirror: the **Tasks** screen is a list
of bordered cards with a **selected index**, a memoized **per-card height + prefix-sum
offset** cache, and a **binary-search first-visible** that keeps the selection on
screen ([ADR 0026](/adr/0026-task-list-as-cards.md),
[ADR 0031](/adr/0031-tasks-card-layout-cache.md), `reflow_tasks`/`first_visible_card`).

## Decision

Add a **focus cursor over the comment cards** — a selection index into the thread,
navigated by keyboard, that **highlights** the focused card and **scrolls it into
view** — while **leaving edit/delete on the existing Ctrl/Cmd+click affordances** of the
focused card. The user picked this hybrid deliberately: keyboard *navigation*, click
*actions*.

### 1. Focus state on the Detail screen

`Screen::Detail` gains `focused_comment: Option<usize>` — an index into the thread's
comment list (`None` when the thread has no comments). It is **separate from `offset`**
(the line scroll): focus selects a *card*, `offset` positions the *viewport*. Moving
focus derives a new `offset`; free line-scrolling (`PageUp`/`PageDown` and the mouse
wheel) leaves `focused_comment` unchanged. This keeps focus a **cursor over the one
global scroll**, never the per-section independent scroll ADR 0010 reverted.

### 2. A per-comment line-range map (mirror ADR 0031)

`build_detail_content` already lays every comment card into the global `lines`. It
additionally exports a **`comment_spans: Vec<(start_line, line_count)>`** — the global
line range of each comment card — cached on `Screen::Detail` and rebuilt by
`reflow_detail` exactly when `lines` is rebuilt (same `rendered_width` invalidation as
the line cache). This is the comment analogue of the Tasks `card_offsets` prefix-sum:
it maps a `focused_comment` index → the card's first line and height, which is all the
"scroll into view" math needs.

### 3. Keyboard mapping (focus move vs. line scroll)

`map_browse_key_event` (Detail context) gains **`j`/`k`** — and reuses **`Up`/`Down`** —
to move focus to the next/previous comment card (`Up`/`Down` route through `handle_up`/
`handle_down`, which dispatch to the focus move on the Detail screen). To keep the two
move models from colliding, the rule is: **`j`/`k` and `Up`/`Down` move comment focus;
`PageUp`/`PageDown` and the mouse wheel scroll raw lines.** Compose mode is unaffected —
when `compose.is_some()` the compose key map owns every key (ADR 0034).

### 4. Move focus → highlight + scroll into view

On a focus move: clamp `focused_comment` into `0..comments.len()`, then set `offset` so
the focused card is fully visible — reusing the **Tasks `first_visible_card` discipline**
(if the card is above the viewport, scroll up to its start; if below, scroll down so its
end is visible; otherwise leave `offset`). The focused card renders with a **focus
highlight** (a distinct border/`theme.rs` style — e.g. an accent border or a left focus
bar), drawn from `focused_comment` at render time. The highlight is the *only* new visual;
the `[editar]`/`[excluir]` tokens and their Ctrl/Cmd+click behaviour are unchanged.

### Guard / fitness function

- **Focus move is pure + bounded (unit):** `Msg::FocusNextComment`/`FocusPrevComment`
  on `update()` move `focused_comment` by one, clamp at both ends (no wraparound past
  the last/first), and emit no Cmd. A thread with zero comments keeps `focused_comment =
  None` and is a no-op.
- **Scroll-into-view (unit):** with a focused card below the viewport, a focus move sets
  `offset` so the card's last line is visible; with it above, `offset` lands on its first
  line; with it already visible, `offset` is unchanged. Pinned against a `comment_spans`
  fixture (the ADR 0031 binary-search test pattern).
- **Highlight render (buffer-derived):** a `TestBackend` render shows the focused card
  carrying the focus style and the others not; moving focus moves the highlight.
- **Actions still click-driven (regression):** `[editar]`/`[excluir]` Ctrl/Cmd+click on
  the focused card still emits `ComposeOpen(Edit)` / `DeleteCommentRequest` (ADR 0036
  path unchanged); no `e`/`x` key acts on the selection.

## Alternatives considered

- **`e`/`x` keys act on the focused comment** (the full cursor model ADR 0036 sketched).
  Rejected for this slice by the user's choice: keep actions on the proven click targets
  so the keyboard surface stays small and there is **one** action path (the affordances),
  not two to keep in sync. A key-action layer can still come later on top of this focus
  state.
- **Resurrect ADR 0010's per-section focus** (independent scroll per comment). Rejected:
  it is the exact model that was reverted (U6c); this decision adds a *cursor over the
  single global scroll*, not independent viewports — strictly less machinery.
- **No focus state; only auto-scroll to "next own comment".** Rejected: it conflates
  navigation with ownership and gives no visible "where am I" cursor; a plain focused
  index is simpler and works for every comment, own or not.

## Consequences

**Positive:** the thread becomes keyboard-navigable with a visible focus cursor and
scroll-into-view, reusing the Tasks card-selection discipline (consistent with the rest
of the app). Edit/delete keep their single, proven click path. The focus index also gives
the footer a precise "own comment focused" mode to key its contextual hint on
([ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md)).

**Accepted trade-offs:** `Screen::Detail` gains a `focused_comment` field and a
`comment_spans` cache (rebuilt with the existing line cache — same invalidation, no new
churn beyond the field-add to every construction site, the mechanical atomic diff class
of [ADR 0031](/adr/0031-tasks-card-layout-cache.md)). Focus and line-scroll are two ways
to move the viewport; the rule (`j`/`k` focus, page/wheel scroll) is documented so they
do not surprise each other. Navigation is keyboard; actions remain mouse — an intentional
asymmetry for this slice.

## Related

- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (deferred this keyboard path; the click affordances kept)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (the single global scroll focus rides on)
- ADR: [/adr/0010-detail-sectioned-panels-focus-scroll.md](/adr/0010-detail-sectioned-panels-focus-scroll.md) (the reverted per-section model this deliberately does NOT revive)
- ADR: [/adr/0031-tasks-card-layout-cache.md](/adr/0031-tasks-card-layout-cache.md), [/adr/0026-task-list-as-cards.md](/adr/0026-task-list-as-cards.md) (the card selection + prefix-sum + first-visible pattern mirrored)
- ADR: [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md) (the footer consumes this focus state)
- BDR: [/bdr/0025-comment-card-navigation-and-contextual-footer.md](/bdr/0025-comment-card-navigation-and-contextual-footer.md)
- Issue: [/issues/0035-comment-card-keyboard-focus.md](/issues/0035-comment-card-keyboard-focus.md)
