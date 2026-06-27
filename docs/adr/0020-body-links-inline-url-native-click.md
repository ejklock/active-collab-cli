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

- the anchor `text` as normal body text (any inline emphasis preserved);
- followed by ` [URL]` — the bracketed URL is the link-styled token (link color),
  the single visible/clickable/copyable affordance (mirrors `toPlainText`, which
  shows the bare URL in brackets).
- When `text` is empty or equals the URL, render just `[URL]` (mirrors
  `toPlainText`'s "url only when text == url"). `mailto:` URLs render the bare
  address inside the brackets (`text [a@b.com]`); the open `Cmd` re-adds the
  `mailto:` scheme.

The `↗ Link N` label and its separate URL list are **retired for body links**, and
with them the indirected `link_index_at` → `body_links[N-1]` correlation that made
clicks miss.

### 2. Click target is the visible URL token

The click map (`body_link_cmd_at`) resolves the URL **from the visible text at the
click column** — a `url_at(line, col)` scan returns the `[url]` bracket content (or a
raw URL printed in the body) under the click, and the app opens it directly. No
index, no separate URL list: the open target is whatever URL is visibly clicked
(a bracketed e-mail opens via `mailto:`). Because the URL is on screen, the mapping
is direct and robust — eliminating the fragile correlation. A URL long enough to wrap
stays fully visible/copyable.

**Amended (D1c, issue 0022):** the V5 scanner resolved a click only against a single
rendered line, so a click on a **wrapped** URL fragment (where the `[url]` token is split
across lines) was a no-op and the terminal-native fallback proved insufficient in real
use. The click now maps to the **pre-wrap logical line** before `url_at` runs, so a click
on **any** fragment of a wrapped URL resolves the **whole** token. Asset/"Anexo N"
affordances are unchanged (separate panel).

### 2a. Activation requires a Ctrl/Cmd modifier (amended D1c, issue 0022)

V5 opened the URL on a **plain** (unmodified) left click. Real-terminal use showed two
problems: (a) a plain click felt "strange" and fired accidentally while the operator was
only trying to position the cursor or read, and (b) it collides head-on with the
app-managed text selection planned for V6 ([ADR 0021](/adr/0021-app-managed-text-selection-clipboard.md)),
where a plain click+drag must mean *select*, not *navigate*.

Activation is therefore **gated on a keyboard modifier**: the open `Cmd` is emitted only
when the click carries **Ctrl or Cmd/Super** (`crossterm` `MouseEvent.modifiers` contains
`CONTROL` or `SUPER`). A plain, unmodified click does **not** open the URL — it is reserved
for text selection (V6) and a future caret. This is exactly the terminal-native convention
the operator named: terminals open a detected URL on **Cmd/Ctrl+click**, never on a bare
click.

The two activation paths coexist and are both correct:

- **Mouse capture on (the TUI's normal state):** the app receives the modifier+click as a
  `MouseEvent` and emits the open `Cmd` itself.
- **Terminal intercepts the modifier+click** (some terminals, e.g. iTerm2, open a detected
  URL on Cmd+click before forwarding to the app): the terminal opens the **visible** URL.
  Either way the link opens, because the real URL string is on screen (§1). The app never
  needs to fight the terminal for the event.

Selection (plain click/drag) and activation (modifier+click) are thus disjoint inputs — the
modifier is the discriminator, mirroring every modern terminal.

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
- **Activate on a plain (unmodified) click** (the V5 behavior). Rejected after real use
  (D1c): fires accidentally during read/cursor movement and collides with the V6 plain
  click+drag selection. Superseded by the Ctrl/Cmd-modifier gate (§2a), which matches the
  terminal-native convention the operator asked for.

## Consequences

**Positive:** the URL is always visible, copyable, and clickable where it appears;
clicking is robust because the target is the visible region; behavior matches the
product's own plain-text convention. Degrades gracefully on any terminal.

**Accepted trade-offs:** inline URLs make long links consume body width (mitigated by
style-aware wrapping — the `[url]` wraps like any text). The `↗ Link N` affordance is
gone for body links; anyone who relied on the label uses the visible URL instead.
Explicit OSC 8 is deferred. **Activation now requires Ctrl/Cmd (§2a):** a discoverability
cost (the modifier is not self-evident) traded for no accidental navigation and clean
coexistence with V6 selection — the footer hint advertises it, and it is the convention
operators already know from their terminal.

## Related

- ADR: [/adr/0009-tui-visual-redesign-vibrant-dashboard.md](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) (V4 link label, retired for body links)
- ADR: [/adr/0015-richtext-html-subset-styled-segments.md](/adr/0015-richtext-html-subset-styled-segments.md) (link row amended)
- ADR: [/adr/0019-richtext-full-activecollab-tag-coverage.md](/adr/0019-richtext-full-activecollab-tag-coverage.md)
- BDR: [/bdr/0014-body-link-inline-url-activation.md](/bdr/0014-body-link-inline-url-activation.md)
- Research: [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md)
- Issue: [/issues/0020-v5-body-links-inline-url.md](/issues/0020-v5-body-links-inline-url.md)
- Architecture: [/architecture.md](/architecture.md)
