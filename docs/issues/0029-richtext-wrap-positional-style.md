---
type: Issue
title: "Rich-line wrap threads style positionally — fix repeated/substring word emphasis, delete style_of_word_in_rich_line"
description: Replace wrap_rich's flatten-then-rematch styling (style_of_word_in_rich_line returns the first span whose text contains the word) with per-character style threading, so repeated words and substring words keep their own emphasis. Delete the substring lookup. Preserve the public wrap_rich contract and single-span wrap behavior.
status: closed
labels: [tui, render, richtext, wrap, correctness, bug]
blocked_by:
tracker:
timestamp: 2026-06-28T00:00:00Z
---

## Rich-line wrap threads style positionally

Implements [ADR 0030](/adr/0030-richtext-wrap-positional-style.md), observable
behavior pinned by [BDR 0023](/bdr/0023-richtext-wrap-positional-style.md). Surfaced
as observation `obs 33`: `style_of_word_in_rich_line` re-derives each wrapped word's
emphasis by text-contains, which is positionally ambiguous.

### Problem

`wrap_rich` (`src/render.rs`) flattens a `RichLine` to plain text, splits into
words, and re-derives each word's style with `style_of_word_in_rich_line(word,
rich_source)` → the **first** span whose `text.contains(word)`. Because `RichSpan`
carries no position, this guesses the origin back and gets it wrong when:

1. a word repeats in the line with different emphasis (the second occurrence takes
   the first's style), or
2. a short word is a substring of a larger styled token (it inherits that token's
   style).

Both are silent rendering-correctness defects in the detail read view.

### Decision

Expand each source `RichLine` to ordered `(char, RichStyle)` pairs and run the
existing greedy width-wrap over those styled characters, coalescing each wrapped
word's chars into spans (`push_rich_span` already merges adjacent same-style runs).
Style is then correct by construction; `style_of_word_in_rich_line` and the
`style_for_word` closure are deleted. See ADR 0030 for the decision and rejected
alternatives (occurrence-index counter; byte offsets on `RichSpan`).

### Scope

Included:

- `src/render.rs` — rewrite `wrap_rich` / `wrap_rich_single_line` to thread per-char
  style; carry per-char style through `append_rich_word` / `hard_split_rich_word`;
  delete `style_of_word_in_rich_line` and the `style_for_word` closure. Keep the
  public `wrap_rich` signature and the empty/whitespace/over-width edges.
- `tests/unit/richtext.rs` (or the unit file that exercises `wrap_rich`) — add the
  BDR 0023 disambiguation tests (repeated word, substring word, cross-boundary word,
  style-after-break), keep the existing single-span survive-wrap tests green.

Excluded: the `RichSpan` type (no new field — ADR 0030 rejected widening it); the
richtext parser/mapper; the StyleRun path (`rich_line_to_style_runs` already consumes
the wrapped spans positionally and is correct); the CLI plain path.

### Acceptance

- A word repeated in one source line with different emphasis keeps **each
  occurrence's** style in the wrapped output (BDR 0023 Sc.1).
- A short word that is a substring of a larger styled token does **not** inherit that
  token's style (Sc.2).
- A single styled span longer than the width keeps its style on every wrapped
  fragment — BDR 0013 Sc.7 regression guard (Sc.3).
- A word straddling an emphasis boundary keeps **both** styles as adjacent spans
  (Sc.4); a styled word after a wrap break keeps its style with no positional drift
  (Sc.5).
- `style_of_word_in_rich_line` is gone; `wrap_rich`'s public signature and edge
  behavior are unchanged; the CLI plain path is untouched (Sc.6).
- Full suite green; `clippy -D warnings`, `fmt`, comment-policy clean; per-function
  complexity within budget (cyclomatic ≤ 10 / ≤ 8 new); tests assert observable span
  styles and are mutation-resistant.

### Plan

Single vertical slice (one production file + its unit tests): rewrite the wrap to
thread per-char style, delete the substring lookup, add the disambiguation tests.

### Resolution

Closed. Delivered in one slice (two rounds), opus-reviewed and green:

- `8a1095b` docs — ADR 0030 / BDR 0023 / this issue.
- `908d81e` fix — `expand_to_styled_chars` converts each source `RichLine` to an
  ordered `Vec<(char, RichStyle)>` once; the greedy width-wrap runs over those styled
  chars and coalesces each word's chars into spans via `push_rich_span`. A word
  crossing an emphasis boundary keeps both styles as adjacent spans by construction.
  `style_of_word_in_rich_line` and the `style_for_word` closure are deleted (no
  callers). `wrap_rich`'s public signature, the empty/whitespace/over-width edges,
  and `RichSpan` are unchanged.

**Reviewer round-1 catch (TE):** the substring no-inherit test was first written as
`['cat ' Plain, 'category' Bold]`. Under the reverted first-contains mutant, word
`cat` matches span[0] `'cat '` → Plain — the exact value the test asserts, so the
mutant **survived**. Round 2 reordered the fixture to `['category ' Bold, 'cat'
Plain]` (larger styled token first): the mutant now resolves `cat` to Bold ≠ the
asserted Plain → **killed**. Production code was correct throughout; only the test
fixture moved.

Final: `cargo test -- --test-threads=1` = 868 unit + 31 comment-policy passed;
`clippy -D warnings` / `fmt --check` clean (dev-container authoritative — the
quality-gate image's clippy is the known `linker cc not found` false-negative).
