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
| [0021](/issues/0021-v6-app-managed-selection.md) | V6 | app-managed text selection: drag to highlight, copy to clipboard with feedback | open | — |
