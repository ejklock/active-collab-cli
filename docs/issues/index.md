# Issues

Slices of the Rust rewrite (plan `rust-rewrite`, R0–R8) and the post-parity
TUI/UX work (V/R/ARCH slices), tracing to
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) and the ADRs/BDRs each row links.

## Open

* [0015 — U21 — curated minified --json contract for get/current/mine/browse + agent skill](0015-u21-agent-json-output.md) - open
* [0027 — ARCH — one layout source of truth for the Anexos/Artefatos panel (asset_panel module)](0027-arch-asset-panel-single-layout.md) - open
* [0035 — Comment-card keyboard focus — j/k focuses a comment card, highlight + scroll-into-view, actions stay on click](0035-comment-card-keyboard-focus.md) - open
* [0036 — Detail contextual footer + status line — mode-aware instruction line, a thin transient status row, compose status moved out of the inline block](0036-detail-contextual-footer-status-line.md) - open
* [0037 — Reusable modal primitive + migrate the comment compose to it (centered overlay, dimmed backdrop)](0037-modal-primitive-and-compose.md) - open
* [0038 — Migrate the delete-confirm to the reusable modal (buttons + Enter/Esc), out of the comment card](0038-confirm-delete-modal.md) - open
* [0039 — Non-TTY `comment` command — post a comment to a task as the logged-in user (-m or stdin, --json result)](0039-non-tty-comment-command.md) - open
* [0056 — Pin the Docker dev image's Rust toolchain to match CI stable so local clippy stops emitting false-negatives](0056-pin-docker-rust-toolchain-to-ci.md) - open
* [0057 — Comment compose adopts tui-textarea — caret/selection/undo editor behind Msg::ComposeInput(Input)](0057-comment-compose-tui-textarea.md) - open
* [0058 — Image viewer slice 1 — image-asset classification, ViewImage affordance, DetailOverlay::ImageViewer, pure load lifecycle](0058-image-viewer-overlay-lifecycle.md) - open
* [0059 — Image viewer slice 2 — ratatui-image shell protocol: fetch + decode + render, tmux half-block fallback, teardown redraw](0059-image-viewer-shell-render.md) - open
* [0060 — Dockerfile builder stage omits .claude/ from the build context — cold `docker compose run dev` fails on skill.rs include_str!](0060-dockerfile-copy-claude-skill-build-context.md) - open
* [0061 — ac get/current --download-attachments — fetch task assets to a local temp dir for agent analysis](0061-download-attachments-flag.md) - open
* [0062 — Security audit findings (pending independent validation) — terminal-escape injection + token cleartext scheme-downgrade](0062-security-audit-findings-pending-validation.md) - open
* [0063 — Triage the remaining security leads and the pre-existing IaC misconfiguration from issue 0062](0063-triage-remaining-security-leads-and-the-pre-existing-iac-misconfiguration-from-issue-0062.md) - Proposed
* [0064 — Installers drop the short ac command — restore it as an alias next to active-collab](0064-installers-drop-the-short-ac-command-restore-it-as-an-alias-next-to-active-collab.md) - Proposed
* [0065 — The ac alias only half works: help says active-collab, make install ships a Linux binary, and a stale ac wins the PATH](0065-the-ac-alias-only-half-works-help-says-active-collab-make-install-ships-a-linux-binary-and-a-stale-ac-wins-the-path.md) - Proposed
* [0066 — Comment writes preserve paragraph breaks — encode newlines as HTML before POST/PUT](0066-comment-writes-preserve-paragraph-breaks-encode-newlines-as-html-before-post-put.md) - open
* [0067 — Log time on a task — add a time entry from CLI and TUI](0067-log-time-on-a-task-add-a-time-entry-from-cli-and-tui.md) - open
* [0068 — Edit task fields — status, assignee, time estimate from CLI and TUI](0068-edit-task-fields-status-assignee-time-estimate-from-cli-and-tui.md) - open

## Closed

* [0001 — R0 — Rust scaffold + Docker (dev/build) + ratatui mouse spike](0001-r0-spike-scaffold-docker-tui.md) - closed
* [0002 — R1 — Config + SQLite store parity](0002-r1-config-sqlite-store.md) - closed
* [0003 — R2 — HTTP + ActiveCollab API client parity](0003-r2-http-api-client.md) - closed
* [0004 — R3 — CLI scaffold + setup commands + bare-invocation](0004-r3-cli-setup-commands.md) - closed
* [0005 — R4 — get + current commands (fetch + render)](0005-r4-get-current-commands.md) - closed
* [0006 — R5 — mine/list command (table + TUI entry)](0006-r5-mine-command.md) - closed
* [0007 — R6 — browse TUI to parity (screens + loader + non-stacking refresh)](0007-r6-browse-tui-parity.md) - closed
* [0008 — R7 — i18n (en + pt-BR) + asset open/download](0008-r7-i18n-assets.md) - closed
* [0009 — R8 — cutover: promote Rust, remove Python](0009-r8-cutover.md) - closed
* [0010 — V3 — selection mode: toggle mouse capture for native text selection](0010-v3-text-selection-mode.md) - closed
* [0011 — C1 — bare `ac` in a TTY defaults to mine](0011-c1-bare-ac-tty-default-mine.md) - closed
* [0012 — R2 — cache the per-instance project directory so list refresh stops re-fetching it](0012-r2-browse-list-project-name-cache.md) - closed
* [0013 — R3 — preserve comment/description rich-text via an HTML-subset styled mapper](0013-r3-richtext-formatting.md) - closed
* [0014 — ARCH — refactor render.rs: decompose meta-table, drop dead seams, relocate extract_assets](0014-arch-refactor-render-decompose-relocate.md) - closed
* [0016 — S8 — task-list first-paint-from-cache SWR on browse/mine entry](0016-s8-task-list-first-paint-swr.md) - closed
* [0017 — Detail chrome responsiveness — wrap header, task title, footer, and artifacts on narrow widths](0017-detail-chrome-responsive-wrap.md) - closed
* [0018 — B1 — multi-instance project-name isolation: key the in-memory name map by (instance, project_id)](0018-b1-multi-instance-project-name-isolation.md) - closed
* [0019 — R4 — rich-text completeness: tables, strikethrough, underline, preformatted blocks](0019-r4-richtext-full-tag-coverage.md) - closed
* [0020 — V5 — body links render inline as text + visible URL, clickable from the visible region](0020-v5-body-links-inline-url.md) - closed
* [0021 — V6 — app-managed text selection: drag to highlight, copy to clipboard with feedback](0021-v6-app-managed-selection.md) - closed
* [0022 — D1 — detail polish: wrapped-link click, Anexos label, empty project, title placement](0022-detail-link-wrap-artifacts-project-title.md) - closed
* [0023 — D1d — Anexos/Artefatos card breathing room: blank line between links + interior padding](0023-d1d-asset-card-spacing.md) - closed
* [0024 — D1e — open assets via Ctrl/Cmd+click; remove the numeric 1-9 open and d+1-9 download shortcuts](0024-d1e-asset-activation-ctrl-cmd-click.md) - closed
* [0025 — D2 — task list as per-task cards with a relative, colored due date (D2a card shell, D2b project + due)](0025-d2-task-list-cards.md) - closed
* [0026 — D1f — move the asset-open hint into the Anexos card (italic), out of the footer](0026-d1f-asset-hint-in-card.md) - closed
* [0028 — Assets render inline in the globally-scrollable detail content (retire the fixed asset panel + cap)](0028-assets-inline-scrollable-content.md) - closed
* [0029 — Rich-line wrap threads style positionally — fix repeated/substring word emphasis, delete style_of_word_in_rich_line](0029-richtext-wrap-positional-style.md) - closed
* [0030 — Memoize the Tasks-screen card layout — cache heights + prefix-sum offsets, binary-search first-visible, widen offsets to u32](0030-tasks-card-layout-cache.md) - closed
* [0031 — Fix inline Anexos/Artefatos rows rendering as plain text (no link style) and reading as not clickable](0031-fix-inline-asset-link-style-click.md) - closed
* [0032 — Create a comment on the open task — multi-line compose, authenticated POST, server-truth refresh](0032-create-comment.md) - closed
* [0033 — Edit your own comment — permission-aware [editar] affordance, pre-filled compose, authenticated PUT](0033-edit-comment.md) - closed
* [0034 — Delete your own comment — permission-aware [excluir] affordance, inline confirm, authenticated DELETE](0034-delete-comment.md) - closed
* [0040 — Comment edit/delete affordances render as colored underlined links — [editar] cyan, [excluir] destructive red, emitted structurally](0040-comment-affordance-colored-links.md) - closed
* [0041 — Delete-confirm modal presents Sim/Não buttons — relabel [confirmar]/[cancelar] to a yes/no choice](0041-yes-no-confirm-modal.md) - closed
* [0042 — CLI detail + comment detect HTTP 401 → actionable re-auth message + non-zero exit (slice 1)](0042-cli-401-detail-and-comment.md) - closed
* [0043 — CLI mine detects HTTP 401 via a typed Unauthorized error → re-auth message + non-zero exit (slice 2)](0043-cli-401-mine.md) - closed
* [0044 — TUI surfaces HTTP 401 in the thin status line → guide the user to `ac setup add` (slice 3)](0044-tui-401-status-line.md) - closed
* [0045 — Asset hit-target is emitted structurally into the affordance registry; asset_panel_cmd_at becomes a lookup (slice 1)](0045-asset-hit-target-structural.md) - closed
* [0046 — Body-link hit-target is emitted structurally; resolve_wrapped_url + inverse-wrap helpers are deleted (slice 2)](0046-body-link-hit-target-structural.md) - closed
* [0047 — Detail click resolution becomes one deep tui/hit_test module (resolve_detail_click → DetailClickTarget); the five scattered click functions are deleted](0047-hit-test-module-extraction.md) - closed
* [0048 — Detail viewport↔content geometry becomes one pure src/tui/detail_geometry.rs; the row→line_idx mapping and text_top=2 stop being copied across hit_test, is_in_body_area, and extract_selected_text](0048-detail-viewport-geometry-module.md) - closed
* [0049 — Retire the vestigial LinkCollector — delete the write-only struct and the nine collector parameters threaded through the render/richtext rich-text pipeline](0049-retire-vestigial-link-collector.md) - closed
* [0050 — Close the plain/rich wrap test asymmetry — add direct wrap_rich geometry characterization specs (hard-split, exact-width boundary, zero-width, all-whitespace, embedded newline, empty fallback)](0050-wrap-rich-geometry-specs.md) - closed
* [0051 — Replace Screen::Detail's two independent overlay Options (compose, confirm_delete) with one typed DetailOverlay enum so the composing-and-confirming-delete state is unrepresentable](0051-detail-overlay-typed-state.md) - closed
* [0052 — Single-home the detail offset-clamp height in detail_geometry::content_height_clamped — the ADR 0045 deferred fold](0052-fold-offset-clamp-height.md) - closed
* [0053 — Unify the parallel plain/rich word-wrap engines into one greedy_wrap core over a cell abstraction — and fix the rich blank-line drop (ADR 0048)](0053-unify-wrap-engines.md) - closed
* [0054 — Bring the comment-policy harness find_line_comment within the complexity budget — extract the duplicated quoted-literal skip (obs 34)](0054-simplify-find-line-comment.md) - closed
* [0055 — Reconcile CHANGELOG.md with the Rust crate tag line — retire/segregate the legacy Python history](0055-reconcile-changelog-with-rust-tag-line.md) - closed
