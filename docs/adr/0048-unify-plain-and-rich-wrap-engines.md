---
type: ADR
title: One greedy word-wrap engine — plain and rich wrap share a single core over a cell abstraction, and the canonical semantics fixes the rich blank-line drop
description: src/render.rs carries two parallel word-wrap engines — wrap_text (plain &str → Vec<String>) and wrap_rich (styled RichLine → Vec<RichLine>). They implement the identical greedy display-width algorithm plus hard-split, differing only in the cell they carry (char vs (char, RichStyle)), yet diverge on edge cases the duplication let drift: the rich engine DROPS blank lines between paragraphs (a\n\nb → two lines, losing the gap), plain tokenizes on unicode whitespace while rich tokenizes on ascii, and the word-measure composition differs. Extract one generic greedy_wrap core over a WrapCell/WrapLine abstraction; both engines become thin adapters; the core defines one canonical wrap contract (blank newline-segments preserved, ascii-whitespace tokenization, per-character display-width measure) that fixes the rich blank-line bug. wrap_text is TUI-only (never on the CLI byte-for-byte parity path), so the reconciliation is safe.
status: Accepted
supersedes:
superseded_by:
tags: [tui, richtext, render, wrap, refactor, locality, depth, ratatui]
timestamp: 2026-06-30T00:00:00Z
---

# 0048. One greedy word-wrap engine over a cell abstraction

## Context

`src/render.rs` word-wraps text in two places with two engines that share one algorithm:

- **Plain** — `wrap_text(text: &str, width) -> Vec<String>` (`render.rs:1073`), with the private
  helpers `wrap_single_line`, `append_word_to_line`, `hard_split_word`. Twelve TUI call sites
  (header, footer, meta rows, comment box, task/project cards, height measurement).
- **Rich** — `wrap_rich(line: &RichLine, width) -> Vec<RichLine>` (`render.rs:1160`), with
  `expand_to_styled_chars`, `wrap_rich_single_line`, `append_rich_word`, `hard_split_rich_word`,
  `word_display_width`, `emit_styled_chars`. One call site — `build_rich_body_rows`, the visible
  detail description/comment body ([ADR 0030](/adr/0030-richtext-wrap-positional-style.md)).

Both implement the **same greedy word-wrap by display width**: place a word on the current line
if it fits (with a single separating space), else flush and start a new line; a word wider than
`width` is hard-split by accumulated display width. The only genuine difference is the **cell**
each carries — a `char` for plain, a `(char, RichStyle)` pair for rich — and therefore the line
accumulator (`String` vs `RichLine`) and output (`Vec<String>` vs `Vec<RichLine>`).

Because the algorithm is duplicated rather than shared, the two copies have **drifted on edge
cases** — the exact smell the structural-review lesson flagged:

1. **Blank line between paragraphs — a latent rich bug.** Plain splits on `\n` and wraps each
   segment, so `"a\n\nb"` preserves the empty middle segment → `["a", "", "b"]`. Rich's
   segment loop pushes the accumulator only when non-empty, so the empty segment is **dropped**
   → `["a", "b"]`. In the visible detail body, paragraph spacing from a multi-paragraph
   description silently collapses.
2. **Whitespace tokenization.** Plain uses `str::split_whitespace` (breaks on *Unicode*
   whitespace, e.g. NBSP U+00A0); rich skips only `is_ascii_whitespace`. The two disagree on
   whether a non-breaking space forces a wrap.
3. **Word-measure composition.** Plain measures a whole word with `display_width` (a
   `UnicodeWidthStr` call) in the fits-check but per-character in the hard-split; rich measures
   per-character throughout. They agree on the tested cases but are not defined to.

Keeping two engines means every wrap fix or width tweak must be made twice and kept in
agreement by convention — and, as (1) shows, they were not.

## Decision

Extract **one generic greedy-wrap core** and make both engines thin adapters over it, with a
single canonical wrap contract that resolves the drift.

1. **A cell/line abstraction.** Two small traits express what the algorithm needs from a cell
   and a line, nothing more:
   - `WrapCell` — `display_width(&self) -> usize`, `is_word_separator(&self) -> bool`
     (ascii whitespace that is not a newline), `is_newline(&self) -> bool`.
   - `WrapLine: Default` — `push_cell`, `push_separator` (one space), `is_empty`. The core owns
     the running display-width counter; the line only builds.

2. **One core algorithm.** `greedy_wrap<C: WrapCell, L: WrapLine<Cell = C>>(cells: &[C], width)
   -> Vec<L>` splits the cell stream on newline cells into segments and greedy-wraps each
   segment's words, hard-splitting any word wider than `width`. It is the single home of the
   placement decision and the hard-split loop.

3. **Two adapters.**
   - `wrap_text` guards empty input / `width == 0` → `[]`, expands the `&str` to `char` cells,
     calls `greedy_wrap::<char, String>`, returns the `Vec<String>`.
   - `wrap_rich` guards empty input / `width == 0` → `[]`, expands the `RichLine` to
     `(char, RichStyle)` cells via `expand_to_styled_chars`, calls
     `greedy_wrap::<_, RichLine>` (the `RichLine` `WrapLine` impl coalesces adjacent same-style
     spans via `push_rich_span`), returns the `Vec<RichLine>`.
   The six per-engine helpers (`wrap_single_line`, `append_word_to_line`, `hard_split_word`,
   `wrap_rich_single_line`, `append_rich_word`, `hard_split_rich_word`) are deleted — the core
   subsumes them.

4. **The canonical wrap contract** (defined once, in the core):
   - **Newline segments are preserved, including empty ones** — every `\n`-delimited segment
     yields at least one line; a segment with no words yields one empty line. This **fixes the
     rich blank-line drop**: `"a\n\nb"` → three lines in both engines. (Plain already behaved
     this way for genuinely-empty segments.)
   - **Tokenization is ascii-whitespace** — matches the current *visible body* (rich) behavior,
     so the detail body render is unchanged; a non-breaking space no longer forces a wrap
     (correct for terminal text). Plain chrome adopts the same rule; unicode-space-only values
     are effectively nonexistent in the TUI chrome.
   - **Display width is measured per character** (`UnicodeWidthChar`), summed — the composition
     both engines already used in their hard-split, now used throughout.
   - A whitespace-only line normalizes to one **blank** line (no trailing spaces); on a terminal
     this is visually identical to preserving the spaces, so no visible output changes.

### Guard / fitness function

- **The visible detail body is unchanged except for the bugfix.** All existing buffer-derived
  detail-body specs stay green; the one intended change is that multi-paragraph descriptions now
  render their blank-line spacing. A characterization test asserts `"a\n\nb"` wraps to three
  lines (blank preserved) in *both* engines.
- **One algorithm, one home.** `greedy_wrap` is the only greedy placement + hard-split loop;
  grep finds no `wrap_single_line` / `wrap_rich_single_line` / `hard_split_word` /
  `hard_split_rich_word` after the change. Deleting `greedy_wrap` would force the loop back into
  two copies — the deletion test passes (it concentrates complexity, not merely moves it).
- **The interface is the test surface.** The plain (14) and rich (17) wrap suites both exercise
  the shared core through their adapters; the rich all-whitespace spec is updated to the
  canonical blank-line result, with the blank-line-preservation case added on both sides.
- **CLI parity untouched.** `wrap_text` is TUI-only; `html_to_text` (the `ac get`/`current`/
  `mine` plain-text output, [BDR 0003] parity) never wraps, so no non-TTY output changes.
- Full suite green; `clippy --all-targets -D warnings`, `fmt`, comment-policy clean; complexity
  within budget (cyclomatic ≤ 10, ≤ 8 for new functions).

## Alternatives considered

- **Plain-as-degenerate-rich: route `wrap_text` through `wrap_rich` and flatten.** Since plain
  is a projection of rich (all-`Plain` spans), `wrap_text` could build a one-span `RichLine`,
  call `wrap_rich`, and flatten each result to a `String`, deleting the whole plain engine.
  Rejected as the *sole* mechanism because it inherits the rich blank-line bug (it would have to
  be fixed in `wrap_rich` anyway) and pays a span-allocation + flatten cost on every chrome
  wrap. The generic core fixes the bug once for both and keeps plain's native `String` output.
- **Pure structural extraction — share only the inner loops, keep each tokenizer/output exact.**
  Rejected (and explicitly declined by the maintainer): it preserves the asymmetries behind the
  shared core — the blank-line bug survives — for a smaller win. The point of unifying is to
  make the two engines *agree*, not to hide their disagreement.
- **Leave the two engines (status quo).** Rejected: the duplication is the reason the blank-line
  behavior drifted, and any future wrap change must be made and verified twice.

## Consequences

**Positive:** one home for "how text greedily wraps to a width"; the plain and rich engines can
no longer disagree on blank lines, whitespace, or measure; the detail body renders paragraph
spacing correctly; six duplicated helpers collapse into one generic core plus two trivial trait
impls; a future wrap change is a one-place edit covered by both suites.

**Accepted trade-offs:** two small traits (`WrapCell`, `WrapLine`) are introduced — deliberate
seams that make the cell/line abstraction explicit rather than duplicated; the rich
all-whitespace output changes from the preserved-spaces line to a normalized blank line
(invisible on a terminal) and its characterization test is updated accordingly; plain chrome
tokenization moves from unicode- to ascii-whitespace (no reachable difference in practice).

## Related

- ADR: [/adr/0030-richtext-wrap-positional-style.md](/adr/0030-richtext-wrap-positional-style.md) (the per-character positional style the rich `WrapLine` impl preserves)
- ADR: [/adr/0015-richtext-html-subset-styled-segments.md](/adr/0015-richtext-html-subset-styled-segments.md) (the styled segments the rich engine wraps)
- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (the chrome-height wrapping that calls `wrap_text`)
- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md) (the prior render.rs de-duplication discipline this continues)
- Issue: [/issues/0053-unify-wrap-engines.md](/issues/0053-unify-wrap-engines.md)
