---
type: Issue
title: "Delete-confirm modal presents Sim/Não buttons — relabel [confirmar]/[cancelar] to a yes/no choice"
description: Relabel the delete-confirm modal's two buttons from [confirmar]/[cancelar] to [Sim]/[Não] (English source keys Yes/No, pt-BR values Sim/Não). The button click-target geometry already derives from the label widths, so it reflows automatically; the is_confirm mapping and Enter/Esc keys are unchanged.
status: closed
labels: [tui, comments, modal, confirm, i18n, slice]
blocked_by:
tracker:
timestamp: 2026-06-29T00:00:00Z
---

## Sim/Não delete-confirm modal

Implements [BDR 0028](/bdr/0028-comment-affordance-links-and-yes-no-confirm.md) Scenarios 6–8
under [ADR 0041](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md), amending
the confirm-button labels of [ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md).

### Problem

The delete-confirm modal labels its buttons `[confirmar]`/`[cancelar]`. The user asked for a
plainer confirm: *"os modais para confirmar pode ser apenas opção sim/não"*.

### Decision (from ADR)

`render_confirm_modal` (`src/tui/view.rs`) builds its labels from `t("Yes")` / `t("No")`
(pt-BR `Sim`/`Não`), rendered `[Sim]`/`[Não]`. `register_confirm_button_targets` already
derives the click Rects from the label display widths, so geometry reflows for free; the
`is_confirm` mapping (first button confirms) and the Enter/Esc keys are unchanged.

### Scope

Included:

- `src/tui/view.rs` — `render_confirm_modal`: `confirm_label`/`cancel_label` from
  `t("Yes")`/`t("No")` (was `t("confirmar")`/`t("cancelar")`).
- `locales/pt_BR.json` — `"Yes": "Sim"`, `"No": "Não"`; the now-unused `confirmar`/`cancelar`
  keys removed if no other caller references them.
- Tests: `tests/unit/tui_render.rs` (confirm modal renders `Sim`/`Não`; pt-BR mapping),
  `tests/unit/model.rs` or `tui_render.rs` (Sim target confirms, Não cancels).

Excluded: the affordance link styling (issue 0040); any change to the modal primitive,
geometry, or the Enter/Esc key handling.

### Acceptance

- AC1 — render (`TestBackend`): the delete-confirm modal's hint row shows `[Sim]` and `[Não]`
  (not `confirmar`/`cancelar`). (`verify_by: test`)
- AC2 — the two registered button targets, derived from the rendered modal Rect, map the
  first (`is_confirm: true`) to the `[Sim]` columns and the second (`is_confirm: false`) to
  `[Não]`; a click on Sim dispatches `ConfirmDeleteComment`, on Não `CancelDeleteComment`;
  Enter confirms, Esc cancels (unchanged). (`verify_by: test`)
- AC3 — i18n: the labels resolve through `i18n::t()`; `locales/pt_BR.json` carries
  `Yes`→`Sim`, `No`→`Não`; English source keys are identity. (`verify_by: test`)
- CC — clean code (no superfluous comments / banners / commented-out code) (`verify_by: inspection`).
- CX — complexity budget (cyclomatic ≤ 10 / ≤ 8 new; cognitive ≤ gate) (`verify_by: command`).
- TE — tests assert observable behavior (rendered labels; the Sim/Não → Confirm/Cancel
  mapping) and survive the mutation floor (swapping the two buttons fails) (`verify_by: command`).

### Plan

1. `view.rs`: `render_confirm_modal` labels from `t("Yes")`/`t("No")`.
2. `pt_BR.json`: add `Yes`/`No`; remove orphaned `confirmar`/`cancelar` if unreferenced.
3. Tests: hint-row shows Sim/Não; Sim-confirms / Não-cancels via the resolved targets; pt-BR mapping.

Observable end-to-end: request a comment delete and see a modal asking with `[Sim]` / `[Não]`;
Sim deletes, Não cancels.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
