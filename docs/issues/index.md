# Issues

Slices of the Rust rewrite (plan `rust-rewrite`, R0–R8) and the post-parity
TUI/UX work (V/R/ARCH slices), tracing to
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) and the ADRs/BDRs each row links.

| # | Slice | Title | Status | Blocked by |
|---|---|---|---|---|
| [0001](/issues/0001-r0-spike-scaffold-docker-tui.md) | R0 | Rust scaffold + Docker (dev/build) + ratatui mouse spike | closed | — |
| [0002](/issues/0002-r1-config-sqlite-store.md) | R1 | Config + SQLite store parity | closed | 0001 |
| [0003](/issues/0003-r2-http-api-client.md) | R2 | HTTP + ActiveCollab API client parity | closed | 0002 |
| [0004](/issues/0004-r3-cli-setup-commands.md) | R3 | CLI scaffold + setup commands + bare-invocation | closed | 0003 |
| [0005](/issues/0005-r4-get-current-commands.md) | R4 | get + current commands (fetch + render) | closed | 0004 |
| [0006](/issues/0006-r5-mine-command.md) | R5 | mine/list command (table + TUI entry) | closed | 0005 |
| [0007](/issues/0007-r6-browse-tui-parity.md) | R6 | browse TUI to parity (screens + loader + refresh) | closed | 0006 |
| [0008](/issues/0008-r7-i18n-assets.md) | R7 | i18n (en + pt-BR) + asset open/download | closed | 0007 |
| [0009](/issues/0009-r8-cutover.md) | R8 | cutover: promote Rust, remove Python | closed | 0008 |
| [0010](/issues/0010-v3-text-selection-mode.md) | V3 | selection mode: toggle mouse capture for native text selection | closed | — |
| [0011](/issues/0011-c1-bare-ac-tty-default-mine.md) | C1 | bare `ac` in a TTY defaults to mine | closed | — |
| [0012](/issues/0012-r2-browse-list-project-name-cache.md) | R2 | cache the per-instance project directory (SWR list refresh) | closed | — |
| [0013](/issues/0013-r3-richtext-formatting.md) | R3 | preserve comment/description rich-text via an HTML-subset styled mapper | closed | — |
| [0014](/issues/0014-arch-refactor-render-decompose-relocate.md) | ARCH | refactor render.rs: decompose meta-table, drop dead seams, relocate extract_assets | closed | 0010–0013 |
| [0015](/issues/0015-u21-agent-json-output.md) | U21 | curated minified --json contract for get/current/mine/browse + agent skill | closed | — |
| [0016](/issues/0016-s8-task-list-first-paint-swr.md) | S8 | task-list first-paint-from-cache SWR on browse/mine entry | closed | — |
| [0017](/issues/0017-detail-chrome-responsive-wrap.md) | DW | Detail chrome responsiveness — wrap header, task title, footer, artifacts | closed | — |
| [0018](/issues/0018-b1-multi-instance-project-name-isolation.md) | B1 | multi-instance project-name isolation: key the in-memory name map by (instance, project_id) | closed | — |
| [0019](/issues/0019-r4-richtext-full-tag-coverage.md) | R4 | rich-text completeness: tables, strikethrough, underline, preformatted blocks | closed | — |
| [0020](/issues/0020-v5-body-links-inline-url.md) | V5 | body links render inline as text + visible URL, clickable from the visible region | closed | — |
| [0021](/issues/0021-v6-app-managed-selection.md) | V6 | app-managed text selection: drag to highlight, copy to clipboard with feedback | closed | — |
| [0022](/issues/0022-detail-link-wrap-artifacts-project-title.md) | D1 | detail polish: wrapped-link click, Anexos label, empty project, title placement | closed | 0020 |
| [0023](/issues/0023-d1d-asset-card-spacing.md) | D1d | Anexos/Artefatos card breathing room — blank line between links + interior padding | closed | — |
| [0024](/issues/0024-d1e-asset-activation-ctrl-cmd-click.md) | D1e | open assets via Ctrl/Cmd+click; remove the numeric 1-9 open + d+1-9 download shortcuts | closed | 0023 |
| [0025](/issues/0025-d2-task-list-cards.md) | D2 | task list as per-task cards with a relative, colored due date (D2a shell, D2b project + due) | closed | — |
| [0026](/issues/0026-d1f-asset-hint-in-card.md) | D1f | move the asset-open hint into the Anexos card (italic), out of the footer | closed | 0024 |
| [0027](/issues/0027-arch-asset-panel-single-layout.md) | ARCH | one layout source of truth for the Anexos/Artefatos panel (asset_panel module) | closed | — |
| [0028](/issues/0028-assets-inline-scrollable-content.md) | DV | assets render inline in the globally-scrollable detail content (retire the fixed panel + cap) | closed | — |
| [0029](/issues/0029-richtext-wrap-positional-style.md) | RT | rich-line wrap threads style positionally — fix repeated/substring word emphasis, delete style_of_word_in_rich_line | closed | — |
| [0030](/issues/0030-tasks-card-layout-cache.md) | PERF | memoize the Tasks-screen card layout (heights + prefix-sum offsets, binary-search first-visible, u32 offsets) | closed | — |
| [0031](/issues/0031-fix-inline-asset-link-style-click.md) | BUG | inline Anexos/Artefatos rows render plain (no link style) + read as not clickable — restore structural link styling, prove Ctrl/Cmd+click open | closed | — |
| [0032](/issues/0032-create-comment.md) | C1 | create a comment on the open task — multi-line compose, authenticated POST, server-truth refresh (PRD 0002 slice 1) | closed | — |
| [0033](/issues/0033-edit-comment.md) | C2 | edit your own comment — permission-aware [editar] affordance, pre-filled compose, authenticated PUT (slice 2) | closed | 0032 |
| [0034](/issues/0034-delete-comment.md) | C3 | delete your own comment — permission-aware [excluir] affordance, inline confirm, authenticated DELETE (slice 3) | closed | 0032 |
| [0035](/issues/0035-comment-card-keyboard-focus.md) | N1 | comment-card keyboard focus — j/k focuses a card, highlight + scroll-into-view, actions stay on click (slice 1) | closed | — |
| [0036](/issues/0036-detail-contextual-footer-status-line.md) | N2 | Detail contextual footer + thin status line — mode-aware hint, transient status row, compose status moved out of the inline block (slice 2) | closed | 0035 |
| [0037](/issues/0037-modal-primitive-and-compose.md) | M1 | reusable modal primitive (centered overlay + dimmed backdrop) + migrate the comment compose to it (slice 1) | closed | — |
| [0038](/issues/0038-confirm-delete-modal.md) | M2 | migrate the delete-confirm to the reusable modal (buttons + Enter/Esc), out of the comment card (slice 2) | closed | 0037 |
| [0039](/issues/0039-non-tty-comment-command.md) | A1 | non-TTY `comment` command — post a comment to a task as the logged-in user (-m or stdin, --json result) | closed | — |
| [0040](/issues/0040-comment-affordance-colored-links.md) | L1 | comment edit/delete affordances as colored underlined links ([editar] cyan, [excluir] red), emitted structurally | closed | — |
| [0041](/issues/0041-yes-no-confirm-modal.md) | L2 | delete-confirm modal presents Sim/Não buttons (relabel [confirmar]/[cancelar]) | closed | — |
| [0042](/issues/0042-cli-401-detail-and-comment.md) | RA1 | CLI get/current/comment detect HTTP 401 → actionable re-auth message + non-zero exit | closed | — |
| [0043](/issues/0043-cli-401-mine.md) | RA2 | CLI mine detects 401 via a typed Unauthorized error → re-auth message + non-zero exit | closed | 0042 |
| [0044](/issues/0044-tui-401-status-line.md) | RA3 | TUI surfaces 401 in the thin status line → guide to `ac setup add` | closed | 0042, 0043 |
| [0045](/issues/0045-asset-hit-target-structural.md) | HT1 | asset hit-target emitted structurally (OpenAsset span); asset_panel_cmd_at becomes a lookup | closed | — |
| [0046](/issues/0046-body-link-hit-target-structural.md) | HT2 | body-link hit-target emitted structurally (OpenUrl span); delete resolve_wrapped_url + inverse-wrap helpers (obs 35) | closed | 0045 |
| [0047](/issues/0047-hit-test-module-extraction.md) | HT3 | detail click resolution becomes one deep tui/hit_test module (resolve_detail_click → DetailClickTarget); delete the five scattered click functions | closed | — |
| [0048](/issues/0048-detail-viewport-geometry-module.md) | HT4 | detail viewport↔content geometry becomes one pure tui/detail_geometry module; the row→line_idx mapping + text_top=2 stop being copied across hit_test, is_in_body_area, extract_selected_text | closed | — |
| [0049](/issues/0049-retire-vestigial-link-collector.md) | ARCH | retire the vestigial LinkCollector — delete the write-only struct + the nine collector params threaded through render/richtext, delete dead structured_text_with_links, rename the _with_collector build fns | closed | — |
