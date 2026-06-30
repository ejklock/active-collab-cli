---
type: Issue
title: "Close the plain/rich wrap test asymmetry — add direct wrap_rich geometry characterization specs (hard-split, exact-width boundary, zero-width, all-whitespace, embedded newline, empty fallback)"
description: wrap_text and wrap_rich are parallel greedy-wrap engines in src/render.rs. wrap_text has 11 direct named geometry specs; wrap_rich has only style-survival specs (ADR 0030 / BDR 0023) — its identical geometry branches (zero-width guard, all-whitespace passthrough, hard-split of a word wider than width, exact-width boundary, embedded-newline split, empty-result fallback) are covered only incidentally through build_detail_content. Add direct wrap_rich specs through its interface so those off-by-one-sensitive branches are pinned by named specs and the mutation floor, matching wrap_text. Characterization only — no production change.
status: closed
labels: [render, richtext, wrap, testing, characterization, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Direct wrap_rich geometry characterization specs

### Problem

`src/render.rs` carries two parallel greedy-wrap engines:

- **plain:** `wrap_text` → `wrap_single_line` → `append_word_to_line` / `hard_split_word`.
- **rich:** `wrap_rich` → `wrap_rich_single_line` → `append_rich_word` / `hard_split_rich_word`.

`wrap_text` has **11 direct named geometry specs** (`tests/unit/render.rs:752-859`): empty input,
greedy break, hard-split of a word longer than width, embedded newlines, CJK/wide chars,
decomposed accents. `wrap_rich` has **6 direct specs** (`tests/unit/richtext.rs:671-896`) but
they are all about **style survival** (ADR 0030 / BDR 0023): strike/underline persist, repeated
word per-occurrence style, substring non-inheritance, cross-boundary, post-break style.

`wrap_rich`'s **geometry** branches — identical in shape to `wrap_text`'s, but a separate
implementation — have **no direct spec**:

- zero-width / empty-line guard (`render.rs:1162`).
- all-whitespace passthrough returns the line unwrapped (`render.rs:1166`).
- hard-split of a word wider than `width`, preserving each char's style (`render.rs:1284` →
  `hard_split_rich_word`).
- exact-width boundary: the `current_dw + 1 + word_dw <= width` space accounting
  (`render.rs:1242`) — an off-by-one-sensitive check.
- embedded-newline split into independent wraps (`render.rs:1176`).
- empty-result fallback (`render.rs:1185-1189`).

These are exercised only incidentally by `build_detail_content` buffer tests at a fixed width,
which may not land on the exact boundary — so a boundary or hard-split mutant in the rich engine
can survive. The Explorer flagged the boundary checks as off-by-one-sensitive.

### Decision

No decision/structural change — this is coverage execution under ADR 0030 (positional wrap
style) and ADR 0018 (detail chrome wrap). Add direct `wrap_rich` geometry specs that mirror the
existing `wrap_text` geometry specs, so the rich engine's branches are pinned through its own
interface and survive the mutation floor.

### Scope

- `tests/unit/richtext.rs` (co-located with the existing `wrap_rich` style specs) — add named
  geometry specs calling `wrap_rich` directly:
  1. zero-width (`width == 0`) and empty-line input → empty `Vec`.
  2. all-whitespace line → returns the original line unwrapped (one result line, unchanged
     spans).
  3. a styled word wider than `width` hard-splits into multiple result lines, and **each chunk
     keeps the word's style** (not just plain) — the rich-specific guarantee `wrap_text` cannot
     express.
  4. exact-width boundary: two words that fit in `width` exactly stay on one line; one column
     over breaks to a second line (asserts the `+ 1` space accounting).
  5. an embedded `\n` produces two independent wrapped segments.
  6. a word exactly equal to `width` occupies its own line without a spurious extra break.

### Out of scope

- Any production change to the wrap engines (no behavior change; if a spec reveals a real
  off-by-one, that becomes a separate bug-fix slice).
- Unifying the plain/rich engines into one generic wrap (a distinct deepening candidate, not
  this slice).

### Acceptance criteria

- **AC1** (test, behavior): the six geometry specs above call `wrap_rich` directly and pass,
  asserting wrapped-line counts and per-fragment text+style — not via `build_detail_content`.
- **AC2** (constraint, command): the new specs assert observable wrap output and survive the
  mutation floor — flipping the boundary comparison (`<=` → `<`) or dropping the hard-split must
  fail at least one new spec. No spec is a change-detector with no behavioral assertion.
- **AC3** (constraint, inspection): no production code changes; the wrap engines are untouched.
  If a spec exposes a real defect, stop and surface it rather than adapting the assertion to the
  bug.
- **CC** (constraint, inspection): clean test code — no banners/commented-out code; only
  non-obvious why-comments; comment-policy gate green.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0030-richtext-wrap-positional-style.md](/adr/0030-richtext-wrap-positional-style.md) (the per-character style threading these specs guard)
- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (the wrap-to-content-width behavior)
- BDR: [/bdr/0023-richtext-wrap-positional-style.md](/bdr/0023-richtext-wrap-positional-style.md) (the wrap style scenarios the existing wrap_rich specs cover)
