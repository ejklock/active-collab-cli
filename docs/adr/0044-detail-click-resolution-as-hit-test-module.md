---
type: ADR
title: Detail click resolution is one deep tui/hit_test module — a single coordinate translation and one registry lookup behind a typed target
description: ADR 0043 made DetailContent.affordances the single registry but left the click path as five scattered functions in model.rs (affordance_at, body_link_cmd_at, asset_panel_cmd_at, click_to_line_col, dispatch_affordance_click) that each re-derive the viewport→line_idx translation (two inline it, one is even missing the lines.len() guard), re-check the modifier gate, and repeat the lookup+unwrap. Collapse them into a deep src/tui/hit_test.rs whose one pure entry resolve_detail_click maps a click to a typed DetailClickTarget via a single coordinate translation and one positional registry lookup; the model maps the target to the TEA effect.
status: Accepted
supersedes:
superseded_by:
tags: [tui, hit-test, affordance, refactor, locality, depth, ratatui]
timestamp: 2026-06-29T00:00:00Z
---

# 0044. Detail click resolution is one deep `tui/hit_test` module

## Context

[ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md) made
`DetailContent.affordances` the single source for every detail hit-target and reduced each
click hit-test to a positional lookup. It explicitly left one follow-up open: *"Collapsing
the three lookups into one `tui/hit_test` module (the locality follow-up in the architecture
review) is left for a later change."* This is that change.

The resolution logic is correct but **shallow and scattered** across five functions in the
2100-line `src/tui/model.rs`:

- `affordance_at` (`model.rs:1275`) — resolves `Edit`/`Delete`; **inlines** the viewport→
  `line_idx` translation (`text_top = 2`, `content_text_height`, the bounds check,
  `line_idx = offset + (row - text_top)`) and **omits** the `line_idx >= lines.len()` guard.
- `click_to_line_col` (`model.rs:1315`) — the *same* translation, extracted, but used by
  **only** `body_link_cmd_at`.
- `body_link_cmd_at` (`model.rs:1348`) — resolves `OpenUrl`; re-checks the modifier gate,
  calls `click_to_line_col`, repeats the `find(... col in span ... kind filter)` lookup and
  the `match &aff.kind { OpenUrl(url) => …, _ => None }` unwrap.
- `asset_panel_cmd_at` (`model.rs:1402`) — resolves `OpenAsset`; **inlines the translation a
  third time**, re-checks the modifier gate, repeats the lookup and unwrap.
- `dispatch_affordance_click` (`model.rs:1104`) — maps a resolved `AffordanceKind` to an
  effect, with a dead `OpenAsset`/`OpenUrl` arm because those flow through the other two
  functions instead.

So the coordinate translation has **three copies** (one missing a guard), the Ctrl/Cmd gate
is checked in `handle_click_detail` **and** re-checked inside two of the callees, and the
"find the affordance, unwrap its payload" shape is written three times. Answering "what does
a detail click at this cell resolve to?" means bouncing across five functions. The interface
(five private fns with overlapping responsibilities) is nearly as complex as the work; the
deletion test passes — folding them into one module **concentrates** the logic rather than
moving it.

## Decision

Extract a deep module **`src/tui/hit_test.rs`** whose single public entry resolves a detail
click to a typed target, behind one coordinate translation and one registry lookup.

1. **Typed target.** Introduce `pub enum DetailClickTarget { CommentEdit(i64),
   CommentDelete(i64), OpenUrl(String), OpenAsset(String) }`. It names *what a click
   resolved to*, decoupled from `render::AffordanceKind` (the layout artifact) and from
   `Cmd` (the effect).

2. **One pure entry.** `pub fn resolve_detail_click(model: &Model, column: u16, row: u16,
   has_modifier: bool) -> Option<DetailClickTarget>`. It performs the viewport→`line_idx`
   translation **once** (the former `click_to_line_col`, now the module's single private
   helper, with the `lines.len()` guard applied for **every** kind), then one positional
   scan over `affordances` returning the typed target. It is **pure** — no `Model` mutation,
   no `Cmd` construction, no I/O — so the interface is the test surface: feed an affordances
   list + a click coordinate, assert the target.

3. **One lookup rule.** The scan matches `a.line_idx == line_idx` and, for the col-bounded
   kinds (`Edit`/`Delete`/`OpenUrl`), `col ∈ [col_start, col_end)`. `OpenAsset` is a
   **row target** — the entire asset row is clickable ([ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md)) —
   expressed as a single, named, documented predicate (`AffordanceKind::is_row_target`),
   **not** a hidden re-derivation. This makes the asset row's whole-row semantics explicit in
   one place (resolving the "asset col bounds are silently ignored" observation from the
   ADR 0043 review) instead of an unstated convention in a separate function.

4. **The model maps target → effect.** `handle_click_detail` calls `resolve_detail_click`
   once; on `Some(target)` it maps to the TEA effect (`CommentEdit`/`CommentDelete` →
   `handle_edit/delete_comment_request`; `OpenUrl`/`OpenAsset` → `Cmd::OpenAsset { instance,
   url }`, reading `instance` from the screen). The **state transition stays in the model**;
   `hit_test` stays pure. `affordance_at`, `body_link_cmd_at`, `asset_panel_cmd_at`,
   `click_to_line_col`, and `dispatch_affordance_click` are **deleted**.

### Guard / fitness function

- **Behavior preserved — the deepening is invisible to the user.** The exact modifier policy
  that `handle_click_detail` implements today is kept: every affordance (comment Edit/Delete,
  body-link OpenUrl, asset OpenAsset) activates on **Ctrl/Cmd+click**; a plain click is
  reserved for text selection. All existing buffer-derived click specs stay green unchanged.
- **One coordinate translation.** No `text_top`/`content_text_height`/`line_idx` arithmetic
  remains in `model.rs`'s click path; it lives once in `hit_test`. The previously-missing
  `lines.len()` guard now applies to comment Edit/Delete too (a latent off-by-one on a click
  below the last line is closed).
- **Deletion is the guard.** `affordance_at`, `body_link_cmd_at`, `asset_panel_cmd_at`,
  `click_to_line_col`, `dispatch_affordance_click` no longer exist; resolution is reachable
  only through `hit_test::resolve_detail_click`.
- **The interface is the test surface.** `hit_test` unit tests feed an `affordances` list +
  a click coordinate and assert the `DetailClickTarget`, with no terminal and no `Model`
  effect — including the row-target asset rule and the out-of-range guard.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean;
  complexity within budget; mutation floor (Reviewer backstop): swapping a target arm or
  dropping the guard must fail a test.

## Alternatives considered

- **Keep the five functions, extract only the shared `click_to_line_col` (status quo+).**
  Rejected: still three lookups, two re-checked modifier gates, and a dead dispatch arm; the
  "what does a click resolve to" answer still spans several functions — a shallow change.
- **Emit `OpenAsset` as a full-row span (`col_start = 0, col_end = width`) so the lookup is
  uniformly col-bounded with no row-target predicate.** A clean uniformity, but it touches
  the `render.rs` emit site and its tests, widening the slice past its file budget for a
  marginal gain over a single named predicate. Deferred as an optional later tidy; the col
  bounds stay on the asset span (carrying the link location) and the whole-row rule is made
  explicit in `hit_test` instead.
- **Let `hit_test` return the `Cmd` / mutate the `Model` directly.** Rejected: it would pull
  TEA state-transition and effect construction into the resolver, destroying its purity and
  the "interface is the test surface" property. The resolver answers *what was clicked*; the
  model owns *what happens*.

## Consequences

**Positive:** one home for "what does a detail click resolve to" — `hit_test::
resolve_detail_click`. The viewport→`line_idx` translation drops from three copies to one
(and gains the missing guard); the Ctrl/Cmd gate is checked once; the asset whole-row rule
becomes an explicit, tested predicate instead of an unstated convention in a third function.
`model.rs` sheds five functions for one call site, improving locality. The resolver is pure
and directly unit-testable through its interface. A typed `DetailClickTarget` decouples the
layout artifact (`AffordanceKind`) from the resolution result from the effect (`Cmd`).

**Accepted trade-offs:** a new `DetailClickTarget` enum sits beside `AffordanceKind` — a
small, deliberate indirection that keeps the resolver independent of both the layout and the
effect. `OpenAsset` keeps col bounds the lookup does not use for matching (it matches the
whole row); they are retained as the link location and the whole-row behavior is documented
on `is_row_target` rather than emitted away (the full-row-span alternative remains open).

## Related

- ADR: [/adr/0043-detail-hit-targets-emitted-structurally.md](/adr/0043-detail-hit-targets-emitted-structurally.md) (the registry this consumes; foreshadowed this module)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md) (the layered `src/tui/` module tree `hit_test` joins)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (asset rows are whole-row click targets)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (the Edit/Delete affordances)
- BDR: [/bdr/0014-body-link-inline-url-activation.md](/bdr/0014-body-link-inline-url-activation.md) (Sc.7/Sc.8 — the click semantics preserved)
- Issue: [/issues/0047-hit-test-module-extraction.md](/issues/0047-hit-test-module-extraction.md)
