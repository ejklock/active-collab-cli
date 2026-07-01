---
type: Context
title: Glossary
description: Terms and acronyms used across the active-collab-cli docs, defined once.
tags: [glossary]
timestamp: 2026-06-25T00:00:00Z
---

# Glossary

One home per term. Acronym headwords are kept as-is; only the explanation is in
the doc language (English).

## Acronyms

- **ADR** — Architecture Decision Record. A record of one architectural/
  implementation decision, its context, and consequences. See [adr/](/adr/index.md).
- **API** — Application Programming Interface. Here, the ActiveCollab HTTP API the
  client talks to.
- **ATAM** — Architecture Tradeoff Analysis Method (SEI). Source of the six-part
  quality-attribute scenario form used for NFRs in the PRD.
- **BDR** — Behavior Decision Record. A record of observable behavior as
  Given/When/Then scenarios plus a Test Design matrix. See [bdr/](/bdr/index.md).
- **CLI** — Command-Line Interface. The non-interactive command surface
  (`setup`, `get`, `current`, `mine`, `browse`).
- **MSRV** — Minimum Supported Rust Version. The oldest rustc a crate compiles on;
  relevant when pinning the builder image.
- **NFR** — Non-Functional Requirement. A quality attribute (deployability,
  responsiveness, security…) written as a quality-attribute scenario bound to an
  instrument.
- **OKF** — Open Knowledge Format. The markdown + YAML-frontmatter format these
  docs conform to.
- **PRD** — Product Requirements Document. What a capability must do and why. See
  [prd/](/prd/index.md).
- **SGR** — Select Graphic Rendition. The terminal escape-sequence family whose
  mouse-mode (`?1006h`) the old curses code hand-parsed (and mis-parsed).
- **TEA** — The Elm Architecture. The Model / Msg / `update` / `view` pattern the
  Rust TUI uses; the `update` core is pure and unit-tested.
- **TUI** — Text/Terminal User Interface. The interactive full-screen browser
  (`browse`).

## Terms

- **Instance** — one configured ActiveCollab deployment (name, base URL, email,
  token, user id). Tasks are scoped per instance.
- **Slice** — one issue-sized unit of the rewrite (R0–R8) that is independently
  reviewable; tracked in [issues/](/issues/index.md) and plan `rust-rewrite`.
- **Parity** — feature/output equivalence: a Rust command produces the same
  observable result as the Python command it replaces.
- **Single-flight refresh** — at most one in-flight fetch per group; a refresh
  requested while one is running does not enqueue a second.
- **Token host isolation** — an instance's API token is attached only to requests
  to that instance's own host (a non-negotiable).
- **Affordance registry** — `DetailContent.affordances`, the single list of typed,
  layout-emitted clickable spans (`{ line_idx, col_start, col_end, kind }`) the
  detail click hit-test resolves by a positional lookup. The home of every detail
  **hit-target** (comment edit/delete, body-link URL, asset row); emitted by
  `build_detail_content`, never re-derived from rendered text (ADR 0043).
- **Hit-target** — what a click at a given cell resolves to (an `AffordanceKind`:
  edit/delete a comment, open a URL, open an asset). Emitted structurally by the
  layout alongside the lines and style runs, paralleling structural link **style**
  (ADR 0032) — style and hit-target share the same single-source discipline.
- **Detail content viewport** — the scrollable text region of the Detail screen: the rows
  `[DETAIL_TEXT_TOP, DETAIL_TEXT_TOP + content_height)` (top row `2`, height
  `viewport_rows - DETAIL_CHROME_ROWS`) mapping a terminal row to a content `line_idx` via
  `offset + (row - DETAIL_TEXT_TOP)`. Single-homed in the pure `src/tui/detail_geometry.rs`
  (`is_in_content`, `row_to_line_idx`), shared by hit-test, selection, and copy (ADR 0045). The
  same module owns `content_height` and `content_height_clamped` (the body height floored at one
  row) that the scroll/offset-clamp math derives from (issue 0052).
- **Wrap engine** — the greedy word-wrap by display width in `src/render.rs`. One core,
  `greedy_wrap`, over a `WrapCell`/`WrapLine` abstraction: it splits a cell stream on newlines
  (preserving blank segments), places each word on the current line if it fits within `width`
  display columns (else flushes), and hard-splits a word wider than `width`. Two thin adapters
  carry the two cell types — `wrap_text` (`char` → `String`, TUI chrome/cards) and `wrap_rich`
  (`(char, RichStyle)` → `RichLine`, the styled detail body, preserving positional style per
  ADR 0030). The single canonical contract (blank lines preserved, ascii-whitespace
  tokenization, per-character measure) is why the two adapters cannot drift (ADR 0048).
- **DetailClickTarget** — the typed result of resolving a detail click
  (`CommentEdit`/`CommentDelete`/`OpenUrl`/`OpenAsset`), returned by the pure
  `hit_test::resolve_detail_click`. It decouples the layout artifact (`AffordanceKind`)
  from the resolution result from the effect (`Cmd`); the model maps it to the TEA effect
  (ADR 0044). The home of detail click resolution: one coordinate translation + one
  registry lookup, replacing the five scattered click functions formerly in `model.rs`.
