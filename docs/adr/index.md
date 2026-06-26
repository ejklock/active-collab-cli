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
