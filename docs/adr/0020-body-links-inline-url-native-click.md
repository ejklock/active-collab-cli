---
type: ADR
title: Render body links as inline text + visible URL, clickable via the visible region
description: Replace the indirected "↗ Link N" body-link label (V4) with ActiveCollab-style inline "text [url]" rendering; make the visible link region the app's click target (robust column mapping) and rely on terminal native URL detection for Cmd/Ctrl+click, with OSC 8 as an optional enhancement.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, links, ratatui, ux]
timestamp: 2026-06-26T00:00:00Z
---

# 0020. Body links: inline URL, clickable via the visible region

## Context

Body links in the detail view currently render as an indirected label `↗ Link N`
([V4](/adr/0009-tui-visual-redesign-vibrant-dashboard.md), carried by the richtext
mapper). The URL is not shown; the operator must correlate "Link N" with a list, and
clicking depends on a column→label-index mapping (`body_link_cmd_at`) that walks
wrapped spans. The operator reports link clicks "mostly don't work".

Research ([/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md))
established two things:

1. ActiveCollab's own plain-text renderer (`toPlainText`) shows links as
   **`text [url]`** — the URL inline, in brackets — never an indirected reference.
   The URL is always visible and copyable.
2. When the *real URL* is on screen, modern terminals make it clickable via
   Cmd/Ctrl+click through their own URL detection, with **no OSC 8** required.
   Explicit OSC 8 emission is hard inside ratatui's cell-buffer renderer, so it is an
   enhancement, not a dependency.

The root cause of "clicks mostly don't work" is the **indirection**: the click target
is a separate `Link N` token, not where the URL visibly is.

Force: **usability of a read view** — operators open and copy links. Presentation
concern; the pure TEA core stays pure (click handling already flows through `Cmd`).

## Decision

Render body links **inline** as `text [url]` and make the **visible link region the
click target**. Delivered as slice **V5**.

### 1. Inline rendering

The richtext mapper emits, for `<a href="URL">text</a>`:

- the anchor `text` styled as a link (link color + underline);
- followed by ` [URL]` with the URL dimmed.
- When `text` is empty or equals the URL, render just `[URL]` (mirrors
  `toPlainText`'s "url only when text == url"). `mailto:` URLs render the address.

The `↗ Link N` label and its separate URL list are **retired for body links**. The
`LinkCollector` still records each URL keyed to its on-screen region so the app can
resolve a click to a URL (the collector stays; only the rendered label changes).

### 2. Click target is the visible region

The click map (`body_link_cmd_at`) maps a click anywhere on the rendered link region
(the `text [url]` run) to that link's open `Cmd`. Because the region is contiguous
and visible, the column mapping is direct — eliminating the fragile correlation that
made the old label hard to hit. Asset/"Anexo N" affordances are unchanged (separate
panel).

### 3. Terminal-native + optional OSC 8

Because the real URL string is rendered, terminals with URL detection make it
clickable via Cmd/Ctrl+click for free. Explicit OSC 8 hyperlink emission is an
**optional enhancement** behind the rendering layer; it is not required for the
behavior and is not a dependency (ratatui's diff renderer does not model OSC 8
ranges). The inline URL guarantees the link is visible and copyable regardless.

## Alternatives considered

- **Keep `↗ Link N`, fix only the click mapping.** Rejected: keeps the indirection
  the operator dislikes and the URL hidden; does not make links copyable inline.
- **OSC 8 only (no visible URL).** Rejected: ratatui cannot cleanly emit OSC 8 mid
  buffer, and with mouse capture on most terminals forward clicks to the app anyway;
  the URL would be invisible and uncopyable on unsupported terminals.
- **Footnote-style URL list at the bottom.** Rejected: reintroduces indirection; the
  operator must scroll to correlate.

## Consequences

**Positive:** the URL is always visible, copyable, and clickable where it appears;
clicking is robust because the target is the visible region; behavior matches the
product's own plain-text convention. Degrades gracefully on any terminal.

**Accepted trade-offs:** inline URLs make long links consume body width (mitigated by
style-aware wrapping — the `[url]` wraps like any text). The `↗ Link N` affordance is
gone for body links; anyone who relied on the label uses the visible URL instead.
Explicit OSC 8 is deferred.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) (V4 link label, retired for body links)
- ADR: [/adr/0015-richtext-html-subset-styled-segments.md](/adr/0015-richtext-html-subset-styled-segments.md) (link row amended)
- ADR: [/adr/0019-richtext-full-activecollab-tag-coverage.md](/adr/0019-richtext-full-activecollab-tag-coverage.md)
- BDR: [/bdr/0014-body-link-inline-url-activation.md](/bdr/0014-body-link-inline-url-activation.md)
- Research: [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md)
- Issue: [/issues/0020-v5-body-links-inline-url.md](/issues/0020-v5-body-links-inline-url.md)
- Architecture: [/architecture.md](/architecture.md)
