---
type: ADR
title: The detail-footer decision is one pure footer module — hint selection, transient status, and layout plan live behind a single footer::plan(screen, …) -> FooterPlan, and view.rs becomes draw-only over it
description: The "what should the footer say and how tall is it" decision is spread across five pure helpers in view.rs — hint_for_screen (with the ADR 0039 §5 one-home suppression), detail_hint (the ADR 0038 priority ladder), is_own_comment_focused, detail_status_line (auth_error > copied), and FooterPlan::compute (height + stacking geometry) — and view() wires them together inline, including a Screen::Detail destructuring to derive the status line. The priority invariants are enforced by runtime peeking scattered across functions, not by one interface. The Detail side already has the pure detail_geometry + draw-only screens/detail.rs discipline, and the Tasks side the pure task_layout split; the footer has neither. Extract a pure src/tui/footer.rs owning the whole decision behind one deep footer::plan(screen, last_loaded, copied_feedback, width) -> FooterPlan; view() calls it once and the render_footer draw helpers (Frame) stay in view.rs as the draw-only adapter. Behavior-preserving — the existing hint/status/footer specs are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [tui, footer, view, refactor, locality, depth, ratatui]
timestamp: 2026-07-01T00:00:00Z
---

# 0053. The detail-footer decision is one pure footer module

## Context

The footer decision — *what text the footer shows and how tall it is* — is a **pure state machine
spread across five helpers** in `src/tui/view.rs`:

- `hint_for_screen(screen) -> String` (`view.rs:39`) picks the hint and applies the
  [ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md) §5 one-home suppression
  (when a compose/confirm modal is open, the modal owns its hint, so the footer passes `None`).
- `detail_hint(compose, confirm_delete, focused_comment, comments, current_user_id) -> String`
  (`view.rs:77`) is the [ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md) §1
  priority ladder: composing > confirming-delete > own-comment-focused > browsing default.
- `is_own_comment_focused(…)` (`view.rs:96`) — the focus predicate the ladder branches on.
- `detail_status_line(compose, copied_feedback, auth_error) -> Option<String>` (`view.rs:117`) — the
  transient status row: `auth_error` > `copied_feedback`.
- `FooterPlan::compute(hint, last_loaded, copied_feedback, status_line, width) -> FooterPlan`
  (`view.rs:176`) — the layout plan: height, side-by-side vs stacked, the "Updated at" timestamp.

`view()` (`view.rs:247`) wires them together **inline**: it calls `hint_for_screen`, then
re-destructures `Screen::Detail { overlay, auth_error, .. }` to derive `status_line` via
`detail_status_line`, then calls `FooterPlan::compute`. So the single question "given the screen +
footer state + width, what is the footer?" is answered in three steps scattered between five pure
functions and the draw orchestrator, and the priority invariants are enforced by runtime peeking in
several places rather than behind one interface.

The rest of the TUI already has the right shape: pure `detail_geometry`
([ADR 0045](/adr/0045-detail-viewport-geometry-module.md)) and pure `task_layout`
([ADR 0051](/adr/0051-extract-task-layout-module.md)), each with a draw-only screen adapter. The
footer decision is the remaining pure logic still interleaved with the ratatui draw code.

## Decision

Extract a **pure** `src/tui/footer.rs` that owns the footer decision behind one deep entry, and make
`view.rs` a draw-only adapter over it — mirroring the `detail_geometry` / `task_layout` splits.

1. **One deep entry.** `footer::plan(screen: &Screen, last_loaded: Option<&str>, copied_feedback:
   bool, width: usize) -> FooterPlan` composes the whole decision: it selects the hint
   (`hint_for_screen`), derives the transient status line (moving the `Screen::Detail` destructuring
   *into* the module), and computes the layout (`FooterPlan::compute`). `view()` calls it **once**
   and passes the result to `render_footer`.

2. **The pure helpers move.** `hint_for_screen`, `detail_hint`, `is_own_comment_focused`,
   `detail_status_line`, `wrapped_height`, `format_br_datetime`, and the `FooterPlan` struct +
   `compute` relocate from `view.rs` into `footer.rs` unchanged. They stay `pub(crate)` so the
   existing unit tests reach them; `FooterPlan`'s fields become `pub(crate)` so the draw adapter
   reads them.

3. **view.rs stays draw-only for the footer.** `render_footer`, `render_footer_hint_region`,
   `split_footer_status_row`, `render_footer_side_by_side`, `render_footer_stacked`,
   `render_footer_right_segment` — all of which need a `Frame` — remain in `view.rs`. The compose /
   confirm modal renderers and the selection-highlight helpers stay put (they are not footer logic).

4. **Register the module.** `src/tui/mod.rs` gains `mod footer;`. Test references repoint from
   `crate::tui::view::{hint_for_screen, format_br_datetime, …}` to `crate::tui::footer::…`.

### Guard / fitness function

- **Behavior preserved — invisible to the user.** Same hint per screen, same priority ladder, same
  one-home modal suppression, same status line, same footer height/stacking. All existing hint,
  status-line, and footer-plan specs stay green.
- **One home for the footer decision.** `view()` derives the footer in exactly one call —
  `footer::plan(…)` — with no inline `Screen::Detail` status destructuring left in the draw
  orchestrator. The priority invariants live behind the module interface.
- **The interface is the test surface.** `footer` unit tests assert `plan`, `detail_hint`
  (each ladder rung), `detail_status_line`, and `FooterPlan` height/stacking from primitives — no
  `Frame`, no draw.
- **The deletion test passes.** Deleting `footer` would scatter the hint ladder, the status
  priority, and the layout plan back across `view.rs` and its callers — it concentrates complexity,
  not merely moves it.
- Full suite green; `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, `comment_policy`
  clean; complexity within budget.

## Alternatives considered

- **Keep the helpers in `view.rs`, add only `plan` as a wrapper.** Rejected: the pure decision would
  still be interleaved with the ratatui draw code, so the footer stays asymmetric with
  `detail_geometry` / `task_layout` and the logic is not testable without the draw module around it.
- **Move the `render_footer` draw helpers into `footer` too.** Rejected: that pulls `Frame` into the
  module and breaks the pure/impure split — the constitution's "pure, testable TUI core"
  non-negotiable. `footer` stays pure; drawing stays in `view.rs`.
- **Fold `compose_modal_status` / modal renderers into `footer`.** Rejected: those are compose-modal
  concerns, not footer logic; the module stays focused on the footer hint + status + plan.
- **Leave the split (status quo).** Rejected: the footer decision spread across five functions plus
  inline wiring is exactly the shallow, scattered shape the `detail_geometry` and `task_layout`
  splits already retired elsewhere.

## Consequences

**Positive:** the footer decision has one home and one interface (`footer::plan`), symmetric with
`detail_geometry` and `task_layout`; the priority/suppression invariants are single-homed and
directly testable; `view.rs` becomes a focused draw-only adapter for the footer; a future footer
change (new hint state, new status source) is a one-module edit.

**Accepted trade-offs:** another small pure TUI submodule (`footer`) beside `detail_geometry`,
`task_layout`, and `hit_test` — a deliberate seam; `FooterPlan`'s fields widen to `pub(crate)` so the
draw adapter reads them (acceptable: the struct is a plan the adapter consumes, not an encapsulated
invariant).

## Related

- ADR: [/adr/0038-detail-footer-contextual-hint-and-status-line.md](/adr/0038-detail-footer-contextual-hint-and-status-line.md) (the two-region footer + priority ladder this single-homes)
- ADR: [/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md) (the §5 one-home hint suppression `hint_for_screen` enforces)
- ADR: [/adr/0045-detail-viewport-geometry-module.md](/adr/0045-detail-viewport-geometry-module.md) (the pure-geometry + draw-only split this mirrors)
- ADR: [/adr/0051-extract-task-layout-module.md](/adr/0051-extract-task-layout-module.md) (the sibling pure-module extraction on the Tasks side)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md) (the `src/tui/` module tree `footer` joins)
