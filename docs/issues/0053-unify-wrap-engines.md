---
type: Issue
title: "Unify the parallel plain/rich word-wrap engines into one greedy_wrap core over a cell abstraction — and fix the rich blank-line drop (ADR 0048)"
description: src/render.rs carries two word-wrap engines implementing the identical greedy display-width algorithm — wrap_text (plain &str → Vec<String>) and wrap_rich (styled RichLine → Vec<RichLine>) — differing only in the cell they carry (char vs (char, RichStyle)), and drifted on edge cases: rich DROPS blank lines between paragraphs (a\n\nb loses the gap), plain tokenizes on unicode whitespace vs rich on ascii. Extract one generic greedy_wrap<C: WrapCell, L: WrapLine<Cell=C>> core; make both engines thin adapters; canonical contract preserves blank newline-segments (fixes rich), tokenizes on ascii-whitespace, measures per-character. Delete the six per-engine helpers. wrap_text is TUI-only (not on the CLI parity path), so the reconciliation is safe.
status: closed
labels: [tui, richtext, render, wrap, refactor, locality, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Unify the plain and rich wrap engines (ADR 0048)

### Problem

`src/render.rs` word-wraps in two places with two engines that share one algorithm but drifted:

- **Plain** — `wrap_text(text: &str, width) -> Vec<String>` (`render.rs:1073`) + helpers
  `wrap_single_line` (`:1100`), `append_word_to_line` (`:1084`), `hard_split_word` (`:1130`).
  12 TUI call sites (chrome, cards, meta, comment box, height measurement).
- **Rich** — `wrap_rich(line: &RichLine, width) -> Vec<RichLine>` (`render.rs:1160`) + helpers
  `wrap_rich_single_line` (`:1212`), `append_rich_word` (`:1276`), `hard_split_rich_word`
  (`:1295`), and the styled-char utilities `expand_to_styled_chars`, `word_display_width`,
  `emit_styled_chars`. 1 call site — `build_rich_body_rows` (the visible body).

Both run the same greedy word-wrap by display width + hard-split; the only real difference is
the cell (`char` vs `(char, RichStyle)`). The duplication let the edge behavior diverge:

- **Rich drops blank lines** — `"a\n\nb"` → `["a", "b"]` (the empty middle segment is lost),
  while plain preserves it → `["a", "", "b"]`. Multi-paragraph descriptions lose their spacing.
- **Whitespace class differs** — plain `split_whitespace` (unicode) vs rich `is_ascii_whitespace`.
- **Measure composition differs** — plain uses `display_width(word)` in the fits-check, per-char
  in hard-split; rich is per-char throughout.

### Decision (ADR 0048)

Extract one generic core; both engines become thin adapters; one canonical contract fixes the
drift. Maintainer-confirmed direction: **canonicalize + fix** (not pure structural extraction).

### Scope

- `src/render.rs`:
  - Add trait `WrapCell` — `display_width(&self) -> usize`, `is_word_separator(&self) -> bool`
    (ascii whitespace, not newline), `is_newline(&self) -> bool` — impl'd for `char` and for
    `(char, RichStyle)`.
  - Add trait `WrapLine: Default` — `push_cell(&mut self, &Self::Cell)`, `push_separator(&mut self)`
    (one space), `is_empty(&self) -> bool` — impl'd for `String` (plain) and `RichLine` (rich,
    coalescing via the existing `push_rich_span`). The core owns the running display-width count.
  - Add `greedy_wrap<C: WrapCell, L: WrapLine<Cell = C>>(cells: &[C], width: usize) -> Vec<L>`:
    split the cell stream on newline cells into segments (**every segment yields ≥1 line; an
    empty segment yields one empty line**); greedy-place words (fits → push_separator + word;
    else flush + word); hard-split any word wider than `width` by accumulated display width.
  - Rewrite `wrap_text` as a `char`-cell adapter and `wrap_rich` as a `(char, RichStyle)`-cell
    adapter over `greedy_wrap`, each keeping the empty-input / `width == 0` → `[]` guard.
  - **Delete** `wrap_single_line`, `append_word_to_line`, `hard_split_word`,
    `wrap_rich_single_line`, `append_rich_word`, `hard_split_rich_word`. Keep
    `expand_to_styled_chars`, `word_display_width` (or fold into `WrapCell`), `push_rich_span`.
- `tests/unit/richtext.rs`: update `wrap_rich_all_whitespace_returns_single_line_unchanged` to
  the canonical blank-line result; add a blank-line-preservation spec (`"a\n\nb"` → 3 lines).
- `tests/unit/render.rs`: add the plain blank-line-preservation characterization spec.
- Any `tests/unit/tui_render.rs` body-line-count spec that shifts because a multi-paragraph
  description now keeps its blank line is updated to the corrected count (the bugfix).

### Out of scope

- The CLI / non-TTY plain-text path (`html_to_text`) — it never wraps; BDR 0003 parity untouched.
- Changing the greedy algorithm itself, the hard-split rule, or the display-width crate.
- Touching the affordance registry / hit-test / geometry — they derive from the wrapped lines
  and stay correct as the line count changes.

### Acceptance criteria

- **AC1** (constraint, inspection): a single `greedy_wrap<C: WrapCell, L: WrapLine<Cell = C>>`
  core exists; `wrap_text` and `wrap_rich` are thin adapters over it; the six per-engine helpers
  (`wrap_single_line`, `append_word_to_line`, `hard_split_word`, `wrap_rich_single_line`,
  `append_rich_word`, `hard_split_rich_word`) are deleted — grep finds none of them.
- **AC2** (behavior, test): blank lines between paragraphs are preserved in **both** engines —
  `wrap_text("a\n\nb", w)` and the rich equivalent each yield three lines with an empty middle
  line (the rich blank-line drop is fixed). Characterization specs on both sides.
- **AC3** (behavior, test): the visible detail body render is otherwise unchanged — all existing
  buffer-derived detail-body / chrome specs stay green (the only intended diff is paragraph
  spacing); the rich all-whitespace spec asserts the canonical blank line.
- **AC4** (behavior, test): the greedy geometry is preserved — the existing wrap suites
  (exact-width boundary, one-over-width break, hard-split of an over-wide word with style
  preserved, embedded newline, CJK/accent display-width) stay green through the adapters.
- **CC** (constraint, inspection): clean code — no banners/commented-out code; only non-obvious
  why-comments; comment-policy gate green.
- **CX** (constraint, command): complexity within budget — cyclomatic ≤ 10 (≤ 8 for new
  functions), cognitive ≤ 12 (quality-gate arborist); the generic core must not exceed the
  budget the two loops it replaces sat within.
- **TE** (constraint, command): tests assert observable wrapped output and survive the mutation
  floor — dropping the blank-segment emission, the `<=` width boundary, or the hard-split flush
  must fail a spec.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0048-unify-plain-and-rich-wrap-engines.md](/adr/0048-unify-plain-and-rich-wrap-engines.md)
- ADR: [/adr/0030-richtext-wrap-positional-style.md](/adr/0030-richtext-wrap-positional-style.md) (positional style the rich adapter preserves)
- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (chrome wrapping that calls wrap_text)
