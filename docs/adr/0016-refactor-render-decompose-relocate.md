---
type: ADR
title: Refactor render.rs — decompose the meta-table god function, drop dead seams, relocate asset extraction
description: A behavior-preserving maintainability pass — split the 100-line build_meta_table_rows into named helpers, remove the three #[allow(dead_code)] backward-compat line-builder wrappers (repointing their tests), and move extract_assets (data aggregation) out of the rendering module into the controller/domain layer.
status: Accepted
supersedes:
superseded_by:
tags: [refactor, render, controller, maintainability, complexity]
timestamp: 2026-06-26T00:00:00Z
---

# 0016. Refactor render.rs — decompose, de-dead-code, relocate

## Context

The user asked, explicitly last, to run an architecture pass to "eliminar god
functions ou coisa em domínio errado. Tem que ser manutenível e entendível"
(eliminate god functions or logic in the wrong domain; it must be maintainable and
understandable).

A structural read of the current tree (codegraph + inspection) surfaces three
concrete, named smells — not a vague "clean it up":

1. **God function.** `build_meta_table_rows` (`src/render.rs`, ~100 lines) computes
   and formats every meta row (task, project, status, assignee, start, due,
   estimate, logged) in one body. It is the longest function in the crate and mixes
   field-value computation with two-column alignment formatting.
2. **Dead seams.** `build_detail_lines`, `build_body_lines`, and
   `build_comment_lines` are `#[allow(dead_code)]` wrappers kept alive **only for
   test callers** (the production path uses the `_with_collector` variants /
   `build_detail_content` since V4a). Dead-code-by-allow is debt: the wrappers and
   the real functions can drift, and the allow hides it.
3. **Wrong-domain logic.** `extract_assets` (`src/render.rs`) is **data
   aggregation** — it pulls assets from the task body HTML, comments, and
   attachments and deduplicates by URL. It lives in the **rendering** module, but
   its only non-test caller is `controller::load_task_core`. Aggregation belongs in
   the domain/controller layer, not in render.

The arborist complexity gate currently passes (`qg-run.sh arborist src
--exceeds-only` EXIT:0), so these are **readability/placement** smells within the
cognitive threshold — not gate violations. The user wants them addressed for
maintainability regardless.

Force: **maintainability and right-place** (Conway/structure) — a rendering module
should render; aggregation should sit with the other domain orchestration in
`controller`; a 100-line function should be a deep module with a narrow interface,
not a flat wall.

## Decision

A **behavior-preserving** refactor, delivered as slice **ARCH** (no observable
behavior change → no BDR; parity is the contract).

### 1. Decompose `build_meta_table_rows`

Extract per-concern helpers: a single labeled-row builder (`label: value` with the
existing two-column alignment) plus small field formatters for the value side
(assignee from `user_map`, the timestamp/estimate/logged formatters already exist
and are reused). `build_meta_table_rows` becomes a short composition that gathers
`(label, value)` pairs and runs them through the row builder. Each resulting
function stays within cyclomatic ≤ 10 / cognitive ≤ 12.

### 2. Remove the dead-code wrappers

Delete `build_detail_lines`, `build_body_lines`, `build_comment_lines` and their
`#[allow(dead_code)]`. Repoint their test callers
(`tests/unit/render.rs`) to the real production functions
(`build_body_lines_with_collector` / `build_comment_lines_with_collector` /
`build_detail_content`), constructing a `LinkCollector` in the test as production
does. No production behavior changes; the test-only seam disappears.

### 3. Relocate `extract_assets`

Move `extract_assets` (and the `Asset` aggregation helpers it owns that are not
rendering) from `src/render.rs` to the domain/controller layer (it already lives
next to its only caller, `controller::load_task_core`). `render.rs` keeps only
display concerns. Update imports; the function signature and behavior are
unchanged.

### Guard

The whole slice is gated by **parity**: the full test suite stays green
(`docker compose run --rm dev cargo test`), `clippy -D warnings` and `fmt` clean,
the comment-policy test passes, and the complexity command stays at or below the
budget. Any test that has to change is a *call-site* change (dead-wrapper repoint),
never an assertion change.

## Alternatives considered

- **Leave it as-is.** Rejected: the user asked for the pass, and the three smells
  are concrete maintainability debt (longest function in the crate, hidden dead
  code, aggregation in the render module).
- **A full domain-module split (extract a `domain/` layer).** Deferred: out of
  proportion for three local smells. Relocating `extract_assets` to the existing
  `controller` is the minimal right-place move; a broader split can be its own ADR
  if the domain logic grows.
- **Keep the wrappers but document them.** Rejected: a `#[allow(dead_code)]` seam
  that exists only for tests is exactly the drift this pass removes; tests should
  call the production functions.

## Consequences

**Positive:** `render.rs` shrinks and renders only; the longest function becomes a
short composition of named helpers; the test suite exercises the real production
functions (no parallel dead seam to drift); aggregation sits with the other
controller orchestration. More maintainable and understandable, as asked.

**Accepted trade-offs:** a churny diff across `render.rs`, `controller.rs`, and
`tests/unit/render.rs`; the slice must be verified purely by parity (green tests +
clean gates) since there is no new behavior to assert. Risk is contained by doing
it **last**, after the V3/C1/R2/R3 features land, so the moved/decomposed code is
the final shape.

## Related

- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md)
- ADR: [/adr/0015-richtext-html-subset-styled-segments.md](/adr/0015-richtext-html-subset-styled-segments.md)
- Issue: [/issues/0014-arch-refactor-render-decompose-relocate.md](/issues/0014-arch-refactor-render-decompose-relocate.md)
- Architecture: [/architecture.md](/architecture.md)
