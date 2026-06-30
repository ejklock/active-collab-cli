# Architecture Decision Records

| # | Title | Status |
|---|---|---|
| [0001](/adr/0001-replace-curses-tui-with-textual.md) | Replace the curses TUI with Textual | Superseded by 0002 |
| [0002](/adr/0002-rewrite-in-rust-with-ratatui.md) | Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker | Accepted |
| [0003](/adr/0003-http-transport-and-mocked-server-testing.md) | HTTP transport (reqwest + rustls, no auto-redirect, host-gated token) tested against a mocked server | Accepted |
| [0004](/adr/0004-tests-in-tests-dir-via-path-include.md) | Unit tests live under rust/tests/unit/ and are included into their module via #[path] | Accepted |
| [0005](/adr/0005-i18n-catalog-as-embedded-json.md) | The i18n message catalog is a per-locale JSON file embedded at compile time | Accepted |
| [0006](/adr/0006-promote-crate-to-repo-root.md) | Promote the Rust crate to the repo root and remove Python | Accepted |
| [0007](/adr/0007-tui-module-structure.md) | Organize the TUI as a layered module tree under src/tui/ | Accepted |
| [0008](/adr/0008-async-event-loop-with-eventstream-and-select.md) | Drive the TUI from an async event loop (EventStream + tokio::select!) | Accepted |
| [0009](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) | TUI visual redesign — vibrant dashboard (user header, unified lists, scrollbar) | Accepted |
| [0010](/adr/0010-detail-sectioned-panels-focus-scroll.md) | Detail screen as fixed, independently-scrollable sections (focus + Tab + numeric jump) | Reverted (U6c) |
| [0011](/adr/0011-agent-json-output-contract.md) | Curated, minified JSON output for agent/LLM consumption (--json contract) (U21) | Accepted |
| [0012](/adr/0012-mouse-capture-toggle-for-text-selection.md) | Toggle terminal mouse capture for native text selection (V3) | Superseded by 0021 |
| [0013](/adr/0013-tty-gated-default-subcommand.md) | A bare `ac` invocation in a TTY defaults to `mine` (C1) | Accepted |
| [0014](/adr/0014-browse-list-project-name-cache-swr.md) | Browse-list project-name cache (SWR) — stop re-fetching the directory on refresh (R2) | Accepted |
| [0015](/adr/0015-richtext-html-subset-styled-segments.md) | Render comment/description HTML as styled segments over a known tag subset (R3) | Accepted |
| [0016](/adr/0016-refactor-render-decompose-relocate.md) | Refactor render.rs — decompose the meta-table god function, drop dead seams, relocate asset extraction (ARCH) | Accepted |
| [0017](/adr/0017-task-list-first-paint-cache-swr-entry.md) | First-paint-from-cache SWR on browse/mine entry (task-list snapshot cache) (S8) | Accepted |
| [0018](/adr/0018-detail-chrome-dynamic-height-wrap.md) | Detail chrome wraps via dynamic region heights; task name moves off the un-wrappable frame title | Accepted |
| [0019](/adr/0019-richtext-full-activecollab-tag-coverage.md) | Extend the rich-text mapper to the full ActiveCollab allowed-tag set (tables, strike/del, underline, pre) (R4) | Accepted |
| [0020](/adr/0020-body-links-inline-url-native-click.md) | Body links render inline as text + visible URL, clickable from the visible region (V5) | Accepted |
| [0021](/adr/0021-app-managed-text-selection-clipboard.md) | App-managed text selection with a drawn highlight and clipboard copy (V6) | Accepted |
| [0022](/adr/0022-detail-title-as-meta-row.md) | The task title renders as a Título row inside the Detalhes panel, not a loose header (D1a) | Accepted |
| [0023](/adr/0023-asset-label-derivation.md) | Derive the Anexos/Artefatos label from anchor text, then a real filename, then the host (D1b) | Accepted |
| [0024](/adr/0024-asset-card-breathing-room.md) | Anexos/Artefatos card breathing room — per-link separators, interior padding, named height ceiling (D1d) | Superseded by 0029 |
| [0025](/adr/0025-asset-activation-ctrl-cmd-click.md) | Open assets with Ctrl/Cmd+click; retire the numeric 1-9 open and d+1-9 download shortcuts (D1e) | Accepted |
| [0026](/adr/0026-task-list-as-cards.md) | Render the task list as per-task cards with a relative, colored due date (D2) | Accepted |
| [0027](/adr/0027-asset-open-hint-in-card.md) | Move the asset-open hint into the Anexos card as an italic footnote; drop it from the footer (D1f) | Accepted |
| [0028](/adr/0028-asset-panel-single-layout-source.md) | One layout source of truth for the Anexos/Artefatos panel — a pure asset_panel module the renderer, height, and hit-test all derive from (ARCH) | Accepted (amended by 0029) |
| [0029](/adr/0029-assets-inline-in-scrollable-detail-content.md) | Assets render inline in the globally-scrollable detail content — retire the fixed asset panel and its height cap; scroll-aware asset click | Accepted |
| [0030](/adr/0030-richtext-wrap-positional-style.md) | Rich-line wrap threads span style positionally (per character), retiring the substring style lookup | Accepted |
| [0031](/adr/0031-tasks-card-layout-cache.md) | Memoize the Tasks-screen card layout (prefix-sum offsets + binary-search first-visible) so per-event cost scales with the viewport, not the task count | Accepted |
| [0032](/adr/0032-asset-row-link-style-structural.md) | Asset-row link styling is structural (a RichStyle::Link run emitted by the layout), not text-pattern URL detection | Accepted |
| [0033](/adr/0033-authenticated-write-seam-comment-client.md) | Authenticated write seam — host-gated POST/PUT/DELETE on Http, comment-mutation methods on the client | Accepted |
| [0034](/adr/0034-comment-compose-mode-multiline.md) | Multi-line comment compose is a mode on the Detail screen, driven by mode-aware key mapping in the shell | Accepted |
| [0035](/adr/0035-server-truth-refresh-after-comment-mutation.md) | After a comment mutation, re-derive the thread from the server (LoadDetail refresh) — no optimistic UI | Accepted |
| [0036](/adr/0036-permission-aware-comment-targeting.md) | Edit/delete target a comment via permission-aware inline affordances rendered only on the user's own comments | Accepted |
| [0037](/adr/0037-comment-card-keyboard-focus.md) | Comment cards gain a keyboard focus cursor (highlight + scroll-into-view) over the global scroll, with actions left on the click affordances | Accepted |
| [0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md) | The detail footer becomes two regions — a context-aware instruction line plus a thin transient status line | Accepted |
| [0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md) | A reusable centered modal overlay (dimmed backdrop) renders the comment compose and the delete-confirm, replacing their inline-spliced rendering | Accepted |
| [0040](/adr/0040-non-interactive-comment-write-command.md) | A non-interactive `comment` CLI command posts a comment as the logged-in user, reusing the authenticated client seam and extending the agent --json contract to a write | Accepted |
| [0041](/adr/0041-comment-affordance-colored-links-and-yes-no-confirm.md) | Comment edit/delete affordances render as structurally-emitted colored links (edit cyan, delete destructive red); the delete-confirm modal presents Sim/Não buttons | Accepted |
| [0042](/adr/0042-detect-401-and-guide-reauthentication.md) | Detect HTTP 401 as a distinct condition and surface actionable re-authentication guidance (CLI message + non-zero exit; TUI status line) — ActiveCollab has no refresh-token endpoint; its token is durable until revoked | Accepted |
| [0043](/adr/0043-detail-hit-targets-emitted-structurally.md) | Detail hit-targets (body-link URL, asset row) are emitted structurally into the one `affordances` registry at layout time; the three click hit-tests become positional lookups and `resolve_wrapped_url` + inverse-wrap helpers are deleted (extends ADR 0028/0032 from style to hit-target) | Accepted |
| [0044](/adr/0044-detail-click-resolution-as-hit-test-module.md) | Detail click resolution collapses into one deep `src/tui/hit_test.rs` — a single coordinate translation + one registry lookup behind a pure `resolve_detail_click` → typed `DetailClickTarget`; the five scattered click functions in `model.rs` are deleted (the ADR 0043 locality follow-up) | Accepted |
| [0045](/adr/0045-detail-viewport-geometry-module.md) | Detail viewport↔content geometry (the row→`line_idx` mapping + the `text_top=2` literal) is single-homed in a pure `src/tui/detail_geometry.rs` shared by `hit_test`, `is_in_body_area`, and `extract_selected_text` — retiring the three-place magic literal and triplicate mapping (the ADR 0044 follow-up, obs 53) | Accepted |
| [0046](/adr/0046-retire-vestigial-link-collector.md) | Retire the vestigial `LinkCollector` — after ADR 0030/0032/0043 made link style and hit-targets structural, the write-only struct threaded as `&mut` through nine render/richtext functions has no reader; delete the struct, the nine collector parameters, and the dead `structured_text_with_links` (deletion test) | Accepted |
