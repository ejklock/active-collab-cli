---
type: Issue
title: "V5 — body links render inline as text + visible URL, clickable from the visible region"
description: Replace the indirected '↗ Link N' body-link label with ActiveCollab-style inline 'text [url]' rendering; make the visible link region the click target so activation is robust; retire the separate link list for body links.
status: closed
labels: [tui, render, links, ux]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## V5 — body links as inline URL

Implements [ADR 0020](/adr/0020-body-links-inline-url-native-click.md), observable
behavior pinned by [BDR 0014](/bdr/0014-body-link-inline-url-activation.md). Traces to
research [/research/0001-tui-richtext-links-selection.md](/research/0001-tui-richtext-links-selection.md).

### Problem

Body links render as `↗ Link N` (V4): the URL is hidden, and clicking depends on a
fragile column→label mapping, so clicks "mostly don't work". ActiveCollab's own
plain-text path shows `text [url]` inline — visible, copyable, no indirection.

### Decision

Render `<a href="URL">text</a>` as inline `text [URL]` (link-styled text + dimmed URL;
`[URL]` only when text is empty or equals URL; `mailto:` shows the address). Make the
visible link region the app's click target so the column mapping is direct. Retire the
`↗ Link N` label and the separate URL list for body links. OSC 8 left optional (ratatui
constraint); terminal URL auto-detection covers Cmd/Ctrl+click for free.

### Scope

Included: `src/richtext.rs` (anchor emission → inline `text [url]` styled spans),
`src/render.rs` link-styling + `LinkCollector` region keying, `src/tui/model.rs`
`body_link_cmd_at` (map visible region → open Cmd), and the relevant
`tests/unit/{richtext,render}.rs` + model tests. Excluded: rich-text tag coverage (R4),
selection (V6); asset/"Anexo" affordances (unchanged); CLI path (unchanged).

### Acceptance

- `text != URL` → `text [URL]` (anchor text normal, bracketed URL link-styled);
  `text == URL` or empty → `[URL]` only; `mailto:` → `text [email]` (BDR 0014 Sc. 1–3, 5).
- A click on the visible `[URL]` token (or a raw body URL) emits that URL's open `Cmd`
  via the pure `url_at(line, col)` scanner — no `↗ Link N` index (Sc. 4).
- The `↗ Link N` label and separate list are gone for body links; URL text is on screen
  (selectable/copyable).
- Full suite green; clippy `-D warnings`, fmt, comment-policy clean; complexity within
  budget; click-mapping tests assert observable Cmd (mutation-resistant).

### Outcome (delivered)

Shipped the inline `text [url]` render + the single-line `url_at` scanner; `body_link_cmd_at`
opens the exact clicked URL with no indirection. **Known follow-up (not in V5):** when a
bracketed URL is long enough to **wrap** across rendered lines, the single-line scanner
cannot reassemble the token, so an app-side click on a wrapped fragment is a no-op (the
amended BDR 0014 Sc. 6 defers wrapped-fragment clicks to the terminal's native
Cmd/Ctrl+click). Real-terminal use shows that is not enough — app-side wrapped-token click
resolution is tracked as a follow-up alongside the Anexos label, empty-project, and
title-placement fixes (next detail-polish slice).

### Plan

Single slice (V5). 1) `emit_anchor_label` → inline `text [url]` / `[url]`; strip
`mailto:` for display. 2) Retire `replace_urls_with_labels`/`link_index_at`/`link_label_re`;
add pure `url_at(line, col)` (bracketed INNER first, then raw URL). 3) `link_segments`
tags the URL/email token (never the trailing `]`). 4) `body_link_cmd_at` uses `url_at`;
re-add `mailto:` on bare-email click; remove `body_links`/`DetailContent.links`. 5) Update
fixtures + click-mapping tests.
