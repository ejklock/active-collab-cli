---
type: Issue
title: "D1 — detail polish: wrapped-link click, Anexos label, empty project, title placement"
description: Four detail-view fixes surfaced by real-terminal use after V5 — app-side click on a wrapped URL fragment, meaningful Anexos/Artefatos labels, a populated Projeto row, and the task title moved into the Detalhes panel as a Título row.
status: open
labels: [tui, detail, render, links, ux]
blocked_by:
tracker:
timestamp: 2026-06-27T00:00:00Z
---

## D1 — detail polish

Four defects/decisions surfaced while dogfooding V5. Traces to
[ADR 0022](/adr/0022-detail-title-as-meta-row.md) (title placement),
[ADR 0023](/adr/0023-asset-label-derivation.md) (asset label),
[BDR 0016](/bdr/0016-detail-title-row-project-name.md) (Título row + Projeto),
[BDR 0017](/bdr/0017-asset-label-derivation.md) (Anexos label), and the
[BDR 0014](/bdr/0014-body-link-inline-url-activation.md) amendment (Sc. 7, wrapped click)
under the [ADR 0020](/adr/0020-body-links-inline-url-native-click.md) amendment.

### Problems

1. **Projeto empty** — the detail task JSON is loaded (`controller::load_task_data_from_path`)
   without enrichment, so `meta_field_pairs` reads a missing `project_name` and the
   `Projeto` row renders blank. `resolve_project_names` exists but is not applied to the
   detail task.
2. **Title loose above the box** — `render_content` (detail.rs) draws the title as an
   unframed line above the `Detalhes` panel (ADR 0018). It reads as orphaned.
3. **Anexos label is a query tail** — `asset_link_line` shows `url_basename`'s output
   (`edit?tab=t.0`) because `looks_like_filename` accepts `.0` as an extension.
4. **Wrapped-link click is a no-op** — `body_link_cmd_at` calls `url_at` on a single
   wrapped line; a long `[url]` split by `wrap_rich` resolves on no fragment, so clicks
   miss after the URL wraps / the terminal is resized.

### Slices (vertical, each demoable)

- **D1a — Projeto populated + Título as a row.** Enrich the detail task with its project
  name from the per-instance project-name cache; render a `Título` row after `Tarefa`;
  drop the loose title header. Files: `src/controller.rs`, `src/render.rs`,
  `src/tui/screens/detail.rs` (+ load wiring in `src/tui/mod.rs` if needed), tests.
  ADR 0022 / BDR 0016.
- **D1b — Meaningful Anexos label.** Derive the label as anchor text → real filename
  (tightened `looks_like_filename`: reject `?`/`=`/`&`, alpha extension 2–6, not purely
  numeric) → host. Files: `src/controller.rs` (label/name derivation, carry anchor text),
  `src/render.rs` (`looks_like_filename`, `asset_link_line`), tests. ADR 0023 / BDR 0017.
- **D1c — Wrapped-link click.** Map a body click to the **pre-wrap logical line** before
  `url_at`, so a click on any wrapped fragment opens the full URL. Files: `src/render.rs`
  (logical-line/column mapping helper), `src/tui/model.rs` (`body_link_cmd_at`), tests.
  BDR 0014 Sc. 7 / ADR 0020 amendment.

### Acceptance

- **D1a:** `Projeto` row shows the resolved name (fallback, never blank, on miss);
  `Título` row appears directly after `Tarefa`; no title line above the panel; rows wrap
  (BDR 0016 Sc. 1–4).
- **D1b:** a non-file web link labels as its host (or anchor text when present); a real
  file labels as its filename; a `?`/`=` tail is never a label (BDR 0017 Sc. 1–4).
- **D1c:** a click on any wrapped fragment of a long body URL emits the open `Cmd` for
  the complete URL (BDR 0014 Sc. 7).
- Each slice: full suite green; clippy `-D warnings`, fmt, comment-policy clean;
  complexity within budget; tests assert observable behavior (rendered rows/labels,
  emitted `Cmd`), mutation-resistant.

### Plan

Three sequential slices D1a → D1b → D1c (shared file `src/render.rs`, so sequential, not
parallel). Persist the plan once; dispatch each slice slim. Architecture: D1a adds a
detail-load project-name enrichment step (note in [architecture.md](/architecture.md)).
