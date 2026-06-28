---
type: BDR
title: "Assets are part of the scrollable detail content: every attachment is reachable by scrolling and opens on Ctrl/Cmd+click at any scroll position"
description: In the detail view the task's attachments render at the end of the single globally-scrollable content (a localized Anexos/Artefatos header, the labeled link rows with a blank row between consecutive links, and an italic Ctrl/Cmd+click footnote), not in a fixed bottom panel. No attachment is ever clipped — scrolling reaches them all. A Ctrl/Cmd/Super+click on a link row opens that asset regardless of scroll position; a plain click does not. There is no separate bordered card and no height ceiling.
status: Accepted
superseded_by:
supersedes: [0018, 0021]
tags: [tui, ux, detail, assets, scroll, layout]
timestamp: 2026-06-27T00:00:00Z
---

# 0022. Assets are part of the scrollable detail content

## Context

The detail view previously rendered the task's attachments in a **fixed bottom
panel** with a named height ceiling — a multi-link card with breathing room
([BDR 0018](/bdr/0018-asset-card-breathing-room.md)) and an italic in-card hint
([BDR 0021](/bdr/0021-asset-open-hint-in-card.md)). The ceiling **silently clipped**
attachments beyond it, with no scroll and no indicator. Delivered by the
assets-inline slice ([Issue 0028](/issues/0028-assets-inline-scrollable-content.md))
under [ADR 0029](/adr/0029-assets-inline-in-scrollable-detail-content.md), which
folds the assets into the single global scroll and retires the fixed panel.

This record **supersedes** BDR 0018 and BDR 0021: it carries forward their visible
behavior (per-link spacing, the labeled rows, the italic Ctrl/Cmd hint, click to
open) into the inline-scrollable context and **retires** their fixed-panel
geometry scenarios (panel height matching a fixed-chunk layout; clicks mapping
against a bottom-pinned panel).

## Textual Description

In the **detail view**, after the description and comments, the **same scrollable
content** continues with the attachments:

- A localized **`Anexos`/`Artefatos` header line** introduces the section.
- Each attachment is a **`[n] ↗ label` row** (the label per
  [BDR 0017](/bdr/0017-asset-label-derivation.md)); the link token is visibly
  link-styled.
- **Consecutive links are separated by one blank row** (the carried-forward
  breathing room).
- The **last line of the section is an italic, dimmed `Ctrl/Cmd+click` footnote**.
- **Every attachment is reachable by scrolling** — the content scroll extends over
  the asset rows; nothing is clipped or hidden, however many attachments there are.
- A **Ctrl/Cmd/Super+click on a link row opens that attachment** (`Cmd::OpenAsset`
  with the asset's URL) **regardless of the current scroll position**; a click on
  the header, a blank/separator row, or the footnote opens nothing.
- A **plain (unmodified) click opens nothing** (reserved for V6 text selection).
- There is **no separate bordered card and no fixed bottom panel**; an **empty
  attachment list renders no section** at all.

## Scenarios

**Scenario 1: attachments are part of the scrollable content** — Given a task with
attachments, When the detail view renders, Then the `Anexos`/`Artefatos` header and
the `[n] ↗ label` rows appear at the end of the same scrollable content as the body
and comments, with no separate bordered panel.

**Scenario 2: every attachment is reachable by scrolling** — Given a task with more
attachments than fit on screen at once, When the user scrolls to the bottom, Then
every attachment row is shown — none is clipped or hidden (the old height ceiling is
gone).

**Scenario 3: links are separated by a blank row** — Given two or more attachments,
When the section renders, Then a blank row appears between each pair of consecutive
link rows.

**Scenario 4: italic Ctrl/Cmd footnote is the last section line** — Given a task
with attachments, When the section renders, Then its last line is the
`Ctrl/Cmd+click` hint in italic/dim style.

**Scenario 5: Ctrl/Cmd+click opens the asset at any scroll position** — Given the
content is scrolled so an attachment row is visible at viewport row R, When the user
Ctrl/Cmd/Super+clicks row R, Then `Cmd::OpenAsset` is emitted with that attachment's
URL — the scroll offset is accounted for, so the row maps to the correct
attachment.

**Scenario 6: non-link rows and plain clicks open nothing** — Given the attachment
section, When the user clicks the header row, a blank/separator row, or the footnote
(with or without a modifier), or Ctrl/Cmd+clicks no attachment row, Then no
`OpenAsset` Cmd is emitted; and a plain unmodified click on a link row also emits
nothing (reserved for selection).

**Scenario 7: empty list renders no section** — Given a task with no attachments,
When the detail view renders, Then no header, no rows, and no footnote are drawn,
and the content scroll bound reflects only the body and comments.

## Test Design

The content composition and the click hit-test are pure/deterministic and asserted
via the `Vec<PanelRow>` composition, the section line→asset-index map, direct
`Cmd` assertions, and the TestBackend buffer. Geometry expectations are derived
from the **real** rendered buffer, never assumed.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Asset section appended to content | unit + render | 1 | the asset header + `[n]` rows are in the scrollable `lines`, after comments; no separate panel chunk drawn | assets are scrollable content |
| All attachments reachable | unit | 2 | with N attachments exceeding a screen, max scroll offset exposes the last asset row; no ceiling clips it | no silent clipping |
| Blank row between links | render (TestBackend) | 3 | a blank row sits between two rendered `[n]` rows | breathing room carried forward |
| Italic footnote last | render (TestBackend) | 4 | the section's last line carries italic/dim style and the hint text | hint carried forward inline |
| Ctrl/Cmd+click maps with offset | unit | 5 | at scroll offset O, a click at row R emits OpenAsset for the asset at content line `O + (R − text_top)` | scroll-aware asset click |
| Non-link / plain click no-op | unit | 6 | header/blank/footnote rows and unmodified clicks emit no Cmd | click scoping preserved |
| Empty list, no section | unit | 7 | no asset lines appended; scroll bound excludes a panel term | empty-list invariant |

## Related

- ADR: [/adr/0029-assets-inline-in-scrollable-detail-content.md](/adr/0029-assets-inline-in-scrollable-detail-content.md)
- BDR: [/bdr/0018-asset-card-breathing-room.md](/bdr/0018-asset-card-breathing-room.md) (superseded — breathing room carried forward inline)
- BDR: [/bdr/0021-asset-open-hint-in-card.md](/bdr/0021-asset-open-hint-in-card.md) (superseded — the italic hint, now an inline footnote)
- BDR: [/bdr/0014-body-link-inline-url-activation.md](/bdr/0014-body-link-inline-url-activation.md) (the scroll-aware click model assets now share)
- Issue: [/issues/0028-assets-inline-scrollable-content.md](/issues/0028-assets-inline-scrollable-content.md)
