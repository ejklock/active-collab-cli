---
type: Research
title: "Rich-text rendering, link interaction, and mouse text-selection for the detail view — evidence and options"
description: How ActiveCollab itself renders comment/description rich text (its canonical allowed-tag whitelist and its plain-text converter), and an evaluation of Rust/ratatui options for richer rendering, clickable links, and mouse-selection feedback in the TUI detail view.
tags: [tui, richtext, links, selection, ratatui, activecollab, research]
visibility: private
timestamp: 2026-06-26T00:00:00Z
---

# 0001. Rich-text, links, and selection in the detail view — evidence

## Question

Three operator-reported defects on the TUI detail view, all rooted in the
text/interaction layer fighting the terminal:

1. Running text in descriptions/comments still renders poorly ("ainda tá zoada")
   — structure is lost for markup the current mapper does not cover.
2. Link clicks "mostly don't work".
3. There is no visual feedback while selecting text with the mouse.

The brief: find out how ActiveCollab itself renders this, and whether an
off-the-shelf Rust/ratatui package handles rich text + links + selection with
decent visual quality, functionality, and performance.

## Source of truth — how ActiveCollab renders rich text

ActiveCollab's body pipeline (its own source, vendored under `base-digital`) is:

```
TinyMCE (editor) → IBodyImplementation → BodyProcessor + HtmlCleaner (HTMLPurifier)
                 → tag processors (links, mentions, inline images, legacy)
                 → 3 display modes: screen / email / plain-text
```

Two findings decide our design.

### A. The canonical allowed-tag whitelist (what formatting can exist)

`HtmlCleanerInterface::DEFAULT_ALLOWED_TAGS` is the authoritative universe of tags
a stored body may contain. It is the exact target set our mapper must cover:

`p, br, div, span, a, img, h1, h2, h3, b, strong, i, em, u, strike, del,
blockquote, ul, ol, li, pre, table, thead, tbody, tfoot, tr, td, th`.

Anything else is purified away server-side, so the TUI never needs to handle it.

### B. The plain-text converter (the direct terminal analog)

`Angie\HTML::toPlainText()` is ActiveCollab's own "render this body as text" path
(used for email/notifications). It is the closest existing analog to a terminal
renderer, and its tag→text mapping is the proven contract we mirror:

| Tag | ActiveCollab plain-text mapping |
|---|---|
| `<p>` | two newlines |
| `<h1>`–`<h6>` | blank line + UPPERCASE text + blank line |
| `<b>` / `<strong>` | UPPERCASE text |
| `<i>` / `<em>` | `_text_` |
| `<ul>` / `<ol>` / `<table>` | two newlines around |
| `<br>` / `<tr>` | single newline |
| `<hr>` | a `-----` rule line |
| `<td>` | text + newline |
| `<th>` | UPPERCASE text + newline |
| `<li>` | `* text` per item |
| `<a href>` | `text [url]` (or just the url when text == url); `mailto:` → `text [email]` |
| `<blockquote>` | each line prefixed `> ` |

The load-bearing insight for the link defect: **ActiveCollab shows the URL inline,
in brackets, next to its text.** It does not use an indirected "Link N" reference.
The URL is always visible and copyable. Our current `↗ Link N` indirection (V4) is
exactly the indirection ActiveCollab avoids — and the source of the fragile
click-mapping the operator hit.

A TUI improves on `toPlainText` only by replacing its monochrome conventions with
real styling: UPPERCASE → bold, `_text_` → italic, `> ` → a dim quote bar — same
structure, better visual.

## Package evaluation (the "investigate on the web" ask)

### Rich-text rendering: HTML → ratatui styled spans

- **`tui-markdown`** — renders *Markdown*, not HTML. Our source is HTML. A
  lossy HTML→Markdown pre-pass would be required, plus we would still post-process
  its output into `Style` spans. Rejected (impedance mismatch).
- **`ansi-to-tui`** — parses *ANSI escape sequences* into `ratatui::text::Text`.
  Our source is HTML, not ANSI. Not applicable.
- **`html2text`** — targets plain-text reflow, not ratatui `Style` segments; we
  would post-process its output anyway (the same conclusion ADR 0015 reached).
- **Conclusion:** there is no well-fitting off-the-shelf HTML→ratatui crate. The
  existing in-house `richtext` mapper (ADR 0015) is the right vehicle; the work is
  to **extend its tag coverage to the ActiveCollab whitelist** (tables, `strike`/
  `del`, `u`, `pre`), mirroring `toPlainText`'s proven mapping with real styling.
  This keeps full control of terminal styling, avoids a heavy mismatched dependency,
  and preserves the pure, headless-testable design.

### Clickable links: OSC 8 vs visible URL vs app-handled click

- **OSC 8 hyperlinks** (terminal hyperlink escape sequence) make a link region
  natively clickable and selectable. Caveat: ratatui's cell-buffer/diff renderer
  does not model OSC 8 escape ranges, so emitting them cleanly inside a ratatui
  frame is non-trivial; and while application mouse capture (SGR mouse mode) is on,
  most terminals forward clicks to the app rather than following the link.
- **Visible URL (terminal auto-linkify)** — when the *real URL string* is rendered
  on screen, modern terminals (iTerm2, Terminal.app, kitty, WezTerm, Ghostty) make
  it clickable via Cmd/Ctrl+click through their own URL detection, **no OSC 8
  needed**. This is the reliable, portable path and it is exactly what showing
  `text [url]` buys us.
- **App-handled click** — the app already intercepts clicks (mouse capture on); the
  current bug is that the click target is a *separate* `↗ Link N` token whose
  column→index mapping is fragile. Rendering the URL inline makes the **visible link
  region itself** the contiguous click target, which the app can map directly.
- **Conclusion:** render `text [url]` inline; make the visible region the app's
  click target (robust mapping); rely on terminal native URL detection for
  Cmd/Ctrl+click; treat explicit OSC 8 emission as an optional enhancement, not a
  dependency, given the ratatui constraint. The inline URL guarantees the link is
  always visible and copyable regardless of terminal support.

### Mouse-selection feedback: terminal-native vs app-managed

- **Terminal-native selection (current, V3/ADR 0012)** — the `s` key disables mouse
  capture so the terminal does its own click-drag selection. Fundamental limit:
  **the app cannot draw any feedback**, because once capture is off the app never
  sees the drag — the terminal owns the highlight. This is precisely the operator's
  complaint ("não tenho feedback").
- **App-managed selection** — keep mouse capture on; the app tracks press → drag →
  release, draws its own reverse-video highlight over the selected cells, and copies
  the selected text to the system clipboard. Gives real, drawn feedback and works in
  any terminal regardless of native-selection support.
- **`arboard`** — OS-independent Rust clipboard crate (text + image), high source
  reputation, the standard choice for cross-platform clipboard in Rust TUIs. Fits
  the app-managed model: on selection release, `Clipboard::set_text(selected)`.
- **Conclusion:** move to app-managed selection with a drawn highlight and `arboard`
  copy. This replaces the V3 capture-toggle (ADR 0012) and delivers the feedback the
  operator asked for.

## Recommendation (carried into decisions)

1. **Rich-text completeness** → extend the `richtext` mapper to the full ActiveCollab
   whitelist (tables, strike/del, underline, pre/code-block), mirroring `toPlainText`
   with real styling. → ADR 0019 / BDR 0013 / issue 0019 (R4).
2. **Links** → render `text [url]` inline; visible region is the click target; rely
   on terminal URL auto-detection; OSC 8 optional. Retire the `↗ Link N` indirection
   for body links. → ADR 0020 / BDR 0014 / issue 0020 (V5).
3. **Selection** → app-managed selection with drawn highlight + `arboard` copy,
   superseding the V3 capture toggle. → ADR 0021 / BDR 0015 / issue 0021 (V6),
   superseding ADR 0012 + BDR 0006.

## References

ACTIVECOLLAB. *HtmlCleanerInterface — DEFAULT_ALLOWED_TAGS*. Vendored source,
ActiveCollab 7.1.141. Available at:
`base-digital-active-collab/activecollab/7.1.141/Foundation/Text/HtmlCleaner/HtmlCleanerInterface.php`
(lines 17–96). Accessed on: 2026-06-26.

ACTIVECOLLAB. *Angie\HTML::toPlainText() — HTML-to-plain-text converter*. Vendored
source, ActiveCollab 7.1.141. Available at:
`base-digital-active-collab/activecollab/7.1.141/angie/src/Angie/HTML.php`
(lines 74–200). Accessed on: 2026-06-26.

ACTIVECOLLAB. *BodyProcessor and tag processors (links, mentions, inline images,
legacy)*. Vendored source, ActiveCollab 7.1.141. Available at:
`base-digital-active-collab/activecollab/7.1.141/Foundation/Text/BodyProcessor/`.
Accessed on: 2026-06-26.

RATATUI. *Text and styling — Text, Line, Span, Style*. Ratatui 0.30.0
documentation. Available at: https://docs.rs/ratatui/0.30.0/. Accessed on:
2026-06-26.

ARBOARD. *Arboard — OS-independent clipboard library for Rust*. crates.io / docs.rs.
Available at: https://docs.rs/arboard/. Accessed on: 2026-06-26.

GNOME / Egmont Koblinger. *Hyperlinks (a.k.a. HTML-like anchors) in terminal
emulators — OSC 8 specification*. Available at:
https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda. Accessed on:
2026-06-26.
