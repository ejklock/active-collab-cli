---
type: ADR
title: Detail hit-targets are emitted structurally into one affordance registry; click-time re-derivation is retired
description: ADR 0032 made link STYLE structural (a RichStyle::Link run emitted by the layout) but left the HIT-TARGET re-derived at click time — body_link_cmd_at re-scans rendered lines via resolve_wrapped_url (+ inverse-wrap helpers, which carry the obs-35 latent over-join bug) and asset_panel_cmd_at re-calls section_lines()/asset_index_for_section_row(). Finish what ADR 0028/0032 started: extend the existing DetailContent.affordances registry (already the source for comment Edit/Delete) to carry OpenUrl/OpenAsset hit-targets emitted at layout time, turn the three click hit-tests into positional lookups, and delete resolve_wrapped_url + its inverse-wrap helpers.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, assets, link, hit-test, affordance, refactor, locality, ratatui]
timestamp: 2026-06-29T00:00:00Z
---

# 0043. Detail hit-targets are emitted structurally; click-time re-derivation is retired

## Context

The Detail screen already has the right shape for one kind of click. `build_detail_content`
(`src/render.rs:1831`) produces a `DetailContent` whose `affordances: Vec<LocalAffordance>`
(`render.rs:79`) records typed, layout-emitted hit-spans — `{ line_idx, col_start, col_end,
kind }` — and the model resolves a click by a single positional scan (`affordance_at`,
`src/tui/model.rs:1271`). Today `AffordanceKind` (`render.rs:67`) carries only
`Edit(i64)` / `Delete(i64)`, populated for comment cards
([ADR 0036](/adr/0036-permission-aware-comment-targeting.md)). For those targets, the
layout is the single source of truth and the click path is a pure lookup. This is the
pattern [ADR 0028](/adr/0028-asset-panel-single-layout-source.md) (one layout source for
the asset panel) and [ADR 0032](/adr/0032-asset-row-link-style-structural.md) (link
**style** is structural — emitted by the layout, never re-inferred from rendered text)
established.

The other two detail hit-targets were **not** brought into that registry, so the model
re-derives them at click time:

- **Body links** ([ADR 0020](/adr/0020-body-links-inline-url-native-click.md)):
  `body_link_cmd_at` (`model.rs:1310`) calls `render::resolve_wrapped_url` (`render.rs:230`),
  which **re-scans the rendered `lines`** and runs inverse-wrap math
  (`logical_position_in_wrap_group`, `url_at_in_wrap_group`) to reconstruct the complete
  URL from whichever wrapped fragment was clicked — re-computing what the layout knew before
  it wrapped the token. That inverse-wrap helper carries a **known latent bug** (obs 35:
  wrap-group continuation detection over-joins a word-boundary line exactly `content_width`
  wide).
- **Asset rows** ([ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md)):
  `asset_panel_cmd_at` (`model.rs:1384`) **re-calls** `asset_panel::section_lines(...).len()`
  and `asset_index_for_section_row(...)` at click time to rebuild the row→asset map the
  layout already computed when it spliced those rows into `lines`. The two callers are kept
  in agreement only by a shared-formula comment (`inline_content_width`).

ADR 0032 itself flagged this asymmetry — it made the asset row's *style* structural but
explicitly left the *click hit-test* as a separate re-derivation ("the click hit-test … is
itself structurally intact"). The result is two leaky seams: the renderer's wrap and layout
knowledge leaks into the model's click handlers, kept in sync by convention rather than by a
single emitted artifact.

## Decision

Make `DetailContent.affordances` the **single source for every detail hit-target**. Resolve
each target's payload **once, at layout time**, emit it on the affordance span, and reduce
the click path to a positional lookup. Then delete the click-time re-derivation.

1. **Generalize `AffordanceKind`** (`render.rs`): add `OpenUrl(String)` and
   `OpenAsset(String)` alongside `Edit(i64)` / `Delete(i64)`. The `String` is the final,
   openable target — a normalized URL or `mailto:` for `OpenUrl`, `asset.url` for
   `OpenAsset` — resolved at emit time. (`Edit`/`Delete` keep a comment id because the
   action needs the id, not a pre-resolved value.)

2. **Emit `OpenAsset` at layout time** (slice 0045): where `build_detail_content` splices
   `asset_panel::section_lines`, push one `LocalAffordance { kind: OpenAsset(asset.url) }`
   per asset content row — every wrapped continuation line of an asset registers a span with
   the **same** url, using the layout's own row→idx knowledge (`PanelRow::Asset { idx }` /
   `asset_index_for_section_row`). `asset_panel_cmd_at` becomes a lookup; the click-time
   re-call of `section_lines`/`asset_index_for_section_row` is removed.

3. **Emit `OpenUrl` at layout time** (slice 0046): in the body-line build path, where the
   complete URL token is known **before** it is wrapped, push one
   `LocalAffordance { kind: OpenUrl(normalized) }` per wrapped fragment of the token — and
   only when the token is openable. The `normalize_link_url` / `is_openable_url` / mailto
   validation moves to emit time (a non-openable `[note]` registers no span). `body_link_cmd_at`
   becomes a lookup.

4. **Reduce the click path to a lookup and retire the re-derivation.** The modifier policy
   is preserved exactly: a **plain** click resolves only `Edit`/`Delete`; a **Ctrl/Cmd**
   click resolves only `OpenUrl`/`OpenAsset`
   ([BDR 0014](/bdr/0014-body-link-inline-url-activation.md) Sc.8 — plain click is reserved
   for text selection). Once nothing calls them at click time, **delete `resolve_wrapped_url`
   and its inverse-wrap helpers** (`logical_position_in_wrap_group`, `url_at_in_wrap_group`)
   — which also retires the obs-35 latent over-join bug.

### Guard / fitness function

- **Behavior preserved (the deepening is invisible to the user).** The existing buffer-derived
  click specs stay green unchanged: Ctrl/Cmd+click on a wrapped body-link fragment opens the
  **complete** URL (BDR 0014 Sc.7); Ctrl/Cmd+click on any asset row, including a wrapped
  continuation line, opens `asset.url` (BDR 0022 Sc.1); a plain click on `[editar]`/`[excluir]`
  still edits/deletes (ADR 0036).
- **Structural emission proven.** New tests assert the emitted `affordances` list **carries**
  the `OpenUrl`/`OpenAsset` payload over the right spans, and that a token wrapped across N
  lines registers a hit on **every** fragment (derived from the real `build_detail_content`
  output, not a re-implementation).
- **Anti-regression on the retired seam.** `resolve_wrapped_url` and the two inverse-wrap
  helpers are gone (their deletion is the guard); a body-link wrap test covering the
  obs-35 over-join case passes. The asset click no longer re-calls `section_lines`.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean;
  complexity within budget; mutation floor (Node report-only — Reviewer is the backstop):
  swapping the emitted payload or dropping a per-fragment span must fail a click test.

## Alternatives considered

- **Keep click-time re-derivation (status quo).** Rejected: two leaky seams kept in sync by
  a shared-formula comment and inverse-wrap reconstruction; carries the obs-35 latent bug;
  and it is an inconsistent half-measure — ADR 0032 made *style* structural while the
  *hit-target* stays re-derived from the same rows.
- **Carry an asset index (not the url) on `OpenAsset`.** Rejected: carrying the resolved url
  keeps dispatch trivial and removes the final `assets[idx]` lookup; the layout already holds
  `asset.url` at emit time. (The asset-only slice boundary — 0045 before 0046 — is still
  honored; this is about the *payload shape*, not the slicing.)
- **Emit only the asset hit-target and keep `resolve_wrapped_url` for body links.** Rejected
  as the end state: it leaves the most leak-prone path (text re-scan + inverse-wrap) and its
  latent bug in place. It remains a valid *slice boundary*: 0045 ships the asset half, 0046
  ships the body half and the deletion.

## Consequences

**Positive:** one home for "what is clickable at this cell, and what does it do" — the
emitted `affordances` registry; the renderer's wrap/layout knowledge stops leaking into the
model's click handlers; `resolve_wrapped_url` plus two inverse-wrap helpers (and the obs-35
latent bug) are **deleted** — the deletion test passes, since removing them concentrates the
knowledge into the layout rather than moving it elsewhere. The three hit-test functions
shrink to lookups, improving locality in the 2108-line `model.rs`.

**Accepted trade-offs:** `AffordanceKind` now mixes id-carrying (`Edit`/`Delete`) and
value-carrying (`OpenUrl`/`OpenAsset`) variants — acceptable; `dispatch` matches on the
variant. A long wrapped token registers N affordance entries (one per fragment) instead of
being reconstructed on demand — a few small structs per detail, negligible, and the price of
single-source emission. The emit site still uses the `inline_content_width` formula, but now
as the **only** caller (the click-time second caller is gone), so the formula's shared-home
comment is retained without a drift risk. Collapsing the three lookups into one
`tui/hit_test` module (the locality follow-up in the architecture review) is left for a later
change.

## Related

- ADR: [/adr/0028-asset-panel-single-layout-source.md](/adr/0028-asset-panel-single-layout-source.md) (one layout source — the principle this extends)
- ADR: [/adr/0032-asset-row-link-style-structural.md](/adr/0032-asset-row-link-style-structural.md) (link **style** is structural — this does the same for the **hit-target**)
- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md) (assets inline in the scroll)
- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (body-link text-detection path being retired)
- ADR: [/adr/0025-asset-activation-ctrl-cmd-click.md](/adr/0025-asset-activation-ctrl-cmd-click.md) (Ctrl/Cmd+click to open)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (the Edit/Delete affordance registry this generalizes)
- BDR: [/bdr/0014-body-link-inline-url-activation.md](/bdr/0014-body-link-inline-url-activation.md) (Sc.7 complete-URL on wrapped click; Sc.8 plain-click reserved)
- BDR: [/bdr/0022-assets-inline-scrollable-detail-content.md](/bdr/0022-assets-inline-scrollable-detail-content.md) (asset click behavior preserved)
- Issues: [/issues/0045-asset-hit-target-structural.md](/issues/0045-asset-hit-target-structural.md), [/issues/0046-body-link-hit-target-structural.md](/issues/0046-body-link-hit-target-structural.md)
