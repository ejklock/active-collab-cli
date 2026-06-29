---
type: Issue
title: "Comment edit/delete affordances render as colored underlined links — [editar] cyan, [excluir] destructive red, emitted structurally"
description: Style the [editar]/[excluir] affordance tokens on an own comment's card header as links — soft-cyan underlined for edit, destructive-red underlined for delete — with the StyleRun emitted by build_comment_card over the edit_span/delete_span it already computes (structural, ADR 0032), not by text-pattern detection. Own-comment-only and click-to-act are unchanged.
status: closed
labels: [tui, comments, affordance, link, style, theme, slice]
blocked_by:
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## Comment affordance colored links

Implements [BDR 0028](/bdr/0028-comment-affordance-links-and-yes-no-confirm.md) Scenarios 1–5
under [ADR 0041](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md), applying
the structural-styling discipline of [ADR 0032](/adr/0032-asset-row-link-style-structural.md).

### Problem

The `[editar]`/`[excluir]` tokens on an own comment's card header carry a click affordance
([ADR 0036](/adr/0036-permission-aware-comment-targeting.md)) but **no style** — they read as
plain timestamp text, so the user cannot tell they are actionable.

### Decision (from ADR)

`build_comment_card` (`src/render.rs`) emits a `StyleRun` over `edit_span` and `delete_span`
(the spans it already computes for the hit-test), carrying two new `RichStyle` variants
mapped in `emphasis_style` to two new theme styles: `edit_affordance_style()` =
SOFT_CYAN + UNDERLINED, `delete_affordance_style()` = DUE_RED + UNDERLINED. Structural
emission (same coordinates as the affordance), never a text-pattern scan.

A `StyleRun` carries a `richtext::RichStyle` (not a raw `ratatui::Style`); the render path
maps each variant to a concrete style in `emphasis_style`, exactly as `RichStyle::Link` →
`theme::link_style()`. So the affordances are carried as two new variants —
`RichStyle::EditAffordance` / `RichStyle::DeleteAffordance` — keeping the run pipeline and
the structural-styling discipline intact.

### Scope

Included:

- `src/richtext.rs` — add `RichStyle::EditAffordance` and `RichStyle::DeleteAffordance`
  variants (the structural style carriers); no parser change.
- `src/tui/screens/detail.rs` — map the two new variants in `emphasis_style` to
  `theme::edit_affordance_style()` / `theme::delete_affordance_style()` (mirrors the
  `RichStyle::Link` arm).
- `src/tui/theme.rs` — `edit_affordance_style()` (SOFT_CYAN + UNDERLINED) and
  `delete_affordance_style()` (DUE_RED + UNDERLINED), reusing the existing color constants.
- `src/render.rs` — in `build_comment_card`, push a `StyleRun` onto the header line's
  `line_styles[0]` over `edit_span` (edit variant) and `delete_span` (delete variant), at the
  same coordinates recorded for the affordance; the runs travel through the existing
  indent/chrome offsetting unchanged.
- Tests: `tests/unit/tui_render.rs` (buffer-derived fg + UNDERLINED over the tokens),
  `tests/unit/render.rs` (style-run span == affordance span; no style on others' comments).

Excluded: the Sim/Não confirm modal (issue 0041); any change to who-can-act (`is_own` gate)
or how-to-act (click path, ADR 0037).

### Acceptance

- AC1 — render (`TestBackend`): on an own comment, the `[editar]` token cells carry SOFT_CYAN
  foreground + the UNDERLINED modifier. (`verify_by: test`)
- AC2 — render (`TestBackend`): on an own comment, the `[excluir]` token cells carry DUE_RED
  foreground + the UNDERLINED modifier. (`verify_by: test`)
- AC3 — the emitted style run's `(start, len)` for each token equals the affordance span the
  hit-test uses (single-source, structural); on a comment authored by another user no
  affordance style is emitted and no affordance is registered. (`verify_by: test`)
- AC4 — regression: a Ctrl/Cmd+click on the styled `[editar]`/`[excluir]` still emits
  `ComposeOpen(Edit)` / the delete request; focus/keys never act on the comment. (`verify_by: test`)
- CC — clean code (no superfluous comments / banners / commented-out code; well-named
  styles) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior (cell fg + modifier; span equality) and survive the
  mutation floor on changed lines (a swapped color or dropped underline fails) (`verify_by: command`).

### Plan

1. `richtext.rs`: add `RichStyle::EditAffordance` and `RichStyle::DeleteAffordance` variants.
2. `theme.rs`: add `edit_affordance_style()` and `delete_affordance_style()`.
3. `detail.rs`: map the two new variants in `emphasis_style` to the two new theme styles.
4. `render.rs`: in `build_comment_card`, after computing `edit_span`/`delete_span`, push the
   matching `StyleRun` (carrying the new variant) onto `line_styles[0]` for each present span.
5. Tests: own-comment edit-cyan + delete-red buffer assertions; span-equality + others-have-no-style;
   click-still-acts regression.

Observable end-to-end: open a task with your own comment and see `[editar]` as a cyan link
and `[excluir]` as a red link on the card header; clicking them still works.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
