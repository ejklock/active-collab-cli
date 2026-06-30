---
type: ADR
title: Retire the vestigial LinkCollector — the rich-text body/comment pipeline stops threading a link collector
description: ADR 0020 introduced LinkCollector to gather body/comment link URLs into a numbered registry; ADR 0030 (positional wrap style) and ADR 0032/0043 (structural link style + structural hit-targets emitted at layout time) removed every consumer of that registry, leaving LinkCollector as a write-only-on-init struct threaded as &mut through nine functions across render.rs and richtext.rs purely to satisfy the signature chain (close_anchor_rich already discards it with `let _ = collector;`, flush_open_contexts_rich takes `_collector`, and structured_text_with_links is itself dead_code). Delete the struct and all its parameters; the deletion concentrates the rich-text pipeline rather than moving complexity.
status: Accepted
supersedes:
superseded_by:
tags: [render, richtext, refactor, dead-code, deletion-test, locality]
timestamp: 2026-06-30T00:00:00Z
---

# 0046. Retire the vestigial `LinkCollector`

## Context

`LinkCollector` (`src/render.rs`) was introduced by
[ADR 0020](/adr/0020-body-links-inline-url-native-click.md) to gather inline link URLs from
the body/comment rich-text into a numbered registry (`next_index`, `urls`) that a numbered
"open link N" affordance then consumed.

Three later decisions removed every consumer of that registry without removing the registry
itself:

- [ADR 0030](/adr/0030-richtext-wrap-positional-style.md) made wrap style positional, so the
  link **style** no longer needs a collected URL list.
- [ADR 0032](/adr/0032-asset-row-link-style-structural.md) made link styling structural (a
  `RichStyle::Link` run emitted by the layout).
- [ADR 0043](/adr/0043-detail-hit-targets-emitted-structurally.md) made the link **hit-target**
  structural too — body-link URLs are emitted as `OpenUrl` affordance spans at layout time
  (`collect_body_url_affordances`), not looked up from a collector.

What remains is a struct that is **written only at construction and never read**. An Explorer
map of the current tree found:

- **`LinkCollector::new()`** sets `next_index = 1`, `urls = Vec::new()` — the only writes.
- **Zero reads** of `next_index` or `urls` anywhere; `.push`/`.extend`/indexing never occur.
- It is threaded as `&mut` through **nine** functions — `build_body_lines_with_collector`,
  `build_comment_lines_with_collector`, `extract_comment_body_rich` (render.rs);
  `structured_rich_with_links`, `process_tag_rich`, `handle_anchor_tag_rich`,
  `close_anchor_rich`, `flush_open_contexts_rich`, `structured_text_with_links` (richtext.rs)
  — and **no** function touches a field. `close_anchor_rich` ends with `let _ = collector;`
  (an explicit discard), `flush_open_contexts_rich` already names it `_collector`, and
  `structured_text_with_links` carries `#[allow(dead_code)]`.
- The struct itself carries `#[allow(dead_code)]` — the compiler has been told to stay quiet
  about exactly this.

The construction site `build_detail_content` creates the collector and never inspects it after
the two build functions return; their return values are derived from the parsed HTML, not from
the collector. The interface (a struct + nine `&mut` parameters) exists, but the work behind it
is nil. The **deletion test** passes: removing `LinkCollector` and its parameters concentrates
the rich-text pipeline (fewer arguments, no dead struct, no `#[allow(dead_code)]` lie) rather
than pushing complexity elsewhere — nothing reads what it held.

## Decision

Delete `LinkCollector` and stop threading it through the rich-text pipeline.

1. **Delete the struct.** Remove `pub struct LinkCollector { next_index, urls }`, its `impl`
   (`new`), and the `#[allow(dead_code)]` attribute, from `src/render.rs`.

2. **Drop the parameter from every threading function.** Remove `collector: &mut
   LinkCollector` / `_collector: &mut …` from `build_body_lines_with_collector`,
   `build_comment_lines_with_collector`, `extract_comment_body_rich`,
   `structured_rich_with_links`, `process_tag_rich`, `handle_anchor_tag_rich`,
   `close_anchor_rich`, and `flush_open_contexts_rich`. Remove the now-unnecessary
   `let _ = collector;` discard in `close_anchor_rich`. Update every call site, production and
   test, to drop the argument.

3. **Delete the dead non-rich variant.** Remove `structured_text_with_links` entirely — it is
   already `#[allow(dead_code)]`, takes the collector, and has no caller.

4. **Rename the functions whose suffix named the collector.** Once the collector is gone,
   `build_body_lines_with_collector` and `build_comment_lines_with_collector` are misnamed
   after a parameter that no longer exists. Rename them to `build_body_lines` and
   `build_comment_lines`. `structured_rich_with_links` keeps its name — its `_with_links` reads
   as "rich text including anchor/link rendering" (`handle_anchor_tag_rich` still parses `<a>`
   into a styled label), which remains accurate; renaming it would churn 40+ richtext tests for
   no clarity gain. `extract_comment_body_rich` already carries no collector suffix.

### Guard / fitness function

- **Behavior preserved — pure deletion.** No rendered byte changes: body description, comment
  cards, inline link styling, and the `OpenUrl`/`OpenAsset` hit-targets all derive from the
  HTML parse and the structural affordance emission, never from the collector. Every existing
  buffer-derived `build_detail_content` / body-link / comment-card spec stays green unchanged.
- **Deletion is the guard.** `LinkCollector`, `structured_text_with_links`, and the nine
  `collector` parameters no longer exist; `clippy --all-targets -D warnings` passes with **no**
  `#[allow(dead_code)]` standing in for this struct.
- Full suite green; `clippy --all-targets -D warnings`, `fmt --check`, comment-policy clean;
  complexity within budget (the change only removes parameters and a dead function — it cannot
  raise any function's complexity).

## Alternatives considered

- **Keep `LinkCollector`, delete only the unused fields.** Rejected: with both fields removed
  the struct is empty and still threaded as `&mut` through nine functions — the parameter chain
  (the actual noise) survives, and an empty `&mut` struct is a worse lie than a dead field.
- **Leave it with `#[allow(dead_code)]` "in case a future numbered-link feature returns".**
  Rejected: speculative generality. ADR 0043 settled link hit-targets as structural; a future
  feature would re-derive its own registry from the structural affordances, not resurrect a
  write-only collector. The git history records the prior shape if it is ever wanted.
- **Rename `structured_rich_with_links` → `structured_rich` as well.** Rejected for this slice:
  the `_with_links` suffix still describes real anchor rendering, and the rename would touch the
  large richtext test file for no semantic gain. Out of scope, not precluded later.

## Consequences

**Positive:** the rich-text body/comment pipeline sheds a dead struct and nine vestigial
parameters; `build_detail_content` no longer constructs and threads state nothing reads. The
`#[allow(dead_code)]` escape hatch is removed, so the compiler/clippy once again guard this
code for real dead-ness. Function signatures shrink and read honestly (`build_body_lines`,
`build_comment_lines`). One fewer concept to understand when navigating the renderer.

**Accepted trade-offs:** the rename touches the render test call sites; the diff is wide but
purely mechanical (drop one argument; two function renames). `structured_rich_with_links`
keeps a `_with_links` suffix that no longer refers to a collector — a deliberate, documented
choice to bound the slice, revisitable if the name later reads as stale.

## Related

- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (introduced `LinkCollector`)
- ADR: [/adr/0030-richtext-wrap-positional-style.md](/adr/0030-richtext-wrap-positional-style.md) (removed the style consumer)
- ADR: [/adr/0032-asset-row-link-style-structural.md](/adr/0032-asset-row-link-style-structural.md) (structural link style)
- ADR: [/adr/0043-detail-hit-targets-emitted-structurally.md](/adr/0043-detail-hit-targets-emitted-structurally.md) (structural link hit-target — removed the last consumer)
- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md) (the `_with_collector` production/test split this finishes unwinding)
- Issue: [/issues/0049-retire-vestigial-link-collector.md](/issues/0049-retire-vestigial-link-collector.md)
