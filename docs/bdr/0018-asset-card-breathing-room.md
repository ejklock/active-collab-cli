---
type: BDR
title: "Anexos/Artefatos card has breathing room: a blank line between links and interior padding"
description: In the detail view, the Anexos/Artefatos card renders each link separated by a blank row and inset from the box border by interior padding (top, bottom, and a leading horizontal pad). The card's height grows to fit the spaced content up to a named ceiling, and the rendered height stays consistent with the geometry used for scroll and click hit-testing.
status: Accepted
superseded_by:
supersedes:
tags: [tui, ux, detail, assets, layout]
timestamp: 2026-06-27T00:00:00Z
---

# 0018. Anexos/Artefatos card breathing room

## Context

The detail asset card lists assets as `[n] ↗ <label>` rows
([BDR 0017](/bdr/0017-asset-label-derivation.md) pins the label text). Previously the
rows were flush — no blank line between links, no interior padding — so the list read as
a dense block pressed against the border. Delivered by slice **D1d**
([Issue 0023](/issues/0023-d1d-asset-card-spacing.md)) under
[ADR 0024](/adr/0024-asset-card-breathing-room.md).

## Textual Description

In the **detail view's Anexos/Artefatos card**:

- Consecutive links are separated by **one blank row** — the list reads as distinct,
  spaced entries, not a dense block.
- The card has **interior vertical padding**: one blank row between the top border and the
  first link, and one blank row between the last link and the bottom border.
- Each link row is **inset from the left border** by a leading horizontal pad; the label
  wraps before the right border, never colliding with the box chrome.
- The card **grows in height** to fit the spaced, padded content, up to a **named maximum**
  number of rows; the common multi-link card (e.g. four links) is shown in full, not
  clipped.
- The **rendered card height equals the height** used to lay out the body above it and to
  resolve scroll bounds and asset clicks — the drawn rows and the geometry never disagree.
- An **empty asset list renders no card** (height zero), unchanged.

## Scenarios

**Scenario 1: links are separated by a blank row** — Given a task with two or more
assets, When the detail view renders the Anexos/Artefatos card, Then a blank row appears
between each pair of consecutive links.

**Scenario 2: interior vertical padding** — Given the asset card is rendered, When it is
drawn, Then there is one blank row between the top border and the first link and one blank
row between the last link and the bottom border.

**Scenario 3: leading horizontal pad** — Given an asset row, When it is rendered, Then the
`[n] ↗ label` text is inset from the left border by the horizontal pad, and a wrapped
label's continuation lines stay clear of the right border.

**Scenario 3a: the link style does not bleed into the chrome** — Given an asset row with a
link/underline style, When it is rendered, Then ONLY the visible token (`[n] ↗ label`)
carries the link style; the leading horizontal pad, the trailing fill up to the right
border, and the blank vpad/separator rows are unstyled — the underline never extends under
the padding or the box border. *(Reported: the underline leaked into the sides once the
leading pad inherited the link style.)*

**Scenario 4: common multi-link card is not clipped** — Given a task with four assets
whose labels each fit on one line, When the card renders with spacing and padding, Then all
four links are visible (the height ceiling clears the spaced four-link card).

**Scenario 5: height matches geometry** — Given any asset card, When `asset_panel_render_height`
reports its height, Then that height equals the number of rows the renderer emits
(top pad + per-asset wrapped rows + blank separators + bottom pad + 2 borders), so the body
layout, scroll bound, and click hit-test align with what is drawn.

**Scenario 6: empty list renders nothing** — Given a task with no assets, When the detail
view renders, Then no asset card and no padding rows are drawn (height is zero).

## Test Design

The renderer and the height function are pure/deterministic and asserted via the
TestBackend buffer and direct height calls. Each row names what it proves.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Blank row between links | render (TestBackend) | 1 | a blank row sits between two rendered `[n]` link rows | per-link separator |
| Interior vertical padding | render (TestBackend) | 2 | blank row after top border and before bottom border | top/bottom vpad |
| Leading horizontal pad | render (TestBackend) | 3 | link text starts one col in from the border; wrap stays clear of right border | hpad + padded wrap width |
| Style does not bleed | render (TestBackend) | 3a | leading pad cell and right-border-adjacent cell carry no link/underline style; only token cells styled | style scoped to the visible token |
| Four links not clipped | render (TestBackend) | 4 | all four `[n]` rows present in the buffer | ceiling clears common case |
| Height equals rendered rows | unit | 5 | `asset_panel_render_height` == rows the renderer emits, across asset counts and a wrapped label | renderer/height lock-step |
| Empty list, no card | unit | 6 | height is 0; no card drawn | empty-list invariant |

## Related

- ADR: [/adr/0024-asset-card-breathing-room.md](/adr/0024-asset-card-breathing-room.md)
- BDR: [/bdr/0017-asset-label-derivation.md](/bdr/0017-asset-label-derivation.md) (the link label this card lays out)
- BDR: [/bdr/0012-detail-chrome-responsive-wrap.md](/bdr/0012-detail-chrome-responsive-wrap.md) (artifacts wrap on narrow widths)
- Issue: [/issues/0023-d1d-asset-card-spacing.md](/issues/0023-d1d-asset-card-spacing.md)
