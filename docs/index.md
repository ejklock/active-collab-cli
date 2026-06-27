---
okf_version: "0.1"
---

# active-collab-cli — Docs

Living documentation bundle. Every structural decision and behavior has one home
here and is reachable from this index.

## Root of trace

- [Constitution](/constitution.md) — product scope, data model, non-negotiables.

## Context

- [Context index](/context/index.md) — domain & module vocabulary.
- [Glossary](/context/glossary.md) — terms and acronyms, defined once.
- [Architecture](/architecture.md) — Rust module structure and data-flow diagrams.

## Product Requirements (PRD)

See [prd/](/prd/index.md).

- [0001](/prd/0001-rust-tui-cli-parity.md) — ActiveCollab task CLI + TUI in Rust (parity rewrite) *(Accepted)*

## Architecture Decision Records (ADR)

See [adr/](/adr/index.md).

- [0001](/adr/0001-replace-curses-tui-with-textual.md) — Replace the curses TUI with Textual *(Superseded by 0002)*
- [0002](/adr/0002-rewrite-in-rust-with-ratatui.md) — Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker *(Accepted)*
- [0003](/adr/0003-http-transport-and-mocked-server-testing.md) — HTTP transport + mocked-server testing *(Accepted)*
- [0004](/adr/0004-tests-in-tests-dir-via-path-include.md) — Unit tests under tests/unit/ via #[path] *(Accepted)*
- [0005](/adr/0005-i18n-catalog-as-embedded-json.md) — i18n catalog as embedded JSON *(Accepted)*
- [0006](/adr/0006-promote-crate-to-repo-root.md) — Promote the Rust crate to the repo root, remove Python *(Accepted)*
- [0007](/adr/0007-tui-module-structure.md) — Layered TUI module tree under src/tui/ *(Accepted)*
- [0008](/adr/0008-async-event-loop-with-eventstream-and-select.md) — Async event loop (EventStream + tokio::select!) *(Accepted)*
- [0009](/adr/0009-tui-visual-redesign-vibrant-dashboard.md) — TUI visual redesign — vibrant dashboard *(Accepted)*
- [0010](/adr/0010-detail-sectioned-panels-focus-scroll.md) — Detail screen as fixed, independently-scrollable sections *(Reverted)*
- [0012](/adr/0012-mouse-capture-toggle-for-text-selection.md) — Toggle terminal mouse capture for native text selection (V3) *(Superseded by 0021)*
- [0013](/adr/0013-tty-gated-default-subcommand.md) — A bare `ac` invocation in a TTY defaults to `mine` (C1) *(Accepted)*
- [0014](/adr/0014-browse-list-project-name-cache-swr.md) — Browse-list project-name cache (SWR) (R2) *(Accepted)*
- [0015](/adr/0015-richtext-html-subset-styled-segments.md) — Render HTML as styled segments over a tag subset (R3) *(Accepted)*
- [0016](/adr/0016-refactor-render-decompose-relocate.md) — Refactor render.rs: decompose, drop dead seams, relocate (ARCH) *(Accepted)*
- [0019](/adr/0019-richtext-full-activecollab-tag-coverage.md) — Extend the rich-text mapper to the full ActiveCollab allowed-tag set (R4) *(Accepted)*
- [0020](/adr/0020-body-links-inline-url-native-click.md) — Body links inline as text + visible URL, clickable from the visible region (V5) *(Accepted)*
- [0021](/adr/0021-app-managed-text-selection-clipboard.md) — App-managed text selection with drawn highlight + clipboard copy (V6) *(Accepted)*

## Behavior Decision Records (BDR)

See [bdr/](/bdr/index.md).

- [0001](/bdr/0001-task-list-navigation.md) — Task list navigation: mouse, scroll, and bounded selection *(Accepted)*
- [0002](/bdr/0002-token-host-isolation.md) — Token host-isolation *(Accepted)*
- [0003](/bdr/0003-cli-command-output-parity.md) — CLI command-output parity *(Accepted; §3 amended by 0007)*
- [0004](/bdr/0004-browse-navigation-screen-stack.md) — Browse navigation: a screen stack with bounded selection *(Accepted)*
- [0005](/bdr/0005-loader-single-flight-refresh.md) — Loader and single-flight refresh *(Accepted)*
- [0006](/bdr/0006-selection-mode-mouse-capture-toggle.md) — Selection mode: mouse-capture toggle (V3) *(Superseded by 0015)*
- [0007](/bdr/0007-bare-invocation-tty-default.md) — Bare invocation in a TTY defaults to mine (C1) *(Accepted)*
- [0008](/bdr/0008-browse-list-refresh-cached-directory.md) — Browse-list refresh: cached project directory (R2) *(Accepted)*
- [0009](/bdr/0009-richtext-formatting-detail-view.md) — Rich-text formatting in the detail view (R3) *(Accepted)*
- [0013](/bdr/0013-richtext-full-tag-coverage.md) — Rich-text: tables, strike/del, underline, preformatted blocks (R4) *(Accepted)*
- [0014](/bdr/0014-body-link-inline-url-activation.md) — Body links inline as text + visible URL, activate from visible region (V5) *(Accepted)*
- [0015](/bdr/0015-app-managed-text-selection.md) — App-managed text selection: drag highlights, copies to clipboard (V6) *(Accepted)*

## Research

See [research/](/research/index.md).

- [0001](/research/0001-tui-richtext-links-selection.md) — Rich-text rendering, link interaction, and mouse selection — ActiveCollab evidence + crate evaluation

## Issues

See [issues/](/issues/index.md) — slices R0–R8 of the Rust rewrite plus the
post-parity TUI/UX slices (V3, C1, R2, R3, ARCH, R4, V5, V6).
