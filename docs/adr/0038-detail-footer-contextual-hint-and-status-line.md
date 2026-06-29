---
type: ADR
title: The detail footer becomes two regions — a context-aware instruction line plus a thin transient status line
description: hint_for_screen returns one hardcoded hint per screen and the compose status (Submitting / error) renders inline in the scrollable content, away from the eye. Split the footer into a contextual instruction line that changes by mode (browsing vs composing vs own-comment-focused) and a thin status line below it that surfaces transient state (Submitting, write error, clipboard-copied) in a fixed place. The instruction line keys off the Detail mode (compose / confirm_delete / focused own comment); the status line is a single derived string, cleared when idle.
status: Accepted
supersedes:
superseded_by:
tags: [tui, footer, render, comments, status, i18n]
timestamp: 2026-06-28T00:00:00Z
---

# 0038. A contextual footer instruction line plus a thin status line

## Context

The footer is a single line whose text comes from **`hint_for_screen(screen)`**
(`src/tui/view.rs`) — a **hardcoded per-screen** string (Detail returns
`"↑/↓ scroll  r refresh  Esc/b back  q quit"`). It does **not** know the Detail *mode*:
whether the user is composing, confirming a delete, or focused on their own comment. The
relevant action hints therefore either do not appear or live elsewhere.

Meanwhile **transient status** has no fixed home. The compose status
(`ComposeStatus::Submitting` → `"Sending…"`, `Error` → `"Failed to post comment"`) is
rendered **inline** by `compose_block_lines` *inside the scrollable content*
(`src/render.rs`), so it can be scrolled off screen and competes with the thread. The
clipboard-copied feedback rides the footer's `right_text` slot. There is no single,
always-visible place that answers "what is happening right now".

`FooterPlan`/`render_footer` (`view.rs`) already compute a footer **height** and can
render a **stacked** multi-line footer — the structure to host a second line exists; what
is missing is (a) a *contextual* instruction string and (b) a *dedicated status* row.

The force is **usability (feedback + discoverability)**, a UX force with named
instruments (the hint test + the status render test) — a local view-layer change, not an
architecture one.

## Decision

Make the footer **two stacked regions**: a **contextual instruction line** on top and a
**thin status line** below it.

### 1. Contextual instruction line (keys off the Detail mode)

Replace the Detail branch of `hint_for_screen` with a **mode-aware** hint derived from
the Detail state, in priority order:

- **Composing** (`compose.is_some()`): `"Ctrl+S enviar · Esc cancelar"` (the compose
  controls — moved out of the inline `compose_block_lines` hint into the footer).
- **Confirming a delete** (`confirm_delete.is_some()`): `"Enter/clique confirmar · Esc
  cancelar"`.
- **An own comment is focused** (`focused_comment` maps to a `created_by_id ==
  user_id` comment, [ADR 0037](/adr/0037-comment-card-keyboard-focus.md)):
  `"j/k mover · Ctrl+clique editar/excluir · c novo"`.
- **Browsing the thread** (default): `"j/k mover · c comentar · r atualizar · Esc/b
  voltar · q sair"`.

Other screens keep their existing single hint unchanged — only Detail becomes contextual.
All strings go through `i18n::t()` (the established identity-for-English pattern; the
pt-BR values live in `locales/pt_BR.json`).

### 2. A thin status line (one derived string, idle = empty)

The footer gains a **second row** that shows a single **transient status string**,
derived (in priority order) from:

- `compose.status == Submitting` → `"Enviando…"`; `Error(msg)` → the localized failure.
- the **clipboard-copied** feedback (the existing `copied_feedback` flag) →
  `"Copiado ✓"`.
- otherwise **empty** (the row collapses / renders blank — no permanent chrome cost when
  nothing is happening).

The status line is **derived, not stored** (no new persisted state beyond what already
exists): it is a pure function of `compose.status` + `copied_feedback`. The inline
compose status row is **removed** from `compose_block_lines` — the status now has exactly
one home (the footer), satisfying "one home per fact".

### 3. Footer sizing

`FooterPlan::compute` accounts for the extra row: the footer region height becomes the
wrapped instruction line height **plus one** when a status string is present (and the
too-small guard `MIN_HEIGHT` is re-checked against the taller footer). When idle the
status row is blank, so the footer does not permanently steal a content row beyond the
instruction line.

### Guard / fitness function

- **Contextual hint (unit, buffer-derived):** for each Detail mode (browsing /
  composing / confirming / own-comment-focused) the rendered footer instruction line
  shows that mode's hint; switching modes switches the hint.
- **Status line (buffer-derived):** `Submitting` renders `"Enviando…"` on the status
  row; `Error` renders the localized error; `copied_feedback` renders `"Copiado ✓"`;
  idle renders a blank status row. Asserted from the real `TestBackend` buffer.
- **One home for status (regression):** `compose_block_lines` no longer emits the status
  text into the scrollable content (the inline status assertion is removed/inverted).
- **Sizing:** a render at `MIN_HEIGHT` with a status string still passes the too-small
  guard / does not clip the content region.

## Alternatives considered

- **Keep the single hardcoded hint; leave compose status inline.** Rejected: the user
  asked for the instructions in the footer and a dedicated status line; inline status
  scrolls away and has no fixed place (the motivating problem).
- **A persistent status bar that always reserves a row.** Rejected: it spends a content
  row even when idle on a TUI where vertical space is scarce; the derived, collapses-when-
  empty row gives the feedback without the permanent cost.
- **Toast/transient overlay for status.** Rejected: more machinery (timers, redraw
  scheduling) than a derived footer row, and it floats over content; the footer row is
  always in the same place and needs no timer (it clears when the underlying state does).

## Consequences

**Positive:** the footer answers both "what can I do here" (mode-aware) and "what is
happening" (status) in one fixed place; compose status gets a single home and stops
scrolling away; discoverability of the comment actions improves. Reuses the existing
`FooterPlan` stacked-footer structure.

**Accepted trade-offs:** the footer can occupy two rows (one more than before) when a
status is present — a small, bounded content-area cost, re-checked against the too-small
guard. `hint_for_screen` gains Detail-mode branching (its complexity stays within budget
via a small `detail_hint(...)` helper). The contextual hint depends on the
`focused_comment` state from [ADR 0037](/adr/0037-comment-card-keyboard-focus.md), so the
"own comment focused" hint variant lands with (or after) that focus state.

## Related

- ADR: [/adr/0037-comment-card-keyboard-focus.md](/adr/0037-comment-card-keyboard-focus.md) (provides the focused-comment mode the hint keys off)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md) (the compose status text moved here)
- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (the footer/chrome dynamic-height precedent)
- ADR: [/adr/0021-app-managed-text-selection-clipboard.md](/adr/0021-app-managed-text-selection-clipboard.md) (the clipboard-copied feedback surfaced on the status line)
- BDR: [/bdr/0025-comment-card-navigation-and-contextual-footer.md](/bdr/0025-comment-card-navigation-and-contextual-footer.md)
- Issue: [/issues/0036-detail-contextual-footer-status-line.md](/issues/0036-detail-contextual-footer-status-line.md)
