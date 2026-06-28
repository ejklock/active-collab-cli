---
type: ADR
title: Rich-line wrap threads span style positionally (per character), retiring the substring style lookup
description: wrap_rich flattened a RichLine to plain text and re-derived each wrapped word's style by scanning the source spans for the first one whose text contains the word. That substring lookup is positionally ambiguous — a word repeated in the same line with different emphasis always takes the first occurrence's style, and a short word that is a substring of a larger styled token inherits that token's style. Replace it by carrying each character's source style through the wrap, so every wrapped fragment's style is correct by construction and style_of_word_in_rich_line is deleted.
status: Accepted
supersedes:
superseded_by:
tags: [tui, render, richtext, wrap, correctness]
timestamp: 2026-06-28T00:00:00Z
---

# 0030. Rich-line wrap threads span style positionally (per character)

## Context

The detail view renders comment/description HTML as a `RichLine` — a `Vec<RichSpan>`
where each span carries `text` + a `RichStyle` (Plain, Bold, Italic, Code, Strike,
Underline) ([ADR 0015](/adr/0015-richtext-html-subset-styled-segments.md),
[ADR 0019](/adr/0019-richtext-full-activecollab-tag-coverage.md)). Before drawing,
long lines are word-wrapped to the viewport width by `wrap_rich`
(`src/render.rs`), which must preserve each span's emphasis across the breaks
(the style-aware-wrap behavior, [BDR 0012](/bdr/0012-detail-chrome-responsive-wrap.md)
Sc.7 / [BDR 0013](/bdr/0013-richtext-full-tag-coverage.md) Sc.7).

`wrap_rich` does this by **flattening** the `RichLine` to one plain `String`
(`line.iter().map(|s| s.text).collect()`), splitting that plain text into
whitespace-delimited words, and **re-deriving** each word's style with
`style_of_word_in_rich_line(word, rich_source)`:

```rust
for span in rich_source {
    if span.text.contains(word) {   // first span whose text CONTAINS the word
        return span.style;
    }
}
RichStyle::Plain
```

This lookup is **positionally ambiguous** because `RichSpan` carries no position
(only `text` + `style`), so once the line is flattened the per-word origin is gone
and the function guesses it back by substring search. Two real defects follow:

1. **Repeated word, different emphasis.** Source `format the **format** call`
   (span A `"format the "` Plain, span B `"format"` Bold, span C `" call"` Plain).
   Both the first and the *bold* second `format` resolve to span A (the first
   `contains` match) → the bold occurrence renders Plain. The emphasis lands on the
   wrong occurrence, or is dropped.
2. **Substring of a larger token.** Word `cat` against a line containing a styled
   span `category` matches `category.contains("cat")` → the short plain word
   inherits the larger token's style.

Both are silent rendering-correctness bugs in the read view (observation `obs 33`).
The `wrap_rich_single_line` docstring even *claims* it scans "proportionally to byte
position", but the code tracks no position at all — the intent was always positional;
only the implementation was a heuristic.

Force: **rendering correctness** — the emphasis a user sees must match the source
HTML, independent of repeated words or substrings. This is a code-level force (a
lossy transformation), so the fix is a local deepening, not an architecture change.

## Decision

Carry each character's **source style through the wrap** instead of flattening and
re-matching. The wrap becomes positional by construction, and the substring lookup
is deleted.

### 1. Expand the source line to styled characters

A `RichLine` expands to an ordered `Vec<(char, RichStyle)>`: each span contributes
its characters, each tagged with that span's style. This is the one place the
per-character origin is preserved; nothing downstream re-guesses it. Hard line
breaks (`\n`) are split on this styled-char stream, exactly where the old code split
the plain string.

### 2. Greedy-wrap over styled words, emitting styled spans

The greedy width algorithm is unchanged in shape (same display-width measurement,
same single Plain space between words on a line, same hard-split of an over-width
word) — but it now walks **styled characters**. A "word" is a maximal run of
non-whitespace styled chars; its display width is measured from those chars. When a
word is appended to the current wrapped `RichLine`, its chars are coalesced into
spans by `push_rich_span` (which already merges adjacent same-style runs), so a word
that spans an emphasis boundary (`<b>fo</b>o`) keeps **both** styles as adjacent
spans rather than collapsing to one. The inter-word space is Plain, matching today.

### 3. Delete `style_of_word_in_rich_line`

With style threaded positionally, the substring lookup has no caller and is removed
(the **deletion test**: removing it concentrates correctness — the heuristic and its
ambiguity are gone, not relocated). `style_for_word` and the flatten-then-rematch
shape go with it.

### Guard

`wrap_rich`'s public signature (`(&RichLine, width) -> Vec<RichLine>`) and the
empty/whitespace/over-width edge behavior are unchanged — the existing single-span
wrap tests (`strike_style_survives_wrap`, `underline_style_survives_wrap`) keep
passing. New tests pin the two disambiguation cases against the real wrap output
(repeated word with differing emphasis; substring word not inheriting a larger
token's style), per [BDR 0023](/bdr/0023-richtext-wrap-positional-style.md). Full
suite green; `clippy -D warnings`, `fmt`, comment-policy clean; per-function
complexity within budget (cyclomatic ≤ 10 / ≤ 8 new); tests mutation-resistant.

## Alternatives considered

- **Disambiguate by occurrence index.** Keep the flatten, but count how many times
  the word appeared so far and pick the Nth matching span. Rejected: it still relies
  on word-level matching, still mishandles a word split across two styles, and adds
  bookkeeping to preserve information the styled-char expansion never loses. More
  code for a partial fix.
- **Add a byte/char offset field to `RichSpan` and binary-search the offset.**
  Makes position first-class on the type, but it widens a shared data structure used
  across the whole richtext pipeline (parser, mapper, link path, style-run path) for
  a concern local to the wrap, and every producer must now populate and maintain
  correct offsets. Rejected as over-reach: the wrap can derive position locally from
  span order without changing the type. (If a *second* consumer ever needs span
  positions, revisit — one need is a local helper, two is a field.)
- **Leave it; the ambiguity is rare.** Rejected: repeated words and short
  substrings are ordinary in prose, the failure is silent (wrong emphasis, no
  crash), and the cost of the correct version is contained to the wrap.

## Consequences

**Positive:** wrapped emphasis is correct by construction — independent of repeated
words or substrings. The lossy flatten-then-rematch step and its heuristic
(`style_of_word_in_rich_line`) are deleted, so the wrap is a deeper module: one
positional transformation instead of a flatten plus a guess. A word crossing an
emphasis boundary now keeps both styles (previously it took one span's style for the
whole word).

**Accepted trade-offs:** the wrap allocates a per-character `(char, RichStyle)`
vector per source line. Detail bodies are small (comments/descriptions), so this is
negligible; the correctness is worth it. The internal helper shape changes
(`wrap_rich_single_line` consumes styled chars; `append_rich_word` /
`hard_split_rich_word` carry per-char style) — contained to `src/render.rs`, with
the public `wrap_rich` contract preserved.

## Related

- ADR: [/adr/0015-richtext-html-subset-styled-segments.md](/adr/0015-richtext-html-subset-styled-segments.md) (the RichSpan/RichStyle representation this wraps)
- ADR: [/adr/0019-richtext-full-activecollab-tag-coverage.md](/adr/0019-richtext-full-activecollab-tag-coverage.md) (the full styled-tag set the wrap must preserve)
- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (the wrap context — style-aware word wrap in the detail chrome)
- BDR: [/bdr/0023-richtext-wrap-positional-style.md](/bdr/0023-richtext-wrap-positional-style.md)
- BDR: [/bdr/0013-richtext-full-tag-coverage.md](/bdr/0013-richtext-full-tag-coverage.md) (Sc.7 style-survives-wrap, extended here to repeated/substring words)
- Issue: [/issues/0029-richtext-wrap-positional-style.md](/issues/0029-richtext-wrap-positional-style.md)
- Architecture: [/architecture.md](/architecture.md)
