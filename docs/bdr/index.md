# Behavior Decision Records

| # | Title | Status |
|---|---|---|
| [0001](/bdr/0001-task-list-navigation.md) | Task list navigation: mouse, scroll, and bounded selection | Accepted |
| [0002](/bdr/0002-token-host-isolation.md) | Token host-isolation: the instance token reaches only its own host | Accepted |
| [0003](/bdr/0003-cli-command-output-parity.md) | CLI command-output parity: messages, exit codes, and bare-invocation | Accepted |
| [0004](/bdr/0004-browse-navigation-screen-stack.md) | Browse navigation: a screen stack with bounded selection across screens | Accepted |
| [0005](/bdr/0005-loader-single-flight-refresh.md) | Loader and single-flight refresh: effects as Cmd, one in-flight load per screen | Accepted |
| [0006](/bdr/0006-selection-mode-mouse-capture-toggle.md) | Selection mode: a key toggles mouse capture off so the terminal can select text | Superseded by 0015 |
| [0007](/bdr/0007-bare-invocation-tty-default.md) | Bare invocation in a TTY defaults to mine (amends 0003 §3) | Accepted |
| [0008](/bdr/0008-browse-list-refresh-cached-directory.md) | Browse-list refresh: open tasks always fetched, project directory served from cache | Accepted |
| [0009](/bdr/0009-richtext-formatting-detail-view.md) | Rich-text detail: comment/description HTML renders with bold, italic, code, headings, lists, quotes, links | Accepted |
| [0010](/bdr/0010-agent-json-output-contract.md) | Agent JSON output: get/current/mine/browse emit one curated minified line, --json is non-interactive | Accepted |
| [0011](/bdr/0011-task-list-first-paint-swr-entry.md) | Task-list first-paint SWR on entry: paint the cached list instantly, always revalidate | Accepted |
| [0012](/bdr/0012-detail-chrome-responsive-wrap.md) | Detail chrome responsiveness: header, task title, footer, and artifacts wrap on narrow widths | Accepted |
| [0013](/bdr/0013-richtext-full-tag-coverage.md) | Rich-text detail: tables, strikethrough, underline, and preformatted blocks render with structure (R4) | Accepted |
| [0014](/bdr/0014-body-link-inline-url-activation.md) | Body links render inline as text + visible URL and activate from the visible region (V5) | Accepted |
| [0015](/bdr/0015-app-managed-text-selection.md) | App-managed text selection: drag highlights text and copies it to the clipboard with feedback (V6) | Accepted |
| [0016](/bdr/0016-detail-title-row-project-name.md) | Detail view shows the title as a Título row and a populated Projeto row (D1a) | Accepted |
| [0017](/bdr/0017-asset-label-derivation.md) | Anexos/Artefatos labels read as anchor text, a real filename, or the host (D1b) | Accepted |
| [0018](/bdr/0018-asset-card-breathing-room.md) | Anexos/Artefatos card has breathing room: a blank line between links and interior padding (D1d) | Superseded by 0022 |
| [0019](/bdr/0019-asset-activation-ctrl-cmd-click.md) | Assets open on Ctrl/Cmd+click; the numeric 1-9 open and d+1-9 download shortcuts are removed (D1e) | Accepted |
| [0020](/bdr/0020-task-list-cards.md) | The task list shows a bordered card per task with a relative, colored due date (D2) | Accepted |
| [0021](/bdr/0021-asset-open-hint-in-card.md) | The asset-open hint lives inside the Anexos card in italic, not in the footer (D1f) | Superseded by 0022 |
| [0022](/bdr/0022-assets-inline-scrollable-detail-content.md) | Assets are part of the scrollable detail content: every attachment reachable by scrolling, opens on Ctrl/Cmd+click at any scroll position | Accepted |
| [0023](/bdr/0023-richtext-wrap-positional-style.md) | Rich-text wrap: each wrapped word keeps its own source emphasis, even when the word repeats or is a substring of a styled token | Accepted |
| [0024](/bdr/0024-comment-authoring-create-edit-delete.md) | Comment authoring: create a multi-line comment, edit and delete your own, with a server-truth refresh after each mutation | Accepted |
| [0025](/bdr/0025-comment-card-navigation-and-contextual-footer.md) | Comment-card keyboard navigation (j/k focus, highlight + scroll-into-view) and a contextual two-region footer (mode-aware hint + thin status line) | Accepted |
| [0026](/bdr/0026-comment-modal-overlay.md) | Comment compose and delete-confirm open as a centered modal overlay over a dimmed thread, sharing one reusable modal primitive | Accepted |
| [0027](/bdr/0027-non-interactive-comment-creation.md) | A non-interactive `comment` command posts a comment to a task as the logged-in user, from a -m flag or stdin, with a --json write result | Accepted |
| [0028](/bdr/0028-comment-affordance-links-and-yes-no-confirm.md) | Comment edit/delete affordances read as colored underlined links (own comments only); the delete-confirm modal presents Sim/Não | Accepted |
| [0029](/bdr/0029-token-invalidation-reauthentication.md) | A revoked API token (HTTP 401) produces actionable re-auth guidance: the CLI prints a message and exits non-zero; the TUI shows a status-line pointing to `ac setup add` (other non-200 unchanged) | Accepted |
