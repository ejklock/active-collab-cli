# Architecture Decision Records

## Active

* [0002 — Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker](0002-rewrite-in-rust-with-ratatui.md) - Accepted
* [0003 — HTTP transport (reqwest + rustls, no auto-redirect, host-gated token) tested against a mocked server](0003-http-transport-and-mocked-server-testing.md) - Accepted
* [0004 — Unit tests live under rust/tests/unit/ and are included into their module via](0004-tests-in-tests-dir-via-path-include.md) - Accepted
* [0005 — The i18n message catalog is a per-locale JSON file embedded at compile time](0005-i18n-catalog-as-embedded-json.md) - Accepted
* [0006 — Promote the Rust crate to the repo root and remove Python](0006-promote-crate-to-repo-root.md) - Accepted
* [0007 — Organize the TUI as a layered module tree under src/tui/](0007-tui-module-structure.md) - Accepted
* [0008 — Drive the TUI from an async event loop (EventStream + tokio::select!)](0008-async-event-loop-with-eventstream-and-select.md) - Accepted
* [0009 — TUI visual redesign — vibrant dashboard (user header, unified lists, scrollbar)](0009-tui-visual-redesign-vibrant-dashboard.md) - Accepted
* [0010 — Detail screen as fixed, independently-scrollable sections (focus + Tab + numeric jump)](0010-detail-sectioned-panels-focus-scroll.md) - Reverted
* [0011 — Curated, minified JSON output for agent/LLM consumption (--json contract)](0011-agent-json-output-contract.md) - Accepted
* [0013 — A bare `ac` invocation in a TTY defaults to `mine`](0013-tty-gated-default-subcommand.md) - Accepted
* [0014 — Cache the per-instance project-name directory so a list refresh stops re-fetching it](0014-browse-list-project-name-cache-swr.md) - Accepted
* [0015 — Render comment/description HTML as styled segments over a known tag subset](0015-richtext-html-subset-styled-segments.md) - Accepted
* [0016 — Refactor render.rs — decompose the meta-table god function, drop dead seams, relocate asset extraction](0016-refactor-render-decompose-relocate.md) - Accepted
* [0017 — First-paint-from-cache SWR on browse/mine entry (task-list snapshot cache)](0017-task-list-first-paint-cache-swr-entry.md) - Accepted
* [0018 — Detail chrome wraps via dynamic region heights, and the task name moves off the un-wrappable frame title](0018-detail-chrome-dynamic-height-wrap.md) - Accepted
* [0019 — Extend the rich-text mapper to the full ActiveCollab allowed-tag set](0019-richtext-full-activecollab-tag-coverage.md) - Accepted
* [0020 — Render body links as inline text + visible URL, clickable via the visible region](0020-body-links-inline-url-native-click.md) - Accepted
* [0021 — App-managed text selection with a drawn highlight and clipboard copy](0021-app-managed-text-selection-clipboard.md) - Accepted
* [0022 — The task title renders as a "Título" row inside the Detalhes panel, not a loose header](0022-detail-title-as-meta-row.md) - Accepted
* [0023 — Derive the Anexos/Artefatos label from anchor text, then a real filename, then the host](0023-asset-label-derivation.md) - Accepted
* [0025 — Open assets with Ctrl/Cmd+click; retire the numeric 1-9 open and d+1-9 download shortcuts](0025-asset-activation-ctrl-cmd-click.md) - Accepted
* [0026 — Render the task list as per-task cards with a relative, colored due date](0026-task-list-as-cards.md) - Accepted
* [0027 — Move the asset-open hint into the Anexos card as an italic footnote](0027-asset-open-hint-in-card.md) - Accepted
* [0028 — Give the Anexos/Artefatos panel one layout source of truth (asset_panel module)](0028-asset-panel-single-layout-source.md) - Accepted
* [0029 — Assets render inline in the globally-scrollable detail content (retire the fixed asset panel and its height cap)](0029-assets-inline-in-scrollable-detail-content.md) - Accepted
* [0030 — Rich-line wrap threads span style positionally (per character), retiring the substring style lookup](0030-richtext-wrap-positional-style.md) - Accepted
* [0031 — Memoize the Tasks-screen card layout (prefix-sum offsets + binary-search first-visible) so per-event cost scales with the viewport, not the task count](0031-tasks-card-layout-cache.md) - Accepted
* [0032 — Asset-row link styling is structural (a RichStyle::Link run emitted by the layout), not text-pattern URL detection](0032-asset-row-link-style-structural.md) - Accepted
* [0033 — Authenticated write seam — host-gated POST/PUT/DELETE on Http, comment-mutation methods on the client](0033-authenticated-write-seam-comment-client.md) - Accepted
* [0034 — Multi-line comment compose is a mode on the Detail screen, driven by mode-aware key mapping in the shell](0034-comment-compose-mode-multiline.md) - Accepted
* [0035 — After a comment mutation, re-derive the thread from the server (LoadDetail refresh) — no optimistic UI](0035-server-truth-refresh-after-comment-mutation.md) - Accepted
* [0036 — Edit/delete target a comment via permission-aware inline affordances rendered only on the user's own comments](0036-permission-aware-comment-targeting.md) - Accepted
* [0037 — Comment cards gain a keyboard focus cursor (highlight + scroll-into-view) over the global scroll, with actions left on the click affordances](0037-comment-card-keyboard-focus.md) - Accepted
* [0038 — The detail footer becomes two regions — a context-aware instruction line plus a thin transient status line](0038-detail-footer-contextual-hint-and-status-line.md) - Accepted
* [0039 — A reusable centered modal overlay (dimmed backdrop) renders the comment compose and the delete-confirm, replacing their inline-spliced rendering](0039-reusable-modal-overlay-for-compose-and-confirm.md) - Accepted
* [0040 — A non-interactive `comment` CLI command posts a comment as the logged-in user, reusing the authenticated client seam and extending the agent --json contract to a write](0040-non-interactive-comment-write-command.md) - Accepted
* [0041 — Comment edit/delete affordances render as structurally-emitted colored links (edit cyan, delete destructive red); the delete-confirm modal presents Sim/Não buttons](0041-comment-affordance-colored-links-and-yes-no-confirm.md) - Accepted
* [0042 — Detect HTTP 401 as a distinct condition and surface actionable re-authentication guidance (CLI message + non-zero exit; TUI status line)](0042-detect-401-and-guide-reauthentication.md) - Accepted
* [0043 — Detail hit-targets are emitted structurally into one affordance registry; click-time re-derivation is retired](0043-detail-hit-targets-emitted-structurally.md) - Accepted
* [0044 — Detail click resolution is one deep tui/hit_test module — a single coordinate translation and one registry lookup behind a typed target](0044-detail-click-resolution-as-hit-test-module.md) - Accepted
* [0045 — Detail viewport↔content geometry is one pure module — the row→line_idx mapping and text_top live once, shared by hit-test, selection, and copy](0045-detail-viewport-geometry-module.md) - Accepted
* [0046 — Retire the vestigial LinkCollector — the rich-text body/comment pipeline stops threading a link collector](0046-retire-vestigial-link-collector.md) - Accepted
* [0047 — The Detail screen's mutually-exclusive overlays become one typed DetailOverlay enum — making "composing and confirming-delete at once" unrepresentable](0047-detail-overlay-as-one-typed-state.md) - Accepted
* [0048 — One greedy word-wrap engine — plain and rich wrap share a single core over a cell abstraction, and the canonical semantics fixes the rich blank-line drop](0048-unify-plain-and-rich-wrap-engines.md) - Accepted
* [0049 — render.rs splits into a layered stack — a pure text_measure core, a wrap module over it, and two output adapters (cli_render, detail_render) — retiring the 2234-line god-module](0049-split-render-into-text-measure-wrap-and-render-adapters.md) - Accepted
* [0050 — detail_geometry absorbs the selection column math — one deep selected_text(...) interface owns row→line and column→text, retiring the box_inner_content_pub reach-in](0050-detail-geometry-absorbs-selection-column-math.md) - Accepted
* [0051 — Task-card layout is one pure task_layout module — card content, height, inner width, and first-visible live once, shared by reflow_tasks and draw_tasks, retiring the two-place content string](0051-extract-task-layout-module.md) - Accepted
* [0052 — The cache layer owns freshness — ProjectNamesCache gains a read_fresh(max_age) that does the TTL check inside, mirroring TaskListCache::read, so the caller stops doing age arithmetic](0052-cache-owns-freshness-read-fresh.md) - Accepted
* [0053 — The detail-footer decision is one pure footer module — hint selection, transient status, and layout plan live behind a single footer::plan(screen, …) -> FooterPlan, and view.rs becomes draw-only over it](0053-footer-decision-as-pure-module.md) - Accepted
* [0054 — The comment-write seam returns a typed CommentWriteOutcome — the client classifies 401 / success / failure once, retiring the duplicated HTTP_UNAUTHORIZED status compare at the shell and CLI call sites](0054-comment-write-outcome-typed-classification.md) - Accepted
* [0055 — The flat commands.rs splits into a commands/ directory of deep modules — one input-resolution master (resolve), one presentation master (presenter), and one orchestration module per command family (setup / task / comment / mine), behind a thin re-exporting mod.rs](0055-commands-split-three-masters.md) - Accepted
* [0056 — The detail load resolves an unknown project name over the network and writes it back to cache](0056-detail-project-name-network-fallback.md) - Accepted
* [0057 — The agent skill is served by an `ac skill` CLI command; per-harness integrations are thin pointers to it](0057-agent-skill-served-by-ac-skill-command.md) - Accepted
* [0058 — install-skill.sh gains a --scope project|global selector; global writes each harness's user-level path](0058-install-skill-scope-project-or-global.md) - Accepted
* [0059 — Rename the agent skill from ac-json to active-collab and sharpen its frontmatter description](0059-rename-skill-to-active-collab.md) - Accepted
* [0060 — Semantic design-system tokens with swappable palettes (Angie / Slate / Nord, dark + light)](0060-design-system-semantic-tokens.md) - Accepted
* [0061 — Squared legend panels — replace rounded box-drawing corners crate-wide](0061-squared-legend-panels.md) - Accepted
* [0062 — Theme selection — persisted config, env override, and `ac setup theme`](0062-theme-selection-config-and-command.md) - Accepted
* [0063 — Styled panel titles — accent + bold labels in the top rule](0063-styled-panel-titles.md) - Accepted
* [0064 — The comment compose editor adopts tui-textarea — a full caret/selection/undo editor behind one ComposeInput(Input) message](0064-comment-compose-adopts-tui-textarea.md) - Accepted
* [0065 — Image attachments open in a dedicated full-area viewer overlay (ratatui-image) — pure lifecycle in the Model, non-pure protocol state quarantined in the shell](0065-image-attachment-viewer-modal-overlay.md) - Accepted
* [0066 — ac get/current --download-attachments: fetch task assets to a local temp dir for agent/LLM analysis](0066-agent-attachment-download-to-local-temp-dir.md) - Accepted
* [0067 — Origin-gated API token header (scheme + host + port)](0067-origin-gated-api-token-header-scheme-host-port.md) - Proposed
* [0068 — Control characters stripped at the untrusted-text render boundary](0068-control-characters-stripped-at-the-untrusted-text-render-boundary.md) - Proposed
* [0069 — active-collab-cli becomes a limited write client](0069-active-collab-cli-becomes-a-limited-write-client.md) - Accepted

## Superseded

* [0001 — Replace the curses TUI with Textual](0001-replace-curses-tui-with-textual.md) - Superseded
* [0012 — Toggle terminal mouse capture so native text selection works on demand](0012-mouse-capture-toggle-for-text-selection.md) - Superseded
* [0024 — Anexos/Artefatos card breathing room — per-link separators, interior padding, named height ceiling](0024-asset-card-breathing-room.md) - Superseded
