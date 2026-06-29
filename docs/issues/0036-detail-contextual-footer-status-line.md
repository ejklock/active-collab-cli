---
type: Issue
title: "Detail contextual footer + status line — mode-aware instruction line, a thin transient status row, compose status moved out of the inline block"
description: Slice 2 of comment-card navigation. Replace the Detail branch of hint_for_screen with a mode-aware hint (browsing / composing / confirming-delete / own-comment-focused, keyed off the Detail state incl. focused_comment from issue 0035); add a thin footer status row that surfaces a single derived transient string (Enviando…, localized write error, Copiado ✓) and is blank when idle; FooterPlan::compute sizes the extra row and re-checks the too-small guard; remove the inline compose status from compose_block_lines so the status has one home.
status: open
labels: [tui, footer, status, comments, i18n, slice]
blocked_by: [0035]
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Detail contextual footer + thin status line

Slice 2 of comment-card navigation. Implements
[BDR 0025](/bdr/0025-comment-card-navigation-and-contextual-footer.md) Scenarios 7–9
under [ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md). Consumes
the `focused_comment` state from [issue 0035](/issues/0035-comment-card-keyboard-focus.md)
for the own-comment-focused hint variant.

### Problem

The footer shows one hardcoded per-screen hint and does not reflect the Detail mode, so
the comment actions are undiscoverable; transient status (compose Submitting/error,
clipboard-copied) has no fixed home — the compose status renders inline and scrolls away.

### Decision (from ADR)

- **Contextual instruction line (ADR 0038 §1):** the Detail branch of `hint_for_screen`
  becomes mode-aware (a small `detail_hint(...)` helper) in priority order — composing →
  confirming-delete → own-comment-focused → browsing. Other screens unchanged. All strings
  through `i18n::t()` (pt-BR values in `locales/pt_BR.json`).
- **Thin status line (ADR 0038 §2):** a second footer row shows one derived transient
  string — `compose.status == Submitting` → `Enviando…`, `Error(msg)` → the localized
  failure, `copied_feedback` → `Copiado ✓`, else blank. Derived, not stored.
- **Sizing (ADR 0038 §3):** `FooterPlan::compute` adds a row when a status string is
  present and re-checks the `MIN_HEIGHT` too-small guard; idle status row is blank.
- **One home for status:** remove the inline status from `compose_block_lines`.

### Scope

Included:

- `src/tui/view.rs` — mode-aware Detail hint (`detail_hint`/`hint_for_screen`); the status
  row in `FooterPlan::compute` + `render_footer`; sizing + too-small re-check.
- `src/render.rs` — remove the inline compose status from `compose_block_lines`.
- `src/i18n/*` (and `locales/pt_BR.json`) — the contextual hint strings + status strings
  (`Enviando…`, the write error, `Copiado ✓`).
- Tests: `tests/unit/tui_render.rs` (footer hint per mode, status row, sizing, inline-status
  removal regression).

Excluded: the focus cursor itself (issue 0035); changes to other screens' hints.

### Acceptance

- AC1 — render (`TestBackend`): the footer instruction line shows the correct hint for
  each Detail mode — browsing, composing, confirming-delete, own-comment-focused; switching
  modes switches the text.
- AC2 — render (`TestBackend`): the status row shows `Enviando…` for `Submitting`, the
  localized error for `Error`, `Copiado ✓` for `copied_feedback`, and is blank when idle.
  **Superseded in part by [ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md):**
  compose `Submitting`/`Error` now render inside the compose modal overlay; the footer status
  row surfaces `Copiado ✓` and is otherwise blank. The "one home for status" constraint holds.
- AC3 — render (`TestBackend`): the inline compose block no longer contains the status text
  (one home — regression on the ADR 0034 inline status).
- AC4 — `FooterPlan::compute`: the footer height accounts for the status row when present;
  a render at `MIN_HEIGHT` with a status string still passes the too-small guard and does
  not clip the content region.
- AC5 — i18n: every footer/status string resolves through `i18n::t()`; pt-BR values present
  in `locales/pt_BR.json`; English source keys unchanged (identity).
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  functions over explanatory comments) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior and survive the mutation floor on changed lines
  (`verify_by: command`).

### Plan

1. `detail_hint(...)` helper + mode-aware `hint_for_screen` Detail branch (keyed off
   compose / confirm_delete / focused-own-comment / default).
2. Derived status string + the status row in `render_footer`; `FooterPlan::compute` sizing
   + too-small re-check.
3. Remove the inline compose status from `compose_block_lines`.
4. i18n strings (en source keys + pt-BR values).
5. Tests: hint-per-mode, status row (each source + idle), sizing at `MIN_HEIGHT`,
   inline-status-removed regression.

Observable end-to-end: open a task, move between browse / compose / confirm-delete /
focused-own-comment and watch the footer instruction line change; submit a comment and
watch `Enviando…` then the result appear on the status row.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
