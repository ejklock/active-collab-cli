---
type: Issue
title: "Retire the vestigial LinkCollector — delete the write-only struct and the nine collector parameters threaded through the render/richtext rich-text pipeline"
description: After ADR 0030/0032/0043 made link style and link hit-targets structural, LinkCollector (src/render.rs) is written only at construction and never read, yet it is threaded as &mut through nine functions in render.rs and richtext.rs (one already discards it, one names it _collector, structured_text_with_links is dead_code). Delete the struct, drop the parameter from every threading function, delete structured_text_with_links, and rename the two _with_collector build functions. Pure deletion — no rendered output changes.
status: closed
labels: [render, richtext, refactor, dead-code, deletion-test, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Retire the vestigial `LinkCollector` (ADR 0046)

### Problem

`LinkCollector` (`src/render.rs`, `#[allow(dead_code)]`) holds `next_index: usize` and
`urls: Vec<String>`. Both are set once in `LinkCollector::new()` and **never read or written
again** anywhere in the tree. It is still threaded as `&mut` through nine functions purely to
satisfy the signature chain:

- `build_body_lines_with_collector`, `build_comment_lines_with_collector`,
  `extract_comment_body_rich` (`src/render.rs`)
- `structured_rich_with_links`, `process_tag_rich`, `handle_anchor_tag_rich`,
  `close_anchor_rich` (ends with `let _ = collector;`), `flush_open_contexts_rich` (param is
  `_collector`), `structured_text_with_links` (`#[allow(dead_code)]`, no caller) (`src/richtext.rs`)

The consumers that once read the collected URLs were removed by ADR 0030 (positional wrap
style), ADR 0032 (structural link style), and ADR 0043 (structural `OpenUrl` hit-targets). The
deletion test passes: nothing reads what the collector holds.

### Decision (ADR 0046)

Delete `LinkCollector` and stop threading it.

### Scope

- `src/render.rs`:
  - Delete `pub struct LinkCollector`, its `impl`/`new`, and the `#[allow(dead_code)]`.
  - Drop the `collector: &mut LinkCollector` parameter from `build_body_lines_with_collector`,
    `build_comment_lines_with_collector`, `extract_comment_body_rich`, and update the
    `build_detail_content` construction site (remove `let mut collector = …` and the `&mut
    collector` arguments).
  - Rename `build_body_lines_with_collector` → `build_body_lines`,
    `build_comment_lines_with_collector` → `build_comment_lines` (the suffix named the deleted
    parameter). Verify no name collision; keep the old name only if one exists.
- `src/richtext.rs`:
  - Drop the collector parameter from `structured_rich_with_links` (keep this name),
    `process_tag_rich`, `handle_anchor_tag_rich`, `close_anchor_rich` (remove the
    `let _ = collector;`), `flush_open_contexts_rich`.
  - Delete `structured_text_with_links` entirely.
- `tests/unit/render.rs`, `tests/unit/richtext.rs`: drop the collector argument at every call
  site and apply the two function renames; assert no behavior changes.

### Out of scope

- Renaming `structured_rich_with_links` (its `_with_links` still describes real anchor
  rendering; deferred per ADR 0046).
- Any change to rendered output, link styling, or affordance emission. Pure deletion.

### Acceptance criteria

- **AC1** (constraint, inspection): `LinkCollector` (struct + `impl` + `new`) and
  `structured_text_with_links` no longer exist; no function in `src/render.rs` or
  `src/richtext.rs` takes a `collector`/`_collector` parameter; the `let _ = collector;`
  discard in `close_anchor_rich` is gone. No `#[allow(dead_code)]` remains standing in for the
  struct.
- **AC2** (behavior, test): detail rendering is byte-for-byte unchanged — the existing
  buffer-derived `build_detail_content`, body-link, and comment-card specs stay green, now
  calling the collector-free signatures.
- **AC3** (constraint, command): `cargo clippy --all-targets -- -D warnings` passes with no
  dead-code allowance for this struct (proves nothing is left dangling and no parameter is
  unused).
- **AC4** (constraint, inspection): `build_body_lines_with_collector` /
  `build_comment_lines_with_collector` are renamed to `build_body_lines` / `build_comment_lines`
  (no name now refers to a removed collector), unless a real collision forces keeping a name
  (state which).
- **CC** (constraint, inspection): clean code — no banners/commented-out code; only non-obvious
  why-comments; comment-policy gate green.
- **CX** (constraint, command): complexity within budget — cyclomatic ≤ 10 (≤ 8 for new
  functions), cognitive ≤ 12 (quality-gate arborist). The change only removes parameters and a
  dead function; no function's complexity may rise.
- **TE** (constraint, command): tests assert observable rendering behavior and survive the
  mutation floor; no test asserts anything about the deleted collector.

### Verification

`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo test --test comment_policy`,
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- ADR: [/adr/0046-retire-vestigial-link-collector.md](/adr/0046-retire-vestigial-link-collector.md)
- ADR: [/adr/0043-detail-hit-targets-emitted-structurally.md](/adr/0043-detail-hit-targets-emitted-structurally.md) (removed the last collector consumer)
- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (introduced LinkCollector)
