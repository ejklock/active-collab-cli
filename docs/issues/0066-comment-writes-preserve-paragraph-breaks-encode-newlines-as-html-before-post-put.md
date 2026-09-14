---
type: Issue
title: Comment writes preserve paragraph breaks — encode newlines as HTML before POST/PUT
description: "Encode comment bodies as HTML before POST/PUT so paragraph breaks survive ActiveCollab rendering: add a pure encoder at the client write boundary (HTML-escape, blank-line to <p>, single newline to <br>) applied by both create_comment and update_comment. Extends ADR 0033."
status: open
timestamp: 2026-09-14T14:26:32Z
---

<!-- Status lives in frontmatter (`status`), not a body line. Settable values are
     exactly open | in-progress | closed. `living-docs supersede` sets Superseded on
     this issue -- never set it by hand -- when a later issue replaces it. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## Comment writes preserve paragraph breaks

Extends the write seam from [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md).

### Problem

The comment body is sent to ActiveCollab as raw text: `create_comment`/`update_comment`
build `json!({ "body": body })` (`src/client.rs:252`, `:276`). ActiveCollab stores and renders
the `body` field as **HTML**, where newlines carry no meaning. So a comment the user composes
with blank lines between paragraphs is flattened into one run on the server — the paragraph
breaks disappear, and the user has to reformat the text by hand. Both write surfaces are
affected: the CLI `comment_core` (`src/commands/comment.rs:33`) and the TUI compose path both
funnel into the same client calls.

A second, latent consequence of sending raw text as HTML: a literal `<`, `>`, or `&` in a
comment is currently interpreted as markup, not shown verbatim.

### Decision (cheap-to-reverse, kept in this issue)

Add one pure encoder at the client write boundary, applied to both `create_comment` and
`update_comment` so both surfaces inherit the fix:

- HTML-escape the text content first (`&`, `<`, `>` → entities) so literal characters render
  verbatim.
- Split on blank lines into paragraphs; wrap each paragraph in `<p>…</p>`.
- Within a paragraph, encode a single newline as `<br>`.
- An empty/whitespace-only body is already rejected upstream — no change there.

This keeps the round-trip correct: after the server-truth refresh (ADR 0035), the existing
rich-text mapper (ADR 0015/0019) renders the `<p>`/`<br>` structure back as the paragraphs the
user typed.

### Scope

Included:

- `src/client.rs` — a pure `fn encode_comment_body(&str) -> String` (or a shared helper module),
  called by both `create_comment` and `update_comment` before building the JSON payload.
- Tests: unit coverage for the encoder (escaping, paragraph split, single-newline `<br>`,
  idempotence on already-safe text) plus the client write tests that assert the posted payload.

Explicitly kept: the empty-body guard, the write seam, the outcome classification (ADR 0054),
and the server-truth refresh — unchanged.

Excluded: a full Markdown/rich-text compose editor; letting the user type raw HTML; any change
to how comments are *rendered* (that path already works).

### Acceptance

- AC1 — paragraph preservation (unit): a body with two paragraphs separated by a blank line
  encodes to two `<p>…</p>` blocks; the posted JSON `body` contains them. (`verify_by: test`)
- AC2 — single newline (unit): a single `\n` inside a paragraph encodes as `<br>`, not a new
  paragraph. (`verify_by: test`)
- AC3 — HTML-escape (unit): a body containing `<`, `>`, `&` encodes to entities so the
  characters render verbatim rather than as markup. (`verify_by: test`)
- AC4 — both surfaces (unit): `create_comment` and `update_comment` both apply the encoder;
  the posted/put payload for each carries the encoded body. (`verify_by: test`)
- CC — clean code: no superfluous comments / banners / commented-out code. (`verify_by: inspection`)
- TE — tests assert observable output (the encoded payload string), not the implementation.
  (`verify_by: command`)

### Plan

1. `src/client.rs`: add the pure `encode_comment_body` helper; call it in `create_comment` and
   `update_comment` before `json!({ "body": … })`.
2. Tests: unit-test the encoder directly and extend the client write tests to assert the encoded
   payload for POST and PUT.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
